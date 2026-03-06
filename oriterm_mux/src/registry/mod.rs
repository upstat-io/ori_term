//! Central registry for pane metadata.
//!
//! [`PaneRegistry`] tracks metadata for every pane in the system, providing
//! O(1) lookup by `PaneId`. The registry does not own `Pane` structs — it
//! stores only lightweight identity and domain information.

use std::collections::HashMap;

use crate::id::{DomainId, PaneId};

/// Metadata entry for a registered pane.
///
/// Lightweight bookkeeping — the actual `Pane` struct (with terminal state,
/// PTY handles, etc.) lives in the caller's pane map.
#[derive(Debug, Clone)]
pub struct PaneEntry {
    /// Pane identity.
    pub pane: PaneId,
    /// Which domain spawned this pane.
    pub domain: DomainId,
}

/// Registry of all panes in the mux system.
///
/// Provides O(1) lookup by `PaneId`. The registry does not own `Pane`
/// structs — it stores only metadata entries.
#[derive(Debug, Default)]
pub struct PaneRegistry {
    entries: HashMap<PaneId, PaneEntry>,
}

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

    /// Total number of registered panes.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests;
