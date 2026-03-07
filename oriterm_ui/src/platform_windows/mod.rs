//! Windows platform glue — `WndProc` subclass for frameless window management.
//!
//! Installs a `SetWindowSubclass` handler that enables Aero Snap, delegates
//! hit testing to [`hit_test::hit_test()`], handles DPI changes, and supports
//! OS-level drag sessions for tab tear-off. This is the standard approach
//! used by Chrome, `WezTerm`, and Windows Terminal.
//!
//! The entire module is Win32 FFI glue — every public function calls into
//! the Win32 API through `windows-sys`.

#![allow(unsafe_code)]

mod subclass;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_EXTENDED_FRAME_BOUNDS, DWMWA_TRANSITIONS_FORCEDISABLED, DwmExtendFrameIntoClientArea,
    DwmGetWindowAttribute, DwmSetWindowAttribute,
};
use windows_sys::Win32::UI::Controls::MARGINS;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows_sys::Win32::UI::Shell::SetWindowSubclass;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetCursorPos, GetWindowLongPtrW, SW_SHOW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_CAPTION, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_THICKFRAME,
};

use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use crate::geometry::Rect;

const SUBCLASS_ID: usize = 0xBEEF;

/// Timer ID for the modal move/resize loop render tick.
const MODAL_TIMER_ID: usize = 0xCAFE;

/// Timer interval during modal loop (~60 FPS).
const MODAL_TIMER_MS: u32 = 16;

/// Set while a Win32 modal move/resize loop is active.
///
/// During modal loops (`DragWindow`/`ResizeWindow`), the winit event loop
/// is blocked — `about_to_wait` never fires. A `SetTimer` ticks at 60 FPS,
/// invalidating all windows to generate `RedrawRequested` events inside
/// the modal message pump. The app's `RedrawRequested` handler checks this
/// flag to pump mux events and render all windows.
static IN_MODAL_LOOP: AtomicBool = AtomicBool::new(false);

/// Configuration for an OS drag session, passed to [`begin_os_drag()`].
pub struct OsDragConfig {
    /// Cursor-to-window-origin offset at the moment the drag started.
    /// `WM_MOVING` corrects the proposed rect every frame: `pos = cursor - grab_offset`.
    pub grab_offset: (i32, i32),
    /// Tab bar zones of other windows in screen coordinates.
    /// Each entry is `[left, top, right, tab_bar_bottom]`.
    pub merge_rects: Vec<[i32; 4]>,
    /// Number of `WM_MOVING` frames to skip merge detection after tear-off.
    pub skip_count: i32,
}

/// Result of an OS drag session, consumed by [`take_os_drag_result()`].
pub enum OsDragResult {
    /// OS drag ended normally (user released mouse).
    DragEnded {
        /// Screen cursor position at drag end.
        cursor: (i32, i32),
    },
    /// `WM_MOVING` detected cursor in a merge target's tab bar zone.
    /// Window was hidden and `ReleaseCapture` called.
    MergeDetected {
        /// Screen cursor position at merge detection.
        cursor: (i32, i32),
    },
}

/// Mutable state for an active OS drag session.
struct OsDragState {
    grab_offset: (i32, i32),
    merge_rects: Vec<[i32; 4]>,
    skip_remaining: i32,
    result: Option<OsDragResult>,
}

/// Per-window data stored via `SetWindowSubclass`.
struct SnapData {
    /// Border width for resize hit testing (physical pixels).
    border_width: Mutex<f32>,
    /// Caption (tab bar) height (physical pixels).
    caption_height: Mutex<f32>,
    /// Interactive regions (buttons, tabs) in physical pixels.
    interactive_rects: Mutex<Vec<Rect>>,
    /// DPI from the most recent `WM_DPICHANGED`. 0 means not yet received.
    ///
    /// Since we eat `WM_DPICHANGED` (return 0 without calling
    /// `DefSubclassProc`), winit never fires `ScaleFactorChanged`. The app
    /// must read this via [`get_current_dpi()`] in its resize handler.
    last_dpi: AtomicU32,
    /// Active OS drag session state.
    os_drag: Mutex<Option<OsDragState>>,
}

/// Global map from HWND (as usize) to `SnapData` pointer.
static SNAP_PTRS: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();

// Public API

/// Installs snap support on a borderless window.
///
/// Adds `WS_THICKFRAME | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_CAPTION` so
/// Windows recognizes the window for Aero Snap, hides the OS title bar via
/// DWM, and installs a `WndProc` subclass.
///
/// `border_width` and `caption_height` are in physical pixels (scaled by the
/// display scale factor). Use [`set_chrome_metrics()`] to update these after
/// a DPI change, and [`set_client_rects()`] to update interactive regions.
pub fn enable_snap(window: &Window, border_width: f32, caption_height: f32) {
    let Some(hwnd) = hwnd_from_window(window) else {
        log::warn!("enable_snap: failed to extract HWND — snap support not installed");
        return;
    };

    unsafe {
        // Add snap-enabling style bits.
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let snap_bits = (WS_THICKFRAME | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_CAPTION) as isize;
        SetWindowLongPtrW(hwnd, GWL_STYLE, style | snap_bits);

        // Force frame re-evaluation after style change.
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );

        // Hide OS title bar — 1px top margin keeps DWM shadow + snap preview.
        let margins = MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: 1,
            cyBottomHeight: 0,
        };
        DwmExtendFrameIntoClientArea(hwnd, &raw const margins);

        // Install `WndProc` subclass with per-window data.
        let data = Box::new(SnapData {
            border_width: Mutex::new(border_width),
            caption_height: Mutex::new(caption_height),
            interactive_rects: Mutex::new(Vec::new()),
            last_dpi: AtomicU32::new(0),
            os_drag: Mutex::new(None),
        });
        let data_ptr = Box::into_raw(data);
        SetWindowSubclass(
            hwnd,
            Some(subclass::subclass_proc),
            SUBCLASS_ID,
            data_ptr as usize,
        );

        // Register pointer for lookup by set_client_rects / set_chrome_metrics.
        let mut map = snap_ptrs().lock().unwrap_or_else(|e| {
            log::warn!("snap_ptrs mutex poisoned: {e}");
            e.into_inner()
        });
        map.insert(hwnd as usize, data_ptr as usize);
    }
}

/// Updates the interactive regions that receive `HTCLIENT` instead of
/// `HTCAPTION`.
///
/// Each rect is in logical coordinates. Call whenever the tab bar layout
/// changes (resize, tab add/remove).
pub fn set_client_rects(window: &Window, rects: Vec<Rect>) {
    if let Some(data) = snap_data_for_window(window) {
        let mut lock = data.interactive_rects.lock().unwrap_or_else(|e| {
            log::warn!("interactive_rects mutex poisoned: {e}");
            e.into_inner()
        });
        *lock = rects;
    }
}

/// Returns the scale factor from the last `WM_DPICHANGED`, or `None` if
/// no DPI change has been received yet.
///
/// When snap is enabled, this is the **only** source of DPI updates —
/// the subclass consumes `WM_DPICHANGED` before winit sees it, so
/// winit's `ScaleFactorChanged` event will not fire.
pub fn get_current_dpi(window: &Window) -> Option<f64> {
    let data = snap_data_for_window(window)?;
    let dpi = data.last_dpi.load(Ordering::Relaxed);
    if dpi == 0 {
        None
    } else {
        Some(f64::from(dpi) / 96.0)
    }
}

/// Begins an OS drag session for tab tear-off or single-tab window drag.
///
/// Stores drag state so `WM_MOVING` can correct window position and detect
/// cursor-based merges. Call before `window.drag_window()`.
pub fn begin_os_drag(window: &Window, config: OsDragConfig) {
    if let Some(data) = snap_data_for_window(window) {
        let mut lock = data.os_drag.lock().unwrap_or_else(|e| {
            log::warn!("os_drag mutex poisoned: {e}");
            e.into_inner()
        });
        *lock = Some(OsDragState {
            grab_offset: config.grab_offset,
            merge_rects: config.merge_rects,
            skip_remaining: config.skip_count,
            result: None,
        });
    }
}

/// Returns the result of a completed OS drag session, clearing the state.
///
/// Returns `None` if no drag session is active or it hasn't completed yet.
pub fn take_os_drag_result(window: &Window) -> Option<OsDragResult> {
    let data = snap_data_for_window(window)?;
    let mut lock = data.os_drag.lock().unwrap_or_else(|e| {
        log::warn!("os_drag mutex poisoned: {e}");
        e.into_inner()
    });
    let state = lock.as_mut()?;
    let result = state.result.take()?;
    *lock = None;
    Some(result)
}

/// Updates the caption height and border width after a DPI change.
///
/// Both values are in physical pixels (scaled by the new display scale
/// factor). Call from the resize handler when a DPI change is detected.
pub fn set_chrome_metrics(window: &Window, border_width: f32, caption_height: f32) {
    if let Some(data) = snap_data_for_window(window) {
        *data.border_width.lock().unwrap_or_else(|e| {
            log::warn!("border_width mutex poisoned: {e}");
            e.into_inner()
        }) = border_width;
        *data.caption_height.lock().unwrap_or_else(|e| {
            log::warn!("caption_height mutex poisoned: {e}");
            e.into_inner()
        }) = caption_height;
    }
}

// Platform helpers

/// Returns the current screen cursor position via `GetCursorPos`.
pub fn cursor_screen_pos() -> (i32, i32) {
    let mut pt = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&raw mut pt) };
    (pt.x, pt.y)
}

/// Returns the visible frame bounds excluding the invisible DWM extended
/// frame that `GetWindowRect` includes.
///
/// Returns `(left, top, right, bottom)` in screen coordinates.
pub fn visible_frame_bounds(window: &Window) -> Option<(i32, i32, i32, i32)> {
    let hwnd = hwnd_from_window(window)?;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let attr = DWMWA_EXTENDED_FRAME_BOUNDS as u32;
    let hr = unsafe {
        DwmGetWindowAttribute(hwnd, attr, (&raw mut rect).cast(), size_of::<RECT>() as u32)
    };
    if hr == 0 {
        Some((rect.left, rect.top, rect.right, rect.bottom))
    } else {
        None
    }
}

/// Shows a window that was hidden via `SW_HIDE` (used after merge-cancel).
///
/// Uses raw `ShowWindow(SW_SHOW)` to bypass winit's internal visibility
/// tracking, since `WM_MOVING` hides the window directly.
pub fn show_window(window: &Window) {
    if let Some(hwnd) = hwnd_from_window(window) {
        unsafe { ShowWindow(hwnd, SW_SHOW) };
    }
}

/// Releases mouse capture to prevent orphaned mouse-up events on exit.
pub fn release_mouse_capture() {
    unsafe { ReleaseCapture() };
}

/// Whether a Win32 modal move/resize loop is currently active.
///
/// Used by the event loop's `RedrawRequested` handler to substitute for
/// `about_to_wait` (which doesn't fire during the modal loop).
pub fn in_modal_loop() -> bool {
    IN_MODAL_LOOP.load(Ordering::Relaxed)
}

/// Disable or enable DWM window transition animations.
///
/// Chrome pattern: wrap `set_visible(true)` with `set_transitions_enabled(false/true)`
/// to prevent the OS fade-in animation during tab tear-off. This gives an
/// instantaneous window appearance instead of a distracting transition.
pub fn set_transitions_enabled(window: &Window, enabled: bool) {
    let Some(hwnd) = hwnd_from_window(window) else {
        return;
    };
    let value: i32 = i32::from(!enabled);
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED as u32,
            (&raw const value).cast(),
            size_of::<i32>() as u32,
        );
    }
}

// Private helpers

fn snap_ptrs() -> &'static Mutex<HashMap<usize, usize>> {
    SNAP_PTRS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Looks up the `SnapData` for a window. Valid until `WM_NCDESTROY`.
fn snap_data_for_window(window: &Window) -> Option<&'static SnapData> {
    let hwnd = hwnd_from_window(window)?;
    let ptr = {
        let map = snap_ptrs().lock().unwrap_or_else(|e| {
            log::warn!("snap_ptrs mutex poisoned: {e}");
            e.into_inner()
        });
        *map.get(&(hwnd as usize))?
    };
    Some(unsafe { &*(ptr as *const SnapData) })
}

/// Extracts the raw HWND from a winit `Window`.
fn hwnd_from_window(window: &Window) -> Option<HWND> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as HWND),
        _ => None,
    }
}
