//! An axis-aligned rectangle in logical pixel coordinates.
//!
//! Composed as `origin: Point` + `size: Size` (Chromium pattern).
//! Uses half-open interval semantics: the left/top edges are inclusive,
//! the right/bottom edges are exclusive — `[x, x+w) x [y, y+h)`.

use super::insets::Insets;
use super::point::Point;
use super::size::Size;

/// An axis-aligned rectangle in logical pixels.
///
/// Half-open interval: `contains()` treats left/top as inclusive and
/// right/bottom as exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[must_use]
pub struct Rect {
    /// Top-left corner.
    pub origin: Point,
    /// Width and height.
    pub size: Size,
}

impl Rect {
    /// Creates a rectangle from an origin point and a size.
    pub fn from_origin_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    /// Creates a rectangle from raw coordinates and dimensions.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    /// X coordinate of the left edge.
    pub fn x(self) -> f32 {
        self.origin.x
    }

    /// Y coordinate of the top edge.
    pub fn y(self) -> f32 {
        self.origin.y
    }

    /// Width.
    pub fn width(self) -> f32 {
        self.size.width()
    }

    /// Height.
    pub fn height(self) -> f32 {
        self.size.height()
    }

    /// X coordinate of the right edge (exclusive).
    pub fn right(self) -> f32 {
        self.origin.x + self.size.width()
    }

    /// Y coordinate of the bottom edge (exclusive).
    pub fn bottom(self) -> f32 {
        self.origin.y + self.size.height()
    }

    /// Center point.
    pub fn center(self) -> Point {
        Point::new(
            self.origin.x + self.size.width() / 2.0,
            self.origin.y + self.size.height() / 2.0,
        )
    }

    /// Returns `true` if either dimension is zero.
    pub fn is_empty(self) -> bool {
        self.size.is_empty()
    }

    /// Half-open containment: `[x, x+w) x [y, y+h)`.
    ///
    /// Left and top edges are inclusive; right and bottom are exclusive.
    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x()
            && point.x < self.right()
            && point.y >= self.y()
            && point.y < self.bottom()
    }

    /// Returns `true` if `self` and `other` overlap (share interior area).
    ///
    /// Adjacent rectangles (sharing only an edge) do not intersect. Empty
    /// rectangles never intersect.
    pub fn intersects(self, other: Self) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }
        // Standard AABB overlap: each axis must have a non-empty overlap.
        let x_overlap = self.x() < other.right() && other.x() < self.right();
        let y_overlap = self.y() < other.bottom() && other.y() < self.bottom();
        x_overlap && y_overlap
    }

    /// Returns the overlapping region, or an empty rect if disjoint.
    pub fn intersection(self, other: Self) -> Self {
        if !self.intersects(other) {
            return Self::default();
        }
        let x = self.x().max(other.x());
        let y = self.y().max(other.y());
        let r = self.right().min(other.right());
        let b = self.bottom().min(other.bottom());
        Self::new(x, y, r - x, b - y)
    }

    /// Returns the smallest rectangle enclosing both `self` and `other`.
    ///
    /// Empty rectangles are ignored: the union of an empty rect with a
    /// non-empty rect is the non-empty rect.
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x = self.x().min(other.x());
        let y = self.y().min(other.y());
        let r = self.right().max(other.right());
        let b = self.bottom().max(other.bottom());
        Self::new(x, y, r - x, b - y)
    }

    /// Returns a new rectangle shrunk by the given insets.
    ///
    /// Positive insets shrink the rectangle; negative insets expand it.
    pub fn inset(self, insets: Insets) -> Self {
        Self::new(
            self.x() + insets.left,
            self.y() + insets.top,
            self.width() - insets.width(),
            self.height() - insets.height(),
        )
    }

    /// Returns a new rectangle offset by `(dx, dy)`.
    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self::from_origin_size(self.origin.offset(dx, dy), self.size)
    }
}
