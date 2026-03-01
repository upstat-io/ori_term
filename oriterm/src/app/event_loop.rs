//! Winit `ApplicationHandler` impl and helper free functions.
//!
//! Separated from `mod.rs` to keep the main module definition file under the
//! 500-line limit. Contains the event dispatch table and theme/modifier
//! conversion utilities.

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::ModifiersState;
use winit::window::WindowId;

use oriterm_core::Theme;
use oriterm_ui::theme::UiTheme;

use super::App;
use crate::config::Config;
use crate::event::TermEvent;

/// Resolve the [`UiTheme`] from config override + system theme.
///
/// Maps [`ThemeOverride`] → [`UiTheme`]: `Dark` → `dark()`, `Light` → `light()`,
/// `Auto` → delegates to the provided system theme (falls back to dark on `Unknown`).
pub(super) fn resolve_ui_theme_with(config: &Config, system: Theme) -> UiTheme {
    use crate::config::ThemeOverride;

    match config.colors.theme {
        ThemeOverride::Dark => UiTheme::dark(),
        ThemeOverride::Light => UiTheme::light(),
        ThemeOverride::Auto => match system {
            Theme::Light => UiTheme::light(),
            _ => UiTheme::dark(),
        },
    }
}

/// Resolve the [`UiTheme`] at startup by detecting the system theme.
pub(super) fn resolve_ui_theme(config: &Config) -> UiTheme {
    resolve_ui_theme_with(config, crate::platform::theme::system_theme())
}

/// Convert winit modifier state to `oriterm_ui` modifier bitmask.
pub(super) fn winit_mods_to_ui(state: ModifiersState) -> oriterm_ui::input::Modifiers {
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

impl App {
    /// Send a focus-in or focus-out escape sequence to the active pane.
    ///
    /// Only sends when the terminal has `FOCUS_IN_OUT` mode enabled (mode 1004).
    /// Focus-in: `CSI I` (`\x1b[I`), focus-out: `CSI O` (`\x1b[O`).
    fn send_focus_event(&self, focused: bool) {
        let Some(pane) = self.active_pane() else {
            return;
        };
        let mode = pane.mode();
        if mode & oriterm_core::TermMode::FOCUS_IN_OUT.bits() == 0 {
            return;
        }
        let seq = if focused { b"\x1b[I" } else { b"\x1b[O" };
        pane.write_input(seq);
    }
}

impl ApplicationHandler<TermEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.windows.is_empty() {
            return;
        }
        if let Err(e) = self.try_init(event_loop) {
            log::error!("startup failed: {e}");
            event_loop.exit();
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "event dispatch table — inherently one arm per event variant"
    )]
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.close_window(window_id, event_loop);
            }

            WindowEvent::Resized(size) => {
                self.handle_resize(window_id, size);
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
                if let Some(ctx) = self.windows.get_mut(&window_id) {
                    if ctx.window.update_scale_factor(scale_factor) {
                        self.handle_dpi_change(window_id, scale_factor);
                        self.update_resize_increments(window_id);
                    }
                }
            }

            WindowEvent::Focused(focused) => {
                if focused {
                    // Track which winit window is focused and update the
                    // mux active_window to match.
                    self.focused_window_id = Some(window_id);
                    if let Some(mux_id) = self
                        .windows
                        .get(&window_id)
                        .map(|ctx| ctx.window.mux_window_id())
                    {
                        self.active_window = Some(mux_id);
                    }
                }
                self.send_focus_event(focused);
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.chrome.set_active(focused);
                    ctx.dirty = true;
                }
            }

            WindowEvent::CursorLeft { .. } => {
                self.clear_chrome_hover();
                self.clear_tab_bar_hover();
                self.clear_url_hover();
                self.clear_divider_hover();
                self.cancel_tab_drag();
                self.cancel_divider_drag();
                self.cancel_floating_drag();
                self.release_tab_width_lock();
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse.set_cursor_pos(position);
                self.update_chrome_hover(position);
                self.update_tab_bar_hover(position);

                // Tab drag: consume all cursor moves when active.
                if self.update_tab_drag(
                    position,
                    #[cfg(target_os = "windows")]
                    event_loop,
                ) {
                    return;
                }

                // Forward move events to overlays for per-widget hover tracking.
                if self.try_overlay_mouse_move(position) {
                    return;
                }

                // Floating pane hover/drag: check before divider and terminal.
                if self.update_floating_hover(position) {
                    // Only consume if a drag is active; hover just sets cursor.
                    if self
                        .focused_ctx()
                        .is_some_and(|ctx| ctx.floating_drag.is_some())
                    {
                        return;
                    }
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
                // Track button state unconditionally — must run before any
                // early-return branch. Otherwise a press that reaches
                // handle_mouse_input but whose release is consumed by the tab
                // bar (or overlay/chrome) leaves buttons.left() stuck true,
                // causing phantom auto-scroll on subsequent CursorMoved.
                self.mouse
                    .set_button_down(button, state == winit::event::ElementState::Pressed);

                // Modal overlay: intercept mouse events.
                if self.try_overlay_mouse(button, state) {
                    return;
                }
                // Check chrome first — if a control button was clicked,
                // don't propagate to selection/PTY reporting.
                if self.try_chrome_mouse(button, state, event_loop) {
                    return;
                }
                // Suppress stale WM_LBUTTONUP after live merge.
                if button == winit::event::MouseButton::Left
                    && state == winit::event::ElementState::Released
                    && self.merge_drag_suppress_release
                {
                    self.merge_drag_suppress_release = false;
                    return;
                }
                // Tab drag: finish on left-button release.
                if button == winit::event::MouseButton::Left
                    && state == winit::event::ElementState::Released
                    && self.try_finish_tab_drag()
                {
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
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
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
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Process deferred window creation (keybinding actions lack
        // ActiveEventLoop access; the flag is set in execute_action).
        if self.pending_new_window {
            self.pending_new_window = false;
            self.create_window(event_loop);
        }

        // Check for completed OS-level tab drag (tear-off + merge).
        #[cfg(target_os = "windows")]
        self.check_torn_off_merge();

        // Pump mux events: drain PTY reader thread messages and process
        // resulting notifications before rendering.
        self.pump_mux_events();

        // Drive cursor blink timer only when blinking is active.
        if self.blinking_active && self.cursor_blink.update() {
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.dirty = true;
            }
        }

        // Tick compositor animations and clean up fully-faded overlays.
        let any_animating = {
            let now = std::time::Instant::now();
            if let Some(ctx) = self.focused_ctx_mut() {
                let animating = ctx.layer_animator.tick(&mut ctx.layer_tree, now);
                ctx.overlays
                    .cleanup_dismissed(&mut ctx.layer_tree, &ctx.layer_animator);

                // Clean up finished tab slide layers and sync offsets to widget.
                if ctx.tab_slide.has_active() {
                    ctx.tab_slide
                        .cleanup(&mut ctx.layer_tree, &ctx.layer_animator);
                    let count = ctx.tab_bar.tab_count();
                    ctx.tab_slide
                        .sync_to_widget(count, &ctx.layer_tree, &mut ctx.tab_bar);
                }

                animating
            } else {
                false
            }
        };
        if any_animating {
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.dirty = true;
            }
        }

        // Check if any window is dirty and render it.
        let any_dirty = self.focused_ctx().is_some_and(|ctx| ctx.dirty);
        if any_dirty {
            // Clear dirty BEFORE rendering so that if handle_redraw sets
            // it back to true (e.g. chrome hover animations in progress),
            // the flag is preserved for the next frame.
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.dirty = false;
            }

            // Render directly instead of deferring via request_redraw().
            // On Windows, request_redraw() maps to WM_PAINT which has
            // lower priority than input messages (WM_MOUSEMOVE). Rapid
            // mouse movement delays painting indefinitely, causing visible
            // lag for hover effects. Rendering here — at the end of the
            // event batch — ensures the frame reflects the latest state.
            self.handle_redraw();
        }

        // Schedule wakeup for continuous rendering when animations are
        // active or for the next blink toggle. The default ControlFlow::Wait
        // lets the event loop sleep indefinitely when nothing is animating.
        let has_animations = self
            .focused_ctx()
            .is_some_and(|ctx| ctx.layer_animator.is_any_animating());
        if has_animations {
            // Compositor animations: wake up promptly to drive the next frame.
            // 16ms ≈ 60 FPS — smooth enough for fade transitions.
            let next_frame = std::time::Instant::now() + std::time::Duration::from_millis(16);
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_frame));
        } else if self.blinking_active {
            let next_toggle = self.cursor_blink.next_toggle();
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_toggle));
        } else {
            // Nothing animating — sleep until the next external event.
        }
    }
}
