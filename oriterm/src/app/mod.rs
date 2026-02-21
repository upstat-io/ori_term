//! Application struct and winit event loop handler.
//!
//! [`App`] implements winit's [`ApplicationHandler`] to drive the terminal.
//! It wires together the three-phase rendering pipeline (Extract → Prepare →
//! Render), handles window events, and dispatches terminal events from the
//! PTY reader thread.

mod clipboard_ops;
mod cursor_blink;
mod init;
mod mark_mode;
mod mouse_selection;
mod redraw;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey, SmolStr};
use winit::window::WindowId;

use oriterm_core::Event;
use oriterm_ui::window::WindowConfig;

use self::cursor_blink::CursorBlink;
use self::mouse_selection::MouseState;
use crate::clipboard::Clipboard;
use crate::font::{HintingMode, SubpixelMode};
use crate::gpu::{FrameInput, GpuRenderer, GpuState};
use crate::key_encoding::{self, KeyEventType, KeyInput};
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

    /// Dispatch a keyboard event through mark mode or key encoding to the PTY.
    ///
    /// Mark mode intercepts all key events when active. Otherwise, reads the
    /// terminal mode, converts winit modifiers to key encoding modifiers,
    /// encodes the key event, and sends the result to the PTY.
    fn handle_keyboard_input(&mut self, event: &winit::event::KeyEvent) {
        // Mark mode: consume ALL key events (including releases) to prevent
        // leaking input to the PTY while navigating.
        if let Some(tab) = &mut self.tab {
            if tab.is_mark_mode() {
                if event.state == ElementState::Pressed {
                    let action = mark_mode::handle_mark_mode_key(tab, event, self.modifiers);
                    match action {
                        mark_mode::MarkAction::Handled => {
                            self.dirty = true;
                        }
                        mark_mode::MarkAction::Exit { copy } => {
                            if copy {
                                self.copy_selection();
                            }
                            self.dirty = true;
                        }
                        mark_mode::MarkAction::Ignored => {}
                    }
                }
                return;
            }
        }

        // Ctrl+Shift+M enters mark mode.
        if event.state == ElementState::Pressed && !event.repeat {
            if self.modifiers.control_key()
                && self.modifiers.shift_key()
                && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyM))
            {
                if let Some(tab) = &mut self.tab {
                    tab.enter_mark_mode();
                    self.dirty = true;
                }
                return;
            }
        }

        // Copy keybindings: Ctrl+Shift+C, smart Ctrl+C, Ctrl+Insert.
        if matches!(
            self.try_copy_keybinding(event, self.modifiers),
            clipboard_ops::CopyAction::Handled,
        ) {
            self.dirty = true;
            return;
        }

        // Normal key encoding to PTY.
        let Some(tab) = &self.tab else { return };

        let mode = tab.terminal().lock().mode();

        let event_type = match (event.state, event.repeat) {
            (ElementState::Released, _) => KeyEventType::Release,
            (ElementState::Pressed, true) => KeyEventType::Repeat,
            (ElementState::Pressed, false) => KeyEventType::Press,
        };

        let bytes = key_encoding::encode_key(&KeyInput {
            key: &event.logical_key,
            mods: self.modifiers.into(),
            mode,
            text: event.text.as_ref().map(SmolStr::as_str),
            location: event.location,
            event_type,
        });

        if !bytes.is_empty() {
            tab.scroll_to_bottom();
            tab.write_input(&bytes);
            self.cursor_blink.reset();
            self.dirty = true;
        }
    }

    /// Handle IME commit: send committed text directly to the PTY.
    fn handle_ime_commit(&mut self, text: &str) {
        let Some(tab) = &self.tab else { return };
        if !text.is_empty() {
            tab.scroll_to_bottom();
            tab.write_input(text.as_bytes());
            self.cursor_blink.reset();
            self.dirty = true;
        }
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
                            renderer.set_hinting_mode(hinting, gpu);
                            renderer.set_glyph_format(format, gpu);
                        }
                        self.dirty = true;
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse.set_cursor_pos(position);
                if self.mouse.left_down() {
                    self.handle_mouse_drag(position);
                }
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
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
            },

            // Right-click: copy if selection exists.
            // TODO(section-21): when context menu is enabled, show menu instead.
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                self.copy_selection();
            }

            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: TermEvent) {
        let TermEvent::Terminal { tab_id: _, event } = event;
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
