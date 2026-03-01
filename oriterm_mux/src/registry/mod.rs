//! Central registries for mux state.
//!
//! [`PaneRegistry`] tracks metadata for every pane in the system.
//! [`SessionRegistry`] owns the mux-level tabs and windows. Together they
//! provide O(1) lookup by ID and cross-reference queries (e.g., "which tab
//! contains this pane?").

use std::collections::HashMap;

use crate::id::{DomainId, PaneId, TabId, WindowId};
use crate::session::{MuxTab, MuxWindow};

/// Metadata entry for a registered pane.
///
/// This is lightweight bookkeeping — the actual `Pane` struct (with terminal
/// state, PTY handles, etc.) lives in the binary crate. The registry only
/// tracks identity and ownership.
#[derive(Debug, Clone)]
#[allow(
    dead_code,
    reason = "consumed by InProcessMux, wired to App in Section 31.2"
)]
pub struct PaneEntry {
    /// Pane identity.
    pub pane: PaneId,
    /// Which tab this pane belongs to.
    pub tab: TabId,
    /// Which domain that spawned this pane.
    pub domain: DomainId,
}

/// Registry of all panes in the mux system.
///
/// Provides O(1) lookup by `PaneId` and linear scan for tab membership
/// queries. The registry does not own `Pane` structs — it stores only
/// metadata entries.
#[derive(Debug, Default)]
#[allow(
    dead_code,
    reason = "consumed by InProcessMux, wired to App in Section 31.2"
)]
pub struct PaneRegistry {
    entries: HashMap<PaneId, PaneEntry>,
}

#[allow(
    dead_code,
    reason = "consumed by InProcessMux, wired to App in Section 31.2"
)]
impl PaneRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a pane. Overwrites any existing entry with the same ID.
    pub fn register(&mut self, entry: PaneEntry) {
        self.entries.insert(entry.pane, entry);
    }

    /// Remove a pane from the registry.
    ///
    /// Returns the removed entry, or `None` if not found.
    pub fn unregister(&mut self, pane_id: PaneId) -> Option<PaneEntry> {
        self.entries.remove(&pane_id)
    }

    /// Look up a pane by ID.
    pub fn get(&self, pane_id: PaneId) -> Option<&PaneEntry> {
        self.entries.get(&pane_id)
    }

    /// All pane IDs belonging to a given tab.
    pub fn panes_in_tab(&self, tab_id: TabId) -> Vec<PaneId> {
        self.entries
            .values()
            .filter(|e| e.tab == tab_id)
            .map(|e| e.pane)
            .collect()
    }

    /// Total number of registered panes.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Registry of mux-level tabs and windows.
///
/// Provides O(1) lookup by `TabId` and `WindowId`, plus cross-reference
/// queries to find which window contains a given tab.
#[derive(Debug, Default)]
#[allow(
    dead_code,
    reason = "consumed by InProcessMux, wired to App in Section 31.2"
)]
pub struct SessionRegistry {
    tabs: HashMap<TabId, MuxTab>,
    windows: HashMap<WindowId, MuxWindow>,
}

#[allow(
    dead_code,
    reason = "consumed by InProcessMux, wired to App in Section 31.2"
)]
impl SessionRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tab.
    pub fn add_tab(&mut self, tab: MuxTab) {
        self.tabs.insert(tab.id(), tab);
    }

    /// Remove a tab by ID.
    pub fn remove_tab(&mut self, tab_id: TabId) -> Option<MuxTab> {
        self.tabs.remove(&tab_id)
    }

    /// Look up a tab by ID.
    pub fn get_tab(&self, tab_id: TabId) -> Option<&MuxTab> {
        self.tabs.get(&tab_id)
    }

    /// Mutable access to a tab.
    pub fn get_tab_mut(&mut self, tab_id: TabId) -> Option<&mut MuxTab> {
        self.tabs.get_mut(&tab_id)
    }

    /// Register a window.
    pub fn add_window(&mut self, window: MuxWindow) {
        self.windows.insert(window.id(), window);
    }

    /// Remove a window by ID.
    pub fn remove_window(&mut self, window_id: WindowId) -> Option<MuxWindow> {
        self.windows.remove(&window_id)
    }

    /// Look up a window by ID.
    pub fn get_window(&self, window_id: WindowId) -> Option<&MuxWindow> {
        self.windows.get(&window_id)
    }

    /// Mutable access to a window.
    pub fn get_window_mut(&mut self, window_id: WindowId) -> Option<&mut MuxWindow> {
        self.windows.get_mut(&window_id)
    }

    /// Find which window contains a given tab.
    pub fn window_for_tab(&self, tab_id: TabId) -> Option<WindowId> {
        self.windows
            .values()
            .find(|w| w.tabs().contains(&tab_id))
            .map(MuxWindow::id)
    }

    /// Number of registered tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Number of registered windows.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// True when this pane is the only pane across all tabs and windows.
    pub fn is_last_pane(&self, pane_id: PaneId) -> bool {
        if self.tabs.len() != 1 {
            return false;
        }
        let Some((_, tab)) = self.tabs.iter().next() else {
            return false;
        };
        tab.all_panes() == [pane_id]
    }
}

#[cfg(test)]
mod tests;
