//! Tab bar mouse click dispatch.
//!
//! Routes left-click presses in the tab bar zone to the appropriate action
//! based on the [`TabBarHit`](oriterm_ui::widgets::tab_bar::TabBarHit) at
//! the cursor position.

use std::time::{Duration, Instant};

use winit::event::ElementState;
use winit::event_loop::ActiveEventLoop;

use oriterm_ui::widgets::tab_bar::{TabBarHit, TabBarWidget};

use super::App;

/// Time window for two clicks to count as a double-click.
const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(500);

impl App {
    /// Dispatch a mouse click in the tab bar zone.
    ///
    /// Returns `true` if the event was consumed (click landed on a tab bar
    /// element). Returns `false` if the click is outside the tab bar.
    pub(super) fn try_tab_bar_mouse(
        &mut self,
        button: winit::event::MouseButton,
        state: ElementState,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        // Only handle left-click press events.
        if button != winit::event::MouseButton::Left || state != ElementState::Pressed {
            return false;
        }

        let pos = self.mouse.cursor_pos();
        if !self.cursor_in_tab_bar(pos) {
            return false;
        }

        // Use the hover hit already computed by update_tab_bar_hover.
        let hit = self
            .tab_bar
            .as_ref()
            .map_or(TabBarHit::None, TabBarWidget::hover_hit);

        match hit {
            TabBarHit::None => false,

            TabBarHit::Tab(_idx) => {
                // Single-tab: no switching needed yet (multi-tab is Section 15).
                // DragState::Pending will be added in Section 17.
                true
            }

            TabBarHit::CloseTab(_idx) => {
                // Single-tab: close the window. Multi-tab close (with width
                // lock for stable close-button targeting) is Section 15.
                self.shutdown(0);
            }

            // New tab (Section 15) and dropdown menu (Section 21): no-op for now.
            TabBarHit::NewTab | TabBarHit::Dropdown => true,

            TabBarHit::Minimize => {
                let action = oriterm_ui::widgets::WidgetAction::WindowMinimize;
                self.handle_chrome_action(&action, event_loop);
                true
            }

            TabBarHit::Maximize => {
                let action = oriterm_ui::widgets::WidgetAction::WindowMaximize;
                self.handle_chrome_action(&action, event_loop);
                true
            }

            TabBarHit::CloseWindow => {
                let action = oriterm_ui::widgets::WidgetAction::WindowClose;
                self.handle_chrome_action(&action, event_loop);
                true
            }

            TabBarHit::DragArea => {
                self.handle_tab_bar_drag_area();
                true
            }
        }
    }

    /// Handle a click in the tab bar drag area.
    ///
    /// Double-click toggles maximize; single click initiates window drag.
    fn handle_tab_bar_drag_area(&mut self) {
        let now = Instant::now();
        let is_double = self
            .last_drag_area_press
            .is_some_and(|t| now.duration_since(t) < DOUBLE_CLICK_THRESHOLD);
        self.last_drag_area_press = Some(now);

        if is_double {
            // Double-click: toggle maximize. Reset timestamp to prevent
            // a third click from triggering another toggle.
            self.last_drag_area_press = None;
            if let Some(window) = &mut self.window {
                let maximized = !window.is_maximized();
                window.window().set_maximized(maximized);
                window.set_maximized(maximized);
                if let Some(chrome) = &mut self.chrome {
                    chrome.set_maximized(maximized);
                }
                self.dirty = true;
            }
        } else {
            // Single click: initiate native window drag.
            if let Some(window) = &self.window {
                let _ = window.window().drag_window();
            }
        }
    }
}
