//! A 2D point in logical pixel coordinates.

use std::ops::{Add, Sub};

/// A point in 2D space, using `f32` logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[must_use]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f32,
    /// Vertical coordinate.
    pub y: f32,
}

impl Point {
    /// Creates a new point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Returns a new point offset by `(dx, dy)`.
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    /// Returns a new point with both coordinates scaled.
    pub fn scale(self, sx: f32, sy: f32) -> Self {
        Self {
            x: self.x * sx,
            y: self.y * sy,
        }
    }

    /// Euclidean distance to another point.
    pub fn distance_to(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx.hypot(dy)
    }
}

impl Add for Point {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Point {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}
