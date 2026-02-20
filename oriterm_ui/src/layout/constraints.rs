//! Layout constraints defining min/max bounds for widget sizing.

use crate::geometry::Insets;

/// Constraints that bound the width and height of a layout box.
///
/// Used by the layout solver to communicate available space down the tree.
/// Inspired by Flutter's `BoxConstraints`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutConstraints {
    /// Minimum allowed width.
    pub min_width: f32,
    /// Maximum allowed width.
    pub max_width: f32,
    /// Minimum allowed height.
    pub min_height: f32,
    /// Maximum allowed height.
    pub max_height: f32,
}

impl Default for LayoutConstraints {
    fn default() -> Self {
        Self::unbounded()
    }
}

impl LayoutConstraints {
    /// Creates tight constraints that force an exact size.
    pub fn tight(width: f32, height: f32) -> Self {
        Self {
            min_width: width,
            max_width: width,
            min_height: height,
            max_height: height,
        }
    }

    /// Creates loose constraints with zero minimums.
    pub fn loose(max_width: f32, max_height: f32) -> Self {
        Self {
            min_width: 0.0,
            max_width,
            min_height: 0.0,
            max_height,
        }
    }

    /// Creates unbounded constraints (infinite maximums).
    pub fn unbounded() -> Self {
        Self {
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
        }
    }

    /// Clamps `width` to the allowed range.
    ///
    /// If `min_width > max_width` (conflicting constraints), min wins.
    pub fn constrain_width(self, width: f32) -> f32 {
        width.clamp(self.min_width, self.max_width.max(self.min_width))
    }

    /// Clamps `height` to the allowed range.
    ///
    /// If `min_height > max_height` (conflicting constraints), min wins.
    pub fn constrain_height(self, height: f32) -> f32 {
        height.clamp(self.min_height, self.max_height.max(self.min_height))
    }

    /// Clamps both dimensions to the allowed ranges.
    pub fn constrain(self, width: f32, height: f32) -> (f32, f32) {
        (self.constrain_width(width), self.constrain_height(height))
    }

    /// Returns `true` if min equals max for both dimensions.
    pub fn is_tight(self) -> bool {
        (self.min_width - self.max_width).abs() < f32::EPSILON
            && (self.min_height - self.max_height).abs() < f32::EPSILON
    }

    /// Returns `true` if the maximum width is finite.
    pub fn has_bounded_width(self) -> bool {
        self.max_width.is_finite()
    }

    /// Returns `true` if the maximum height is finite.
    pub fn has_bounded_height(self) -> bool {
        self.max_height.is_finite()
    }

    /// Returns constraints shrunk by the given insets on each edge.
    #[must_use]
    pub fn shrink(self, insets: Insets) -> Self {
        let w = insets.width();
        let h = insets.height();
        Self {
            min_width: (self.min_width - w).max(0.0),
            max_width: (self.max_width - w).max(0.0),
            min_height: (self.min_height - h).max(0.0),
            max_height: (self.max_height - h).max(0.0),
        }
    }
}
