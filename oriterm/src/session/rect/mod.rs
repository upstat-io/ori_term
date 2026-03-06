//! Axis-aligned rectangle primitive for layout computation.

/// Axis-aligned rectangle in logical pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    /// Left edge x coordinate.
    pub x: f32,
    /// Top edge y coordinate.
    pub y: f32,
    /// Width in logical pixels.
    pub width: f32,
    /// Height in logical pixels.
    pub height: f32,
}

impl Rect {
    /// Check whether a point (in logical pixels) is inside this rect.
    ///
    /// Uses half-open intervals: left/top edges are inclusive, right/bottom
    /// edges are exclusive.
    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    /// Return the center point of this rect.
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

#[cfg(test)]
mod tests;
