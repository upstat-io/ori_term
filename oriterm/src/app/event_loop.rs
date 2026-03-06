//! Winit `ApplicationHandler` impl and helper free functions.
//!
//! Separated from `mod.rs` to keep the main module definition file under the
//! 500-line limit. Contains the event dispatch table and theme/modifier
//! conversion utilities.

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents};
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
    /// Pump mux events and render all dirty windows during a Win32 modal loop.
    ///
    /// During modal move/resize, `about_to_wait` never fires. A `SetTimer`
    /// in the `WndProc` ticks at 60 FPS, generating `RedrawRequested` via
    /// `InvalidateRect`. This method substitutes for `about_to_wait`'s
    /// render loop: pump mux events, then render every dirty window using
    /// the same focus-swapping pattern.
    #[cfg(target_os = "windows")]
    fn modal_loop_render(&mut self) {
        self.pump_mux_events();

        let dirty_ids: Vec<WindowId> = self
            .windows
            .iter()
            .filter(|(_, ctx)| ctx.dirty)
            .map(|(&id, _)| id)
            .collect();
        if dirty_ids.is_empty() {
            return;
        }

        let saved_focused = self.focused_window_id;
        let saved_active = self.active_window;

        for wid in dirty_ids {
            if let Some(ctx) = self.windows.get_mut(&wid) {
                ctx.dirty = false;
            }
            let mux_wid = self
                .windows
                .get(&wid)
                .map(|ctx| ctx.window.session_window_id());
            self.focused_window_id = Some(wid);
            self.active_window = mux_wid;
            self.handle_redraw();
        }

        self.focused_window_id = saved_focused;
        self.active_window = saved_active;
        self.last_render = std::time::Instant::now();
    }

    /// Send a focus-in or focus-out escape sequence to the active pane.
    ///
    /// Only sends when the terminal has `FOCUS_IN_OUT` mode enabled (mode 1004).
    /// Focus-in: `CSI I` (`\x1b[I`), focus-out: `CSI O` (`\x1b[O`).
    fn send_focus_event(&mut self, focused: bool) {
        let Some(pane_id) = self.active_pane_id() else {
            return;
        };
        let Some(mode) = self.pane_mode(pane_id) else {
            return;
        };
        if !mode.contains(oriterm_core::TermMode::FOCUS_IN_OUT) {
            return;
        }
        let seq: &[u8] = if focused { b"\x1b[I" } else { b"\x1b[O" };
        self.write_pane_input(pane_id, seq);
    }
}

impl ApplicationHandler<TermEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.windows.is_empty() {
            return;
        }
        // Unregister raw input devices (WM_INPUT). winit defaults to
        // WhenFocused, which floods the message queue with mouse raw input
        // at 125-1000 Hz — stalling the render loop during flood output.
        // Terminals only need cooked WindowEvent variants.
        event_loop.listen_device_events(DeviceEvents::Never);
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

            WindowEvent::RedrawRequested => {
                // During Win32 modal move/resize loops, about_to_wait never
                // fires. A SetTimer ticks at 60 FPS, generating
                // RedrawRequested via InvalidateRect. Pump mux events and
                // render all windows here instead.
                #[cfg(target_os = "windows")]
                if oriterm_ui::platform_windows::in_modal_loop() {
                    self.modal_loop_render();
                    return;
                }
                self.handle_redraw();
            }

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
                        .map(|ctx| ctx.window.session_window_id())
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
                self.perf.record_cursor_move();
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
                {
                    let suppress = self
                        .focused_ctx_mut()
                        .and_then(|ctx| ctx.tab_drag.as_mut())
                        .is_some_and(|drag| {
                            if drag.suppress_next_release {
                                drag.suppress_next_release = false;
                                true
                            } else {
                                false
                            }
                        });
                    if suppress {
                        return;
                    }
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

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: TermEvent) {
        match event {
            TermEvent::ConfigReload => {
                self.apply_config_reload();
            }
            TermEvent::MuxWakeup => {
                self.perf.record_wakeup();
                // The real work happens in `pump_mux_events()` during
                // `about_to_wait`. This wakeup ensures the event loop
                // doesn't sleep past pending mux events. Mark ALL windows
                // dirty — PTY output may come from any pane in any window.
                self.mark_all_windows_dirty();
            }
            TermEvent::CreateWindow => {
                self.create_window(event_loop);
            }
            TermEvent::MoveTabToNewWindow(tab_index) => {
                let tab_id = self.active_window.and_then(|wid| {
                    let win = self.session.get_window(wid)?;
                    win.tabs().get(tab_index).copied()
                });
                if let Some(tid) = tab_id {
                    self.move_tab_to_new_window(tid, event_loop);
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.perf.record_tick();

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

        // Check if any window is dirty and render it, subject to frame budget.
        let any_dirty = self.windows.values().any(|ctx| ctx.dirty);
        let now = std::time::Instant::now();
        let budget_elapsed = now.duration_since(self.last_render) >= super::FRAME_BUDGET;

        if any_dirty && budget_elapsed {
            // Render all dirty windows. Each render temporarily swaps
            // `focused_window_id`/`active_window` so `handle_redraw`
            // targets the correct window (same pattern as `tear_off_tab`).
            let dirty_winit_ids: Vec<WindowId> = self
                .windows
                .iter()
                .filter(|(_, ctx)| ctx.dirty)
                .map(|(&id, _)| id)
                .collect();

            let saved_focused = self.focused_window_id;
            let saved_active = self.active_window;

            for wid in dirty_winit_ids {
                if let Some(ctx) = self.windows.get_mut(&wid) {
                    ctx.dirty = false;
                }
                let mux_wid = self
                    .windows
                    .get(&wid)
                    .map(|ctx| ctx.window.session_window_id());
                self.focused_window_id = Some(wid);
                self.active_window = mux_wid;
                self.handle_redraw();
            }

            self.focused_window_id = saved_focused;
            self.active_window = saved_active;
            self.last_render = std::time::Instant::now();
            self.perf.record_render();
        }

        // Periodic performance stats.
        self.perf.maybe_log();

        // Schedule wakeup for continuous rendering when animations are
        // active or for the next blink toggle. The default ControlFlow::Wait
        // lets the event loop sleep indefinitely when nothing is animating.
        let has_animations = self
            .windows
            .values()
            .any(|ctx| ctx.layer_animator.is_any_animating());
        if any_dirty && !budget_elapsed {
            // Dirty but budget not yet elapsed — wake up when budget allows.
            let remaining =
                super::FRAME_BUDGET.saturating_sub(now.duration_since(self.last_render));
            event_loop.set_control_flow(ControlFlow::WaitUntil(now + remaining));
        } else if has_animations {
            // Compositor animations: wake up promptly to drive the next frame.
            // 16ms ≈ 60 FPS — smooth enough for fade transitions.
            let next_frame = now + std::time::Duration::from_millis(16);
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_frame));
        } else if self.blinking_active {
            let next_toggle = self.cursor_blink.next_toggle();
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_toggle));
        } else {
            // Nothing animating — sleep until the next external event.
        }
    }
}
