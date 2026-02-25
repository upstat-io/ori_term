//! Mouse button event handling for the application.
//!
//! Routes mouse button presses and releases through overlays, chrome,
//! PTY mouse reporting, and selection.

use winit::event::{ElementState, MouseButton};

use super::{App, mouse_report, mouse_selection};

impl App {
    /// Route a mouse event through the overlay manager.
    ///
    /// Returns `true` if the overlay consumed the event (caller should return
    /// early). Returns `false` if no overlays are active.
    pub(super) fn try_overlay_mouse(&mut self, button: MouseButton, state: ElementState) -> bool {
        if self.overlays.is_empty() {
            return false;
        }
        let Some(window) = &self.window else {
            return false;
        };
        let scale = window.scale_factor().factor() as f32;
        let pos = self.mouse.cursor_pos();
        let logical = oriterm_ui::geometry::Point::new(pos.x as f32 / scale, pos.y as f32 / scale);
        let mb = match button {
            MouseButton::Left => oriterm_ui::input::MouseButton::Left,
            MouseButton::Right => oriterm_ui::input::MouseButton::Right,
            MouseButton::Middle => oriterm_ui::input::MouseButton::Middle,
            _ => return true,
        };
        let kind = match state {
            ElementState::Pressed => oriterm_ui::input::MouseEventKind::Down(mb),
            ElementState::Released => oriterm_ui::input::MouseEventKind::Up(mb),
        };
        let ui_event = oriterm_ui::input::MouseEvent {
            kind,
            pos: logical,
            modifiers: super::winit_mods_to_ui(self.modifiers),
        };
        let measurer = self
            .renderer
            .as_ref()
            .map(|r| crate::font::UiFontMeasurer::new(r.active_ui_collection(), scale));
        let measurer: &dyn oriterm_ui::widgets::TextMeasurer = match &measurer {
            Some(m) => m,
            None => return true,
        };
        let theme = oriterm_ui::theme::UiTheme::dark();
        let result = self
            .overlays
            .process_mouse_event(&ui_event, measurer, &theme, None);
        self.handle_overlay_result(result);
        true
    }

    /// Route a cursor move event through the overlay manager.
    ///
    /// Returns `true` if an overlay consumed the event (caller should skip
    /// terminal mouse handling). Enables per-button hover tracking inside
    /// dialogs and other overlay widgets.
    pub(super) fn try_overlay_mouse_move(
        &mut self,
        position: winit::dpi::PhysicalPosition<f64>,
    ) -> bool {
        if self.overlays.is_empty() {
            return false;
        }
        let Some(window) = &self.window else {
            return false;
        };
        let scale = window.scale_factor().factor() as f32;
        let logical =
            oriterm_ui::geometry::Point::new(position.x as f32 / scale, position.y as f32 / scale);
        let ui_event = oriterm_ui::input::MouseEvent {
            kind: oriterm_ui::input::MouseEventKind::Move,
            pos: logical,
            modifiers: super::winit_mods_to_ui(self.modifiers),
        };
        let measurer = self
            .renderer
            .as_ref()
            .map(|r| crate::font::UiFontMeasurer::new(r.active_ui_collection(), scale));
        let measurer: &dyn oriterm_ui::widgets::TextMeasurer = match &measurer {
            Some(m) => m,
            None => return true,
        };
        let theme = oriterm_ui::theme::UiTheme::dark();
        let result = self
            .overlays
            .process_mouse_event(&ui_event, measurer, &theme, None);
        self.handle_overlay_result(result);
        // Only consume if a modal overlay blocked or handled it.
        self.overlays.has_modal()
    }

    /// Handle mouse press for selection.
    pub(super) fn handle_mouse_press(&mut self) {
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
    pub(super) fn handle_mouse_drag(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
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
    pub(super) fn handle_mouse_input(&mut self, button: MouseButton, state: ElementState) {
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
                // Ctrl+click opens hovered URL (overrides both reporting and selection).
                if state == ElementState::Pressed && self.try_open_hovered_url() {
                    return;
                }
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
}
