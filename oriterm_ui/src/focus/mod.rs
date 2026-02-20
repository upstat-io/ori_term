//! Focus management for keyboard-driven UI navigation.
//!
//! `FocusManager` tracks which widget has keyboard focus and provides
//! Tab/Shift+Tab cycling through focusable widgets. Focus order is built
//! from a tree traversal of focusable widget IDs.

use crate::widget_id::WidgetId;

/// Manages keyboard focus state and tab-order navigation.
///
/// The focus order is a flat list of focusable `WidgetId`s, built from a
/// depth-first traversal of the widget tree. The manager cycles through
/// this list on Tab/Shift+Tab.
#[derive(Debug, Default)]
pub struct FocusManager {
    /// Currently focused widget.
    focused: Option<WidgetId>,
    /// Tab order — focusable widgets in tree traversal order.
    focus_order: Vec<WidgetId>,
}

impl FocusManager {
    /// Creates a new focus manager with no focus and empty order.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the currently focused widget, if any.
    pub fn focused(&self) -> Option<WidgetId> {
        self.focused
    }

    /// Returns `true` if the given widget has focus.
    pub fn is_focused(&self, id: WidgetId) -> bool {
        self.focused == Some(id)
    }

    /// Sets keyboard focus to a specific widget.
    ///
    /// The widget does not need to be in the focus order — this allows
    /// programmatic focus of any widget.
    pub fn set_focus(&mut self, id: WidgetId) {
        self.focused = Some(id);
    }

    /// Clears keyboard focus (no widget focused).
    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    /// Updates the tab order from a new list of focusable widgets.
    ///
    /// Call this after layout changes (widget tree rebuild). If the
    /// currently focused widget is no longer in the order, focus is cleared.
    pub fn set_focus_order(&mut self, order: Vec<WidgetId>) {
        if let Some(id) = self.focused {
            if !order.contains(&id) {
                self.focused = None;
            }
        }
        self.focus_order = order;
    }

    /// Returns the current focus order.
    pub fn focus_order(&self) -> &[WidgetId] {
        &self.focus_order
    }

    /// Advances focus to the next widget in tab order.
    ///
    /// If nothing is focused, focuses the first widget. If the last widget
    /// is focused, wraps around to the first.
    pub fn focus_next(&mut self) {
        if self.focus_order.is_empty() {
            return;
        }
        self.focused = Some(match self.focused {
            None => self.focus_order[0],
            Some(id) => {
                let idx = self
                    .index_of(id)
                    .map_or(0, |i| (i + 1) % self.focus_order.len());
                self.focus_order[idx]
            }
        });
    }

    /// Moves focus to the previous widget in tab order.
    ///
    /// If nothing is focused, focuses the last widget. If the first widget
    /// is focused, wraps around to the last.
    pub fn focus_prev(&mut self) {
        if self.focus_order.is_empty() {
            return;
        }
        let len = self.focus_order.len();
        self.focused = Some(match self.focused {
            None => self.focus_order[len - 1],
            Some(id) => {
                let idx = self
                    .index_of(id)
                    .map_or(len - 1, |i| if i == 0 { len - 1 } else { i - 1 });
                self.focus_order[idx]
            }
        });
    }

    /// Returns the index of `id` in the focus order.
    fn index_of(&self, id: WidgetId) -> Option<usize> {
        self.focus_order.iter().position(|&x| x == id)
    }
}

#[cfg(test)]
mod tests;
