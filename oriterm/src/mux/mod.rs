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

use std::io;
use std::sync::mpsc;

use winit::event_loop::EventLoopProxy;

use oriterm_core::Theme;
use oriterm_mux::domain::{Domain, SpawnConfig};
use oriterm_mux::layout::SplitDirection;
use oriterm_mux::registry::{PaneEntry, PaneRegistry};
use oriterm_mux::session::{MuxTab, MuxWindow};
use oriterm_mux::{DomainId, IdAllocator, PaneId, SessionRegistry, TabId, WindowId};

use crate::domain::LocalDomain;
use crate::event::TermEvent;
use crate::mux_event::{MuxEvent, MuxNotification};
use crate::pane::Pane;

/// Result of closing a single pane.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ClosePaneResult {
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
pub(crate) struct InProcessMux {
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

impl InProcessMux {
    /// Create a new in-process mux with a local domain.
    pub(crate) fn new() -> Self {
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
    pub(crate) fn spawn_pane(
        &mut self,
        tab_id: TabId,
        config: &SpawnConfig,
        theme: Theme,
        winit_proxy: &EventLoopProxy<TermEvent>,
    ) -> io::Result<(PaneId, Pane)> {
        let pane_id = self.pane_alloc.alloc();
        let domain_id = self.local_domain.id();
        let pane =
            self.local_domain
                .spawn_pane(pane_id, config, theme, &self.event_tx, winit_proxy)?;

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
    pub(crate) fn close_pane(&mut self, pane_id: PaneId) -> ClosePaneResult {
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
            // Last pane in the tab — remove the entire tab.
            let window_id = self.session.window_for_tab(tab_id);
            self.session.remove_tab(tab_id);

            self.notifications
                .push(MuxNotification::PaneClosed(pane_id));

            if let Some(wid) = window_id {
                if self.handle_window_after_tab_removal(wid, tab_id) {
                    return ClosePaneResult::LastWindow;
                }
            }

            ClosePaneResult::TabClosed { tab_id }
        }
    }

    /// True when this pane is the only pane in the entire session.
    pub(crate) fn is_last_pane(&self, pane_id: PaneId) -> bool {
        self.session.is_last_pane(pane_id)
    }

    /// Look up a pane's metadata entry.
    #[allow(dead_code, reason = "called when tab CRUD is fully wired to App")]
    pub(crate) fn get_pane_entry(&self, pane_id: PaneId) -> Option<&PaneEntry> {
        self.pane_registry.get(pane_id)
    }

    // -- Tab operations --

    /// Create a new tab with a single pane in the given window.
    ///
    /// Returns `(TabId, PaneId, Pane)` — the caller stores the `Pane`.
    pub(crate) fn create_tab(
        &mut self,
        window_id: WindowId,
        config: &SpawnConfig,
        theme: Theme,
        winit_proxy: &EventLoopProxy<TermEvent>,
    ) -> io::Result<(TabId, PaneId, Pane)> {
        let tab_id = self.tab_alloc.alloc();
        let (pane_id, pane) = self.spawn_pane(tab_id, config, theme, winit_proxy)?;

        let mux_tab = MuxTab::new(tab_id, pane_id);
        self.session.add_tab(mux_tab);

        if let Some(win) = self.session.get_window_mut(window_id) {
            win.add_tab(tab_id);
        }

        self.notifications
            .push(MuxNotification::TabLayoutChanged(tab_id));
        self.notifications
            .push(MuxNotification::WindowTabsChanged(window_id));

        Ok((tab_id, pane_id, pane))
    }

    /// Close a tab and all its panes.
    ///
    /// Returns the list of `PaneId`s that the caller should drop from its map.
    #[allow(dead_code, reason = "called when tab close is wired to App")]
    pub(crate) fn close_tab(&mut self, tab_id: TabId) -> Vec<PaneId> {
        let pane_ids = match self.session.get_tab(tab_id) {
            Some(tab) => tab.all_panes(),
            None => return Vec::new(),
        };

        // Unregister all panes.
        for &pid in &pane_ids {
            self.pane_registry.unregister(pid);
            self.notifications.push(MuxNotification::PaneClosed(pid));
        }

        // Find the owning window before removing the tab (window_for_tab
        // needs the tab to still exist in windows, but not in session.tabs).
        let window_id = self.session.window_for_tab(tab_id);

        // Remove the tab from the session.
        self.session.remove_tab(tab_id);

        // Update the owning window (cascades to window removal if empty).
        if let Some(wid) = window_id {
            if self.handle_window_after_tab_removal(wid, tab_id) {
                self.notifications.push(MuxNotification::LastWindowClosed);
            }
        }

        pane_ids
    }

    /// Split an existing pane, creating a new pane as its sibling.
    ///
    /// Returns `(PaneId, Pane)` for the newly created pane.
    #[allow(
        clippy::too_many_arguments,
        reason = "split requires source pane + direction on top of spawn params"
    )]
    pub(crate) fn split_pane(
        &mut self,
        tab_id: TabId,
        source_pane: PaneId,
        direction: SplitDirection,
        config: &SpawnConfig,
        theme: Theme,
        winit_proxy: &EventLoopProxy<TermEvent>,
    ) -> io::Result<(PaneId, Pane)> {
        let (new_pane_id, pane) = self.spawn_pane(tab_id, config, theme, winit_proxy)?;

        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            self.pane_registry.unregister(new_pane_id);
            return Err(io::Error::other("tab not found after spawn"));
        };
        let new_tree = tab
            .tree()
            .split_at(source_pane, direction, new_pane_id, 0.5);
        tab.set_tree(new_tree);

        self.notifications
            .push(MuxNotification::TabLayoutChanged(tab_id));

        Ok((new_pane_id, pane))
    }

    /// Set the ratio of a specific divider identified by the pane pair.
    ///
    /// The divider is the split where `first` contains `pane_before` and
    /// `second` contains `pane_after`. Emits `TabLayoutChanged`.
    pub(crate) fn set_divider_ratio(
        &mut self,
        tab_id: TabId,
        pane_before: PaneId,
        pane_after: PaneId,
        new_ratio: f32,
    ) {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return;
        };
        let new_tree = tab
            .tree()
            .set_divider_ratio(pane_before, pane_after, new_ratio);
        if new_tree != *tab.tree() {
            tab.set_tree(new_tree);
            self.notifications
                .push(MuxNotification::TabLayoutChanged(tab_id));
        }
    }

    /// Resize a pane by adjusting the nearest qualifying split border.
    ///
    /// `axis` is the split direction to match, `pane_in_first` selects the
    /// qualifying child side, and `delta` adjusts the ratio. See
    /// [`SplitTree::resize_toward`] for the algorithm. Emits
    /// `TabLayoutChanged` if a qualifying split was found.
    #[expect(
        clippy::too_many_arguments,
        reason = "resize requires tab + pane + axis + side + delta"
    )]
    pub(crate) fn resize_pane(
        &mut self,
        tab_id: TabId,
        pane_id: PaneId,
        axis: SplitDirection,
        pane_in_first: bool,
        delta: f32,
    ) {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return;
        };
        if let Some(new_tree) = tab
            .tree()
            .try_resize_toward(pane_id, axis, pane_in_first, delta)
        {
            tab.set_tree(new_tree);
            self.notifications
                .push(MuxNotification::TabLayoutChanged(tab_id));
        }
    }

    /// Toggle zoom on the active pane in a tab.
    ///
    /// If already zoomed, unzooms. Otherwise zooms the active pane.
    pub(crate) fn toggle_zoom(&mut self, tab_id: TabId) {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return;
        };
        if tab.zoomed_pane().is_some() {
            tab.set_zoomed_pane(None);
        } else {
            tab.set_zoomed_pane(Some(tab.active_pane()));
        }
        self.notifications
            .push(MuxNotification::TabLayoutChanged(tab_id));
    }

    /// Clear zoom on a tab if it is currently zoomed.
    ///
    /// Emits `TabLayoutChanged` when zoom was active. For callers that will
    /// emit their own notification, use [`unzoom_silent`] instead.
    #[cfg(test)]
    pub(crate) fn unzoom(&mut self, tab_id: TabId) {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return;
        };
        if tab.zoomed_pane().is_some() {
            tab.set_zoomed_pane(None);
            self.notifications
                .push(MuxNotification::TabLayoutChanged(tab_id));
        }
    }

    /// Clear zoom without emitting a `TabLayoutChanged` notification.
    ///
    /// Used by operations that will emit their own layout notification
    /// after the subsequent mutation, avoiding a redundant recomputation.
    pub(crate) fn unzoom_silent(&mut self, tab_id: TabId) {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return;
        };
        if tab.zoomed_pane().is_some() {
            tab.set_zoomed_pane(None);
        }
    }

    /// Reset all split ratios to 0.5 (equal sizing).
    pub(crate) fn equalize_panes(&mut self, tab_id: TabId) {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return;
        };
        let new_tree = tab.tree().equalize();
        if new_tree != *tab.tree() {
            tab.set_tree(new_tree);
            self.notifications
                .push(MuxNotification::TabLayoutChanged(tab_id));
        }
    }

    /// Undo the last split tree mutation on the given tab.
    ///
    /// Returns `true` if an undo was applied.
    pub(crate) fn undo_split(
        &mut self,
        tab_id: TabId,
        live_panes: &std::collections::HashSet<PaneId>,
    ) -> bool {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return false;
        };
        if tab.undo_tree(live_panes) {
            self.notifications
                .push(MuxNotification::TabLayoutChanged(tab_id));
            true
        } else {
            false
        }
    }

    /// Redo the last undone split tree mutation on the given tab.
    ///
    /// Returns `true` if a redo was applied.
    pub(crate) fn redo_split(
        &mut self,
        tab_id: TabId,
        live_panes: &std::collections::HashSet<PaneId>,
    ) -> bool {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return false;
        };
        if tab.redo_tree(live_panes) {
            self.notifications
                .push(MuxNotification::TabLayoutChanged(tab_id));
            true
        } else {
            false
        }
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
    pub(crate) fn create_window(&mut self) -> WindowId {
        let id = self.window_alloc.alloc();
        self.session.add_window(MuxWindow::new(id));
        id
    }

    /// Close a window and all its tabs/panes.
    ///
    /// Returns the list of `PaneId`s that the caller should drop.
    #[allow(dead_code, reason = "called when window close is wired to App")]
    pub(crate) fn close_window(&mut self, window_id: WindowId) -> Vec<PaneId> {
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

#[cfg(test)]
mod tests;
