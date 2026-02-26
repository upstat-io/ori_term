//! In-process multiplexer orchestrating pane, tab, and window CRUD.
//!
//! [`InProcessMux`] is the synchronous, single-thread multiplexer that owns
//! the registries, ID allocators, and domain list. It does not own `Pane`
//! structs — those live in `App.panes` to avoid borrow conflicts between
//! layout queries and terminal mutation.
//!
//! Event flow: PTY reader threads → `mpsc` → [`InProcessMux::poll_events`] →
//! [`MuxNotification`] queue → App drains notifications.

use std::collections::HashMap;
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
#[allow(dead_code, reason = "wired to App in Section 31.2")]
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
#[allow(dead_code, reason = "wired to App in Section 31.2")]
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

#[allow(dead_code, reason = "wired to App in Section 31.2")]
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

        self.notifications
            .push(MuxNotification::TabLayoutChanged(tab_id));

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
        let tab = match self.session.get_tab_mut(tab_id) {
            Some(t) => t,
            None => return ClosePaneResult::NotFound,
        };

        // Try to remove the pane from the split tree.
        if let Some(new_tree) = tab.tree().remove(pane_id) {
            // Pane removed, tab still has panes.
            tab.set_tree(new_tree);

            // If the closed pane was active, pick the first remaining pane.
            if tab.active_pane() == pane_id {
                if let Some(&first) = tab.all_panes().first() {
                    tab.set_active_pane(first);
                }
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
                if let Some(win) = self.session.get_window_mut(wid) {
                    win.remove_tab(tab_id);
                    if win.tabs().is_empty() {
                        self.session.remove_window(wid);
                        if self.session.window_count() == 0 {
                            return ClosePaneResult::LastWindow;
                        }
                        self.notifications.push(MuxNotification::WindowClosed(wid));
                    } else {
                        self.notifications
                            .push(MuxNotification::WindowTabsChanged(wid));
                    }
                }
            }

            ClosePaneResult::TabClosed { tab_id }
        }
    }

    /// Look up a pane's metadata entry.
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
            .push(MuxNotification::WindowTabsChanged(window_id));

        Ok((tab_id, pane_id, pane))
    }

    /// Close a tab and all its panes.
    ///
    /// Returns the list of `PaneId`s that the caller should drop from its map.
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

        // Update the owning window.
        if let Some(wid) = window_id {
            if let Some(win) = self.session.get_window_mut(wid) {
                win.remove_tab(tab_id);
                self.notifications
                    .push(MuxNotification::WindowTabsChanged(wid));
            }
        }

        pane_ids
    }

    /// Split an existing pane, creating a new pane as its sibling.
    ///
    /// Returns `(PaneId, Pane)` for the newly created pane.
    #[allow(
        clippy::too_many_arguments,
        reason = "split requires source pane + direction on top of spawn params; \
                  grouped into SplitRequest when Section 31.2 wires this into App"
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

        if let Some(tab) = self.session.get_tab_mut(tab_id) {
            let new_tree = tab
                .tree()
                .split_at(source_pane, direction, new_pane_id, 0.5);
            tab.set_tree(new_tree);
        }

        self.notifications
            .push(MuxNotification::TabLayoutChanged(tab_id));

        Ok((new_pane_id, pane))
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
        all_panes
    }

    // -- Event pump --

    /// Drain `MuxEvent`s from pane reader threads and emit `MuxNotification`s.
    ///
    /// Called from the App's event loop every iteration. The `panes` map is
    /// passed so the mux can update pane metadata (title, CWD, bell) and
    /// write PTY responses without the App needing to know event internals.
    pub(crate) fn poll_events(&mut self, panes: &mut HashMap<PaneId, Pane>) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                MuxEvent::PaneOutput(id) => {
                    if let Some(pane) = panes.get(&id) {
                        pane.clear_wakeup();
                    }
                    self.notifications.push(MuxNotification::PaneDirty(id));
                }
                MuxEvent::PaneExited { pane_id, .. } => {
                    if self.close_pane(pane_id) == ClosePaneResult::LastWindow {
                        self.notifications.push(MuxNotification::LastWindowClosed);
                    }
                }
                MuxEvent::PaneTitleChanged { pane_id, title } => {
                    if let Some(pane) = panes.get_mut(&pane_id) {
                        pane.set_title(title);
                    }
                }
                MuxEvent::PaneCwdChanged { pane_id, cwd } => {
                    if let Some(pane) = panes.get_mut(&pane_id) {
                        pane.set_cwd(cwd);
                    }
                }
                MuxEvent::PaneBell(id) => {
                    if let Some(pane) = panes.get_mut(&id) {
                        pane.set_bell();
                    }
                    self.notifications.push(MuxNotification::Alert(id));
                }
                MuxEvent::PtyWrite { pane_id, data } => {
                    if let Some(pane) = panes.get(&pane_id) {
                        pane.write_input(data.as_bytes());
                    }
                }
                MuxEvent::ClipboardStore {
                    pane_id,
                    clipboard_type,
                    text,
                } => {
                    self.notifications.push(MuxNotification::ClipboardStore {
                        pane_id,
                        clipboard_type,
                        text,
                    });
                }
                MuxEvent::ClipboardLoad {
                    pane_id,
                    clipboard_type,
                    formatter,
                } => {
                    self.notifications.push(MuxNotification::ClipboardLoad {
                        pane_id,
                        clipboard_type,
                        formatter,
                    });
                }
            }
        }
    }

    /// Drain accumulated notifications for the GUI to process.
    pub(crate) fn drain_notifications(&mut self) -> Vec<MuxNotification> {
        std::mem::take(&mut self.notifications)
    }

    // -- Accessors --

    /// Immutable access to the session registry.
    pub(crate) fn session(&self) -> &SessionRegistry {
        &self.session
    }

    /// Immutable access to the pane registry.
    pub(crate) fn pane_registry(&self) -> &PaneRegistry {
        &self.pane_registry
    }

    /// Clone of the event sender for spawning new panes.
    pub(crate) fn event_tx(&self) -> &mpsc::Sender<MuxEvent> {
        &self.event_tx
    }

    /// Default domain ID for spawning.
    pub(crate) fn default_domain(&self) -> DomainId {
        self.local_domain.id()
    }
}

#[cfg(test)]
mod tests;
