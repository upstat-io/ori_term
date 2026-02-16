//! DPI scale factor newtype.

use crate::geometry::{Point, Rect, Size};

/// Minimum scale factor (25%).
const MIN_SCALE: f64 = 0.25;

/// Maximum scale factor (800%).
const MAX_SCALE: f64 = 8.0;

/// A DPI scale factor, clamped to `[0.25, 8.0]`.
///
/// Wraps the raw `f64` value from the windowing system (e.g. winit's
/// `ScaleFactorChanged`) and provides methods for converting between
/// logical and physical coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleFactor(f64);

impl ScaleFactor {
    /// Creates a new scale factor, clamping to `[0.25, 8.0]`.
    pub fn new(factor: f64) -> Self {
        Self(factor.clamp(MIN_SCALE, MAX_SCALE))
    }

    /// Returns the raw scale factor value.
    pub fn factor(self) -> f64 {
        self.0
    }

    /// Converts a logical coordinate to a physical coordinate.
    pub fn scale(self, logical: f64) -> f64 {
        logical * self.0
    }

    /// Converts a physical coordinate to a logical coordinate.
    pub fn unscale(self, physical: f64) -> f64 {
        physical / self.0
    }

    /// Converts a logical coordinate to a physical `u32`, rounding.
    pub fn scale_u32(self, logical: f64) -> u32 {
        (logical * self.0).round() as u32
    }

    /// Scales a logical [`Point`] to physical coordinates.
    pub fn scale_point(self, point: Point) -> Point {
        let f = self.0 as f32;
        point.scale(f, f)
    }

    /// Scales a logical [`Size`] to physical coordinates.
    pub fn scale_size(self, size: Size) -> Size {
        let f = self.0 as f32;
        size.scale(f, f)
    }

    /// Scales a logical [`Rect`] to physical coordinates.
    pub fn scale_rect(self, rect: Rect) -> Rect {
        Rect::from_origin_size(self.scale_point(rect.origin), self.scale_size(rect.size))
    }
}

impl Default for ScaleFactor {
    fn default() -> Self {
        Self(1.0)
    }
}
