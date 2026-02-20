//! Input state machine for mouse routing, hover tracking, and capture.
//!
//! `InputState` sits between the platform event loop and the widget tree.
//! It tracks which widget is hovered (hot), which widget has mouse capture
//! (active), and generates synthetic `Enter`/`Leave` events on hover
//! transitions.

use crate::focus::FocusManager;
use crate::geometry::Point;
use crate::layout::LayoutNode;
use crate::widget_id::WidgetId;

use super::event::{HoverEvent, MouseEvent, MouseEventKind};
use super::hit_test::layout_hit_test;

/// Routing action emitted by `InputState` for each incoming mouse event.
///
/// The application layer dispatches these to the appropriate widget.
#[derive(Debug, Clone, PartialEq)]
pub enum RouteAction {
    /// Deliver a mouse event to a specific widget.
    Deliver {
        /// Target widget.
        target: WidgetId,
        /// The mouse event to deliver.
        event: MouseEvent,
    },
    /// A widget's hover state changed.
    Hover {
        /// Target widget.
        target: WidgetId,
        /// Whether the cursor entered or left.
        kind: HoverEvent,
    },
}

/// Tracks mouse routing state: hover (hot) and capture (active).
///
/// Feed platform mouse events through `process_mouse_event` to get a list
/// of routing actions. The caller dispatches those actions to widgets.
#[derive(Debug, Default)]
pub struct InputState {
    /// Widget currently under the cursor (hot).
    hovered: Option<WidgetId>,
    /// Widget that has captured the mouse (active). While set, all mouse
    /// events route to this widget regardless of cursor position.
    captured: Option<WidgetId>,
    /// Last known cursor position.
    cursor_pos: Option<Point>,
}

impl InputState {
    /// Creates a new input state with no hover or capture.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the currently hovered widget, if any.
    pub fn hovered(&self) -> Option<WidgetId> {
        self.hovered
    }

    /// Returns the widget with mouse capture, if any.
    pub fn captured(&self) -> Option<WidgetId> {
        self.captured
    }

    /// Returns the last known cursor position.
    pub fn cursor_pos(&self) -> Option<Point> {
        self.cursor_pos
    }

    /// Explicitly sets mouse capture to a widget.
    ///
    /// While captured, all mouse events route to this widget. Call
    /// `release_capture` to clear.
    pub fn set_capture(&mut self, id: WidgetId) {
        self.captured = Some(id);
    }

    /// Releases mouse capture.
    pub fn release_capture(&mut self) {
        self.captured = None;
    }

    /// Processes a mouse event and returns routing actions.
    ///
    /// The returned actions include:
    /// - `Hover::Leave` / `Hover::Enter` when the hovered widget changes.
    /// - `Deliver` for the target widget (captured or hovered).
    ///
    /// **Capture semantics** (global, not per-button):
    /// - `MouseDown` sets capture to the hit widget.
    /// - `MouseUp` releases capture and re-evaluates hover.
    /// - While captured, hover transitions are suppressed — other widgets
    ///   do not receive Enter/Leave events during a drag.
    pub fn process_mouse_event(
        &mut self,
        event: MouseEvent,
        layout: &LayoutNode,
    ) -> Vec<RouteAction> {
        let mut actions = Vec::new();
        self.cursor_pos = Some(event.pos);

        let hit = layout_hit_test(layout, event.pos);

        // Hover transitions are suppressed during capture (Chromium pattern).
        // This prevents confusing visual feedback on uninvolved widgets
        // during a drag or press-hold interaction.
        if self.captured.is_none() {
            self.update_hover(hit, &mut actions);
        }

        // Route: captured widget takes priority over hit.
        let target = self.captured.or(hit);
        if let Some(target) = target {
            actions.push(RouteAction::Deliver { target, event });
        }

        // Auto-capture on mouse down, auto-release on mouse up.
        match event.kind {
            MouseEventKind::Down(_) => {
                if let Some(id) = hit {
                    self.captured = Some(id);
                }
            }
            MouseEventKind::Up(_) => {
                self.captured = None;
                // Re-evaluate hover now that capture is released.
                self.update_hover(hit, &mut actions);
            }
            MouseEventKind::Move | MouseEventKind::Scroll(_) => {}
        }

        actions
    }

    /// Emits hover Enter/Leave actions if the hovered widget changed.
    fn update_hover(&mut self, hit: Option<WidgetId>, actions: &mut Vec<RouteAction>) {
        if hit != self.hovered {
            if let Some(old) = self.hovered {
                actions.push(RouteAction::Hover {
                    target: old,
                    kind: HoverEvent::Leave,
                });
            }
            if let Some(new) = hit {
                actions.push(RouteAction::Hover {
                    target: new,
                    kind: HoverEvent::Enter,
                });
            }
            self.hovered = hit;
        }
    }

    /// Returns the target widget for a keyboard event.
    ///
    /// Keyboard events go to the focused widget. If no widget has focus,
    /// returns `None` (the event is unhandled at the widget level).
    /// The caller is responsible for bubbling: if the focused widget
    /// returns `Ignored`, walk up the layout tree to ancestors.
    pub fn keyboard_target(focus: &FocusManager) -> Option<WidgetId> {
        focus.focused()
    }

    /// Processes cursor leaving the window entirely.
    ///
    /// Generates a `Leave` event for the currently hovered widget and
    /// clears cursor position.
    pub fn process_cursor_left(&mut self) -> Vec<RouteAction> {
        let mut actions = Vec::new();
        if let Some(old) = self.hovered.take() {
            actions.push(RouteAction::Hover {
                target: old,
                kind: HoverEvent::Leave,
            });
        }
        self.cursor_pos = None;
        actions
    }
}
