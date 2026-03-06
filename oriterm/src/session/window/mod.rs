//! GUI window: an ordered collection of tabs.
//!
//! Each window tracks which tab is active (has focus). Tab order matches the
//! visual tab bar order. The mux layer knows nothing about windows — they
//! are purely a GUI presentation concept.

use super::id::TabId;
use super::id::WindowId;

/// A GUI window: an ordered collection of tabs.
///
/// Tracks which tab is active (has focus). Tab order matches the visual
/// tab bar order.
#[derive(Debug, Clone)]
pub struct Window {
    /// Unique window identifier (GUI-allocated).
    id: WindowId,
    /// Ordered list of tab IDs.
    tabs: Vec<TabId>,
    /// Index of the active tab in `tabs`.
    active_tab_idx: usize,
}

impl Window {
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

    /// Insert a tab at a specific index. Appends if `index >= len()`.
    ///
    /// Adjusts `active_tab_idx` to continue tracking the same tab when
    /// the insertion shifts it rightward.
    #[allow(
        dead_code,
        reason = "used in tests; cross-compile target omits test code"
    )]
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
                // Removed tab was at or after active — no adjustment needed.
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

    /// Replace the entire tab list with server-authoritative data.
    ///
    /// Preserves the active tab if it still exists in the new list,
    /// otherwise resets to index 0. Used by daemon-mode clients when
    /// another process moves a tab to or from this window.
    #[allow(
        dead_code,
        reason = "used in tests; consumed when daemon-mode sync is wired"
    )]
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
