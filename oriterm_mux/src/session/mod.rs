//! Mux-level tab and window containers.
//!
//! [`MuxTab`] is a layout container that holds panes arranged in a split tree
//! with an optional floating layer. [`MuxWindow`] is a collection of tabs
//! with an active tab index. These are the mux layer's own concepts — distinct
//! from any GUI tab bar or platform window.

use std::collections::VecDeque;

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
    /// discarded when the limit is reached.
    pub fn set_tree(&mut self, tree: SplitTree) {
        if self.undo.len() >= MAX_UNDO_ENTRIES {
            self.undo.pop_front();
        }
        self.undo.push_back(self.tree.clone());
        self.tree = tree;
    }

    /// Undo the last tree mutation, restoring the previous layout.
    ///
    /// Returns `true` if the undo was applied, `false` if the stack is empty.
    pub fn undo_tree(&mut self) -> bool {
        if let Some(prev) = self.undo.pop_back() {
            self.tree = prev;
            true
        } else {
            false
        }
    }

    /// Collect all pane IDs reachable from the split tree.
    pub fn all_panes(&self) -> Vec<PaneId> {
        self.tree.panes()
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

#[allow(
    dead_code,
    reason = "consumed by InProcessMux, wired to App in Section 31.2"
)]
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
}

#[cfg(test)]
mod tests;
