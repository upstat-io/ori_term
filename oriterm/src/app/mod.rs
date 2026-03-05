//! Application struct and winit event loop handler.
//!
//! [`App`] implements winit's [`ApplicationHandler`] to drive the terminal.
//! It wires together the three-phase rendering pipeline (Extract → Prepare →
//! Render), handles window events, and dispatches terminal events from the
//! PTY reader thread.

mod chrome;
mod clipboard_ops;
pub(crate) mod config_reload;
mod constructors;
mod context_menu;
mod cursor_blink;
mod cursor_hover;
mod divider_drag;
mod event_loop;
mod floating_drag;
mod init;
mod keyboard_input;
mod mark_mode;
mod mouse_input;
mod mouse_report;
mod mouse_selection;
mod mux_pump;
mod pane_ops;
mod perf_stats;
mod redraw;
mod search_ui;
pub(crate) mod snapshot_grid;
mod tab_bar_input;
mod tab_drag;
mod tab_management;
pub(crate) mod window_context;
mod window_management;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use winit::event_loop::EventLoopProxy;
use winit::keyboard::ModifiersState;
use winit::window::WindowId;

use oriterm_core::grid::StableRowIndex;
use oriterm_core::{Selection, SelectionPoint, TermMode};
use oriterm_mux::{MarkCursor, PaneId, WindowId as MuxWindowId};

use self::cursor_blink::CursorBlink;
use self::keyboard_input::ImeState;
use self::mouse_selection::MouseState;
use self::perf_stats::PerfStats;
use self::window_context::WindowContext;
use crate::clipboard::Clipboard;
use crate::config::Config;
use crate::config::monitor::ConfigMonitor;
use crate::event::TermEvent;
use crate::font::FontSet;
use crate::gpu::{GpuPipelines, GpuState, WindowRenderer};
use crate::keybindings::KeyBinding;
use oriterm_mux::backend::MuxBackend;
use oriterm_mux::mux_event::MuxNotification;

use oriterm_ui::theme::UiTheme;

/// Default DPI for font rasterization.
const DEFAULT_DPI: f32 = 96.0;

/// Minimum time between renders (~60 FPS cap).
///
/// Prevents burning CPU when PTY output is continuous. The event loop
/// defers rendering until this budget has elapsed since the last frame.
/// 16ms matches the typical 60 Hz display refresh — sufficient for a
/// terminal and leaves ample time for event processing between frames.
const FRAME_BUDGET: Duration = Duration::from_millis(16);

/// Terminal application state and event loop handler.
///
/// Owns all top-level resources: GPU state, renderer, windows, and mux.
/// Implements winit's `ApplicationHandler<TermEvent>` to receive both
/// window events and terminal events from the PTY reader thread.
///
/// Per-window state (widgets, caches, interaction) lives in [`WindowContext`]
/// inside the `windows` map.
pub(crate) struct App {
    // GPU + rendering (lazy init on Resumed).
    gpu: Option<GpuState>,
    /// Shared stateless GPU pipelines and bind group layouts.
    pipelines: Option<GpuPipelines>,
    /// Cached font set with user fallbacks pre-applied (cloned per new window).
    font_set: Option<FontSet>,
    /// Cached UI font set (avoids re-discovery per window).
    ui_font_set: Option<FontSet>,
    /// Number of user-configured fallbacks loaded (for `apply_font_config`).
    user_fb_count: usize,

    // Per-window state, keyed by winit WindowId for event routing.
    windows: HashMap<WindowId, WindowContext>,
    // Winit ID of the currently focused window (set on Focused(true)).
    focused_window_id: Option<WindowId>,

    // Mux backend (Section 44.3): abstracts in-process vs daemon mux access.
    // Owns pane structs (embedded) or proxies IPC (client).
    mux: Option<Box<dyn MuxBackend>>,
    // Active mux window ID (maps to the focused TermWindow).
    active_window: Option<MuxWindowId>,
    // Double-buffer for mux notifications (avoids per-frame allocation).
    notification_buf: Vec<MuxNotification>,

    // Keyboard modifier state (updated on ModifiersChanged).
    modifiers: ModifiersState,

    // Cursor blink state (application-level, not terminal-level).
    cursor_blink: CursorBlink,

    // Whether the terminal's CURSOR_BLINKING mode is active.
    // Cached from the last extracted frame to gate blink timer in about_to_wait.
    blinking_active: bool,

    // Mouse selection state (click detection, drag tracking).
    mouse: MouseState,

    // Per-pane selection state (Section 07: client-side selection).
    // Selection lives on App (not Pane) so daemon mode can operate on
    // snapshot data without locking the terminal.
    pane_selections: HashMap<PaneId, Selection>,

    // Per-pane mark cursor state (Section 08: client-side mark mode).
    // Mark cursor lives on App (not Pane) so daemon mode works.
    mark_cursors: HashMap<PaneId, MarkCursor>,

    // System clipboard for copy/paste.
    clipboard: Clipboard,

    // Event proxy for sending deferred actions through the event loop.
    event_proxy: EventLoopProxy<TermEvent>,

    // User configuration (loaded from TOML, hot-reloaded on file change).
    config: Config,

    // Merged keybinding table (defaults + user overrides).
    bindings: Vec<KeyBinding>,

    // Config file watcher (kept alive for the lifetime of the app).
    _config_monitor: Option<ConfigMonitor>,

    // IME composition state machine.
    ime: ImeState,

    // Active UI theme. Centralized here so all widget creation and event
    // contexts use a single source of truth. When dynamic theming arrives,
    // only this field and the theme-change handler need updating.
    ui_theme: UiTheme,

    // Pending tear-off state. Set by `tear_off_tab()`, consumed by
    // `check_torn_off_merge()` in `about_to_wait`.
    #[cfg(target_os = "windows")]
    torn_off_pending: Option<tab_drag::TornOffPending>,

    // Frame budget: time of last render to enforce FRAME_BUDGET spacing.
    last_render: Instant,

    // Performance counters logged periodically.
    perf: PerfStats,
}

impl App {
    // -- Window context accessors --

    /// The focused window's context, if any.
    fn focused_ctx(&self) -> Option<&WindowContext> {
        self.focused_window_id.and_then(|id| self.windows.get(&id))
    }

    /// The focused window's context (mutable), if any.
    fn focused_ctx_mut(&mut self) -> Option<&mut WindowContext> {
        self.focused_window_id
            .and_then(|id| self.windows.get_mut(&id))
    }

    /// The focused window's renderer, if any.
    fn focused_renderer(&self) -> Option<&WindowRenderer> {
        self.focused_window_id
            .and_then(|id| self.windows.get(&id))
            .and_then(|ctx| ctx.renderer.as_ref())
    }

    /// Mark all windows as needing a redraw.
    ///
    /// Used when mux notifications (PTY output, layout changes) may affect
    /// any window — not just the focused one. In multi-window setups, pane
    /// output in the unfocused window must still trigger a render.
    fn mark_all_windows_dirty(&mut self) {
        for ctx in self.windows.values_mut() {
            ctx.dirty = true;
        }
    }

    /// Re-rasterize fonts and update rendering settings for a new DPI scale.
    ///
    /// Called when the window moves between monitors with different scale
    /// factors. Recalculates font size at physical DPI, updates hinting
    /// and subpixel mode, and clears/recaches glyph atlases.
    ///
    /// `winit_id` identifies the window whose DPI changed. Only that
    /// window's renderer is affected — other windows keep their DPI.
    fn handle_dpi_change(&mut self, winit_id: WindowId, scale_factor: f64) {
        let Some(gpu) = &self.gpu else { return };
        let Some(ctx) = self.windows.get_mut(&winit_id) else {
            return;
        };
        let Some(renderer) = ctx.renderer.as_mut() else {
            return;
        };
        let scale = scale_factor as f32;
        let physical_dpi = DEFAULT_DPI * scale;

        // Re-rasterize at new physical DPI. This recomputes cell metrics
        // and clears the glyph cache + GPU atlases.
        renderer.set_font_size(self.config.font.size, physical_dpi, gpu);

        // Update hinting and subpixel mode for the new scale factor.
        let hinting = config_reload::resolve_hinting(&self.config.font, scale_factor);
        let format =
            config_reload::resolve_subpixel_mode(&self.config.font, scale_factor).glyph_format();
        renderer.set_hinting_and_format(hinting, format, gpu);

        ctx.pane_cache.invalidate_all();
        ctx.dirty = true;

        // Mark all grid lines dirty so the frame extraction re-reads every
        // cell with the new cell metrics. Without this, the terminal content
        // appears stale until PTY output marks individual lines dirty.
        if let Some(pane_id) = self.active_pane_id_for_window(winit_id) {
            if let Some(mux) = self.mux.as_mut() {
                mux.mark_all_dirty(pane_id);
            }
        }
    }

    /// Handle system dark/light theme change.
    ///
    /// Updates the terminal palette and UI chrome colors. Respects
    /// [`ThemeOverride`]: if the user forced dark/light, the system
    /// notification is ignored — only `Auto` delegates to the system.
    fn handle_theme_changed(&mut self, winit_theme: winit::window::Theme) {
        let system_theme = match winit_theme {
            winit::window::Theme::Dark => oriterm_core::Theme::Dark,
            winit::window::Theme::Light => oriterm_core::Theme::Light,
        };
        let theme = self.config.colors.resolve_theme(|| system_theme);
        let palette = config_reload::build_palette_from_config(&self.config.colors, theme);

        // Apply to all panes via MuxBackend.
        if let Some(mux) = self.mux.as_mut() {
            for pane_id in mux.pane_ids() {
                mux.set_pane_theme(pane_id, theme, palette.clone());
            }
        }

        // Update UI chrome theme (tab bar, window controls).
        self.ui_theme = resolve_ui_theme_with(&self.config, system_theme);
        for ctx in self.windows.values_mut() {
            ctx.chrome.apply_theme(&self.ui_theme);
            ctx.tab_bar.apply_theme(&self.ui_theme);
            ctx.pane_cache.invalidate_all();
            ctx.dirty = true;
        }
    }

    /// Read the terminal mode, locking briefly.
    ///
    /// Returns `None` if no active pane is present.
    fn terminal_mode(&self) -> Option<TermMode> {
        let id = self.active_pane_id()?;
        self.pane_mode(id)
    }

    // -- Mux pane accessors --

    /// The active pane's ID for a specific winit window.
    ///
    /// Resolves the mux window from the winit window context, then walks
    /// the session model (window → active tab → active pane) to find the
    /// `PaneId`. Used by window-specific operations (resize, DPI change).
    fn active_pane_id_for_window(&self, winit_id: WindowId) -> Option<PaneId> {
        let ctx = self.windows.get(&winit_id)?;
        let mux_wid = ctx.window.mux_window_id();
        let mux = self.mux.as_ref()?;
        let win = mux.session().get_window(mux_wid)?;
        let tab_id = win.active_tab()?;
        let tab = mux.session().get_tab(tab_id)?;
        Some(tab.active_pane())
    }

    /// The active pane's ID, derived from the mux session model.
    fn active_pane_id(&self) -> Option<PaneId> {
        let mux = self.mux.as_ref()?;
        let win_id = self.active_window?;
        let win = mux.session().get_window(win_id)?;
        let tab_id = win.active_tab()?;
        let tab = mux.session().get_tab(tab_id)?;
        Some(tab.active_pane())
    }

    /// Terminal mode flags for a pane.
    ///
    /// Delegates to [`MuxBackend::pane_mode`] — embedded mode reads the
    /// lock-free atomic cache, daemon mode reads the cached snapshot.
    fn pane_mode(&self, pane_id: PaneId) -> Option<TermMode> {
        self.mux
            .as_ref()?
            .pane_mode(pane_id)
            .map(TermMode::from_bits_truncate)
    }

    // -- Per-pane selection accessors --

    /// The active selection for a pane, if any.
    fn pane_selection(&self, pane_id: PaneId) -> Option<&Selection> {
        self.pane_selections.get(&pane_id)
    }

    /// Replace or create a selection for a pane.
    fn set_pane_selection(&mut self, pane_id: PaneId, sel: Selection) {
        self.pane_selections.insert(pane_id, sel);
    }

    /// Clear the selection for a pane.
    fn clear_pane_selection(&mut self, pane_id: PaneId) {
        self.pane_selections.remove(&pane_id);
    }

    /// Update the endpoint of an existing selection (drag).
    fn update_pane_selection_end(&mut self, pane_id: PaneId, end: SelectionPoint) {
        if let Some(sel) = self.pane_selections.get_mut(&pane_id) {
            sel.end = end;
        }
    }

    // -- Per-pane mark cursor accessors --

    /// Whether mark mode is active for a pane.
    fn is_mark_mode(&self, pane_id: PaneId) -> bool {
        self.mark_cursors.contains_key(&pane_id)
    }

    /// The mark cursor for a pane, if mark mode is active.
    fn pane_mark_cursor(&self, pane_id: PaneId) -> Option<MarkCursor> {
        self.mark_cursors.get(&pane_id).copied()
    }

    /// Enter mark mode for a pane, placing the cursor at the terminal cursor.
    ///
    /// Scrolls to bottom first, refreshes the snapshot, then reads the
    /// terminal cursor position from snapshot data.
    fn enter_mark_mode(&mut self, pane_id: PaneId) {
        if self.mark_cursors.contains_key(&pane_id) {
            return;
        }
        let Some(mux) = self.mux.as_mut() else { return };
        mux.scroll_to_bottom(pane_id);
        if mux.is_pane_snapshot_dirty(pane_id) || mux.pane_snapshot(pane_id).is_none() {
            mux.refresh_pane_snapshot(pane_id);
        }
        if let Some(snapshot) = self.mux.as_ref().and_then(|m| m.pane_snapshot(pane_id)) {
            let mc = MarkCursor {
                row: StableRowIndex(snapshot.stable_row_base + snapshot.cursor.row as u64),
                col: snapshot.cursor.col as usize,
            };
            self.mark_cursors.insert(pane_id, mc);
        }
    }

    /// Exit mark mode for a pane.
    fn exit_mark_mode(&mut self, pane_id: PaneId) {
        self.mark_cursors.remove(&pane_id);
    }

    /// Send input bytes to a pane.
    ///
    /// Delegates to [`MuxBackend::send_input`], which writes to the local PTY
    /// in embedded mode or sends through IPC in daemon mode.
    fn write_pane_input(&mut self, pane_id: PaneId, data: &[u8]) {
        if let Some(mux) = self.mux.as_mut() {
            mux.send_input(pane_id, data);
        }
    }

    /// If `winit_id` was the focused window, transfer focus to the next available.
    ///
    /// Updates both `focused_window_id` (winit) and `active_window` (mux).
    fn transfer_focus_from(&mut self, winit_id: WindowId) {
        if self.focused_window_id == Some(winit_id) {
            self.focused_window_id = self.windows.keys().next().copied();
            self.active_window = self
                .focused_window_id
                .and_then(|id| self.windows.get(&id).map(|ctx| ctx.window.mux_window_id()));
        }
    }

    /// Tab index for a given pane within the active window's tab list.
    ///
    /// Traverses pane registry → tab → window tab list to find the position.
    fn tab_index_for_pane(&self, pane_id: PaneId) -> Option<usize> {
        let mux = self.mux.as_ref()?;
        let tab_id = mux.get_pane_entry(pane_id)?.tab;
        let win_id = self.active_window?;
        let win = mux.session().get_window(win_id)?;
        win.tabs().iter().position(|&t| t == tab_id)
    }

    /// Drain the notification buffer and invoke `handler` on each notification.
    ///
    /// Takes the buffer from `self` to avoid borrow conflicts (the handler
    /// gets `&mut Self` without conflicting with the buffer), then restores
    /// it afterward to preserve `Vec` capacity across frames.
    fn with_drained_notifications(&mut self, mut handler: impl FnMut(&mut Self, MuxNotification)) {
        let mut buf = std::mem::take(&mut self.notification_buf);
        #[allow(
            clippy::iter_with_drain,
            reason = "drain preserves Vec capacity; into_iter drops it"
        )]
        for n in buf.drain(..) {
            handler(self, n);
        }
        self.notification_buf = buf;
    }

    /// Current tab width lock value, if active.
    ///
    /// Delegates to the tab bar widget — the widget is the single source
    /// of truth for this value.
    pub(super) fn tab_width_lock(&self) -> Option<f32> {
        self.focused_ctx()
            .and_then(|ctx| ctx.tab_bar.tab_width_lock())
    }

    /// Freeze tab widths at `width` to prevent layout jitter.
    pub(super) fn acquire_tab_width_lock(&mut self, width: f32) {
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.tab_bar.set_tab_width_lock(Some(width));
        }
    }

    /// Release the tab width lock, allowing tabs to recompute widths.
    pub(super) fn release_tab_width_lock(&mut self) {
        if self.tab_width_lock().is_some() {
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.tab_bar.set_tab_width_lock(None);
                ctx.dirty = true;
            }
        }
    }
}

use event_loop::{resolve_ui_theme, resolve_ui_theme_with, winit_mods_to_ui};

#[cfg(test)]
mod tests;
