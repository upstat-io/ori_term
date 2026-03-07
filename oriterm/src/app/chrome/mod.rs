//! Window chrome: action dispatch and shared helpers.
//!
//! Handles `WidgetAction::WindowMinimize`, `WindowMaximize`, and
//! `WindowClose` by forwarding to the appropriate winit window operations.
//! Provides shared geometry helpers used by both init and resize.

mod resize;

use std::time::Instant;

use winit::event_loop::ActiveEventLoop;

use oriterm_ui::widgets::WidgetAction;

use super::App;
use crate::font::UiFontMeasurer;

/// Scale a logical-pixel rect to physical pixels.
#[cfg(target_os = "windows")]
pub(super) fn scale_rect(r: oriterm_ui::geometry::Rect, scale: f32) -> oriterm_ui::geometry::Rect {
    oriterm_ui::geometry::Rect::new(
        r.x() * scale,
        r.y() * scale,
        r.width() * scale,
        r.height() * scale,
    )
}

/// Compute the grid origin y-coordinate in physical pixels.
///
/// Rounds to an integer pixel to prevent fractional origins that cause
/// visible seams between block character rows on the GPU. Without rounding,
/// DPI scale factors like 1.25 produce half-pixel boundaries
/// (e.g. `82.0 * 1.25 = 102.5`) that mis-align cell rows.
pub(super) fn grid_origin_y(chrome_height_logical: f32, scale: f32) -> f32 {
    (chrome_height_logical * scale).round()
}

impl App {
    /// Dispatch a window chrome action to the corresponding window operation.
    ///
    /// Returns `true` if the action was handled (recognized as a chrome action).
    pub(super) fn handle_chrome_action(
        &mut self,
        action: &WidgetAction,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        match action {
            WidgetAction::WindowMinimize => {
                if let Some(ctx) = self.focused_ctx() {
                    ctx.window.window().set_minimized(true);
                }
                true
            }
            WidgetAction::WindowMaximize => {
                self.toggle_maximize();
                true
            }
            WidgetAction::WindowClose => {
                if let Some(wid) = self.focused_window_id {
                    self.close_window(wid, event_loop);
                }
                true
            }
            _ => false,
        }
    }

    /// Toggle the window between maximized and restored state.
    ///
    /// Updates the winit window, the `TermWindow` state, and the chrome
    /// widget's maximized flag.
    pub(super) fn toggle_maximize(&mut self) {
        if let Some(ctx) = self.focused_ctx_mut() {
            let maximized = !ctx.window.is_maximized();
            ctx.window.window().set_maximized(maximized);
            ctx.window.set_maximized(maximized);
            ctx.tab_bar.set_maximized(maximized);
            ctx.dirty = true;
        }
    }

    /// Returns `true` if the cursor position is within the tab bar zone.
    ///
    /// The tab bar spans from y=0 to `TAB_BAR_HEIGHT` (logical pixels).
    pub(super) fn cursor_in_tab_bar(&self, position: winit::dpi::PhysicalPosition<f64>) -> bool {
        let Some(ctx) = self.focused_ctx() else {
            return false;
        };
        let scale = ctx.window.scale_factor().factor() as f32;
        let logical_y = position.y as f32 / scale;
        logical_y < oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT
    }

    /// Update tab bar hover state and width lock from cursor position.
    ///
    /// Called from `CursorMoved`. Computes which tab bar element the cursor
    /// targets via [`hit_test`](oriterm_ui::widgets::tab_bar::hit_test),
    /// updates the widget's hover hit (marking dirty on change), and manages
    /// the tab width lock (acquire on enter, release on leave).
    pub(super) fn update_tab_bar_hover(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
        let in_tab_bar = self.cursor_in_tab_bar(position);
        let locked = self.tab_width_lock().is_some();

        // Manage tab width lock. Skip when a tab drag is active — the drag
        // owns the lock lifecycle and cursor movement outside the bar (toward
        // tear-off) must not release it prematurely.
        if !self.has_tab_drag() {
            match (in_tab_bar, locked) {
                (true, false) => {
                    let tab_width = self
                        .focused_ctx()
                        .map_or(0.0, |ctx| ctx.tab_bar.layout().base_tab_width());
                    self.acquire_tab_width_lock(tab_width);
                }
                (false, true) => self.release_tab_width_lock(),
                (true, true) | (false, false) => {}
            }
        }

        // Compute hit test result.
        let hit = if in_tab_bar {
            let ctx_data = self.focused_ctx().map(|ctx| {
                (
                    ctx.window.scale_factor().factor() as f32,
                    ctx.tab_bar.layout().clone(),
                )
            });

            match ctx_data {
                Some((scale, layout)) => {
                    let x = position.x as f32 / scale;
                    let y = position.y as f32 / scale;
                    oriterm_ui::widgets::tab_bar::hit_test(x, y, &layout)
                }
                _ => oriterm_ui::widgets::tab_bar::TabBarHit::None,
            }
        } else {
            oriterm_ui::widgets::tab_bar::TabBarHit::None
        };

        // Drive control button hover animation when cursor targets controls.
        {
            use oriterm_ui::widgets::tab_bar::TabBarHit;
            let is_control_hit = matches!(
                hit,
                TabBarHit::Minimize | TabBarHit::Maximize | TabBarHit::CloseWindow
            );

            if let Some(ctx) = self
                .focused_window_id
                .and_then(|id| self.windows.get_mut(&id))
            {
                if is_control_hit || ctx.tab_bar.hover_hit() != hit {
                    let scale = ctx.window.scale_factor().factor() as f32;
                    let pos = oriterm_ui::geometry::Point::new(
                        position.x as f32 / scale,
                        position.y as f32 / scale,
                    );
                    if let Some(renderer) = ctx.renderer.as_ref() {
                        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
                        let event_ctx = oriterm_ui::widgets::EventCtx {
                            measurer: &measurer,
                            bounds: oriterm_ui::geometry::Rect::default(),
                            is_focused: false,
                            focused_widget: None,
                            theme: &self.ui_theme,
                        };
                        let resp = ctx.tab_bar.update_control_hover(pos, &event_ctx);
                        if resp.response == oriterm_ui::input::EventResponse::RequestRedraw {
                            ctx.dirty = true;
                        }
                    }
                }
            }
        }

        // Apply hover hit, redraw on change.
        if let Some(ctx) = self.focused_ctx_mut() {
            if ctx.tab_bar.hover_hit() != hit {
                ctx.tab_bar.set_hover_hit(hit, Instant::now());
                ctx.dirty = true;
            }
        }
    }

    /// Clear tab bar hover state (including control button hover).
    ///
    /// Called when the cursor leaves the window to reset hover highlighting.
    pub(super) fn clear_tab_bar_hover(&mut self) {
        let Some(ctx) = self
            .focused_window_id
            .and_then(|id| self.windows.get_mut(&id))
        else {
            return;
        };
        let had_hover = ctx.tab_bar.hover_hit() != oriterm_ui::widgets::tab_bar::TabBarHit::None;
        if had_hover {
            ctx.tab_bar.set_hover_hit(
                oriterm_ui::widgets::tab_bar::TabBarHit::None,
                Instant::now(),
            );
        }
        // Clear control button hover animation.
        if let Some(renderer) = ctx.renderer.as_ref() {
            let scale = ctx.window.scale_factor().factor() as f32;
            let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
            let event_ctx = oriterm_ui::widgets::EventCtx {
                measurer: &measurer,
                bounds: oriterm_ui::geometry::Rect::default(),
                is_focused: false,
                focused_widget: None,
                theme: &self.ui_theme,
            };
            ctx.tab_bar.clear_control_hover(&event_ctx);
        }
        if had_hover {
            ctx.dirty = true;
        }
    }
}

#[cfg(test)]
mod tests;
