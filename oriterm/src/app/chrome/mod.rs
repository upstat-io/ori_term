//! Window chrome: action dispatch, event routing, and shared helpers.
//!
//! Handles `WidgetAction::WindowMinimize`, `WindowMaximize`, and
//! `WindowClose` by forwarding to the appropriate winit window operations.
//! Routes mouse and hover events to the chrome widget, and provides shared
//! geometry helpers used by both init and resize.

mod resize;

use winit::event::ElementState;
use winit::event_loop::ActiveEventLoop;

use oriterm_ui::widgets::{Widget, WidgetAction};

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
            ctx.chrome.set_maximized(maximized);
            ctx.dirty = true;
        }
    }

    /// Check if a mouse event should be handled by the chrome widget.
    ///
    /// Returns `true` if the event was consumed by chrome (click on a
    /// control button), `false` if it should fall through to the grid.
    pub(super) fn try_chrome_mouse(
        &mut self,
        button: winit::event::MouseButton,
        state: ElementState,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        let pos = self.mouse.cursor_pos();

        // Extract immutable data before mutable borrow.
        let (scale, caption_height, is_visible, logical_w) = {
            let Some(ctx) = self.focused_ctx() else {
                return false;
            };
            if !ctx.chrome.is_visible() {
                return false;
            }
            let scale = ctx.window.scale_factor().factor() as f32;
            let caption_h = ctx.chrome.caption_height();
            let w = ctx.window.size_px().0;
            (scale, caption_h, true, w as f32 / scale)
        };

        if !is_visible {
            return false;
        }

        let logical_y = pos.y as f32 / scale;

        // Only intercept events within the caption height.
        if logical_y >= caption_height {
            return false;
        }

        let logical_pos = oriterm_ui::geometry::Point::new(pos.x as f32 / scale, logical_y);

        // Check if the click is on a control button.
        let kind = match (button, state) {
            (winit::event::MouseButton::Left, ElementState::Pressed) => {
                oriterm_ui::input::MouseEventKind::Down(oriterm_ui::input::MouseButton::Left)
            }
            (winit::event::MouseButton::Left, ElementState::Released) => {
                oriterm_ui::input::MouseEventKind::Up(oriterm_ui::input::MouseButton::Left)
            }
            _ => return false,
        };

        let event = oriterm_ui::input::MouseEvent {
            kind,
            pos: logical_pos,
            modifiers: super::winit_mods_to_ui(self.modifiers),
        };
        let Some(ctx) = self
            .focused_window_id
            .and_then(|id| self.windows.get_mut(&id))
        else {
            return false;
        };
        let Some(renderer) = ctx.renderer.as_ref() else {
            return false;
        };
        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
        let measurer: &dyn oriterm_ui::widgets::TextMeasurer = &measurer;
        let ctx_widget = oriterm_ui::widgets::EventCtx {
            measurer,
            bounds: oriterm_ui::geometry::Rect::new(0.0, 0.0, logical_w, caption_height),
            is_focused: false,
            focused_widget: None,
            theme: &self.ui_theme,
        };
        let resp = ctx.chrome.handle_mouse(&event, &ctx_widget);
        if resp.response != oriterm_ui::input::EventResponse::Ignored {
            if let Some(action) = &resp.action {
                // Clone action before reborrowing self.
                let action = action.clone();
                self.handle_chrome_action(&action, event_loop);
            } else if let Some(ctx) = self.focused_ctx_mut() {
                ctx.dirty = true;
            } else {
                // No focused window — nothing to mark dirty.
            }
            return true;
        }

        // Caption click that didn't hit a control button — initiate window drag.
        // On Windows, the WndProc subclass handles WM_NCHITTEST → Caption → drag.
        // On macOS/Linux, winit's drag_window() triggers the native drag protocol.
        #[cfg(not(target_os = "windows"))]
        if button == winit::event::MouseButton::Left && state == ElementState::Pressed {
            if let Some(ctx) = self.focused_ctx() {
                let _ = ctx.window.window().drag_window();
            }
            return true;
        }

        false
    }

    /// Clear chrome hover state when the cursor leaves the window.
    pub(super) fn clear_chrome_hover(&mut self) {
        let Some(ctx) = self
            .focused_window_id
            .and_then(|id| self.windows.get_mut(&id))
        else {
            return;
        };
        let Some(renderer) = ctx.renderer.as_ref() else {
            return;
        };
        let scale = ctx.window.scale_factor().factor() as f32;
        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
        let ctx_widget = oriterm_ui::widgets::EventCtx {
            measurer: &measurer,
            bounds: oriterm_ui::geometry::Rect::default(),
            is_focused: false,
            focused_widget: None,
            theme: &self.ui_theme,
        };
        let resp = ctx
            .chrome
            .handle_hover(oriterm_ui::input::HoverEvent::Leave, &ctx_widget);
        if resp.response == oriterm_ui::input::EventResponse::RequestRedraw {
            ctx.dirty = true;
        }
    }

    /// Update chrome hover state from a cursor position (physical pixels).
    pub(super) fn update_chrome_hover(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
        let Some(ctx) = self
            .focused_window_id
            .and_then(|id| self.windows.get_mut(&id))
        else {
            return;
        };
        let Some(renderer) = ctx.renderer.as_ref() else {
            return;
        };
        let scale = ctx.window.scale_factor().factor() as f32;
        let logical =
            oriterm_ui::geometry::Point::new(position.x as f32 / scale, position.y as f32 / scale);
        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
        let ctx_widget = oriterm_ui::widgets::EventCtx {
            measurer: &measurer,
            bounds: oriterm_ui::geometry::Rect::default(),
            is_focused: false,
            focused_widget: None,
            theme: &self.ui_theme,
        };
        let resp = ctx.chrome.update_hover(logical, &ctx_widget);
        if resp.response == oriterm_ui::input::EventResponse::RequestRedraw {
            ctx.dirty = true;
        }
    }

    /// Returns `true` if the cursor position is within the chrome caption area.
    pub(super) fn cursor_in_chrome(&self, position: winit::dpi::PhysicalPosition<f64>) -> bool {
        let Some(ctx) = self.focused_ctx() else {
            return false;
        };
        if !ctx.chrome.is_visible() {
            return false;
        }
        let scale = ctx.window.scale_factor().factor() as f32;
        let logical_y = position.y as f32 / scale;
        logical_y < ctx.chrome.caption_height()
    }

    /// Returns `true` if the cursor position is within the tab bar zone.
    ///
    /// The tab bar zone spans from the chrome caption height to
    /// caption height + `TAB_BAR_HEIGHT` (logical pixels).
    pub(super) fn cursor_in_tab_bar(&self, position: winit::dpi::PhysicalPosition<f64>) -> bool {
        let Some(ctx) = self.focused_ctx() else {
            return false;
        };
        if !ctx.chrome.is_visible() {
            return false;
        }
        let scale = ctx.window.scale_factor().factor() as f32;
        let logical_y = position.y as f32 / scale;
        let caption_h = ctx.chrome.caption_height();
        logical_y >= caption_h
            && logical_y < caption_h + oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT
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
                        .map_or(0.0, |ctx| ctx.tab_bar.layout().tab_width);
                    self.acquire_tab_width_lock(tab_width);
                }
                (false, true) => self.release_tab_width_lock(),
                (true, true) | (false, false) => {}
            }
        }

        // Compute hit test result.
        let hit = if in_tab_bar {
            // Extract immutable data before mutating tab_bar.
            let geom = self.focused_ctx().map(|ctx| {
                let scale = ctx.window.scale_factor().factor() as f32;
                let caption_h = ctx.chrome.caption_height();
                (scale, caption_h)
            });
            let layout = self.focused_ctx().map(|ctx| *ctx.tab_bar.layout());

            match (geom, layout) {
                (Some((scale, caption_h)), Some(layout)) => {
                    let x = position.x as f32 / scale;
                    let y = position.y as f32 / scale - caption_h;
                    oriterm_ui::widgets::tab_bar::hit_test(x, y, &layout)
                }
                _ => oriterm_ui::widgets::tab_bar::TabBarHit::None,
            }
        } else {
            oriterm_ui::widgets::tab_bar::TabBarHit::None
        };

        // Apply hover hit, redraw on change.
        if let Some(ctx) = self.focused_ctx_mut() {
            if ctx.tab_bar.hover_hit() != hit {
                ctx.tab_bar.set_hover_hit(hit);
                ctx.dirty = true;
            }
        }
    }

    /// Clear tab bar hover state.
    ///
    /// Called when the cursor leaves the window to reset hover highlighting.
    pub(super) fn clear_tab_bar_hover(&mut self) {
        if let Some(ctx) = self.focused_ctx_mut() {
            if ctx.tab_bar.hover_hit() != oriterm_ui::widgets::tab_bar::TabBarHit::None {
                ctx.tab_bar
                    .set_hover_hit(oriterm_ui::widgets::tab_bar::TabBarHit::None);
                ctx.dirty = true;
            }
        }
    }
}

#[cfg(test)]
mod tests;
