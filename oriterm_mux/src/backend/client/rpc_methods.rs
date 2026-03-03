//! IPC roundtrip implementations for [`MuxBackend`] methods.
//!
//! Each method sends a PDU to the daemon via [`MuxClient::rpc`],
//! extracts the response, and updates the mirrored [`SessionRegistry`].
//! Methods that have no corresponding PDU yet log and return defaults.
//!
//! This file exceeds 500 lines because `MuxBackend` has ~30 required methods
//! and Rust requires all trait method impls in a single `impl` block.

use std::collections::HashSet;
use std::io;
use std::sync::mpsc;

use oriterm_core::Theme;

use crate::PaneSnapshot;
use crate::backend::MuxBackend;
use crate::domain::SpawnConfig;
use crate::in_process::ClosePaneResult;
use crate::layout::{Rect, SplitDirection};
use crate::mux_event::{MuxEvent, MuxNotification};
use crate::pane::Pane;
use crate::protocol::MuxPdu;
use crate::protocol::messages::theme_to_wire;
use crate::registry::{PaneEntry, SessionRegistry};
use crate::session::{MuxTab, MuxWindow};
use crate::{DomainId, PaneId, TabId, WindowId};

use super::MuxClient;

impl MuxBackend for MuxClient {
    fn poll_events(&mut self) {
        #[cfg(unix)]
        if let Some(transport) = &self.transport {
            transport.poll_notifications(&mut self.notifications);
        }

        // Scan buffered notifications to mark panes dirty for rendering.
        for notif in &self.notifications {
            if let MuxNotification::PaneDirty(pane_id) = notif {
                self.dirty_panes.insert(*pane_id);
            }
        }
    }

    fn drain_notifications(&mut self, out: &mut Vec<MuxNotification>) {
        out.clear();
        std::mem::swap(&mut self.notifications, out);
    }

    fn discard_notifications(&mut self) {
        self.notifications.clear();
    }

    fn session(&self) -> &SessionRegistry {
        &self.local_session
    }

    fn active_tab_id(&self, window_id: WindowId) -> Option<TabId> {
        self.local_session.get_window(window_id)?.active_tab()
    }

    fn get_pane_entry(&self, pane_id: PaneId) -> Option<PaneEntry> {
        self.pane_registry.get(pane_id).cloned()
    }

    fn is_last_pane(&self, pane_id: PaneId) -> bool {
        self.local_session.is_last_pane(pane_id)
    }

    fn create_window(&mut self) -> io::Result<WindowId> {
        match self.rpc(MuxPdu::CreateWindow)? {
            MuxPdu::WindowCreated { window_id } => {
                self.local_session.add_window(MuxWindow::new(window_id));
                log::info!("daemon created window {window_id}");
                Ok(window_id)
            }
            other => Err(io::Error::other(format!(
                "create_window: unexpected response: {other:?}"
            ))),
        }
    }

    fn close_window(&mut self, window_id: WindowId) -> Vec<PaneId> {
        match self.rpc(MuxPdu::CloseWindow { window_id }) {
            Ok(MuxPdu::WindowClosed { pane_ids }) => {
                for &pid in &pane_ids {
                    self.remove_snapshot(pid);
                    self.pane_registry.unregister(pid);
                }
                self.local_session.remove_window(window_id);
                log::info!("daemon closed window {window_id}, {} panes", pane_ids.len());
                pane_ids
            }
            Ok(other) => {
                log::error!("close_window: unexpected response: {other:?}");
                Vec::new()
            }
            Err(e) => {
                log::error!("close_window: RPC failed: {e}");
                Vec::new()
            }
        }
    }

    fn create_tab(
        &mut self,
        window_id: WindowId,
        config: &SpawnConfig,
        theme: Theme,
    ) -> io::Result<(TabId, PaneId)> {
        let pdu = MuxPdu::CreateTab {
            window_id,
            shell: config.shell.clone(),
            cwd: config.cwd.as_ref().map(|p| p.display().to_string()),
            theme: theme_to_wire(theme).map(str::to_owned),
        };

        match self.rpc(pdu)? {
            MuxPdu::TabCreated {
                tab_id,
                pane_id,
                domain_id,
            } => {
                let tab = MuxTab::new(tab_id, pane_id);
                self.local_session.add_tab(tab);
                if let Some(win) = self.local_session.get_window_mut(window_id) {
                    win.add_tab(tab_id);
                }
                self.pane_registry.register(PaneEntry {
                    pane: pane_id,
                    tab: tab_id,
                    domain: domain_id,
                });

                // Subscribe to the new pane and cache its initial snapshot.
                self.subscribe_pane(pane_id);

                log::info!("daemon created tab {tab_id} with pane {pane_id}");
                Ok((tab_id, pane_id))
            }
            other => Err(io::Error::other(format!(
                "create_tab: unexpected response: {other:?}"
            ))),
        }
    }

    fn close_tab(&mut self, tab_id: TabId) -> Vec<PaneId> {
        match self.rpc(MuxPdu::CloseTab { tab_id }) {
            Ok(MuxPdu::TabClosed) => {
                let pane_ids: Vec<PaneId> = self
                    .local_session
                    .get_tab(tab_id)
                    .map(MuxTab::all_panes)
                    .unwrap_or_default();

                for &pid in &pane_ids {
                    self.unsubscribe_pane(pid);
                    self.pane_registry.unregister(pid);
                }

                if let Some(win_id) = self.local_session.window_for_tab(tab_id) {
                    if let Some(win) = self.local_session.get_window_mut(win_id) {
                        win.remove_tab(tab_id);
                    }
                }
                self.local_session.remove_tab(tab_id);

                pane_ids
            }
            Ok(other) => {
                log::error!("close_tab: unexpected response: {other:?}");
                Vec::new()
            }
            Err(e) => {
                log::error!("close_tab: RPC failed: {e}");
                Vec::new()
            }
        }
    }

    fn switch_active_tab(&mut self, window_id: WindowId, tab_id: TabId) -> bool {
        match self.rpc(MuxPdu::SetActiveTab { window_id, tab_id }) {
            Ok(MuxPdu::ActiveTabChanged { tab_id: new_id }) => {
                if let Some(win) = self.local_session.get_window_mut(window_id) {
                    if let Some(idx) = win.tabs().iter().position(|&t| t == new_id) {
                        win.set_active_tab_idx(idx);
                    }
                }
                true
            }
            Ok(other) => {
                log::error!("switch_active_tab: unexpected response: {other:?}");
                false
            }
            Err(e) => {
                log::error!("switch_active_tab: RPC failed: {e}");
                false
            }
        }
    }

    fn cycle_active_tab(&mut self, window_id: WindowId, delta: isize) -> Option<TabId> {
        let delta_i32: i32 = delta.try_into().unwrap_or(if delta > 0 { 1 } else { -1 });

        match self.rpc(MuxPdu::CycleTab {
            window_id,
            delta: delta_i32,
        }) {
            Ok(MuxPdu::ActiveTabChanged { tab_id }) => {
                if let Some(win) = self.local_session.get_window_mut(window_id) {
                    if let Some(idx) = win.tabs().iter().position(|&t| t == tab_id) {
                        win.set_active_tab_idx(idx);
                    }
                }
                Some(tab_id)
            }
            Ok(other) => {
                log::warn!("cycle_active_tab: unexpected response: {other:?}");
                None
            }
            Err(e) => {
                log::error!("cycle_active_tab: RPC failed: {e}");
                None
            }
        }
    }

    fn reorder_tab(&mut self, window_id: WindowId, from: usize, to: usize) -> bool {
        // Phase 2: needs new PDU type. Apply locally for UI responsiveness.
        log::debug!("reorder_tab: deferred (no PDU), window={window_id} {from}->{to}");
        if let Some(win) = self.local_session.get_window_mut(window_id) {
            win.reorder_tab(from, to)
        } else {
            false
        }
    }

    fn move_tab_to_window(&mut self, tab_id: TabId, dest: WindowId) -> bool {
        match self.rpc(MuxPdu::MoveTabToWindow {
            tab_id,
            target_window_id: dest,
        }) {
            Ok(MuxPdu::TabMovedAck) => {
                if let Some(src_id) = self.local_session.window_for_tab(tab_id) {
                    if let Some(src_win) = self.local_session.get_window_mut(src_id) {
                        src_win.remove_tab(tab_id);
                    }
                }
                if let Some(dest_win) = self.local_session.get_window_mut(dest) {
                    dest_win.add_tab(tab_id);
                }
                true
            }
            Ok(other) => {
                log::error!("move_tab_to_window: unexpected response: {other:?}");
                false
            }
            Err(e) => {
                log::error!("move_tab_to_window: RPC failed: {e}");
                false
            }
        }
    }

    fn move_tab_to_window_at(&mut self, tab_id: TabId, dest: WindowId, index: usize) -> bool {
        if !self.move_tab_to_window(tab_id, dest) {
            return false;
        }
        // Adjust position locally — MoveTabToWindow appends by default.
        if let Some(win) = self.local_session.get_window_mut(dest) {
            let len = win.tabs().len();
            if len > 1 {
                win.reorder_tab(len - 1, index.min(len - 1));
            }
        }
        true
    }

    fn split_pane(
        &mut self,
        tab_id: TabId,
        source: PaneId,
        dir: SplitDirection,
        config: &SpawnConfig,
        theme: Theme,
    ) -> io::Result<PaneId> {
        let pdu = MuxPdu::SplitPane {
            tab_id,
            pane_id: source,
            direction: dir,
            shell: config.shell.clone(),
            cwd: config.cwd.as_ref().map(|p| p.display().to_string()),
            theme: theme_to_wire(theme).map(str::to_owned),
        };

        match self.rpc(pdu)? {
            MuxPdu::PaneSplit {
                new_pane_id,
                domain_id,
            } => {
                self.pane_registry.register(PaneEntry {
                    pane: new_pane_id,
                    tab: tab_id,
                    domain: domain_id,
                });

                // Subscribe to the new pane and cache its initial snapshot.
                self.subscribe_pane(new_pane_id);

                Ok(new_pane_id)
            }
            other => Err(io::Error::other(format!(
                "split_pane: unexpected response: {other:?}"
            ))),
        }
    }

    fn close_pane(&mut self, pane_id: PaneId) -> ClosePaneResult {
        match self.rpc(MuxPdu::ClosePane { pane_id }) {
            Ok(MuxPdu::PaneClosedAck) => {
                self.unsubscribe_pane(pane_id);
                self.pane_registry.unregister(pane_id);
                ClosePaneResult::PaneRemoved
            }
            Ok(other) => {
                log::error!("close_pane: unexpected response: {other:?}");
                ClosePaneResult::NotFound
            }
            Err(e) => {
                log::error!("close_pane: RPC failed: {e}");
                ClosePaneResult::NotFound
            }
        }
    }

    fn set_active_pane(&mut self, tab_id: TabId, pane_id: PaneId) -> bool {
        // Local-only for now (no PDU).
        if let Some(tab) = self.local_session.get_tab_mut(tab_id) {
            tab.set_active_pane(pane_id);
            true
        } else {
            false
        }
    }

    // -- Layout operations (Phase 2 stubs) --

    fn toggle_zoom(&mut self, tab_id: TabId) {
        log::debug!("toggle_zoom: deferred (no PDU), tab={tab_id}");
    }

    fn unzoom_silent(&mut self, tab_id: TabId) {
        log::debug!("unzoom_silent: deferred (no PDU), tab={tab_id}");
    }

    fn equalize_panes(&mut self, tab_id: TabId) {
        log::debug!("equalize_panes: deferred (no PDU), tab={tab_id}");
    }

    fn set_divider_ratio(&mut self, tab_id: TabId, _before: PaneId, _after: PaneId, _ratio: f32) {
        log::debug!("set_divider_ratio: deferred (no PDU), tab={tab_id}");
    }

    fn resize_pane(
        &mut self,
        tab_id: TabId,
        _pane_id: PaneId,
        _axis: SplitDirection,
        _first: bool,
        _delta: f32,
    ) {
        log::debug!("resize_pane: deferred (no PDU), tab={tab_id}");
    }

    fn undo_split(&mut self, tab_id: TabId, _live: &HashSet<PaneId>) -> bool {
        log::debug!("undo_split: deferred (no PDU), tab={tab_id}");
        false
    }

    fn redo_split(&mut self, tab_id: TabId, _live: &HashSet<PaneId>) -> bool {
        log::debug!("redo_split: deferred (no PDU), tab={tab_id}");
        false
    }

    // -- Floating pane operations (Phase 2 stubs) --

    fn spawn_floating_pane(
        &mut self,
        _tab_id: TabId,
        _config: &SpawnConfig,
        _theme: Theme,
        _available: &Rect,
    ) -> io::Result<PaneId> {
        Err(io::Error::other(
            "spawn_floating_pane: not yet supported in daemon mode",
        ))
    }

    fn move_pane_to_floating(
        &mut self,
        _tab_id: TabId,
        _pane_id: PaneId,
        _available: &Rect,
    ) -> bool {
        false
    }

    fn move_pane_to_tiled(&mut self, _tab_id: TabId, _pane_id: PaneId) -> bool {
        false
    }

    fn move_floating_pane(&mut self, _tab_id: TabId, _pane_id: PaneId, _x: f32, _y: f32) {}

    fn resize_floating_pane(&mut self, _tab_id: TabId, _pane_id: PaneId, _w: f32, _h: f32) {}

    fn set_floating_pane_rect(&mut self, _tab_id: TabId, _pane_id: PaneId, _rect: Rect) {}

    fn raise_floating_pane(&mut self, _tab_id: TabId, _pane_id: PaneId) {}

    fn send_input(&mut self, pane_id: PaneId, data: &[u8]) {
        #[cfg(unix)]
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::Input {
                pane_id,
                data: data.to_vec(),
            });
        }
        #[cfg(not(unix))]
        {
            let _ = (pane_id, data);
        }
    }

    // -- Pane data access --

    fn pane(&self, _pane_id: PaneId) -> Option<&Pane> {
        // Daemon owns pane data — not available locally.
        None
    }

    fn pane_mut(&mut self, _pane_id: PaneId) -> Option<&mut Pane> {
        None
    }

    fn remove_pane(&mut self, _pane_id: PaneId) -> Option<Pane> {
        None
    }

    fn pane_ids(&self) -> Vec<PaneId> {
        Vec::new()
    }

    // -- Event channel --

    fn event_tx(&self) -> Option<&mpsc::Sender<MuxEvent>> {
        // No local event channel in daemon mode.
        None
    }

    fn default_domain(&self) -> DomainId {
        DomainId::from_raw(0)
    }

    fn claim_window(&mut self, window_id: WindowId) -> io::Result<()> {
        match self.rpc(MuxPdu::ClaimWindow { window_id })? {
            MuxPdu::WindowClaimed => {
                log::info!("claimed {window_id} on daemon");

                // Subscribe to all panes in all tabs of this window.
                if let Some(win) = self.local_session.get_window(window_id) {
                    let tab_ids: Vec<TabId> = win.tabs().to_vec();
                    for tab_id in tab_ids {
                        if let Some(tab) = self.local_session.get_tab(tab_id) {
                            for pane_id in tab.all_panes() {
                                self.subscribe_pane(pane_id);
                            }
                        }
                    }
                }

                Ok(())
            }
            other => Err(io::Error::other(format!(
                "claim_window: unexpected response: {other:?}"
            ))),
        }
    }

    fn refresh_window_tabs(&mut self, window_id: WindowId) {
        match self.rpc(MuxPdu::ListTabs { window_id }) {
            Ok(MuxPdu::TabList { tabs }) => {
                if let Some(win) = self.local_session.get_window_mut(window_id) {
                    // Replace the window's tab list with the server-authoritative data.
                    let tab_ids: Vec<TabId> = tabs.iter().map(|t| t.tab_id).collect();
                    win.replace_tabs(&tab_ids);
                }
                log::debug!("refreshed tabs for {window_id}: {} tabs", tabs.len());
            }
            Ok(other) => {
                log::error!("refresh_window_tabs: unexpected response: {other:?}");
            }
            Err(e) => {
                log::error!("refresh_window_tabs: RPC failed: {e}");
            }
        }
    }

    fn is_connected(&self) -> bool {
        Self::is_connected(self)
    }

    fn is_daemon_mode(&self) -> bool {
        true
    }

    // -- Snapshot access --

    fn pane_snapshot(&self, pane_id: PaneId) -> Option<&PaneSnapshot> {
        self.pane_snapshots.get(&pane_id)
    }

    fn is_pane_snapshot_dirty(&self, pane_id: PaneId) -> bool {
        self.dirty_panes.contains(&pane_id)
    }

    fn refresh_pane_snapshot(&mut self, pane_id: PaneId) -> Option<&PaneSnapshot> {
        match self.rpc(MuxPdu::GetPaneSnapshot { pane_id }) {
            Ok(MuxPdu::PaneSnapshotResp { snapshot }) => {
                self.pane_snapshots.insert(pane_id, snapshot);
                self.pane_snapshots.get(&pane_id)
            }
            Ok(other) => {
                log::error!("refresh_pane_snapshot: unexpected response: {other:?}");
                None
            }
            Err(e) => {
                log::error!("refresh_pane_snapshot: RPC failed: {e}");
                None
            }
        }
    }

    fn clear_pane_snapshot_dirty(&mut self, pane_id: PaneId) {
        self.dirty_panes.remove(&pane_id);
    }
}
