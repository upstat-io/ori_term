//! Edge insets (padding/margin) for layout calculations.

use std::ops::{Add, Neg, Sub};

/// Edge insets representing spacing on each side of a rectangle.
///
/// Positive values shrink a rectangle when applied via `Rect::inset()`;
/// negative values expand it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Insets {
    /// Top edge inset.
    pub top: f32,
    /// Right edge inset.
    pub right: f32,
    /// Bottom edge inset.
    pub bottom: f32,
    /// Left edge inset.
    pub left: f32,
}

impl Insets {
    /// Creates insets with all four edges set to the same value.
    pub const fn all(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    /// Creates insets with vertical (top/bottom) and horizontal (left/right).
    pub const fn vh(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Creates insets from top, left, bottom, right.
    pub const fn tlbr(top: f32, left: f32, bottom: f32, right: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Total horizontal inset (left + right).
    pub fn width(self) -> f32 {
        self.left + self.right
    }

    /// Total vertical inset (top + bottom).
    pub fn height(self) -> f32 {
        self.top + self.bottom
    }
}

impl Add for Insets {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            top: self.top + rhs.top,
            right: self.right + rhs.right,
            bottom: self.bottom + rhs.bottom,
            left: self.left + rhs.left,
        }
    }
}

impl Sub for Insets {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            top: self.top - rhs.top,
            right: self.right - rhs.right,
            bottom: self.bottom - rhs.bottom,
            left: self.left - rhs.left,
        }
    }
}

impl Neg for Insets {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            top: -self.top,
            right: -self.right,
            bottom: -self.bottom,
            left: -self.left,
        }
    }
}
