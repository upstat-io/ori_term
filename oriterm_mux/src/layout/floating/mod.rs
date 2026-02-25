//! Floating pane layer for overlay panes.
//!
//! Floating panes have absolute position and size within the tab area, rendered
//! on top of the tiled split layout. Inspired by Zellij's floating pane system.
//!
//! Like `SplitTree`, the `FloatingLayer` is immutable — all mutation methods
//! return a new layer.

use crate::id::PaneId;

/// A single floating pane with absolute position and size.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatingPane {
    /// The pane this floating overlay represents.
    pub pane_id: PaneId,
    /// Logical pixels from left edge of tab area.
    pub x: f32,
    /// Logical pixels from top edge of tab area.
    pub y: f32,
    /// Logical width in pixels.
    pub width: f32,
    /// Logical height in pixels.
    pub height: f32,
    /// Stacking order. Higher values are closer to the viewer.
    pub z_order: u32,
}

impl FloatingPane {
    /// Check whether a point (in logical pixels) is inside this pane.
    fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// Simple axis-aligned rectangle for floating pane bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge x coordinate.
    pub x: f32,
    /// Top edge y coordinate.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// An immutable layer of floating panes, ordered by z-order.
///
/// All mutation methods return a new `FloatingLayer`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FloatingLayer {
    /// Panes ordered by ascending z-order (front-most last).
    panes: Vec<FloatingPane>,
}

impl FloatingLayer {
    /// Create an empty floating layer.
    pub fn new() -> Self {
        Self { panes: Vec::new() }
    }

    // ── Query methods ─────────────────────────────────────────────

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
            .map(|p| Rect {
                x: p.x,
                y: p.y,
                width: p.width,
                height: p.height,
            })
    }

    /// Find the topmost floating pane at the given point.
    ///
    /// Checks in reverse z-order (highest first) so the front-most pane wins.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<PaneId> {
        self.panes
            .iter()
            .rev()
            .find(|p| p.contains_point(x, y))
            .map(|p| p.pane_id)
    }

    // ── Immutable mutation methods ────────────────────────────────

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
    pub fn move_pane(&self, pane_id: PaneId, x: f32, y: f32) -> Self {
        let panes = self
            .panes
            .iter()
            .map(|p| {
                if p.pane_id == pane_id {
                    FloatingPane { x, y, ..p.clone() }
                } else {
                    p.clone()
                }
            })
            .collect();
        Self { panes }
    }

    /// Resize a floating pane. Returns a new layer.
    #[must_use]
    pub fn resize_pane(&self, pane_id: PaneId, width: f32, height: f32) -> Self {
        let panes = self
            .panes
            .iter()
            .map(|p| {
                if p.pane_id == pane_id {
                    FloatingPane {
                        width,
                        height,
                        ..p.clone()
                    }
                } else {
                    p.clone()
                }
            })
            .collect();
        Self { panes }
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
        // Re-sort to maintain z-order invariant.
        let mut layer = Self { panes };
        layer.panes.sort_by_key(|p| p.z_order);
        layer
    }

    /// Send a pane to the back (lowest z-order). Returns a new layer.
    ///
    /// The target pane gets z-order 0 and all other panes are shifted up by
    /// 1 to maintain unique ordering.
    #[must_use]
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

#[cfg(test)]
mod tests;
