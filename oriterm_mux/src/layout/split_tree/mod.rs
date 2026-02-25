//! Immutable binary layout tree with structural sharing.
//!
//! Every mutation returns a new tree; unchanged subtrees share memory via
//! `Arc`. This enables undo/redo by storing a history of tree versions.
//!
//! Inspired by Ghostty's `SplitTree` — ratio-based splits, computed layout,
//! and persistent history.

use std::fmt;
use std::sync::Arc;

use crate::id::PaneId;

/// Minimum ratio for any split. Prevents zero-size panes.
const MIN_RATIO: f32 = 0.1;

/// Maximum ratio for any split. Prevents zero-size panes.
const MAX_RATIO: f32 = 0.9;

/// Clamp a ratio to the valid range.
fn clamp_ratio(ratio: f32) -> f32 {
    ratio.clamp(MIN_RATIO, MAX_RATIO)
}

/// Direction of a split between two panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SplitDirection {
    /// Top/bottom split. First child is on top, second is on the bottom.
    Horizontal,
    /// Left/right split. First child is on the left, second is on the right.
    Vertical,
}

impl fmt::Display for SplitDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Horizontal => write!(f, "Horizontal"),
            Self::Vertical => write!(f, "Vertical"),
        }
    }
}

/// Immutable binary layout tree.
///
/// Every mutation method returns a new tree (COW via `Arc`).
/// History of previous trees enables undo/redo.
#[derive(Debug, Clone, PartialEq)]
pub enum SplitTree {
    /// A single pane occupying the entire area.
    Leaf(PaneId),
    /// A split dividing the area between two subtrees.
    Split {
        /// How the area is divided.
        direction: SplitDirection,
        /// Fraction of space given to `first` (clamped to 0.1..=0.9).
        ratio: f32,
        /// Left (vertical) or top (horizontal) subtree.
        first: Arc<Self>,
        /// Right (vertical) or bottom (horizontal) subtree.
        second: Arc<Self>,
    },
}

impl SplitTree {
    // ── Constructors ──────────────────────────────────────────────

    /// Create a tree containing a single pane.
    pub fn leaf(pane: PaneId) -> Self {
        Self::Leaf(pane)
    }

    // ── Query methods ─────────────────────────────────────────────

    /// Check whether this tree contains the given pane.
    pub fn contains(&self, pane: PaneId) -> bool {
        match self {
            Self::Leaf(id) => *id == pane,
            Self::Split { first, second, .. } => first.contains(pane) || second.contains(pane),
        }
    }

    /// Return the number of panes (leaves) in this tree.
    pub fn pane_count(&self) -> usize {
        match self {
            Self::Leaf(_) => 1,
            Self::Split { first, second, .. } => first.pane_count() + second.pane_count(),
        }
    }

    /// Return all pane IDs in depth-first order (first child before second).
    pub fn panes(&self) -> Vec<PaneId> {
        let mut result = Vec::with_capacity(self.pane_count());
        self.collect_panes(&mut result);
        result
    }

    /// Return the maximum nesting depth. A single leaf has depth 0.
    pub fn depth(&self) -> usize {
        match self {
            Self::Leaf(_) => 0,
            Self::Split { first, second, .. } => 1 + first.depth().max(second.depth()),
        }
    }

    /// Return the direction and ratio of the split containing the given pane.
    ///
    /// Returns `None` if the pane is the root (not inside any split) or not
    /// found in the tree.
    pub fn parent_split(&self, pane: PaneId) -> Option<(SplitDirection, f32)> {
        match self {
            Self::Leaf(_) => None,
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                // Check if either direct child is the target leaf.
                let first_is_leaf = matches!(first.as_ref(), Self::Leaf(id) if *id == pane);
                let second_is_leaf = matches!(second.as_ref(), Self::Leaf(id) if *id == pane);
                if first_is_leaf || second_is_leaf {
                    return Some((*direction, *ratio));
                }
                // Recurse into children.
                first
                    .parent_split(pane)
                    .or_else(|| second.parent_split(pane))
            }
        }
    }

    /// Return the sibling pane in the same split, if the sibling is a leaf.
    ///
    /// Returns `None` if the pane is not found, is the root, or the sibling
    /// is itself a split (not a single pane).
    pub fn sibling(&self, pane: PaneId) -> Option<PaneId> {
        match self {
            Self::Leaf(_) => None,
            Self::Split { first, second, .. } => {
                if matches!(first.as_ref(), Self::Leaf(id) if *id == pane) {
                    if let Self::Leaf(sib) = second.as_ref() {
                        return Some(*sib);
                    }
                }
                if matches!(second.as_ref(), Self::Leaf(id) if *id == pane) {
                    if let Self::Leaf(sib) = first.as_ref() {
                        return Some(*sib);
                    }
                }
                first.sibling(pane).or_else(|| second.sibling(pane))
            }
        }
    }

    // ── Immutable mutation methods ────────────────────────────────

    /// Split the given pane, placing `new_pane` as the second child.
    ///
    /// Returns a new tree where `Leaf(pane)` is replaced by a `Split` node
    /// with `pane` as first and `new_pane` as second. Unchanged subtrees
    /// share memory via `Arc`.
    ///
    /// Returns `self` unchanged if `pane` is not found.
    #[must_use]
    pub fn split_at(
        &self,
        pane: PaneId,
        direction: SplitDirection,
        new_pane: PaneId,
        ratio: f32,
    ) -> Self {
        let ratio = clamp_ratio(ratio);
        match self {
            Self::Leaf(id) if *id == pane => Self::Split {
                direction,
                ratio,
                first: Arc::new(Self::Leaf(pane)),
                second: Arc::new(Self::Leaf(new_pane)),
            },
            Self::Leaf(_) => self.clone(),
            Self::Split {
                direction: d,
                ratio: r,
                first,
                second,
            } => {
                if first.contains(pane) {
                    Self::Split {
                        direction: *d,
                        ratio: *r,
                        first: Arc::new(first.split_at(pane, direction, new_pane, ratio)),
                        second: Arc::clone(second),
                    }
                } else if second.contains(pane) {
                    Self::Split {
                        direction: *d,
                        ratio: *r,
                        first: Arc::clone(first),
                        second: Arc::new(second.split_at(pane, direction, new_pane, ratio)),
                    }
                } else {
                    self.clone()
                }
            }
        }
    }

    /// Remove a pane from the tree, collapsing the parent split to its
    /// sibling.
    ///
    /// Returns `None` if this removes the last pane (tree becomes empty).
    /// Returns `Some(tree)` with the pane removed and parent collapsed.
    /// Returns `Some(self.clone())` if the pane is not found.
    pub fn remove(&self, pane: PaneId) -> Option<Self> {
        match self {
            Self::Leaf(id) if *id == pane => None,
            Self::Leaf(_) => Some(self.clone()),
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                // Direct child removal: collapse to sibling.
                if matches!(first.as_ref(), Self::Leaf(id) if *id == pane) {
                    return Some(second.as_ref().clone());
                }
                if matches!(second.as_ref(), Self::Leaf(id) if *id == pane) {
                    return Some(first.as_ref().clone());
                }
                // Recurse into the child that contains the pane.
                if first.contains(pane) {
                    let new_first = first.remove(pane)?;
                    Some(Self::Split {
                        direction: *direction,
                        ratio: *ratio,
                        first: Arc::new(new_first),
                        second: Arc::clone(second),
                    })
                } else if second.contains(pane) {
                    let new_second = second.remove(pane)?;
                    Some(Self::Split {
                        direction: *direction,
                        ratio: *ratio,
                        first: Arc::clone(first),
                        second: Arc::new(new_second),
                    })
                } else {
                    Some(self.clone())
                }
            }
        }
    }

    /// Update the ratio of the nearest ancestor split matching `direction`
    /// that contains the given pane.
    ///
    /// The new ratio is clamped to 0.1..=0.9.
    /// Returns `self` unchanged if the pane is not found or no matching
    /// split exists.
    #[must_use]
    pub fn set_ratio(&self, pane: PaneId, direction: SplitDirection, new_ratio: f32) -> Self {
        let new_ratio = clamp_ratio(new_ratio);
        match self {
            Self::Leaf(_) => self.clone(),
            Self::Split {
                direction: d,
                ratio,
                first,
                second,
            } => {
                // If this split matches direction and contains the pane,
                // update the ratio here.
                if *d == direction && self.contains(pane) {
                    Self::Split {
                        direction: *d,
                        ratio: new_ratio,
                        first: Arc::clone(first),
                        second: Arc::clone(second),
                    }
                } else if first.contains(pane) {
                    Self::Split {
                        direction: *d,
                        ratio: *ratio,
                        first: Arc::new(first.set_ratio(pane, direction, new_ratio)),
                        second: Arc::clone(second),
                    }
                } else if second.contains(pane) {
                    Self::Split {
                        direction: *d,
                        ratio: *ratio,
                        first: Arc::clone(first),
                        second: Arc::new(second.set_ratio(pane, direction, new_ratio)),
                    }
                } else {
                    self.clone()
                }
            }
        }
    }

    /// Recursively set all split ratios to 0.5 (equal sizing).
    #[must_use]
    pub fn equalize(&self) -> Self {
        match self {
            Self::Leaf(_) => self.clone(),
            Self::Split {
                direction,
                first,
                second,
                ..
            } => Self::Split {
                direction: *direction,
                ratio: 0.5,
                first: Arc::new(first.equalize()),
                second: Arc::new(second.equalize()),
            },
        }
    }

    /// Swap two pane positions in the tree.
    ///
    /// Returns `self` unchanged if either pane is not found.
    #[must_use]
    pub fn swap(&self, a: PaneId, b: PaneId) -> Self {
        if !self.contains(a) || !self.contains(b) || a == b {
            return self.clone();
        }
        self.swap_inner(a, b)
    }

    // ── Private helpers ───────────────────────────────────────────

    fn collect_panes(&self, out: &mut Vec<PaneId>) {
        match self {
            Self::Leaf(id) => out.push(*id),
            Self::Split { first, second, .. } => {
                first.collect_panes(out);
                second.collect_panes(out);
            }
        }
    }

    fn swap_inner(&self, a: PaneId, b: PaneId) -> Self {
        match self {
            Self::Leaf(id) if *id == a => Self::Leaf(b),
            Self::Leaf(id) if *id == b => Self::Leaf(a),
            Self::Leaf(_) => self.clone(),
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => Self::Split {
                direction: *direction,
                ratio: *ratio,
                first: Arc::new(first.swap_inner(a, b)),
                second: Arc::new(second.swap_inner(a, b)),
            },
        }
    }
}

#[cfg(test)]
mod tests;
