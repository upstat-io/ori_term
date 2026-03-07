//! Tab bar mouse click dispatch.
//!
//! Routes mouse clicks in the tab bar zone to the appropriate action
//! based on the [`TabBarHit`](oriterm_ui::widgets::tab_bar::TabBarHit) at
//! the cursor position.

use std::time::{Duration, Instant};

use winit::event::ElementState;
use winit::event_loop::ActiveEventLoop;

use oriterm_ui::overlay::Placement;
use oriterm_ui::widgets::menu::{MenuStyle, MenuWidget};
use oriterm_ui::widgets::tab_bar::TabBarHit;
use oriterm_ui::widgets::tab_bar::constants::{
    DROPDOWN_BUTTON_WIDTH, TAB_BAR_HEIGHT, TAB_TOP_MARGIN,
};

use super::{App, context_menu};

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
        let pos = self.mouse.cursor_pos();
        if !self.cursor_in_tab_bar(pos) {
            return false;
        }

        // Right-click on a tab opens the tab context menu.
        if button == winit::event::MouseButton::Right && state == ElementState::Pressed {
            let hit = self
                .focused_ctx()
                .map_or(TabBarHit::None, |ctx| ctx.tab_bar.hover_hit());
            if let TabBarHit::Tab(idx) = hit {
                self.open_tab_context_menu(idx);
                return true;
            }
            // Right-click elsewhere in the tab bar is consumed without action.
            return true;
        }

        // Only handle left-button events.
        if button != winit::event::MouseButton::Left {
            return false;
        }

        // Consume release events without action — prevents them from
        // falling through to the terminal selection handler.
        if state != ElementState::Pressed {
            return true;
        }

        // Use the hover hit already computed by update_tab_bar_hover.
        let hit = self
            .focused_ctx()
            .map_or(TabBarHit::None, |ctx| ctx.tab_bar.hover_hit());

        match hit {
            TabBarHit::None => false,

            TabBarHit::Tab(idx) => {
                self.switch_to_tab_index(idx);
                self.try_start_tab_drag(idx);
                true
            }

            TabBarHit::CloseTab(idx) => {
                // Acquire width lock for stable close-button targeting
                // during rapid close clicks.
                if let Some(ctx) = self.focused_ctx() {
                    let w = ctx.tab_bar.layout().base_tab_width();
                    self.acquire_tab_width_lock(w);
                }
                self.close_tab_at_index(idx);
                true
            }

            TabBarHit::NewTab => {
                if let Some(win_id) = self.active_window {
                    self.new_tab_in_window(win_id);
                }
                true
            }

            TabBarHit::Dropdown => {
                self.open_dropdown_menu();
                true
            }

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

    /// Open the tab right-click context menu below the clicked tab.
    fn open_tab_context_menu(&mut self, tab_index: usize) {
        let (entries, state) = context_menu::build_tab_context_menu(tab_index);
        let style = MenuStyle::from_theme(&self.ui_theme);
        let widget = MenuWidget::new(entries).with_style(style);

        // Anchor to the right-clicked tab rect.
        let anchor = self
            .focused_ctx()
            .map(|ctx| {
                let layout = ctx.tab_bar.layout();
                let tx = layout.tab_x(tab_index);
                oriterm_ui::geometry::Rect::new(
                    tx,
                    TAB_BAR_HEIGHT - TAB_TOP_MARGIN,
                    layout.base_tab_width(),
                    TAB_TOP_MARGIN,
                )
            })
            .unwrap_or_default();
        let now = Instant::now();

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.context_menu = Some(state);
            ctx.overlays.push_overlay(
                Box::new(widget),
                anchor,
                Placement::Below,
                &mut ctx.layer_tree,
                &mut ctx.layer_animator,
                now,
            );
            ctx.dirty = true;
        }
    }

    /// Open the dropdown menu below the dropdown button.
    fn open_dropdown_menu(&mut self) {
        let active_scheme = self.config.colors.scheme.clone();
        let names = crate::scheme::builtin_names();
        let (entries, state) = context_menu::build_dropdown_menu(&active_scheme, &names);
        let style = MenuStyle::from_theme(&self.ui_theme);
        let widget = MenuWidget::new(entries).with_style(style);

        // Anchor to the dropdown button rect.
        let anchor = self
            .focused_ctx()
            .map(|ctx| {
                let dx = ctx.tab_bar.layout().dropdown_x();
                oriterm_ui::geometry::Rect::new(
                    dx,
                    TAB_BAR_HEIGHT - TAB_TOP_MARGIN,
                    DROPDOWN_BUTTON_WIDTH,
                    TAB_TOP_MARGIN,
                )
            })
            .unwrap_or_default();
        let now = Instant::now();

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.context_menu = Some(state);
            ctx.overlays.push_overlay(
                Box::new(widget),
                anchor,
                Placement::Below,
                &mut ctx.layer_tree,
                &mut ctx.layer_animator,
                now,
            );
            ctx.dirty = true;
        }
    }

    /// Handle a click in the tab bar drag area.
    ///
    /// Double-click toggles maximize; single click initiates window drag.
    fn handle_tab_bar_drag_area(&mut self) {
        let now = Instant::now();
        let is_double = self
            .focused_ctx()
            .and_then(|ctx| ctx.last_drag_area_press)
            .is_some_and(|t| now.duration_since(t) < DOUBLE_CLICK_THRESHOLD);
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.last_drag_area_press = Some(now);
        }

        if is_double {
            // Double-click: toggle maximize. Reset timestamp to prevent
            // a third click from triggering another toggle.
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.last_drag_area_press = None;
            }
            self.toggle_maximize();
        } else {
            // Single click: initiate native window drag.
            if let Some(ctx) = self.focused_ctx() {
                let _ = ctx.window.window().drag_window();
            }
        }
    }
}
