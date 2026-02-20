//! Overlay placement computation.
//!
//! Pure functions for positioning overlays relative to anchor rectangles.
//! Handles auto-flip when the primary direction has insufficient space
//! and clamping to keep overlays within the viewport.

use crate::geometry::{Point, Rect, Size};

/// Gap between anchor edge and overlay edge (in logical pixels).
const ANCHOR_GAP: f32 = 4.0;

/// Where an overlay should appear relative to its anchor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// Below the anchor, left-aligned.
    Below,
    /// Above the anchor, left-aligned.
    Above,
    /// Right of the anchor, top-aligned.
    Right,
    /// Left of the anchor, top-aligned.
    Left,
    /// Centered in the viewport (ignores anchor).
    Center,
    /// At an absolute point (e.g. right-click context menu).
    AtPoint(Point),
}

/// Computes the overlay rectangle given an anchor, content size, and viewport.
///
/// Applies the requested `placement`, auto-flips if the primary direction
/// would push the overlay outside the viewport, and clamps the final
/// position to stay within bounds.
pub fn compute_overlay_rect(
    anchor: Rect,
    content_size: Size,
    viewport: Rect,
    placement: Placement,
) -> Rect {
    let rect = match placement {
        Placement::Below => place_below_or_above(anchor, content_size, viewport, true),
        Placement::Above => place_below_or_above(anchor, content_size, viewport, false),
        Placement::Right => place_right_or_left(anchor, content_size, viewport, true),
        Placement::Left => place_right_or_left(anchor, content_size, viewport, false),
        Placement::Center => place_center(content_size, viewport),
        Placement::AtPoint(pt) => place_at_point(pt, content_size, viewport),
    };
    clamp_to_viewport(rect, viewport)
}

/// Places below (primary) or above (flip). Left-aligned with anchor.
fn place_below_or_above(anchor: Rect, size: Size, viewport: Rect, prefer_below: bool) -> Rect {
    let x = anchor.x();
    let below_y = anchor.bottom() + ANCHOR_GAP;
    let above_y = anchor.y() - size.height() - ANCHOR_GAP;

    let y = if prefer_below {
        if below_y + size.height() <= viewport.bottom() {
            below_y
        } else {
            above_y
        }
    } else if above_y >= viewport.y() {
        above_y
    } else {
        below_y
    };

    Rect::from_origin_size(Point::new(x, y), size)
}

/// Places right (primary) or left (flip). Top-aligned with anchor.
fn place_right_or_left(anchor: Rect, size: Size, viewport: Rect, prefer_right: bool) -> Rect {
    let y = anchor.y();
    let right_x = anchor.right() + ANCHOR_GAP;
    let left_x = anchor.x() - size.width() - ANCHOR_GAP;

    let x = if prefer_right {
        if right_x + size.width() <= viewport.right() {
            right_x
        } else {
            left_x
        }
    } else if left_x >= viewport.x() {
        left_x
    } else {
        right_x
    };

    Rect::from_origin_size(Point::new(x, y), size)
}

/// Centers the overlay in the viewport.
fn place_center(size: Size, viewport: Rect) -> Rect {
    let x = viewport.x() + (viewport.width() - size.width()) / 2.0;
    let y = viewport.y() + (viewport.height() - size.height()) / 2.0;
    Rect::from_origin_size(Point::new(x, y), size)
}

/// Places the overlay at an absolute point, clamping handled by caller.
fn place_at_point(point: Point, size: Size, _viewport: Rect) -> Rect {
    Rect::from_origin_size(point, size)
}

/// Clamps a rectangle to stay within the viewport bounds.
///
/// If the overlay is wider/taller than the viewport, it is pinned to
/// the top-left of the viewport.
fn clamp_to_viewport(rect: Rect, viewport: Rect) -> Rect {
    let mut x = rect.x();
    let mut y = rect.y();

    // Clamp right/bottom edges first (shift left/up).
    if x + rect.width() > viewport.right() {
        x = viewport.right() - rect.width();
    }
    if y + rect.height() > viewport.bottom() {
        y = viewport.bottom() - rect.height();
    }

    // Then clamp left/top (shift right/down). This wins if overlay is
    // larger than viewport — pinned to top-left.
    if x < viewport.x() {
        x = viewport.x();
    }
    if y < viewport.y() {
        y = viewport.y();
    }

    Rect::from_origin_size(Point::new(x, y), rect.size)
}
