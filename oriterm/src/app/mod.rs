//! Application struct and winit event loop handler.
//!
//! [`App`] implements winit's [`ApplicationHandler`] to drive the terminal.
//! It wires together the three-phase rendering pipeline (Extract → Prepare →
//! Render), handles window events, and dispatches terminal events from the
//! PTY reader thread.

mod chrome;
mod clipboard_ops;
pub(crate) mod config_reload;
mod cursor_blink;
mod cursor_hover;
mod divider_drag;
mod init;
mod keyboard_input;
mod mark_mode;
mod mouse_input;
mod mouse_report;
mod mouse_selection;
mod mux_pump;
mod pane_ops;
mod redraw;
mod search_ui;
mod tab_bar_input;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::WindowId;

use oriterm_core::TermMode;
use oriterm_mux::layout::DividerLayout;
use oriterm_mux::{PaneId, WindowId as MuxWindowId};

use self::cursor_blink::CursorBlink;
use self::keyboard_input::ImeState;
use self::mouse_selection::MouseState;
use crate::clipboard::Clipboard;
use crate::config::Config;
use crate::config::monitor::ConfigMonitor;
use crate::event::TermEvent;
use crate::gpu::{FrameInput, GpuRenderer, GpuState, PaneRenderCache};
use crate::keybindings::{self, KeyBinding};
use crate::mux::InProcessMux;
use crate::mux_event::MuxNotification;
use crate::pane::Pane;
use crate::url_detect::{DetectedUrl, UrlDetectCache};
use crate::widgets::terminal_grid::TerminalGridWidget;
use crate::window::TermWindow;

use oriterm_ui::overlay::OverlayManager;
use oriterm_ui::theme::UiTheme;
use oriterm_ui::widgets::tab_bar::TabBarWidget;
use oriterm_ui::widgets::window_chrome::WindowChromeWidget;

/// Default DPI for font rasterization.
const DEFAULT_DPI: f32 = 96.0;

/// Terminal application state and event loop handler.
///
/// Owns all top-level resources: GPU state, renderer, window, and tab.
/// Implements winit's `ApplicationHandler<TermEvent>` to receive both
/// window events and terminal events from the PTY reader thread.
pub(crate) struct App {
    // GPU + rendering (lazy init on Resumed).
    gpu: Option<GpuState>,
    renderer: Option<GpuRenderer>,
    window: Option<TermWindow>,

    // Mux layer (Section 31): owns registries, ID allocators, and domain.
    mux: Option<InProcessMux>,
    // Pane store: Pane structs live here, keyed by PaneId.
    // The mux tracks metadata (PaneEntry); App owns the actual Pane.
    panes: HashMap<PaneId, Pane>,
    // Active mux window ID (maps to the single TermWindow for now).
    active_window: Option<MuxWindowId>,
    // Double-buffer for mux notifications (avoids per-frame allocation).
    notification_buf: Vec<MuxNotification>,

    // Terminal grid widget (layout + event routing participant).
    terminal_grid: Option<TerminalGridWidget>,

    // Window chrome widget (title bar + controls).
    chrome: Option<WindowChromeWidget>,

    // Tab bar widget (tab strip rendering).
    tab_bar: Option<TabBarWidget>,

    // Event loop proxy for waking the event loop from background threads.
    event_proxy: EventLoopProxy<TermEvent>,

    // Per-pane render cache (multi-pane only; skips re-prepare for clean panes).
    pane_cache: PaneRenderCache,

    // Per-frame reusable extraction buffer (lazily initialized on first redraw).
    frame: Option<FrameInput>,

    // Reusable draw list for chrome rendering (avoids per-frame allocation).
    chrome_draw_list: oriterm_ui::draw::DrawList,

    // Redraw coalescing.
    dirty: bool,

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

    // Overlay manager for modal dialogs and popups.
    overlays: OverlayManager,

    // Text pending paste confirmation (stored while dialog is shown).
    pending_paste: Option<String>,

    // URL detection cache (lazily populated per logical line).
    url_cache: UrlDetectCache,

    // Currently hovered URL (set on Ctrl+mouse move, cleared on Ctrl release).
    hovered_url: Option<DetectedUrl>,

    // Divider currently under the cursor (for hover cursor icon).
    hovering_divider: Option<DividerLayout>,
    // Active divider drag state (ratio tracking during drag).
    divider_drag: Option<divider_drag::DividerDragState>,

    // Active UI theme. Centralized here so all widget creation and event
    // contexts use a single source of truth. When dynamic theming arrives,
    // only this field and the theme-change handler need updating.
    ui_theme: UiTheme,

    // Timestamp of last left-click in the tab bar drag area (for double-click maximize).
    last_drag_area_press: Option<Instant>,
}

impl App {
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
        Self {
            gpu: None,
            renderer: None,
            window: None,
            mux: None,
            panes: HashMap::new(),
            active_window: None,
            notification_buf: Vec::new(),
            terminal_grid: None,
            chrome: None,
            tab_bar: None,
            event_proxy,
            pane_cache: PaneRenderCache::new(),
            frame: None,
            chrome_draw_list: oriterm_ui::draw::DrawList::new(),
            dirty: false,
            modifiers: ModifiersState::empty(),
            cursor_blink: CursorBlink::new(blink_interval),
            blinking_active: false,
            mouse: MouseState::new(),
            clipboard: Clipboard::new(),
            config,
            bindings,
            _config_monitor: monitor,
            ime: ImeState::new(),
            overlays: OverlayManager::new(oriterm_ui::geometry::Rect::default()),
            pending_paste: None,
            url_cache: UrlDetectCache::default(),
            hovered_url: None,
            hovering_divider: None,
            divider_drag: None,
            ui_theme,
            last_drag_area_press: None,
        }
    }

    /// Re-rasterize fonts and update rendering settings for a new DPI scale.
    ///
    /// Called when the window moves between monitors with different scale
    /// factors. Recalculates font size at physical DPI, updates hinting
    /// and subpixel mode, and clears/recaches glyph atlases.
    fn handle_dpi_change(&mut self, scale_factor: f64) {
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
        if let Some(pane) = self.active_pane() {
            pane.terminal().lock().grid_mut().dirty_mut().mark_all();
        }

        // Invalidate pane render cache (atlas + cell metrics changed).
        self.pane_cache.invalidate_all();
        self.dirty = true;
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
        if let Some(chrome) = &mut self.chrome {
            chrome.apply_theme(&self.ui_theme);
        }
        if let Some(tab_bar) = &mut self.tab_bar {
            tab_bar.apply_theme(&self.ui_theme);
        }
        // Invalidate pane render cache (palette colors changed).
        self.pane_cache.invalidate_all();
        self.dirty = true;
    }

    /// Save pipeline cache and exit the process.
    ///
    /// Centralizes shutdown to avoid duplicating cleanup logic across
    /// `ChildExit`, `CloseRequested`, and `WindowClose`. wgpu
    /// `Device::drop()` calls `vkDeviceWaitIdle()` which blocks for
    /// seconds — the OS reclaims all GPU resources on process exit anyway.
    fn shutdown(&self, code: i32) -> ! {
        if let Some(gpu) = &self.gpu {
            gpu.save_pipeline_cache_async();
        }
        std::process::exit(code)
    }

    /// Read the terminal mode, locking briefly.
    ///
    /// Returns `None` if no active pane is present.
    fn terminal_mode(&self) -> Option<TermMode> {
        self.active_pane().map(|p| p.terminal().lock().mode())
    }

    // -- Mux pane accessors --

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
        self.panes.get(&id)
    }

    /// Mutable reference to the active pane.
    fn active_pane_mut(&mut self) -> Option<&mut Pane> {
        let id = self.active_pane_id()?;
        self.panes.get_mut(&id)
    }

    /// Current tab width lock value, if active.
    ///
    /// Delegates to the tab bar widget — the widget is the single source
    /// of truth for this value.
    pub(super) fn tab_width_lock(&self) -> Option<f32> {
        self.tab_bar.as_ref().and_then(TabBarWidget::tab_width_lock)
    }

    /// Freeze tab widths at `width` to prevent layout jitter.
    pub(super) fn acquire_tab_width_lock(&mut self, width: f32) {
        if let Some(tab_bar) = &mut self.tab_bar {
            tab_bar.set_tab_width_lock(Some(width));
        }
    }

    /// Sync tab bar widget titles from the current tab state.
    fn sync_tab_bar_titles(&mut self) {
        let title = self
            .active_pane()
            .map(|p| p.title().to_owned())
            .unwrap_or_default();
        if let Some(tab_bar) = &mut self.tab_bar {
            tab_bar.update_tab_title(0, title);
        }
        self.dirty = true;
    }

    /// Release the tab width lock, allowing tabs to recompute widths.
    pub(super) fn release_tab_width_lock(&mut self) {
        if self.tab_width_lock().is_some() {
            if let Some(tab_bar) = &mut self.tab_bar {
                tab_bar.set_tab_width_lock(None);
            }
            self.dirty = true;
        }
    }
}

/// Resolve the [`UiTheme`] from config override + system theme.
///
/// Maps [`ThemeOverride`] → [`UiTheme`]: `Dark` → `dark()`, `Light` → `light()`,
/// `Auto` → delegates to the provided system theme (falls back to dark on `Unknown`).
fn resolve_ui_theme_with(config: &Config, system: oriterm_core::Theme) -> UiTheme {
    use crate::config::ThemeOverride;

    match config.colors.theme {
        ThemeOverride::Dark => UiTheme::dark(),
        ThemeOverride::Light => UiTheme::light(),
        ThemeOverride::Auto => match system {
            oriterm_core::Theme::Light => UiTheme::light(),
            _ => UiTheme::dark(),
        },
    }
}

/// Resolve the [`UiTheme`] at startup by detecting the system theme.
fn resolve_ui_theme(config: &Config) -> UiTheme {
    resolve_ui_theme_with(config, crate::platform::theme::system_theme())
}

/// Convert winit modifier state to `oriterm_ui` modifier bitmask.
fn winit_mods_to_ui(state: ModifiersState) -> oriterm_ui::input::Modifiers {
    let mut m = oriterm_ui::input::Modifiers::NONE;
    if state.shift_key() {
        m = m.union(oriterm_ui::input::Modifiers::SHIFT_ONLY);
    }
    if state.control_key() {
        m = m.union(oriterm_ui::input::Modifiers::CTRL_ONLY);
    }
    if state.alt_key() {
        m = m.union(oriterm_ui::input::Modifiers::ALT_ONLY);
    }
    if state.super_key() {
        m = m.union(oriterm_ui::input::Modifiers::LOGO_ONLY);
    }
    m
}

#[cfg(test)]
mod tests;

impl ApplicationHandler<TermEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        if let Err(e) = self.try_init(event_loop) {
            log::error!("startup failed: {e}");
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.shutdown(0);
            }

            WindowEvent::Resized(size) => {
                self.handle_resize(size);
            }

            WindowEvent::RedrawRequested => self.handle_redraw(),

            WindowEvent::ModifiersChanged(mods) => {
                let prev_ctrl = self.modifiers.control_key();
                self.modifiers = mods.state();
                // Clear URL hover when Ctrl is released.
                if prev_ctrl && !mods.state().control_key() {
                    self.clear_url_hover();
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_keyboard_input(&event);
            }

            WindowEvent::Ime(ime) => self.handle_ime_event(ime),

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(window) = &mut self.window {
                    if window.update_scale_factor(scale_factor) {
                        self.handle_dpi_change(scale_factor);
                        self.update_resize_increments();
                    }
                }
            }

            WindowEvent::Focused(focused) => {
                if let Some(chrome) = &mut self.chrome {
                    chrome.set_active(focused);
                    self.dirty = true;
                }
            }

            WindowEvent::CursorLeft { .. } => {
                self.clear_chrome_hover();
                self.clear_tab_bar_hover();
                self.clear_url_hover();
                self.clear_divider_hover();
                self.cancel_divider_drag();
                self.release_tab_width_lock();
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse.set_cursor_pos(position);
                self.update_chrome_hover(position);
                self.update_tab_bar_hover(position);

                // Forward move events to overlays for per-widget hover tracking.
                if self.try_overlay_mouse_move(position) {
                    return;
                }

                // Divider hover/drag: check before terminal mouse handling.
                // Active drag consumes all moves.
                if self.update_divider_hover(position) {
                    return;
                }

                // Skip terminal mouse handling when the cursor is in the
                // chrome caption area. This avoids acquiring the terminal
                // lock on every cursor move over the title bar.
                if !self.cursor_in_chrome(position) {
                    if let Some(mode) = self.terminal_mode() {
                        if self.report_mouse_motion(position, mode) {
                            return;
                        }
                    }
                    if self.mouse.left_down() {
                        self.handle_mouse_drag(position);
                    }
                    // URL hover detection (Ctrl+move).
                    self.update_url_hover(position);
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                // Modal overlay: intercept mouse events.
                if self.try_overlay_mouse(button, state) {
                    return;
                }
                // Check chrome first — if a control button was clicked,
                // don't propagate to selection/PTY reporting.
                if self.try_chrome_mouse(button, state, event_loop) {
                    return;
                }
                // Tab bar clicks: switch tab, close tab, window controls, drag.
                if self.try_tab_bar_mouse(button, state, event_loop) {
                    return;
                }
                self.handle_mouse_input(button, state);
            }

            // Mouse wheel: report, alternate scroll, or viewport scroll.
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(mode) = self.terminal_mode() {
                    self.handle_mouse_wheel(delta, mode);
                }
            }

            // File drag-and-drop: paste paths into terminal.
            WindowEvent::DroppedFile(path) => {
                self.paste_dropped_files(&[path]);
                self.dirty = true;
            }

            WindowEvent::ThemeChanged(winit_theme) => {
                self.handle_theme_changed(winit_theme);
            }

            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: TermEvent) {
        match event {
            TermEvent::ConfigReload => {
                self.apply_config_reload();
            }
            TermEvent::MuxWakeup => {
                // The real work happens in `pump_mux_events()` during
                // `about_to_wait`. This wakeup ensures the event loop
                // doesn't sleep past pending mux events. The dirty flag
                // is a safety net for events (e.g. `ColorRequest`) that
                // produce a `MuxWakeup` without a corresponding `MuxEvent`.
                self.dirty = true;
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Pump mux events: drain PTY reader thread messages and process
        // resulting notifications before rendering.
        self.pump_mux_events(event_loop);

        // Drive cursor blink timer only when blinking is active.
        if self.blinking_active && self.cursor_blink.update() {
            self.dirty = true;
        }

        if self.dirty {
            // Clear dirty BEFORE rendering so that if handle_redraw sets
            // it back to true (e.g. chrome hover animations in progress),
            // the flag is preserved for the next frame.
            self.dirty = false;

            // Render directly instead of deferring via request_redraw().
            // On Windows, request_redraw() maps to WM_PAINT which has
            // lower priority than input messages (WM_MOUSEMOVE). Rapid
            // mouse movement delays painting indefinitely, causing visible
            // lag for hover effects. Rendering here — at the end of the
            // event batch — ensures the frame reflects the latest state.
            self.handle_redraw();
        }

        // Schedule wakeup for the next blink toggle so the event loop
        // doesn't sleep past it. When blinking is inactive, the default
        // ControlFlow::Wait lets the event loop sleep indefinitely.
        if self.blinking_active {
            let next_toggle = self.cursor_blink.next_toggle();
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_toggle));
        }
    }
}
