//! Pure layout computation for the tab bar.
//!
//! [`TabBarLayout`] takes tab count and window width, producing tab dimensions
//! and element positions. No rendering, no side effects — fully testable in
//! isolation. All coordinates are in logical pixels; the caller applies the
//! DPI scale factor at the rendering boundary.
//!
//! Follows the same pattern as [`ChromeLayout`](crate::widgets::window_chrome::layout::ChromeLayout):
//! a single pure function from inputs to geometry outputs.

use super::constants::{
    CONTROLS_ZONE_WIDTH, DROPDOWN_BUTTON_WIDTH, NEW_TAB_BUTTON_WIDTH, TAB_LEFT_MARGIN,
    TAB_MAX_WIDTH, TAB_MIN_WIDTH,
};

/// Computed tab bar layout geometry.
///
/// Produced by [`TabBarLayout::compute`]. All dimensions are in logical pixels
/// relative to the window's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabBarLayout {
    /// Computed width per tab in logical pixels (uniform for all tabs).
    pub tab_width: f32,
    /// Number of tabs in the layout.
    pub tab_count: usize,
    /// Window width used for this layout computation.
    pub window_width: f32,
}

impl TabBarLayout {
    /// Compute tab bar layout from tab count and window width.
    ///
    /// If `tab_width_lock` is `Some(w)`, that width is used directly —
    /// prevents tab widths from shifting during rapid close clicks or drag
    /// (the next close button stays in place). Otherwise, tab width is
    /// computed from available space and clamped to
    /// `[TAB_MIN_WIDTH, TAB_MAX_WIDTH]`.
    pub fn compute(tab_count: usize, window_width: f32, tab_width_lock: Option<f32>) -> Self {
        if let Some(locked) = tab_width_lock {
            return Self {
                tab_width: locked,
                tab_count,
                window_width,
            };
        }

        // Available space = window width minus reserved zones.
        let available = (window_width
            - TAB_LEFT_MARGIN
            - NEW_TAB_BUTTON_WIDTH
            - DROPDOWN_BUTTON_WIDTH
            - CONTROLS_ZONE_WIDTH)
            .max(0.0);

        let tab_width = if tab_count == 0 {
            TAB_MIN_WIDTH
        } else {
            (available / tab_count as f32).clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH)
        };

        Self {
            tab_width,
            tab_count,
            window_width,
        }
    }

    /// X coordinate of the right edge of the last tab.
    pub fn tabs_end(&self) -> f32 {
        TAB_LEFT_MARGIN + self.tab_count as f32 * self.tab_width
    }

    /// X coordinate of the new-tab (+) button.
    pub fn new_tab_x(&self) -> f32 {
        self.tabs_end()
    }

    /// X coordinate of the dropdown button.
    pub fn dropdown_x(&self) -> f32 {
        self.tabs_end() + NEW_TAB_BUTTON_WIDTH
    }

    /// X coordinate of the start of the window controls zone.
    pub fn controls_x(&self) -> f32 {
        self.window_width - CONTROLS_ZONE_WIDTH
    }

    /// Left X coordinate of the tab at the given index.
    pub fn tab_x(&self, index: usize) -> f32 {
        TAB_LEFT_MARGIN + index as f32 * self.tab_width
    }

    /// Maximum text width within a tab (total width minus padding and close button).
    pub fn max_text_width(&self) -> f32 {
        use super::constants::{CLOSE_BUTTON_RIGHT_PAD, CLOSE_BUTTON_WIDTH, TAB_PADDING};
        (self.tab_width - 2.0 * TAB_PADDING - CLOSE_BUTTON_WIDTH - CLOSE_BUTTON_RIGHT_PAD).max(0.0)
    }

    /// Returns the tab index at `x` pixels, or `None` if outside the tab strip.
    ///
    /// Uses half-open intervals: tab `i` owns `[tab_x(i), tab_x(i+1))`.
    pub fn tab_index_at(&self, x: f32) -> Option<usize> {
        if self.tab_count == 0 || x < TAB_LEFT_MARGIN || x >= self.tabs_end() {
            return None;
        }
        let index = ((x - TAB_LEFT_MARGIN) / self.tab_width) as usize;
        Some(index.min(self.tab_count - 1))
    }
}
