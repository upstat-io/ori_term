//! Floating pane layer for overlay panes.
//!
//! Floating panes have absolute position and size within the tab area, rendered
//! on top of the tiled split layout. Inspired by Zellij's floating pane system.
//!
//! Most mutation methods return a new `FloatingLayer` (like `SplitTree`).
//! Hot-path operations (drag move/resize) have in-place `_mut` variants
//! that avoid cloning the entire `Vec` on every mouse move.

use super::rect::Rect;
use oriterm_mux::PaneId;

/// Default floating pane width as a fraction of available tab width.
const DEFAULT_WIDTH_FRACTION: f32 = 0.6;

/// Default floating pane height as a fraction of available tab height.
const DEFAULT_HEIGHT_FRACTION: f32 = 0.6;

/// Minimum floating pane size in terminal cells (columns, rows).
///
/// Enforced during layout computation when cell dimensions are available.
pub const MIN_FLOATING_PANE_CELLS: (u16, u16) = (20, 5);

/// Snap-to-edge threshold in logical pixels.
///
/// When a floating pane is dragged within this distance of the tab boundary,
/// its position snaps to the edge.
const SNAP_THRESHOLD_PX: f32 = 10.0;

/// A single floating pane with absolute position and size.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FloatingPane {
    /// The pane this floating overlay represents.
    pub pane_id: PaneId,
    /// Position and size in logical pixels within the tab area.
    pub rect: Rect,
    /// Stacking order. Higher values are closer to the viewer.
    pub z_order: u32,
}

impl FloatingPane {
    /// Create a floating pane centered in the available area with default size.
    ///
    /// Default size is 60% of available width and 60% of available height.
    pub fn centered(pane_id: PaneId, available: &Rect, z_order: u32) -> Self {
        let width = available.width * DEFAULT_WIDTH_FRACTION;
        let height = available.height * DEFAULT_HEIGHT_FRACTION;
        let x = available.x + (available.width - width) / 2.0;
        let y = available.y + (available.height - height) / 2.0;
        Self {
            pane_id,
            rect: Rect {
                x,
                y,
                width,
                height,
            },
            z_order,
        }
    }
}

/// Floating pane layer, ordered by z-order.
///
/// Structural methods (add, remove, raise, lower) return a new layer.
/// Hot-path methods (move, resize) have in-place `_mut` variants.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct FloatingLayer {
    /// Panes ordered by ascending z-order (front-most last).
    panes: Vec<FloatingPane>,
}

impl FloatingLayer {
    /// Create an empty floating layer.
    pub fn new() -> Self {
        Self { panes: Vec::new() }
    }

    /// Check whether this layer contains a pane with the given ID.
    pub fn contains(&self, pane_id: PaneId) -> bool {
        self.panes.iter().any(|p| p.pane_id == pane_id)
    }

    /// Return a reference to all floating panes, ordered by z-order.
    pub fn panes(&self) -> &[FloatingPane] {
        &self.panes
    }

    /// Return whether this layer has no floating panes.
    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    /// Return the pixel rect for a floating pane, or `None` if not found.
    pub fn pane_rect(&self, pane_id: PaneId) -> Option<Rect> {
        self.panes
            .iter()
            .find(|p| p.pane_id == pane_id)
            .map(|p| p.rect)
    }

    /// Find the topmost floating pane at the given point.
    ///
    /// Checks in reverse z-order (highest first) so the front-most pane wins.
    #[allow(
        dead_code,
        reason = "used in tests; consumed when floating pane click dispatch is wired"
    )]
    pub fn hit_test(&self, x: f32, y: f32) -> Option<PaneId> {
        self.panes
            .iter()
            .rev()
            .find(|p| p.rect.contains_point(x, y))
            .map(|p| p.pane_id)
    }

    /// Add a floating pane. Returns a new layer with the pane inserted at
    /// the correct z-order position.
    #[must_use]
    pub fn add(&self, pane: FloatingPane) -> Self {
        let mut panes = self.panes.clone();
        let pos = panes.partition_point(|p| p.z_order <= pane.z_order);
        panes.insert(pos, pane);
        Self { panes }
    }

    /// Remove a floating pane by ID. Returns a new layer without that pane.
    #[must_use]
    pub fn remove(&self, pane_id: PaneId) -> Self {
        let panes = self
            .panes
            .iter()
            .filter(|p| p.pane_id != pane_id)
            .cloned()
            .collect();
        Self { panes }
    }

    /// Move a floating pane to a new position. Returns a new layer.
    #[must_use]
    #[allow(
        dead_code,
        reason = "used in tests; immutable variant of move_pane_mut"
    )]
    pub fn move_pane(&self, pane_id: PaneId, x: f32, y: f32) -> Self {
        let panes = self
            .panes
            .iter()
            .map(|p| {
                if p.pane_id == pane_id {
                    FloatingPane {
                        rect: Rect { x, y, ..p.rect },
                        ..p.clone()
                    }
                } else {
                    p.clone()
                }
            })
            .collect();
        Self { panes }
    }

    /// Resize a floating pane. Returns a new layer.
    #[must_use]
    #[allow(
        dead_code,
        reason = "used in tests; immutable variant of resize_pane_mut"
    )]
    pub fn resize_pane(&self, pane_id: PaneId, width: f32, height: f32) -> Self {
        let panes = self
            .panes
            .iter()
            .map(|p| {
                if p.pane_id == pane_id {
                    FloatingPane {
                        rect: Rect {
                            width,
                            height,
                            ..p.rect
                        },
                        ..p.clone()
                    }
                } else {
                    p.clone()
                }
            })
            .collect();
        Self { panes }
    }

    /// Move a floating pane to a new position in-place.
    ///
    /// Hot-path variant of [`move_pane`](Self::move_pane) that avoids cloning
    /// the entire Vec. Preferred during drag interactions (~60 calls/sec).
    pub fn move_pane_mut(&mut self, pane_id: PaneId, x: f32, y: f32) {
        if let Some(p) = self.panes.iter_mut().find(|p| p.pane_id == pane_id) {
            p.rect.x = x;
            p.rect.y = y;
        }
    }

    /// Resize a floating pane in-place.
    ///
    /// Hot-path variant of [`resize_pane`](Self::resize_pane) that avoids
    /// cloning the entire Vec. Preferred during drag interactions.
    pub fn resize_pane_mut(&mut self, pane_id: PaneId, width: f32, height: f32) {
        if let Some(p) = self.panes.iter_mut().find(|p| p.pane_id == pane_id) {
            p.rect.width = width;
            p.rect.height = height;
        }
    }

    /// Resize and move a floating pane in a single pass, in-place.
    ///
    /// Used during edge/corner resize drags where both position and size
    /// change simultaneously (e.g., dragging the top-left corner).
    pub fn set_pane_rect_mut(&mut self, pane_id: PaneId, rect: Rect) {
        if let Some(p) = self.panes.iter_mut().find(|p| p.pane_id == pane_id) {
            p.rect = rect;
        }
    }

    /// Bring a pane to the front (highest z-order). Returns a new layer.
    #[must_use]
    pub fn raise(&self, pane_id: PaneId) -> Self {
        let max_z = self.panes.iter().map(|p| p.z_order).max().unwrap_or(0);
        let new_z = max_z + 1;
        let panes = self
            .panes
            .iter()
            .map(|p| {
                if p.pane_id == pane_id {
                    FloatingPane {
                        z_order: new_z,
                        ..p.clone()
                    }
                } else {
                    p.clone()
                }
            })
            .collect();
        let mut layer = Self { panes };
        layer.panes.sort_by_key(|p| p.z_order);
        layer
    }

    /// Send a pane to the back (lowest z-order). Returns a new layer.
    ///
    /// The target pane gets z-order 0 and all other panes are shifted up by
    /// 1 to maintain unique ordering.
    #[must_use]
    #[allow(
        dead_code,
        reason = "used in tests; consumed when floating z-order UI is wired"
    )]
    pub fn lower(&self, pane_id: PaneId) -> Self {
        let panes = self
            .panes
            .iter()
            .map(|p| {
                if p.pane_id == pane_id {
                    FloatingPane {
                        z_order: 0,
                        ..p.clone()
                    }
                } else {
                    FloatingPane {
                        z_order: p.z_order + 1,
                        ..p.clone()
                    }
                }
            })
            .collect();
        let mut layer = Self { panes };
        layer.panes.sort_by_key(|p| p.z_order);
        layer
    }
}

/// Snap a floating pane position to the tab boundary edges.
///
/// If the pane is within `SNAP_THRESHOLD_PX` (10px) of any edge of `bounds`,
/// the position is adjusted to align with that edge. Both left/top edges
/// (position snap) and right/bottom edges (far-side snap accounting for pane
/// size) are checked independently.
///
/// Call this before `FloatingLayer::move_pane()` to get snap-to-edge behavior.
pub fn snap_to_edge(
    x: f32,
    y: f32,
    pane_width: f32,
    pane_height: f32,
    bounds: &Rect,
) -> (f32, f32) {
    let mut sx = x;
    let mut sy = y;

    // Snap left edge.
    if (sx - bounds.x).abs() <= SNAP_THRESHOLD_PX {
        sx = bounds.x;
    }
    // Snap right edge.
    let right_gap = (bounds.x + bounds.width) - (sx + pane_width);
    if right_gap.abs() <= SNAP_THRESHOLD_PX {
        sx = bounds.x + bounds.width - pane_width;
    }
    // Snap top edge.
    if (sy - bounds.y).abs() <= SNAP_THRESHOLD_PX {
        sy = bounds.y;
    }
    // Snap bottom edge.
    let bottom_gap = (bounds.y + bounds.height) - (sy + pane_height);
    if bottom_gap.abs() <= SNAP_THRESHOLD_PX {
        sy = bounds.y + bounds.height - pane_height;
    }

    (sx, sy)
}

#[cfg(test)]
mod tests;
