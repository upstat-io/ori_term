//! GUI tab: a layout container for panes.
//!
//! Each tab owns a split tree that arranges panes, a floating layer for
//! overlay panes, and undo/redo stacks for layout mutations. The mux layer
//! knows nothing about tabs — they are purely a GUI presentation concept.

use std::collections::{HashSet, VecDeque};

use oriterm_mux::PaneId;

use super::floating::FloatingLayer;
use super::id::TabId;
use super::split_tree::SplitTree;

/// Maximum number of undo entries in the split tree history.
const MAX_UNDO_ENTRIES: usize = 32;

/// A GUI tab: a layout container for panes.
///
/// Owns the split tree, floating layer, active pane tracking, zoom state,
/// and undo/redo for layout mutations. This replaces `oriterm_mux::MuxTab`
/// as the GUI's own session state.
#[derive(Debug, Clone)]
pub struct Tab {
    /// Unique tab identifier (GUI-allocated).
    id: TabId,
    /// Layout tree describing how panes are split.
    tree: SplitTree,
    /// Floating panes overlay.
    floating: FloatingLayer,
    /// Currently focused pane.
    active_pane: PaneId,
    /// Undo stack for split tree mutations.
    undo: VecDeque<SplitTree>,
    /// Redo stack for undone tree mutations.
    redo: VecDeque<SplitTree>,
    /// Zoomed pane (fills entire tab area, hiding other panes).
    zoomed_pane: Option<PaneId>,
}

impl Tab {
    /// Create a new tab with a single pane filling the entire area.
    pub fn new(id: TabId, pane_id: PaneId) -> Self {
        Self {
            id,
            tree: SplitTree::leaf(pane_id),
            floating: FloatingLayer::new(),
            active_pane: pane_id,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            zoomed_pane: None,
        }
    }

    /// Tab identity.
    pub fn id(&self) -> TabId {
        self.id
    }

    /// Current split tree layout.
    pub fn tree(&self) -> &SplitTree {
        &self.tree
    }

    /// Floating pane layer.
    pub fn floating(&self) -> &FloatingLayer {
        &self.floating
    }

    /// Mutable access to the floating pane layer.
    pub fn floating_mut(&mut self) -> &mut FloatingLayer {
        &mut self.floating
    }

    /// Currently focused pane in this tab.
    pub fn active_pane(&self) -> PaneId {
        self.active_pane
    }

    /// Set the active (focused) pane.
    pub fn set_active_pane(&mut self, pane_id: PaneId) {
        self.active_pane = pane_id;
    }

    /// The pane currently zoomed to fill the entire tab area, if any.
    pub fn zoomed_pane(&self) -> Option<PaneId> {
        self.zoomed_pane
    }

    /// Set or clear the zoomed pane.
    pub fn set_zoomed_pane(&mut self, pane: Option<PaneId>) {
        self.zoomed_pane = pane;
    }

    /// Replace the split tree, pushing the current tree onto the undo stack.
    ///
    /// The undo stack is capped at [`MAX_UNDO_ENTRIES`]; oldest entries are
    /// discarded when the limit is reached. The redo stack is cleared on
    /// every new mutation (standard undo/redo semantics).
    pub fn set_tree(&mut self, tree: SplitTree) {
        if self.undo.len() >= MAX_UNDO_ENTRIES {
            self.undo.pop_front();
        }
        self.undo.push_back(self.tree.clone());
        self.redo.clear();
        self.tree = tree;
    }

    /// Undo the last tree mutation, restoring the previous layout.
    ///
    /// Skips undo entries that reference panes not in `live_panes` (stale
    /// entries from closed panes). The current tree is pushed onto the redo
    /// stack before restoring.
    ///
    /// Returns `true` if the undo was applied, `false` if no valid entry
    /// was found.
    pub fn undo_tree(&mut self, live_panes: &HashSet<PaneId>) -> bool {
        while let Some(candidate) = self.undo.pop_back() {
            if candidate.panes().iter().all(|p| live_panes.contains(p)) {
                if self.redo.len() >= MAX_UNDO_ENTRIES {
                    self.redo.pop_front();
                }
                self.redo.push_back(self.tree.clone());
                self.tree = candidate;
                return true;
            }
        }
        false
    }

    /// Redo a previously undone tree mutation.
    ///
    /// Skips redo entries that reference panes not in `live_panes`. The
    /// current tree is pushed onto the undo stack before restoring.
    ///
    /// Returns `true` if the redo was applied, `false` if no valid entry
    /// was found.
    pub fn redo_tree(&mut self, live_panes: &HashSet<PaneId>) -> bool {
        while let Some(candidate) = self.redo.pop_back() {
            if candidate.panes().iter().all(|p| live_panes.contains(p)) {
                if self.undo.len() >= MAX_UNDO_ENTRIES {
                    self.undo.pop_front();
                }
                self.undo.push_back(self.tree.clone());
                self.tree = candidate;
                return true;
            }
        }
        false
    }

    /// Replace the split tree without pushing onto the undo stack.
    ///
    /// Used for server-driven layout updates and optimistic local updates
    /// that will be overwritten by the authoritative server state.
    pub fn replace_layout(&mut self, tree: SplitTree) {
        self.tree = tree;
    }

    /// Collect all pane IDs from both the split tree and floating layer.
    pub fn all_panes(&self) -> Vec<PaneId> {
        let mut panes = self.tree.panes();
        for fp in self.floating.panes() {
            panes.push(fp.pane_id);
        }
        panes
    }

    /// Replace the floating layer.
    pub fn set_floating(&mut self, layer: FloatingLayer) {
        self.floating = layer;
    }

    /// Check whether a pane is in the floating layer.
    pub fn is_floating(&self, pane_id: PaneId) -> bool {
        self.floating.contains(pane_id)
    }
}

#[cfg(test)]
mod tests;
