//! In-process multiplexer orchestrating pane, tab, and window CRUD.
//!
//! [`InProcessMux`] is the synchronous, single-thread multiplexer that owns
//! the registries, ID allocators, and domain list. It does not own `Pane`
//! structs — those live in `App.panes` to avoid borrow conflicts between
//! layout queries and terminal mutation.
//!
//! Event flow: PTY reader threads → `mpsc` → [`InProcessMux::poll_events`] →
//! [`MuxNotification`] queue → App drains notifications.

mod event_pump;
mod floating_ops;
mod tab_ops;

use std::io;
use std::sync::Arc;
use std::sync::mpsc;

use crate::domain::{Domain, SpawnConfig};
use crate::registry::{PaneEntry, PaneRegistry};
use crate::session::MuxWindow;
use crate::{DomainId, IdAllocator, PaneId, SessionRegistry, TabId, WindowId};
use oriterm_core::Theme;

use crate::domain::LocalDomain;
use crate::mux_event::{MuxEvent, MuxNotification};
use crate::pane::Pane;

/// Result of closing a single pane.
#[derive(Debug, PartialEq, Eq)]
pub enum ClosePaneResult {
    /// Pane removed; its tab still has other panes.
    PaneRemoved,
    /// Last pane in the tab — tab was closed too.
    TabClosed {
        /// The tab that was removed.
        tab_id: TabId,
    },
    /// Last tab in the last window — application should exit.
    LastWindow,
    /// Pane ID was not found in the registry.
    NotFound,
}

/// Synchronous in-process multiplexer.
///
/// Orchestrates pane/tab/window CRUD, owns registries and ID allocators,
/// and bridges PTY events to GUI notifications. All operations run on the
/// main thread — no daemon, no IPC.
pub struct InProcessMux {
    // Registries.
    pane_registry: PaneRegistry,
    session: SessionRegistry,

    // Domain — stored concretely to call `spawn_pane` without downcasting.
    // Extended to a domain registry when WSL/SSH domains are added (Section 35).
    local_domain: LocalDomain,

    // ID allocators.
    #[allow(
        dead_code,
        reason = "used when WSL/SSH domains are added in Section 35"
    )]
    domain_alloc: IdAllocator<DomainId>,
    pane_alloc: IdAllocator<PaneId>,
    tab_alloc: IdAllocator<TabId>,
    window_alloc: IdAllocator<WindowId>,

    // Event channels.
    event_tx: mpsc::Sender<MuxEvent>,
    event_rx: mpsc::Receiver<MuxEvent>,
    notifications: Vec<MuxNotification>,
}

impl Default for InProcessMux {
    fn default() -> Self {
        Self::new()
    }
}

impl InProcessMux {
    /// Create a new in-process mux with a local domain.
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        let mut domain_alloc: IdAllocator<DomainId> = IdAllocator::new();
        let local_id = domain_alloc.alloc();
        let local = LocalDomain::new(local_id);

        Self {
            pane_registry: PaneRegistry::new(),
            session: SessionRegistry::new(),
            local_domain: local,
            domain_alloc,
            pane_alloc: IdAllocator::new(),
            tab_alloc: IdAllocator::new(),
            window_alloc: IdAllocator::new(),
            event_tx,
            event_rx,
            notifications: Vec::new(),
        }
    }

    // -- Pane operations --

    /// Spawn a pane in an existing tab.
    ///
    /// Returns `(PaneId, Pane)` — the caller stores the `Pane` in its own map.
    /// The mux registers the pane's metadata in the pane registry.
    pub fn spawn_pane(
        &mut self,
        tab_id: TabId,
        config: &SpawnConfig,
        theme: Theme,
        wakeup: &Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<(PaneId, Pane)> {
        let pane_id = self.pane_alloc.alloc();
        let domain_id = self.local_domain.id();
        let pane = self.local_domain.spawn_pane(
            pane_id,
            config,
            theme,
            &self.event_tx,
            Arc::clone(wakeup),
        )?;

        self.pane_registry.register(PaneEntry {
            pane: pane_id,
            tab: tab_id,
            domain: domain_id,
        });

        Ok((pane_id, pane))
    }

    /// Close a pane, updating the split tree and registries.
    ///
    /// The caller is responsible for dropping the `Pane` struct from its map.
    pub fn close_pane(&mut self, pane_id: PaneId) -> ClosePaneResult {
        let entry = match self.pane_registry.unregister(pane_id) {
            Some(e) => e,
            None => return ClosePaneResult::NotFound,
        };

        let tab_id = entry.tab;
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            // Invariant: a registered pane's tab must exist. If we reach
            // here the registry and session are out of sync — a bug.
            debug_assert!(
                false,
                "pane {pane_id:?} was registered under tab {tab_id:?} but tab is missing"
            );
            return ClosePaneResult::NotFound;
        };

        // Clear zoom if the closed pane was zoomed.
        if tab.zoomed_pane() == Some(pane_id) {
            tab.set_zoomed_pane(None);
        }

        // Check floating layer first.
        if tab.floating().contains(pane_id) {
            let new_floating = tab.floating().remove(pane_id);
            tab.set_floating(new_floating);

            // If the closed pane was active, pick the next floating pane or
            // fall back to the first tiled pane.
            if tab.active_pane() == pane_id {
                let next = tab
                    .floating()
                    .panes()
                    .last()
                    .map_or_else(|| tab.tree().first_pane(), |fp| fp.pane_id);
                tab.set_active_pane(next);
            }

            self.notifications
                .push(MuxNotification::PaneClosed(pane_id));
            self.notifications
                .push(MuxNotification::TabLayoutChanged(tab_id));
            return ClosePaneResult::PaneRemoved;
        }

        // Try to remove the pane from the split tree.
        if let Some(new_tree) = tab.tree().remove(pane_id) {
            // Pane removed, tab still has panes.
            tab.set_tree(new_tree);

            // If the closed pane was active, pick the first tiled pane.
            if tab.active_pane() == pane_id {
                tab.set_active_pane(tab.tree().first_pane());
            }

            self.notifications
                .push(MuxNotification::PaneClosed(pane_id));
            self.notifications
                .push(MuxNotification::TabLayoutChanged(tab_id));
            ClosePaneResult::PaneRemoved
        } else {
            // Last pane in the tiled tree — collect floating panes before
            // removing the tab so their registry entries and notifications
            // aren't silently dropped.
            let floating_pane_ids: Vec<PaneId> = self
                .session
                .get_tab(tab_id)
                .map(|tab| tab.floating().panes().iter().map(|fp| fp.pane_id).collect())
                .unwrap_or_default();

            for &fp_id in &floating_pane_ids {
                self.pane_registry.unregister(fp_id);
                self.notifications.push(MuxNotification::PaneClosed(fp_id));
            }

            let window_id = self.session.window_for_tab(tab_id);
            self.session.remove_tab(tab_id);

            self.notifications
                .push(MuxNotification::PaneClosed(pane_id));

            if let Some(wid) = window_id {
                if self.handle_window_after_tab_removal(wid, tab_id) {
                    self.notifications.push(MuxNotification::LastWindowClosed);
                    return ClosePaneResult::LastWindow;
                }
            }

            ClosePaneResult::TabClosed { tab_id }
        }
    }

    /// True when this pane is the only pane in the entire session.
    pub fn is_last_pane(&self, pane_id: PaneId) -> bool {
        self.session.is_last_pane(pane_id)
    }

    /// Look up a pane's metadata entry.
    pub fn get_pane_entry(&self, pane_id: PaneId) -> Option<&PaneEntry> {
        self.pane_registry.get(pane_id)
    }

    /// Handle the window after a tab has been removed from it.
    ///
    /// Removes the tab from the window. If the window is now empty, removes
    /// it and emits `WindowClosed` (non-last) or nothing (last — caller
    /// decides whether to emit `LastWindowClosed` or return a status).
    /// If tabs remain, emits `WindowTabsChanged`.
    ///
    /// Returns `true` if the last window was removed (session has zero
    /// windows). The caller must handle `LastWindowClosed` signalling.
    fn handle_window_after_tab_removal(&mut self, window_id: WindowId, tab_id: TabId) -> bool {
        let Some(win) = self.session.get_window_mut(window_id) else {
            return false;
        };
        win.remove_tab(tab_id);
        if win.tabs().is_empty() {
            self.session.remove_window(window_id);
            if self.session.window_count() == 0 {
                return true;
            }
            self.notifications
                .push(MuxNotification::WindowClosed(window_id));
        } else {
            self.notifications
                .push(MuxNotification::WindowTabsChanged(window_id));
        }
        false
    }

    // -- Window operations --

    /// Create a new empty mux window.
    pub fn create_window(&mut self) -> WindowId {
        let id = self.window_alloc.alloc();
        self.session.add_window(MuxWindow::new(id));
        id
    }

    /// Close a window and all its tabs/panes.
    ///
    /// Returns the list of `PaneId`s that the caller should drop.
    pub fn close_window(&mut self, window_id: WindowId) -> Vec<PaneId> {
        let tab_ids = match self.session.get_window(window_id) {
            Some(win) => win.tabs().to_vec(),
            None => return Vec::new(),
        };

        let mut all_panes = Vec::new();
        for tid in tab_ids {
            if let Some(tab) = self.session.get_tab(tid) {
                let panes = tab.all_panes();
                for &pid in &panes {
                    self.pane_registry.unregister(pid);
                    self.notifications.push(MuxNotification::PaneClosed(pid));
                }
                all_panes.extend(panes);
            }
            self.session.remove_tab(tid);
        }

        self.session.remove_window(window_id);

        if self.session.window_count() == 0 {
            self.notifications.push(MuxNotification::LastWindowClosed);
        } else {
            self.notifications
                .push(MuxNotification::WindowClosed(window_id));
        }

        all_panes
    }
}

// -- Test helpers --

#[cfg(test)]
impl InProcessMux {
    /// Inject a window + tab + pane without spawning a PTY.
    ///
    /// Use raw IDs starting at 100+ to avoid collision with the allocator.
    /// Drains any notifications emitted during setup.
    pub(crate) fn inject_test_tab(&mut self, wid: WindowId, tid: TabId, pid: PaneId) {
        use crate::session::{MuxTab, MuxWindow};

        if self.session.get_window(wid).is_none() {
            self.session.add_window(MuxWindow::new(wid));
        }
        self.session.get_window_mut(wid).unwrap().add_tab(tid);
        self.session.add_tab(MuxTab::new(tid, pid));
        self.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: self.local_domain.id(),
        });
        self.notifications.clear();
    }

    /// Inject a second pane into an existing tab via a split.
    ///
    /// Modifies the tab's split tree to include `new_pid` alongside the
    /// existing root pane.
    pub(crate) fn inject_split(
        &mut self,
        tid: TabId,
        new_pid: PaneId,
        dir: crate::layout::SplitDirection,
    ) {
        let tab = self.session.get_tab_mut(tid).unwrap();
        let root = tab.tree().first_pane();
        let tree = tab.tree().split_at(root, dir, new_pid, 0.5);
        tab.set_tree(tree);
        self.pane_registry.register(PaneEntry {
            pane: new_pid,
            tab: tid,
            domain: self.local_domain.id(),
        });
        self.notifications.clear();
    }
}

#[cfg(test)]
mod tests;
