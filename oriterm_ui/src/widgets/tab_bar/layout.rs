//! Pure layout computation for the tab bar.
//!
//! [`TabBarLayout`] takes tab count and window width, producing tab dimensions
//! and element positions. No rendering, no side effects — fully testable in
//! isolation. All coordinates are in logical pixels; the caller applies the
//! DPI scale factor at the rendering boundary.
//!
//! Follows the same pattern as [`ChromeLayout`](crate::widgets::window_chrome::layout::ChromeLayout):
//! a single pure function from inputs to geometry outputs.
//!
//! When width multipliers are provided (during open/close animations), each
//! tab has an individual effective width and cumulative X position stored in
//! `per_tab_widths` and `tab_positions`.

use super::constants::{
    CONTROLS_ZONE_WIDTH, DROPDOWN_BUTTON_WIDTH, NEW_TAB_BUTTON_WIDTH, TAB_LEFT_MARGIN,
    TAB_MAX_WIDTH, TAB_MIN_WIDTH,
};

/// Computed tab bar layout geometry.
///
/// Produced by [`TabBarLayout::compute`]. All dimensions are in logical pixels
/// relative to the window's top-left corner.
#[derive(Debug, Clone, PartialEq)]
pub struct TabBarLayout {
    /// Base (uniform) width per tab in logical pixels.
    pub(crate) tab_width: f32,
    /// Number of tabs in the layout.
    pub(crate) tab_count: usize,
    /// Window width used for this layout computation.
    pub(crate) window_width: f32,
    /// Extra left margin for platform chrome (macOS traffic lights).
    pub(crate) left_inset: f32,
    /// Pre-computed X position of each tab (cumulative sums with multipliers).
    tab_positions: Vec<f32>,
    /// Effective width of each tab (`tab_width * multiplier`).
    per_tab_widths: Vec<f32>,
}

impl TabBarLayout {
    /// Compute tab bar layout from tab count and window width.
    ///
    /// `left_inset` reserves extra space on the left for platform chrome
    /// (macOS traffic lights = 76px; Windows/Linux = 0). If `tab_width_lock`
    /// is `Some(w)`, that width is used directly — prevents tab widths from
    /// shifting during rapid close clicks or drag.
    ///
    /// `width_multipliers` provides per-tab width scaling (0.0 = collapsed,
    /// 1.0 = full width). When `None`, all tabs use uniform width.
    pub fn compute(
        tab_count: usize,
        window_width: f32,
        tab_width_lock: Option<f32>,
        left_inset: f32,
    ) -> Self {
        Self::compute_with_multipliers(tab_count, window_width, tab_width_lock, left_inset, None)
    }

    /// Compute layout with optional per-tab width multipliers.
    pub fn compute_with_multipliers(
        tab_count: usize,
        window_width: f32,
        tab_width_lock: Option<f32>,
        left_inset: f32,
        width_multipliers: Option<&[f32]>,
    ) -> Self {
        let base_width = if let Some(locked) = tab_width_lock {
            locked
        } else {
            let available = (window_width
                - TAB_LEFT_MARGIN
                - left_inset
                - NEW_TAB_BUTTON_WIDTH
                - DROPDOWN_BUTTON_WIDTH
                - CONTROLS_ZONE_WIDTH)
                .max(0.0);

            if tab_count == 0 {
                TAB_MIN_WIDTH
            } else {
                (available / tab_count as f32).clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH)
            }
        };

        let left = TAB_LEFT_MARGIN + left_inset;
        let mut tab_positions = Vec::with_capacity(tab_count);
        let mut per_tab_widths = Vec::with_capacity(tab_count);
        let mut x = left;

        for i in 0..tab_count {
            tab_positions.push(x);
            let m = width_multipliers
                .and_then(|ms| ms.get(i).copied())
                .unwrap_or(1.0);
            let w = base_width * m;
            per_tab_widths.push(w);
            x += w;
        }

        Self {
            tab_width: base_width,
            tab_count,
            window_width,
            left_inset,
            tab_positions,
            per_tab_widths,
        }
    }

    /// Base (uniform) tab width in logical pixels.
    pub fn base_tab_width(&self) -> f32 {
        self.tab_width
    }

    /// Number of tabs in this layout.
    pub fn tab_count(&self) -> usize {
        self.tab_count
    }

    /// Window width used for this layout computation.
    pub fn window_width(&self) -> f32 {
        self.window_width
    }

    /// X coordinate of the right edge of the last tab.
    pub fn tabs_end(&self) -> f32 {
        if let (Some(&last_x), Some(&last_w)) =
            (self.tab_positions.last(), self.per_tab_widths.last())
        {
            last_x + last_w
        } else {
            TAB_LEFT_MARGIN + self.left_inset
        }
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
        self.tab_positions
            .get(index)
            .copied()
            .unwrap_or_else(|| self.tabs_end())
    }

    /// Effective width of the tab at the given index.
    ///
    /// Returns the base `tab_width` if `index` is out of bounds.
    pub fn tab_width_at(&self, index: usize) -> f32 {
        self.per_tab_widths
            .get(index)
            .copied()
            .unwrap_or(self.tab_width)
    }

    /// Maximum text width within a tab (total width minus padding and close button).
    pub fn max_text_width(&self) -> f32 {
        use super::constants::{CLOSE_BUTTON_RIGHT_PAD, CLOSE_BUTTON_WIDTH, TAB_PADDING};
        (self.tab_width - 2.0 * TAB_PADDING - CLOSE_BUTTON_WIDTH - CLOSE_BUTTON_RIGHT_PAD).max(0.0)
    }

    /// Returns the tab index at `x` pixels, or `None` if outside the tab strip.
    ///
    /// Uses half-open intervals: tab `i` owns `[tab_x(i), tab_x(i) + width(i))`.
    /// Uses binary search over pre-computed positions for O(log n) lookup.
    pub fn tab_index_at(&self, x: f32) -> Option<usize> {
        if self.tab_count == 0 || x < self.tab_x(0) || x >= self.tabs_end() {
            return None;
        }
        // Binary search: find the rightmost position <= x.
        let idx = self
            .tab_positions
            .partition_point(|&pos| pos <= x)
            .saturating_sub(1);
        Some(idx.min(self.tab_count - 1))
    }
}
