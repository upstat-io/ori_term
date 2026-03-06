//! Immutable mutation methods for [`SplitTree`].
//!
//! Every method returns a new tree; unchanged subtrees share memory via `Arc`.
//! Extracted from `mod.rs` to keep file sizes under the 500-line limit.

use std::sync::Arc;

use oriterm_mux::PaneId;

use super::{SplitDirection, SplitTree, clamp_ratio};

impl SplitTree {
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
    #[allow(
        dead_code,
        reason = "used in tests; consumed when divider drag applies ratios"
    )]
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

    /// Set the ratio of the split node identified by the pane pair.
    ///
    /// The divider is the split where `first` contains `pane_before` and
    /// `second` contains `pane_after`. Returns `self` unchanged if no such
    /// split exists.
    #[must_use]
    pub fn set_divider_ratio(
        &self,
        pane_before: PaneId,
        pane_after: PaneId,
        new_ratio: f32,
    ) -> Self {
        let new_ratio = clamp_ratio(new_ratio);
        match self {
            Self::Leaf(_) => self.clone(),
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                if first.contains(pane_before) && second.contains(pane_after) {
                    Self::Split {
                        direction: *direction,
                        ratio: new_ratio,
                        first: Arc::clone(first),
                        second: Arc::clone(second),
                    }
                } else if first.contains(pane_before) && first.contains(pane_after) {
                    Self::Split {
                        direction: *direction,
                        ratio: *ratio,
                        first: Arc::new(first.set_divider_ratio(
                            pane_before,
                            pane_after,
                            new_ratio,
                        )),
                        second: Arc::clone(second),
                    }
                } else if second.contains(pane_before) && second.contains(pane_after) {
                    Self::Split {
                        direction: *direction,
                        ratio: *ratio,
                        first: Arc::clone(first),
                        second: Arc::new(second.set_divider_ratio(
                            pane_before,
                            pane_after,
                            new_ratio,
                        )),
                    }
                } else {
                    self.clone()
                }
            }
        }
    }

    /// Resize a pane by adjusting the nearest qualifying split border.
    ///
    /// Finds the deepest ancestor split on the path from root to `pane`
    /// where:
    /// - The split direction matches `axis`.
    /// - The pane is in `first` when `pane_in_first` is true, or in
    ///   `second` when false.
    ///
    /// Then adjusts that split's ratio by `delta` (positive grows first
    /// child, negative shrinks it). Clamped to 0.1..=0.9.
    ///
    /// Returns `self` unchanged if no qualifying split exists.
    #[must_use]
    #[allow(
        dead_code,
        reason = "used in tests; consumed when keyboard pane resizing is wired"
    )]
    pub fn resize_toward(
        &self,
        pane: PaneId,
        axis: SplitDirection,
        pane_in_first: bool,
        delta: f32,
    ) -> Self {
        self.resize_toward_inner(pane, axis, pane_in_first, delta).0
    }

    /// Like [`resize_toward`](Self::resize_toward), but returns `Some(new_tree)`
    /// only when a qualifying split was adjusted, or `None` when no change
    /// occurred. Avoids the caller needing a clone + `PartialEq` comparison.
    #[must_use]
    pub fn try_resize_toward(
        &self,
        pane: PaneId,
        axis: SplitDirection,
        pane_in_first: bool,
        delta: f32,
    ) -> Option<Self> {
        let (tree, changed) = self.resize_toward_inner(pane, axis, pane_in_first, delta);
        changed.then_some(tree)
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
    #[allow(
        dead_code,
        reason = "used in tests; consumed when pane swap keybinding is wired"
    )]
    pub fn swap(&self, a: PaneId, b: PaneId) -> Self {
        if !self.contains(a) || !self.contains(b) || a == b {
            return self.clone();
        }
        self.swap_inner(a, b)
    }

    // Private helpers

    /// Returns `(new_tree, changed)` where `changed` is true when a deeper
    /// split was adjusted.
    fn resize_toward_inner(
        &self,
        pane: PaneId,
        axis: SplitDirection,
        pane_in_first: bool,
        delta: f32,
    ) -> (Self, bool) {
        match self {
            Self::Leaf(_) => (self.clone(), false),
            Self::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                if first.contains(pane) {
                    let (new_first, changed) =
                        first.resize_toward_inner(pane, axis, pane_in_first, delta);
                    if changed {
                        return (
                            Self::Split {
                                direction: *direction,
                                ratio: *ratio,
                                first: Arc::new(new_first),
                                second: Arc::clone(second),
                            },
                            true,
                        );
                    }
                    if *direction == axis && pane_in_first {
                        (
                            Self::Split {
                                direction: *direction,
                                ratio: clamp_ratio(*ratio + delta),
                                first: Arc::clone(first),
                                second: Arc::clone(second),
                            },
                            true,
                        )
                    } else {
                        (self.clone(), false)
                    }
                } else if second.contains(pane) {
                    let (new_second, changed) =
                        second.resize_toward_inner(pane, axis, pane_in_first, delta);
                    if changed {
                        return (
                            Self::Split {
                                direction: *direction,
                                ratio: *ratio,
                                first: Arc::clone(first),
                                second: Arc::new(new_second),
                            },
                            true,
                        );
                    }
                    if *direction == axis && !pane_in_first {
                        (
                            Self::Split {
                                direction: *direction,
                                ratio: clamp_ratio(*ratio + delta),
                                first: Arc::clone(first),
                                second: Arc::clone(second),
                            },
                            true,
                        )
                    } else {
                        (self.clone(), false)
                    }
                } else {
                    (self.clone(), false)
                }
            }
        }
    }

    #[allow(dead_code, reason = "called by swap()")]
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
