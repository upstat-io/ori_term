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
mod init;
mod keyboard_input;
mod mark_mode;
mod mouse_report;
mod mouse_selection;
mod redraw;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::WindowId;

use oriterm_core::{Event, TermMode};

use self::cursor_blink::CursorBlink;
use self::mouse_selection::MouseState;
use crate::clipboard::Clipboard;
use crate::config::Config;
use crate::config::monitor::ConfigMonitor;
use crate::event::TermEvent;
use crate::gpu::{FrameInput, GpuRenderer, GpuState};
use crate::keybindings::{self, KeyBinding};
use crate::tab::Tab;
use crate::widgets::terminal_grid::TerminalGridWidget;
use crate::window::TermWindow;

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

    // Terminal state (single tab for now; multi-tab in Section 15).
    tab: Option<Tab>,

    // Terminal grid widget (layout + event routing participant).
    terminal_grid: Option<TerminalGridWidget>,

    // Window chrome widget (title bar + controls).
    chrome: Option<WindowChromeWidget>,

    // Event loop proxy for creating per-tab EventProxy instances.
    event_proxy: EventLoopProxy<TermEvent>,

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
}

impl App {
    /// Create a new application instance.
    ///
    /// All GPU/window/tab state is `None` until [`resumed`] is called by
    /// the event loop (lazy initialization pattern from winit docs).
    pub(crate) fn new(event_proxy: EventLoopProxy<TermEvent>, config: Config) -> Self {
        let bindings = keybindings::merge_bindings(&config.keybind);
        let monitor = ConfigMonitor::new(event_proxy.clone());
        let blink_interval =
            std::time::Duration::from_millis(config.terminal.cursor_blink_interval_ms);
        Self {
            gpu: None,
            renderer: None,
            window: None,
            tab: None,
            terminal_grid: None,
            chrome: None,
            event_proxy,
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
        if let Some(tab) = &self.tab {
            tab.terminal().lock().grid_mut().dirty_mut().mark_all();
        }

        self.dirty = true;
    }

    /// Read the terminal mode, locking briefly.
    ///
    /// Returns `None` if no tab is present.
    fn terminal_mode(&self) -> Option<TermMode> {
        self.tab.as_ref().map(|t| t.terminal().lock().mode())
    }

    /// Handle mouse press for selection.
    fn handle_mouse_press(&mut self) {
        let pos = self.mouse.cursor_pos();
        if let (Some(tab), Some(grid), Some(renderer)) =
            (&mut self.tab, &self.terminal_grid, &self.renderer)
        {
            let ctx = mouse_selection::GridCtx {
                widget: grid,
                cell: renderer.cell_metrics(),
                word_delimiters: &self.config.behavior.word_delimiters,
            };
            if mouse_selection::handle_press(&mut self.mouse, tab, &ctx, pos, self.modifiers) {
                self.dirty = true;
            }
        }
    }

    /// Handle mouse drag for selection.
    fn handle_mouse_drag(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
        if let (Some(tab), Some(grid), Some(renderer)) =
            (&mut self.tab, &self.terminal_grid, &self.renderer)
        {
            let ctx = mouse_selection::GridCtx {
                widget: grid,
                cell: renderer.cell_metrics(),
                word_delimiters: &self.config.behavior.word_delimiters,
            };
            if mouse_selection::handle_drag(&mut self.mouse, tab, &ctx, position) {
                self.dirty = true;
            }
        }
    }

    /// Handle a mouse button event (left, middle, right).
    fn handle_mouse_input(&mut self, button: MouseButton, state: ElementState) {
        // Track button state unconditionally — mouse reporting needs this
        // for drag/motion events even when the press itself was reported.
        let pressed = state == ElementState::Pressed;
        self.mouse.set_button_down(button, pressed);

        // Read terminal mode once (single lock acquisition) and determine
        // whether mouse events should be reported to the PTY.
        let report_mode = self
            .terminal_mode()
            .filter(|&m| self.should_report_mouse(m));

        match button {
            MouseButton::Left => {
                if let Some(mode) = report_mode {
                    let kind = match state {
                        ElementState::Pressed => mouse_report::MouseEventKind::Press,
                        ElementState::Released => mouse_report::MouseEventKind::Release,
                    };
                    self.report_mouse_button(mouse_report::MouseButton::Left, kind, mode);
                } else {
                    match state {
                        ElementState::Pressed => self.handle_mouse_press(),
                        ElementState::Released => {
                            let had_drag = self.mouse.is_dragging();
                            mouse_selection::handle_release(&mut self.mouse);
                            // CopyOnSelect: auto-copy to primary selection after drag.
                            if had_drag && self.config.behavior.copy_on_select {
                                self.copy_selection_to_primary();
                            }
                        }
                    }
                }
            }
            MouseButton::Middle => {
                if let Some(mode) = report_mode {
                    let kind = match state {
                        ElementState::Pressed => mouse_report::MouseEventKind::Press,
                        ElementState::Released => mouse_report::MouseEventKind::Release,
                    };
                    self.report_mouse_button(mouse_report::MouseButton::Middle, kind, mode);
                } else if state == ElementState::Pressed {
                    self.paste_from_primary();
                    self.dirty = true;
                } else {
                    // Release without reporting: no action needed.
                }
            }
            MouseButton::Right => {
                if let Some(mode) = report_mode {
                    let kind = match state {
                        ElementState::Pressed => mouse_report::MouseEventKind::Press,
                        ElementState::Released => mouse_report::MouseEventKind::Release,
                    };
                    self.report_mouse_button(mouse_report::MouseButton::Right, kind, mode);
                } else if state == ElementState::Pressed {
                    let has_sel = self.tab.as_ref().is_some_and(|t| t.selection().is_some());
                    if has_sel {
                        self.copy_selection();
                    } else {
                        self.paste_from_clipboard();
                    }
                    self.dirty = true;
                } else {
                    // Release without reporting: no action needed.
                }
            }
            _ => {}
        }
    }

    /// Dispatch a terminal event from the PTY reader thread.
    fn handle_terminal_event(&mut self, _event_loop: &ActiveEventLoop, event: Event) {
        match event {
            Event::Wakeup => {
                if let Some(tab) = &mut self.tab {
                    tab.check_selection_invalidation();
                }
                self.dirty = true;
            }
            Event::Bell => {
                if let Some(tab) = &mut self.tab {
                    tab.set_bell();
                }
            }
            Event::Title(title) => {
                if let Some(tab) = &mut self.tab {
                    tab.set_title(title);
                }
            }
            Event::ResetTitle => {
                if let Some(tab) = &mut self.tab {
                    tab.set_title(String::new());
                }
            }
            Event::ClipboardStore(ty, text) => {
                self.clipboard.store(ty, &text);
            }
            Event::ClipboardLoad(ty, formatter) => {
                let text = self.clipboard.load(ty);
                let response = formatter(&text);
                if let Some(tab) = &self.tab {
                    tab.write_input(response.as_bytes());
                }
            }
            Event::PtyWrite(s) => {
                if let Some(tab) = &self.tab {
                    tab.write_input(s.as_bytes());
                }
            }
            Event::ChildExit(code) => {
                log::info!("child process exited with code {code}");
                if let Some(gpu) = &self.gpu {
                    gpu.save_pipeline_cache_async();
                }
                std::process::exit(code);
            }
            _ => {
                log::debug!("unhandled terminal event: {event:?}");
            }
        }
    }
}

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
                if let Some(gpu) = &self.gpu {
                    gpu.save_pipeline_cache_async();
                }
                std::process::exit(0);
            }

            WindowEvent::Resized(size) => {
                self.handle_resize(size);
            }

            WindowEvent::RedrawRequested => self.handle_redraw(),

            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_keyboard_input(&event);
            }

            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                self.handle_ime_commit(&text);
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(window) = &mut self.window {
                    if window.update_scale_factor(scale_factor) {
                        self.handle_dpi_change(scale_factor);
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
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse.set_cursor_pos(position);
                self.update_chrome_hover(position);

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
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                // Check chrome first — if a control button was clicked,
                // don't propagate to selection/PTY reporting.
                if self.try_chrome_mouse(button, state, event_loop) {
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

            // System dark/light theme changed — rebuild palette with config override.
            WindowEvent::ThemeChanged(winit_theme) => {
                // Respect ThemeOverride: if the user forced dark/light, ignore
                // the system notification. Only Auto delegates to the system.
                let system_theme = match winit_theme {
                    winit::window::Theme::Dark => oriterm_core::Theme::Dark,
                    winit::window::Theme::Light => oriterm_core::Theme::Light,
                };
                let theme = self.config.colors.resolve_theme(|| system_theme);
                if let Some(tab) = &self.tab {
                    let mut term = tab.terminal().lock();
                    term.set_theme(theme);
                    config_reload::apply_color_overrides(term.palette_mut(), &self.config.colors);
                }
                self.dirty = true;
            }

            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: TermEvent) {
        match event {
            TermEvent::ConfigReload => {
                self.apply_config_reload();
            }
            TermEvent::Terminal { tab_id: _, event } => {
                self.handle_terminal_event(event_loop, event);
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
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
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.cursor_blink.next_toggle()));
        }
    }
}
