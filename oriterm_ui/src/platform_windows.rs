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

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LRESULT, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_EXTENDED_FRAME_BOUNDS, DwmExtendFrameIntoClientArea, DwmGetWindowAttribute,
};
use windows_sys::Win32::UI::Controls::MARGINS;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetCursorPos, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, HTBOTTOM,
    HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT,
    HTTOPRIGHT, IsZoomed, NCCALCSIZE_PARAMS, SM_CXFRAME, SM_CXPADDEDBORDER, SM_CYFRAME, SW_HIDE,
    SW_SHOW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, WM_DPICHANGED, WM_EXITSIZEMOVE, WM_MOVING,
    WM_NCCALCSIZE, WM_NCDESTROY, WM_NCHITTEST, WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    WS_THICKFRAME,
};

use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use crate::geometry::{Point, Rect, Size};
use crate::hit_test::{self, HitTestResult, ResizeDirection};

const SUBCLASS_ID: usize = 0xBEEF;

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
    /// Logical border width for resize hit testing.
    border_width: f32,
    /// Logical caption (tab bar) height.
    caption_height: f32,
    /// Logical window size, updated via [`set_window_size()`].
    window_size: Mutex<Size>,
    /// Interactive regions (buttons, tabs) in logical coordinates.
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

// --- Public API -----------------------------------------------------------

/// Installs snap support on a borderless window.
///
/// Adds `WS_THICKFRAME | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_CAPTION` so
/// Windows recognizes the window for Aero Snap, hides the OS title bar via
/// DWM, and installs a `WndProc` subclass. Call [`set_window_size()`] after
/// this to initialize the cached size for hit testing.
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
            border_width,
            caption_height,
            window_size: Mutex::new(Size::default()),
            interactive_rects: Mutex::new(Vec::new()),
            last_dpi: AtomicU32::new(0),
            os_drag: Mutex::new(None),
        });
        let data_ptr = Box::into_raw(data);
        SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, data_ptr as usize);

        // Register pointer for lookup by set_client_rects / set_window_size.
        if let Ok(mut map) = snap_ptrs().lock() {
            map.insert(hwnd as usize, data_ptr as usize);
        }
    }
}

/// Updates the interactive regions that receive `HTCLIENT` instead of
/// `HTCAPTION`.
///
/// Each rect is in logical coordinates. Call whenever the tab bar layout
/// changes (resize, tab add/remove).
pub fn set_client_rects(window: &Window, rects: Vec<Rect>) {
    if let Some(data) = snap_data_for_window(window) {
        if let Ok(mut lock) = data.interactive_rects.lock() {
            *lock = rects;
        }
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
        if let Ok(mut lock) = data.os_drag.lock() {
            *lock = Some(OsDragState {
                grab_offset: config.grab_offset,
                merge_rects: config.merge_rects,
                skip_remaining: config.skip_count,
                result: None,
            });
        }
    }
}

/// Returns the result of a completed OS drag session, clearing the state.
///
/// Returns `None` if no drag session is active or it hasn't completed yet.
pub fn take_os_drag_result(window: &Window) -> Option<OsDragResult> {
    let data = snap_data_for_window(window)?;
    let mut lock = data.os_drag.lock().ok()?;
    let state = lock.as_mut()?;
    let result = state.result.take()?;
    *lock = None;
    Some(result)
}

/// Updates the cached window size (logical pixels) for hit testing.
///
/// Call on every resize event. `WM_NCHITTEST` uses this to determine
/// window bounds for the `hit_test()` function.
pub fn set_window_size(window: &Window, size: Size) {
    if let Some(data) = snap_data_for_window(window) {
        if let Ok(mut lock) = data.window_size.lock() {
            *lock = size;
        }
    }
}

// --- Platform helpers ------------------------------------------------------

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

// --- Private helpers -------------------------------------------------------

fn snap_ptrs() -> &'static Mutex<HashMap<usize, usize>> {
    SNAP_PTRS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Looks up the `SnapData` for a window. Valid until `WM_NCDESTROY`.
fn snap_data_for_window(window: &Window) -> Option<&'static SnapData> {
    let hwnd = hwnd_from_window(window)?;
    let ptr = {
        let map = snap_ptrs().lock().ok()?;
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

fn get_x_lparam(lp: isize) -> i32 {
    i32::from((lp & 0xFFFF) as i16)
}

fn get_y_lparam(lp: isize) -> i32 {
    i32::from(((lp >> 16) & 0xFFFF) as i16)
}

/// Returns the DPI scale factor (DPI / 96). Defaults to 1.0 if no
/// `WM_DPICHANGED` has been received.
fn dpi_scale_factor(data: &SnapData) -> f32 {
    let dpi = data.last_dpi.load(Ordering::Relaxed);
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

/// Maps a [`HitTestResult`] to a Windows HT constant.
fn map_hit_result(result: HitTestResult) -> LRESULT {
    (match result {
        HitTestResult::Client => HTCLIENT,
        HitTestResult::Caption => HTCAPTION,
        HitTestResult::ResizeBorder(dir) => match dir {
            ResizeDirection::Top => HTTOP,
            ResizeDirection::Bottom => HTBOTTOM,
            ResizeDirection::Left => HTLEFT,
            ResizeDirection::Right => HTRIGHT,
            ResizeDirection::TopLeft => HTTOPLEFT,
            ResizeDirection::TopRight => HTTOPRIGHT,
            ResizeDirection::BottomLeft => HTBOTTOMLEFT,
            ResizeDirection::BottomRight => HTBOTTOMRIGHT,
        },
    }) as LRESULT
}

// --- Message handlers (extracted from subclass_proc for clarity) -----------

/// Handles `WM_NCHITTEST` by delegating to [`hit_test::hit_test()`].
fn handle_nchittest(hwnd: HWND, lparam: isize, data: &SnapData) -> LRESULT {
    let cursor_x = get_x_lparam(lparam);
    let cursor_y = get_y_lparam(lparam);

    // Window rect in screen coordinates (physical pixels).
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetWindowRect(hwnd, &raw mut rect) };

    // Client-relative physical coordinates.
    let phys_x = cursor_x - rect.left;
    let phys_y = cursor_y - rect.top;

    // Convert to logical coordinates for hit_test().
    let scale = dpi_scale_factor(data);
    let point = Point::new(phys_x as f32 / scale, phys_y as f32 / scale);

    // Use cached size, falling back to window rect if not yet set.
    let window_size = {
        let cached = data.window_size.lock().map(|s| *s).unwrap_or_default();
        if cached.is_empty() {
            let w = (rect.right - rect.left) as f32 / scale;
            let h = (rect.bottom - rect.top) as f32 / scale;
            Size::new(w, h)
        } else {
            cached
        }
    };

    let is_maximized = unsafe { IsZoomed(hwnd) != 0 };

    let rects_lock = data.interactive_rects.lock();
    let rects: &[Rect] = rects_lock.as_ref().map(|g| g.as_slice()).unwrap_or(&[]);
    let result = hit_test::hit_test(
        point,
        window_size,
        data.border_width,
        data.caption_height,
        rects,
        is_maximized,
    );

    map_hit_result(result)
}

/// Handles `WM_MOVING`: position correction + cursor-based merge detection.
///
/// Modifies the proposed rect via `lparam` for position correction.
/// If a merge is detected, hides the window and releases capture.
/// Caller always calls `DefSubclassProc` afterward.
fn handle_moving(hwnd: HWND, lparam: isize, data: &SnapData) {
    let Ok(mut lock) = data.os_drag.lock() else {
        return;
    };
    let Some(state) = lock.as_mut() else {
        return;
    };

    let proposed = unsafe { &mut *(lparam as *mut RECT) };
    let w = proposed.right - proposed.left;
    let h = proposed.bottom - proposed.top;

    // Always correct position: window origin = cursor - grab_offset.
    let mut pt = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&raw mut pt) };
    let (gx, gy) = state.grab_offset;
    proposed.left = pt.x - gx;
    proposed.top = pt.y - gy;
    proposed.right = proposed.left + w;
    proposed.bottom = proposed.top + h;

    // Skip merge check during cooldown (position still corrected).
    if state.skip_remaining > 0 {
        state.skip_remaining -= 1;
        return;
    }

    // Cursor-based merge detection (Chrome's DoesTabStripContain pattern).
    for &[cl, ct, cr, ctb] in &state.merge_rects {
        if pt.x >= cl && pt.x < cr && pt.y >= ct && pt.y < ctb {
            state.result = Some(OsDragResult::MergeDetected {
                cursor: (pt.x, pt.y),
            });
            // Hide window + release capture to end the move loop.
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
                ReleaseCapture();
            }
            return;
        }
    }
}

// --- Subclass procedure ----------------------------------------------------

/// `WndProc` subclass callback installed by [`enable_snap()`].
///
/// `ref_data` is a valid `*const SnapData` allocated in `enable_snap` and
/// freed in the `WM_NCDESTROY` handler.
unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _uid: usize,
    ref_data: usize,
) -> LRESULT {
    unsafe {
        let data = &*(ref_data as *const SnapData);

        match msg {
            // Return 0 so the entire window is client area (no OS frame).
            // When maximized, inset by frame thickness to prevent
            // adjacent-monitor bleed (Chrome's GetClientAreaInsets pattern).
            WM_NCCALCSIZE if wparam == 1 => {
                if IsZoomed(hwnd) != 0 {
                    let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);
                    let fx = GetSystemMetrics(SM_CXFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
                    let fy = GetSystemMetrics(SM_CYFRAME) + GetSystemMetrics(SM_CXPADDEDBORDER);
                    params.rgrc[0].left += fx;
                    params.rgrc[0].top += fy;
                    params.rgrc[0].right -= fx;
                    params.rgrc[0].bottom -= fy;
                }
                0
            }

            WM_NCHITTEST => handle_nchittest(hwnd, lparam, data),

            WM_DPICHANGED => {
                // HIWORD(wParam) = new Y-axis DPI.
                let new_dpi = ((wparam >> 16) & 0xFFFF) as u32;
                data.last_dpi.store(new_dpi, Ordering::Relaxed);

                // Apply OS-suggested rect to prevent DPI oscillation.
                let suggested = &*(lparam as *const RECT);
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                0
            }

            WM_MOVING => {
                handle_moving(hwnd, lparam, data);
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }

            WM_EXITSIZEMOVE => {
                if let Ok(mut lock) = data.os_drag.lock() {
                    if let Some(state) = lock.as_mut() {
                        if state.result.is_none() {
                            let mut pt = POINT { x: 0, y: 0 };
                            GetCursorPos(&raw mut pt);
                            state.result = Some(OsDragResult::DragEnded {
                                cursor: (pt.x, pt.y),
                            });
                        }
                    }
                }
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }

            WM_NCDESTROY => {
                RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
                if let Ok(mut map) = snap_ptrs().lock() {
                    map.remove(&(hwnd as usize));
                }
                drop(Box::from_raw(ref_data as *mut SnapData));
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }

            _ => DefSubclassProc(hwnd, msg, wparam, lparam),
        }
    }
}
