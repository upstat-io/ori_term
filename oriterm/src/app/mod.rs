//! Application struct and winit event loop handler.
//!
//! [`App`] implements winit's [`ApplicationHandler`] to drive the terminal.
//! It wires together the three-phase rendering pipeline (Extract → Prepare →
//! Render), handles window events, and dispatches terminal events from the
//! PTY reader thread.

mod clipboard_ops;
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
use oriterm_ui::window::WindowConfig;

use self::cursor_blink::CursorBlink;
use self::mouse_selection::MouseState;
use crate::clipboard::Clipboard;
use crate::font::{HintingMode, SubpixelMode};
use crate::gpu::{FrameInput, GpuRenderer, GpuState};
use crate::tab::{Tab, TermEvent};
use crate::widgets::terminal_grid::TerminalGridWidget;
use crate::window::TermWindow;

/// Default font size in points (wired from config in Section 13).
const DEFAULT_FONT_SIZE_PT: f32 = 11.0;

/// Default DPI for font rasterization (wired from config in Section 13).
const DEFAULT_DPI: f32 = 96.0;

/// Default font weight (CSS-style 100–900).
const DEFAULT_FONT_WEIGHT: u16 = 400;

/// Default scrollback buffer size in lines.
const DEFAULT_SCROLLBACK: usize = 10_000;

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

    // Event loop proxy for creating per-tab EventProxy instances.
    event_proxy: EventLoopProxy<TermEvent>,

    // Per-frame reusable extraction buffer (lazily initialized on first redraw).
    frame: Option<FrameInput>,

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

    // Configuration.
    window_config: WindowConfig,
}

impl App {
    /// Create a new application instance.
    ///
    /// All GPU/window/tab state is `None` until [`resumed`] is called by
    /// the event loop (lazy initialization pattern from winit docs).
    pub(crate) fn new(event_proxy: EventLoopProxy<TermEvent>, window_config: WindowConfig) -> Self {
        Self {
            gpu: None,
            renderer: None,
            window: None,
            tab: None,
            terminal_grid: None,
            event_proxy,
            frame: None,
            dirty: false,
            modifiers: ModifiersState::empty(),
            cursor_blink: CursorBlink::new(),
            blinking_active: false,
            mouse: MouseState::new(),
            clipboard: Clipboard::new(),
            window_config,
        }
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
                            // TODO(section-13): wire CopyOnSelect config setting.
                            if had_drag {
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
    fn handle_terminal_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
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
                    gpu.save_pipeline_cache();
                }
                event_loop.exit();
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
                    gpu.save_pipeline_cache();
                }
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let (Some(gpu), Some(window), Some(renderer)) =
                    (&self.gpu, &mut self.window, &self.renderer)
                {
                    window.resize_surface(size.width, size.height, gpu);

                    let cell = renderer.cell_metrics();
                    let cols = cell.columns(size.width).max(1);
                    let rows = cell.rows(size.height).max(1);

                    if let Some(grid) = &mut self.terminal_grid {
                        grid.set_cell_metrics(cell.width, cell.height);
                        grid.set_grid_size(cols, rows);
                        grid.set_bounds(oriterm_ui::geometry::Rect::new(
                            0.0,
                            0.0,
                            cols as f32 * cell.width,
                            rows as f32 * cell.height,
                        ));
                    }

                    if let Some(tab) = &self.tab {
                        tab.resize(rows as u16, cols as u16);
                    }

                    self.dirty = true;
                }
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
                        // Re-evaluate hinting and subpixel mode for the new scale.
                        let hinting = HintingMode::from_scale_factor(scale_factor);
                        let format = SubpixelMode::from_scale_factor(scale_factor).glyph_format();
                        if let (Some(renderer), Some(gpu)) = (&mut self.renderer, &self.gpu) {
                            renderer.set_hinting_and_format(hinting, format, gpu);
                        }
                        self.dirty = true;
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse.set_cursor_pos(position);
                if let Some(mode) = self.terminal_mode() {
                    if self.report_mouse_motion(position, mode) {
                        return;
                    }
                }
                if self.mouse.left_down() {
                    self.handle_mouse_drag(position);
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
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

            // System dark/light theme changed — rebuild palette.
            WindowEvent::ThemeChanged(winit_theme) => {
                let theme = match winit_theme {
                    winit::window::Theme::Dark => oriterm_core::Theme::Dark,
                    winit::window::Theme::Light => oriterm_core::Theme::Light,
                };
                if let Some(tab) = &self.tab {
                    tab.terminal().lock().set_theme(theme);
                }
                self.dirty = true;
            }

            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: TermEvent) {
        match event {
            TermEvent::ConfigReload => {
                log::info!("config reload requested");
                // TODO(section-13.4): apply_config_reload()
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
            log::debug!("about_to_wait: dirty, requesting redraw");
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            self.dirty = false;
        }

        // Schedule wakeup for the next blink toggle so the event loop
        // doesn't sleep past it. When blinking is inactive, the default
        // ControlFlow::Wait lets the event loop sleep indefinitely.
        if self.blinking_active {
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.cursor_blink.next_toggle()));
        }
    }
}
