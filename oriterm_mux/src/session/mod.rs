//! Mux-level tab and window containers.
//!
//! [`MuxTab`] is a layout container that holds panes arranged in a split tree
//! with an optional floating layer. [`MuxWindow`] is a collection of tabs
//! with an active tab index. These are the mux layer's own concepts — distinct
//! from any GUI tab bar or platform window.

use std::collections::{HashSet, VecDeque};

use crate::id::{PaneId, TabId, WindowId};
use crate::layout::floating::FloatingLayer;
use crate::layout::split_tree::SplitTree;

/// Maximum number of undo entries in the split tree history.
const MAX_UNDO_ENTRIES: usize = 32;

/// A mux-level tab: a layout container for panes.
///
/// Owns the split tree that arranges panes, a floating layer for overlay
/// panes, and an undo stack for tree mutations. The active pane tracks
/// which pane has keyboard focus.
#[allow(
    dead_code,
    reason = "consumed by InProcessMux, wired to App in Section 31.2"
)]
#[derive(Debug, Clone)]
pub struct MuxTab {
    /// Unique tab identifier.
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

#[allow(
    dead_code,
    reason = "consumed by InProcessMux, wired to App in Section 31.2"
)]
impl MuxTab {
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

/// A mux-level window: an ordered collection of tabs.
///
/// Tracks which tab is active (has focus). Tab order matches the visual
/// tab bar order.
#[allow(
    dead_code,
    reason = "consumed by InProcessMux, wired to App in Section 31.2"
)]
#[derive(Debug, Clone)]
pub struct MuxWindow {
    /// Unique window identifier.
    id: WindowId,
    /// Ordered list of tab IDs.
    tabs: Vec<TabId>,
    /// Index of the active tab in `tabs`.
    active_tab_idx: usize,
}

impl MuxWindow {
    /// Create a new window with no tabs.
    pub fn new(id: WindowId) -> Self {
        Self {
            id,
            tabs: Vec::new(),
            active_tab_idx: 0,
        }
    }

    /// Window identity.
    pub fn id(&self) -> WindowId {
        self.id
    }

    /// Ordered tab list.
    pub fn tabs(&self) -> &[TabId] {
        &self.tabs
    }

    /// Reorder a tab within this window.
    ///
    /// Moves the tab at index `from` to index `to`, adjusting the active
    /// tab index so it continues to track the same tab.
    ///
    /// Returns `true` if the move was performed, `false` if either index
    /// is out of bounds.
    pub fn reorder_tab(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tabs.len() || to >= self.tabs.len() {
            return false;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);

        // Adjust active index to keep tracking the same tab.
        let active = self.active_tab_idx;
        self.active_tab_idx = if active == from {
            to
        } else if from < active && to >= active {
            active - 1
        } else if from > active && to <= active {
            active + 1
        } else {
            active
        };
        true
    }

    /// Index of the currently active tab.
    pub fn active_tab_idx(&self) -> usize {
        self.active_tab_idx
    }

    /// ID of the currently active tab, if any.
    pub fn active_tab(&self) -> Option<TabId> {
        self.tabs.get(self.active_tab_idx).copied()
    }

    /// Add a tab at the end.
    pub fn add_tab(&mut self, tab_id: TabId) {
        self.tabs.push(tab_id);
    }

    /// Insert a tab at a specific index. Appends if `index >= len()`.
    ///
    /// Adjusts `active_tab_idx` to continue tracking the same tab when
    /// the insertion shifts it rightward.
    pub fn insert_tab_at(&mut self, index: usize, tab_id: TabId) {
        if index >= self.tabs.len() {
            self.tabs.push(tab_id);
        } else {
            self.tabs.insert(index, tab_id);
            if index <= self.active_tab_idx {
                self.active_tab_idx += 1;
            }
        }
    }

    /// Remove a tab by ID. Adjusts `active_tab_idx` if needed.
    ///
    /// Returns `true` if the tab was found and removed.
    pub fn remove_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(pos) = self.tabs.iter().position(|&t| t == tab_id) {
            self.tabs.remove(pos);
            if self.tabs.is_empty() {
                self.active_tab_idx = 0;
            } else if self.active_tab_idx >= self.tabs.len() {
                self.active_tab_idx = self.tabs.len() - 1;
            } else if pos < self.active_tab_idx {
                self.active_tab_idx -= 1;
            } else {
                // Removed tab was after the active tab — no adjustment needed.
            }
            true
        } else {
            false
        }
    }

    /// Set the active tab by index. Clamps to valid range.
    pub fn set_active_tab_idx(&mut self, idx: usize) {
        if !self.tabs.is_empty() {
            self.active_tab_idx = idx.min(self.tabs.len() - 1);
        }
    }

    /// Replace the entire tab list with server-authoritative data.
    ///
    /// Preserves the active tab if it still exists in the new list,
    /// otherwise resets to index 0. Used by daemon-mode clients when
    /// another process moves a tab to or from this window.
    pub fn replace_tabs(&mut self, tab_ids: &[TabId]) {
        let prev_active = self.active_tab();
        self.tabs.clear();
        self.tabs.extend_from_slice(tab_ids);
        self.active_tab_idx = prev_active
            .and_then(|id| self.tabs.iter().position(|&t| t == id))
            .unwrap_or(0);
    }
}

#[cfg(test)]
mod tests;
