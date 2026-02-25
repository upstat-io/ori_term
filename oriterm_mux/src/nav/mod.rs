//! Spatial navigation between panes.
//!
//! Navigate between panes using directional movement (up/down/left/right)
//! and sequential cycling. Works identically for tiled and floating panes.

use std::fmt;

use crate::id::PaneId;
use crate::layout::compute::PaneLayout;
use crate::layout::floating::Rect;

/// Direction for spatial navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Navigate upward.
    Up,
    /// Navigate downward.
    Down,
    /// Navigate left.
    Left,
    /// Navigate right.
    Right,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Up => write!(f, "Up"),
            Self::Down => write!(f, "Down"),
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
        }
    }
}

/// Navigate from one pane to another in the given direction.
///
/// From the center of the `from` pane's rect, finds the nearest pane whose
/// center is in the specified direction. Uses perpendicular distance as a
/// tiebreaker when multiple panes are ahead.
///
/// Returns `None` if no pane exists in that direction.
pub fn navigate(layouts: &[PaneLayout], from: PaneId, direction: Direction) -> Option<PaneId> {
    let from_layout = layouts.iter().find(|l| l.pane_id == from)?;
    let from_center = rect_center(&from_layout.pixel_rect);

    let mut best: Option<(PaneId, f32)> = None;

    for layout in layouts {
        if layout.pane_id == from {
            continue;
        }
        let candidate_center = rect_center(&layout.pixel_rect);

        // Check if the candidate is in the correct direction.
        let (primary_dist, perp_dist) = match direction {
            Direction::Up => {
                if candidate_center.1 >= from_center.1 {
                    continue;
                }
                (
                    from_center.1 - candidate_center.1,
                    (from_center.0 - candidate_center.0).abs(),
                )
            }
            Direction::Down => {
                if candidate_center.1 <= from_center.1 {
                    continue;
                }
                (
                    candidate_center.1 - from_center.1,
                    (from_center.0 - candidate_center.0).abs(),
                )
            }
            Direction::Left => {
                if candidate_center.0 >= from_center.0 {
                    continue;
                }
                (
                    from_center.0 - candidate_center.0,
                    (from_center.1 - candidate_center.1).abs(),
                )
            }
            Direction::Right => {
                if candidate_center.0 <= from_center.0 {
                    continue;
                }
                (
                    candidate_center.0 - from_center.0,
                    (from_center.1 - candidate_center.1).abs(),
                )
            }
        };

        // Score: primary distance + weighted perpendicular distance.
        // The weight on perpendicular ensures "directly ahead" wins over
        // "far away but slightly to the side."
        let score = primary_dist + perp_dist * 0.5;

        if best
            .as_ref()
            .is_none_or(|(_, best_score)| score < *best_score)
        {
            best = Some((layout.pane_id, score));
        }
    }

    best.map(|(id, _)| id)
}

/// Cycle through panes in layout order.
///
/// Tiled panes come first (depth-first from the split tree), then floating
/// panes by z-order. Wraps around: last pane → first (forward),
/// first → last (backward).
pub fn cycle(layouts: &[PaneLayout], from: PaneId, forward: bool) -> Option<PaneId> {
    if layouts.is_empty() {
        return None;
    }

    let idx = layouts.iter().position(|l| l.pane_id == from)?;

    let next_idx = if forward {
        (idx + 1) % layouts.len()
    } else if idx == 0 {
        layouts.len() - 1
    } else {
        idx - 1
    };

    Some(layouts[next_idx].pane_id)
}

/// Find the pane at a given point, preferring floating panes.
///
/// Checks floating panes first (reverse z-order, highest first), then tiled
/// panes. Used for mouse click → focus.
pub fn nearest_pane(layouts: &[PaneLayout], x: f32, y: f32) -> Option<PaneId> {
    // Check floating panes first (later in the list = higher z_order).
    let floating_hit = layouts
        .iter()
        .rev()
        .filter(|l| l.is_floating)
        .find(|l| rect_contains(&l.pixel_rect, x, y))
        .map(|l| l.pane_id);

    if floating_hit.is_some() {
        return floating_hit;
    }

    // Fall back to tiled panes.
    layouts
        .iter()
        .filter(|l| !l.is_floating)
        .find(|l| rect_contains(&l.pixel_rect, x, y))
        .map(|l| l.pane_id)
}

// ── Private helpers ───────────────────────────────────────────────

fn rect_center(r: &Rect) -> (f32, f32) {
    (r.x + r.width / 2.0, r.y + r.height / 2.0)
}

fn rect_contains(r: &Rect, px: f32, py: f32) -> bool {
    px >= r.x && px < r.x + r.width && py >= r.y && py < r.y + r.height
}

#[cfg(test)]
mod tests;
