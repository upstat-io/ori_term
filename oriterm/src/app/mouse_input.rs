//! Mouse button event handling for the application.
//!
//! Routes mouse button presses and releases through overlays, chrome,
//! PTY mouse reporting, and selection.

use std::time::Instant;

use winit::event::{ElementState, MouseButton};

use oriterm_ui::geometry::{Point, Rect};
use oriterm_ui::overlay::Placement;
use oriterm_ui::widgets::menu::{MenuStyle, MenuWidget};

use super::{App, context_menu, mouse_report, mouse_selection};

impl App {
    /// Route a mouse event through the overlay manager.
    ///
    /// Returns `true` if the overlay consumed the event (caller should return
    /// early). Returns `false` if no overlays are active.
    pub(super) fn try_overlay_mouse(&mut self, button: MouseButton, state: ElementState) -> bool {
        let scale = match self.focused_ctx() {
            Some(ctx) if !ctx.overlays.is_empty() => ctx.window.scale_factor().factor() as f32,
            _ => return false,
        };
        let pos = self.mouse.cursor_pos();
        let logical = Point::new(pos.x as f32 / scale, pos.y as f32 / scale);
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
        // Borrow split: inline window lookup borrows only self.windows,
        // leaving self.renderer and self.ui_theme available as disjoint borrows.
        let now = Instant::now();
        let result = {
            let Some(ctx) = self
                .focused_window_id
                .and_then(|id| self.windows.get_mut(&id))
            else {
                return true;
            };
            ctx.overlays.process_mouse_event(
                &ui_event,
                measurer,
                &self.ui_theme,
                None,
                &ctx.layer_tree,
                &mut ctx.layer_animator,
                now,
            )
        };
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
        let scale = match self.focused_ctx() {
            Some(ctx) if !ctx.overlays.is_empty() => ctx.window.scale_factor().factor() as f32,
            _ => return false,
        };
        let logical = Point::new(position.x as f32 / scale, position.y as f32 / scale);
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
        let now = Instant::now();
        let result = {
            let Some(ctx) = self
                .focused_window_id
                .and_then(|id| self.windows.get_mut(&id))
            else {
                return true;
            };
            ctx.overlays.process_mouse_event(
                &ui_event,
                measurer,
                &self.ui_theme,
                None,
                &ctx.layer_tree,
                &mut ctx.layer_animator,
                now,
            )
        };
        self.handle_overlay_result(result);
        // Only consume if a modal overlay blocked or handled it.
        self.focused_ctx()
            .is_some_and(|ctx| ctx.overlays.has_modal())
    }

    /// Open the grid right-click context menu at the cursor position.
    fn open_grid_context_menu(&mut self) {
        let has_sel = self
            .active_pane_id()
            .and_then(|id| self.pane_selection(id))
            .is_some();
        let (entries, state) = context_menu::build_grid_context_menu(has_sel);
        let style = MenuStyle::from_theme(&self.ui_theme);
        let widget = MenuWidget::new(entries).with_style(style);

        // Convert cursor position to logical pixels for overlay placement.
        let pos = self.mouse.cursor_pos();
        let scale = self
            .focused_ctx()
            .map_or(1.0, |ctx| ctx.window.scale_factor().factor() as f32);
        let logical = Point::new(pos.x as f32 / scale, pos.y as f32 / scale);
        let now = Instant::now();

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.context_menu = Some(state);
            ctx.overlays.push_overlay(
                Box::new(widget),
                Rect::default(),
                Placement::AtPoint(logical),
                &mut ctx.layer_tree,
                &mut ctx.layer_animator,
                now,
            );
            ctx.dirty = true;
        }
    }

    /// Handle mouse press for selection.
    pub(super) fn handle_mouse_press(&mut self) {
        let pos = self.mouse.cursor_pos();
        let Some(pane_id) = self.active_pane_id() else {
            return;
        };
        let (Some(wctx), Some(renderer)) = (
            self.focused_window_id.and_then(|id| self.windows.get(&id)),
            self.renderer.as_ref(),
        ) else {
            return;
        };
        let ctx = mouse_selection::GridCtx {
            widget: &wctx.terminal_grid,
            cell: renderer.cell_metrics(),
            word_delimiters: &self.config.behavior.word_delimiters,
        };

        // Build SnapshotGrid from the current snapshot.
        let mux = self.mux.as_mut().expect("mux checked at pane_id");
        if mux.pane_snapshot(pane_id).is_none() || mux.is_pane_snapshot_dirty(pane_id) {
            mux.refresh_pane_snapshot(pane_id);
        }
        let Some(snapshot) = self.mux.as_ref().and_then(|m| m.pane_snapshot(pane_id)) else {
            return;
        };
        let grid = super::snapshot_grid::SnapshotGrid::new(snapshot);
        let existing_mode = self.pane_selection(pane_id).map(|s| s.mode);

        let Some(action) = mouse_selection::handle_press(
            &mut self.mouse,
            &grid,
            &ctx,
            pos,
            self.modifiers,
            existing_mode,
        ) else {
            return;
        };

        // Apply the action to App-owned selection state.
        match action {
            mouse_selection::PressAction::Extend(point) => {
                self.update_pane_selection_end(pane_id, point);
            }
            mouse_selection::PressAction::New(selection) => {
                self.set_pane_selection(pane_id, selection);
            }
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Handle mouse drag for selection.
    pub(super) fn handle_mouse_drag(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
        let Some(pane_id) = self.active_pane_id() else {
            return;
        };
        let (Some(wctx), Some(renderer)) = (
            self.focused_window_id.and_then(|id| self.windows.get(&id)),
            self.renderer.as_ref(),
        ) else {
            return;
        };
        let ctx = mouse_selection::GridCtx {
            widget: &wctx.terminal_grid,
            cell: renderer.cell_metrics(),
            word_delimiters: &self.config.behavior.word_delimiters,
        };

        // Build SnapshotGrid from the current snapshot.
        let mux = self.mux.as_mut().expect("mux checked at pane_id");
        if mux.pane_snapshot(pane_id).is_none() || mux.is_pane_snapshot_dirty(pane_id) {
            mux.refresh_pane_snapshot(pane_id);
        }
        let Some(snapshot) = self.mux.as_ref().and_then(|m| m.pane_snapshot(pane_id)) else {
            return;
        };
        let grid = super::snapshot_grid::SnapshotGrid::new(snapshot);
        let selection = self.pane_selection(pane_id).copied();

        let result = mouse_selection::handle_drag(
            &mut self.mouse,
            &grid,
            selection.as_ref(),
            &ctx,
            position,
        );

        match result {
            mouse_selection::DragResult::None => {}
            mouse_selection::DragResult::Updated(endpoint) => {
                self.update_pane_selection_end(pane_id, endpoint);
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            mouse_selection::DragResult::NeedsAutoScroll {
                delta,
                scrolling_up,
            } => {
                // Apply scroll through MuxBackend.
                if let Some(mux) = self.mux.as_mut() {
                    mux.scroll_display(pane_id, delta);
                }
                // Refresh snapshot after scroll, then compute endpoint.
                if let Some(mux) = self.mux.as_mut() {
                    mux.refresh_pane_snapshot(pane_id);
                }
                if let Some(snap) = self.mux.as_ref().and_then(|m| m.pane_snapshot(pane_id)) {
                    let grid = super::snapshot_grid::SnapshotGrid::new(snap);
                    let endpoint = mouse_selection::helpers::compute_auto_scroll_endpoint(
                        &grid,
                        position,
                        &ctx,
                        scrolling_up,
                    );
                    self.update_pane_selection_end(pane_id, endpoint);
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
        }
    }

    /// Handle a mouse button event (left, middle, right).
    ///
    /// Button state (`set_button_down`) is already tracked unconditionally
    /// in the `MouseInput` match arm before this function is called.
    pub(super) fn handle_mouse_input(&mut self, button: MouseButton, state: ElementState) {
        let pressed = state == ElementState::Pressed;

        // Floating pane drag: start on left-press over title bar or edge,
        // finish on left-release when dragging. Takes priority over dividers.
        if button == MouseButton::Left {
            let consumed = if pressed {
                self.try_start_floating_drag()
            } else {
                self.try_finish_floating_drag()
            };
            if consumed {
                return;
            }
        }

        // Divider drag: start on left-press when hovering a divider,
        // finish on left-release when dragging.
        if button == MouseButton::Left {
            let consumed = if pressed {
                self.try_start_divider_drag()
            } else {
                self.try_finish_divider_drag()
            };
            if consumed {
                return;
            }
        }

        // Multi-pane click-to-focus: on any press in a multi-pane tab,
        // hit-test to find the target pane and switch focus if needed.
        // The click is NOT consumed — it falls through to selection/reporting
        // so the target pane receives the event.
        if pressed {
            self.try_pane_focus_click();
        }

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
                    if let Some(ctx) = self.focused_ctx_mut() {
                        ctx.dirty = true;
                    }
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
                    self.open_grid_context_menu();
                } else {
                    // Release without reporting: no action needed.
                }
            }
            _ => {}
        }
    }
}
