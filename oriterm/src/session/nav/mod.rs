//! Spatial navigation between panes.
//!
//! Navigate between panes using directional movement (up/down/left/right)
//! and sequential cycling. Works identically for tiled and floating panes.

use std::fmt;

use oriterm_mux::PaneId;

use super::compute::PaneLayout;

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

impl Direction {
    /// The opposite direction (for wrap-around navigation).
    fn opposite(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
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
    let from_center = from_layout.pixel_rect.center();

    let mut best: Option<(PaneId, f32)> = None;

    for layout in layouts {
        if layout.pane_id == from {
            continue;
        }
        let candidate_center = layout.pixel_rect.center();

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

/// Navigate with wrap-around: if no pane exists in the given direction,
/// wrap to the farthest pane on the opposite edge.
///
/// For example, pressing Right from the rightmost pane wraps to the
/// leftmost pane. This makes directional navigation cover all panes
/// without needing a separate cycle keybinding.
pub fn navigate_wrap(layouts: &[PaneLayout], from: PaneId, direction: Direction) -> Option<PaneId> {
    if let Some(id) = navigate(layouts, from, direction) {
        return Some(id);
    }

    // No pane in the requested direction — wrap to the opposite edge.
    // Pick the farthest pane in the opposite direction (same scoring logic).
    let from_layout = layouts.iter().find(|l| l.pane_id == from)?;
    let from_center = from_layout.pixel_rect.center();
    let opposite = direction.opposite();

    let mut best: Option<(PaneId, f32)> = None;
    for layout in layouts {
        if layout.pane_id == from {
            continue;
        }
        let c = layout.pixel_rect.center();

        let (primary_dist, perp_dist) = match opposite {
            Direction::Up => {
                if c.1 >= from_center.1 {
                    continue;
                }
                (from_center.1 - c.1, (from_center.0 - c.0).abs())
            }
            Direction::Down => {
                if c.1 <= from_center.1 {
                    continue;
                }
                (c.1 - from_center.1, (from_center.0 - c.0).abs())
            }
            Direction::Left => {
                if c.0 >= from_center.0 {
                    continue;
                }
                (from_center.0 - c.0, (from_center.1 - c.1).abs())
            }
            Direction::Right => {
                if c.0 <= from_center.0 {
                    continue;
                }
                (c.0 - from_center.0, (from_center.1 - c.1).abs())
            }
        };

        // Pick the farthest pane in the opposite direction (most extreme wrap target).
        let score = primary_dist + perp_dist * 0.5;
        if best
            .as_ref()
            .is_none_or(|(_, best_score)| score > *best_score)
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
        .find(|l| l.pixel_rect.contains_point(x, y))
        .map(|l| l.pane_id);

    if floating_hit.is_some() {
        return floating_hit;
    }

    // Fall back to tiled panes.
    layouts
        .iter()
        .filter(|l| !l.is_floating)
        .find(|l| l.pixel_rect.contains_point(x, y))
        .map(|l| l.pane_id)
}

#[cfg(test)]
mod tests;
