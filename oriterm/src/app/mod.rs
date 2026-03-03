//! Application struct and winit event loop handler.
//!
//! [`App`] implements winit's [`ApplicationHandler`] to drive the terminal.
//! It wires together the three-phase rendering pipeline (Extract → Prepare →
//! Render), handles window events, and dispatches terminal events from the
//! PTY reader thread.

mod chrome;
mod clipboard_ops;
pub(crate) mod config_reload;
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
mod tab_bar_input;
mod tab_drag;
mod tab_management;
pub(crate) mod window_context;
mod window_management;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::event_loop::EventLoopProxy;
use winit::keyboard::ModifiersState;
use winit::window::WindowId;

use oriterm_core::TermMode;
use oriterm_mux::{PaneId, WindowId as MuxWindowId};

use self::cursor_blink::CursorBlink;
use self::keyboard_input::ImeState;
use self::mouse_selection::MouseState;
use self::perf_stats::PerfStats;
use self::window_context::WindowContext;
use crate::clipboard::Clipboard;
use crate::config::Config;
use crate::config::monitor::ConfigMonitor;
use crate::event::TermEvent;
use crate::gpu::{GpuRenderer, GpuState};
use crate::keybindings::{self, KeyBinding};
use oriterm_mux::backend::MuxBackend;
use oriterm_mux::mux_event::MuxNotification;
use oriterm_mux::pane::Pane;

use oriterm_ui::theme::UiTheme;

/// Default DPI for font rasterization.
const DEFAULT_DPI: f32 = 96.0;

/// Minimum time between renders (~120 FPS cap).
///
/// Prevents burning CPU when PTY output is continuous. The event loop
/// defers rendering until this budget has elapsed since the last frame.
const FRAME_BUDGET: Duration = Duration::from_millis(8);

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
    renderer: Option<GpuRenderer>,

    // Per-window state, keyed by winit WindowId for event routing.
    windows: HashMap<WindowId, WindowContext>,
    // Winit ID of the currently focused window (set on Focused(true)).
    focused_window_id: Option<WindowId>,

    // Mux backend (Section 44.3): abstracts in-process vs daemon mux access.
    // Owns pane structs (embedded) or proxies IPC (client).
    mux: Option<Box<dyn MuxBackend>>,
    // Wakeup callback for mux backends (shared across fallback transitions).
    mux_wakeup: Arc<dyn Fn() + Send + Sync>,
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

    // System clipboard for copy/paste.
    clipboard: Clipboard,

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

    // Deferred window creation request. Set by keybinding actions that
    // need `ActiveEventLoop` (which keyboard input handlers lack).
    // Processed in `about_to_wait` where the event loop is available.
    pending_new_window: bool,

    // Deferred move-tab-to-new-window request. Stores the tab index to
    // resolve when `ActiveEventLoop` is available in `about_to_wait`.
    pending_move_tab_to_window: Option<usize>,

    // Pending tear-off state. Set by `tear_off_tab()`, consumed by
    // `check_torn_off_merge()` in `about_to_wait`.
    #[cfg(target_os = "windows")]
    torn_off_pending: Option<tab_drag::TornOffPending>,

    // Suppress the stale WM_LBUTTONUP after a live merge.
    merge_drag_suppress_release: bool,

    // Frame budget: time of last render to enforce FRAME_BUDGET spacing.
    last_render: Instant,

    // Performance counters logged periodically.
    perf: PerfStats,
}

impl App {
    /// Create a new application instance in daemon mode.
    ///
    /// Instead of an embedded mux, connects to a running `oriterm-mux`
    /// daemon at `socket_path`. If `window_id` is provided, claims an
    /// existing mux window; otherwise creates a new one during init.
    #[cfg(unix)]
    pub(crate) fn new_daemon(
        event_proxy: EventLoopProxy<TermEvent>,
        config: Config,
        socket_path: &std::path::Path,
        window_id: Option<u64>,
    ) -> Self {
        let bindings = keybindings::merge_bindings(&config.keybind);
        let monitor = ConfigMonitor::new(event_proxy.clone());
        let blink_interval = Duration::from_millis(config.terminal.cursor_blink_interval_ms);
        let ui_theme = resolve_ui_theme(&config);
        let mux_wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = event_proxy.send_event(TermEvent::MuxWakeup);
        });

        let mux: Option<Box<dyn MuxBackend>> =
            match oriterm_mux::MuxClient::connect(socket_path, mux_wakeup.clone()) {
                Ok(client) => {
                    log::info!("daemon mode: connected to {}", socket_path.display());
                    Some(Box::new(client))
                }
                Err(e) => {
                    log::error!(
                        "failed to connect to daemon at {}: {e}",
                        socket_path.display()
                    );
                    None
                }
            };

        let mut app = Self {
            gpu: None,
            renderer: None,
            windows: HashMap::new(),
            focused_window_id: None,
            mux,
            mux_wakeup,
            active_window: None,
            notification_buf: Vec::new(),
            modifiers: ModifiersState::empty(),
            cursor_blink: CursorBlink::new(blink_interval),
            blinking_active: false,
            mouse: MouseState::new(),
            clipboard: Clipboard::new(),
            config,
            bindings,
            _config_monitor: monitor,
            ime: ImeState::new(),
            ui_theme,
            pending_new_window: false,
            pending_move_tab_to_window: None,
            #[cfg(target_os = "windows")]
            torn_off_pending: None,
            merge_drag_suppress_release: false,
            last_render: Instant::now(),
            perf: PerfStats::new(),
        };

        // Store the claimed window ID so init can use it instead of creating one.
        if let Some(wid) = window_id {
            app.active_window = Some(oriterm_mux::WindowId::from_raw(wid));
        }

        app
    }

    /// Create a new application instance.
    ///
    /// All GPU/window/tab state is `None` until [`resumed`] is called by
    /// the event loop (lazy initialization pattern from winit docs).
    pub(crate) fn new(event_proxy: EventLoopProxy<TermEvent>, config: Config) -> Self {
        let bindings = keybindings::merge_bindings(&config.keybind);
        let monitor = ConfigMonitor::new(event_proxy.clone());
        let (builtin_count, user_count) = crate::scheme::discover_count();
        log::info!(
            "themes: {} available ({} built-in, {} user)",
            builtin_count + user_count,
            builtin_count,
            user_count,
        );
        let blink_interval = Duration::from_millis(config.terminal.cursor_blink_interval_ms);
        let ui_theme = resolve_ui_theme(&config);
        let mux_wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = event_proxy.send_event(TermEvent::MuxWakeup);
        });
        let mux = oriterm_mux::EmbeddedMux::new(mux_wakeup.clone());
        Self {
            gpu: None,
            renderer: None,
            windows: HashMap::new(),
            focused_window_id: None,
            mux: Some(Box::new(mux)),
            mux_wakeup,
            active_window: None,
            notification_buf: Vec::new(),
            modifiers: ModifiersState::empty(),
            cursor_blink: CursorBlink::new(blink_interval),
            blinking_active: false,
            mouse: MouseState::new(),
            clipboard: Clipboard::new(),
            config,
            bindings,
            _config_monitor: monitor,
            ime: ImeState::new(),
            ui_theme,
            pending_new_window: false,
            pending_move_tab_to_window: None,
            #[cfg(target_os = "windows")]
            torn_off_pending: None,
            merge_drag_suppress_release: false,
            last_render: Instant::now(),
            perf: PerfStats::new(),
        }
    }

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

    /// Re-rasterize fonts and update rendering settings for a new DPI scale.
    ///
    /// Called when the window moves between monitors with different scale
    /// factors. Recalculates font size at physical DPI, updates hinting
    /// and subpixel mode, and clears/recaches glyph atlases.
    ///
    /// `winit_id` identifies the window whose DPI changed. Font
    /// re-rasterization affects the shared renderer (all windows share one),
    /// but cache invalidation and dirty marking target only this window.
    fn handle_dpi_change(&mut self, winit_id: WindowId, scale_factor: f64) {
        let (Some(renderer), Some(gpu)) = (&mut self.renderer, &self.gpu) else {
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

        // Mark all grid lines dirty so the frame extraction re-reads every
        // cell with the new cell metrics. Without this, the terminal content
        // appears stale until PTY output marks individual lines dirty.
        if let Some(pane) = self.active_pane_for_window(winit_id) {
            pane.terminal().lock().grid_mut().dirty_mut().mark_all();
        }

        // Invalidate pane render cache (atlas + cell metrics changed).
        if let Some(ctx) = self.windows.get_mut(&winit_id) {
            ctx.pane_cache.invalidate_all();
            ctx.dirty = true;
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
        if let Some(pane) = self.active_pane() {
            let mut term = pane.terminal().lock();
            term.set_theme(theme);
            let palette = config_reload::build_palette_from_config(&self.config.colors, theme);
            *term.palette_mut() = palette;
            term.grid_mut().dirty_mut().mark_all();
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
        self.active_pane().map(|p| p.terminal().lock().mode())
    }

    // -- Mux pane accessors --

    /// The active pane for a specific winit window.
    ///
    /// Resolves the mux window from the winit window context, then walks
    /// the session model (window → active tab → active pane) to find the
    /// pane. Used by window-specific operations (resize, DPI change) that
    /// cannot rely on `active_pane()` which uses the globally focused window.
    fn active_pane_for_window(&self, winit_id: WindowId) -> Option<&Pane> {
        let ctx = self.windows.get(&winit_id)?;
        let mux_wid = ctx.window.mux_window_id();
        let mux = self.mux.as_ref()?;
        let win = mux.session().get_window(mux_wid)?;
        let tab_id = win.active_tab()?;
        let tab = mux.session().get_tab(tab_id)?;
        let pane_id = tab.active_pane();
        mux.pane(pane_id)
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

    /// Immutable reference to the active pane.
    fn active_pane(&self) -> Option<&Pane> {
        let id = self.active_pane_id()?;
        self.mux.as_ref()?.pane(id)
    }

    /// Mutable reference to the active pane.
    fn active_pane_mut(&mut self) -> Option<&mut Pane> {
        let id = self.active_pane_id()?;
        self.mux.as_mut()?.pane_mut(id)
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

/// Apply the color palette to a pane's terminal without borrowing `App`.
///
/// Free function taking `&Config` directly, safe to call while `self.mux`
/// is mutably borrowed (no `&self` conflict).
fn apply_palette(config: &Config, pane: &Pane, theme: oriterm_core::Theme) {
    let mut term = pane.terminal().lock();
    let palette = config_reload::build_palette_from_config(&config.colors, theme);
    *term.palette_mut() = palette;
}

use event_loop::{resolve_ui_theme, resolve_ui_theme_with, winit_mods_to_ui};

#[cfg(test)]
mod tests;
