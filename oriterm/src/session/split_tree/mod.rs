//! Immutable binary layout tree with structural sharing.
//!
//! Every mutation returns a new tree; unchanged subtrees share memory via
//! `Arc`. This enables undo/redo by storing a history of tree versions.
//!
//! Inspired by Ghostty's `SplitTree` — ratio-based splits, computed layout,
//! and persistent history.

mod mutations;

use std::fmt;
use std::sync::Arc;

use oriterm_mux::PaneId;

/// Minimum ratio for any split. Prevents zero-size panes.
const MIN_RATIO: f32 = 0.1;

/// Maximum ratio for any split. Prevents zero-size panes.
const MAX_RATIO: f32 = 0.9;

/// Clamp a ratio to the valid range.
fn clamp_ratio(ratio: f32) -> f32 {
    ratio.clamp(MIN_RATIO, MAX_RATIO)
}

/// Direction of a split between two panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    // Constructors

    /// Create a tree containing a single pane.
    pub fn leaf(pane: PaneId) -> Self {
        Self::Leaf(pane)
    }

    // Query methods

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

    /// Return the first (leftmost/topmost) pane without allocating.
    pub fn first_pane(&self) -> PaneId {
        match self {
            Self::Leaf(id) => *id,
            Self::Split { first, .. } => first.first_pane(),
        }
    }

    /// Return all pane IDs in depth-first order (first child before second).
    pub fn panes(&self) -> Vec<PaneId> {
        let mut result = Vec::with_capacity(self.pane_count());
        self.collect_panes(&mut result);
        result
    }

    /// Return the maximum nesting depth. A single leaf has depth 0.
    #[allow(dead_code, reason = "used in tests; part of split tree query API")]
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
    #[allow(dead_code, reason = "used in tests; part of split tree query API")]
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
    #[allow(dead_code, reason = "used in tests; part of split tree query API")]
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

    fn collect_panes(&self, out: &mut Vec<PaneId>) {
        match self {
            Self::Leaf(id) => out.push(*id),
            Self::Split { first, second, .. } => {
                first.collect_panes(out);
                second.collect_panes(out);
            }
        }
    }
}

#[cfg(test)]
mod tests;
