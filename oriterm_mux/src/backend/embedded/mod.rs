//! In-process backend wrapping [`InProcessMux`] with local pane ownership.
//!
//! [`EmbeddedMux`] stores `Pane` structs internally alongside the mux
//! orchestrator, presenting them through the [`MuxBackend`] trait. The
//! wakeup callback is captured at construction — individual methods never
//! need it as a parameter.

use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::Arc;
use std::sync::mpsc;

use oriterm_core::Theme;

use super::MuxBackend;
use crate::domain::SpawnConfig;
use crate::in_process::{ClosePaneResult, InProcessMux};
use crate::layout::{Rect, SplitDirection};
use crate::mux_event::{MuxEvent, MuxNotification};
use crate::pane::Pane;
use crate::registry::{PaneEntry, SessionRegistry};
use crate::{DomainId, PaneId, TabId, WindowId};

/// In-process mux backend for single-process mode.
///
/// Owns the [`InProcessMux`] orchestrator, the `Pane` map, and the wakeup
/// callback. The App interacts exclusively through [`MuxBackend`] methods.
pub struct EmbeddedMux {
    mux: InProcessMux,
    panes: HashMap<PaneId, Pane>,
    wakeup: Arc<dyn Fn() + Send + Sync>,
}

impl EmbeddedMux {
    /// Create a new embedded backend.
    ///
    /// `wakeup` is called by PTY reader threads to wake the event loop.
    pub fn new(wakeup: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            mux: InProcessMux::new(),
            panes: HashMap::new(),
            wakeup,
        }
    }
}

impl MuxBackend for EmbeddedMux {
    fn poll_events(&mut self) {
        self.mux.poll_events(&mut self.panes);
    }

    fn drain_notifications(&mut self, out: &mut Vec<MuxNotification>) {
        self.mux.drain_notifications(out);
    }

    fn discard_notifications(&mut self) {
        self.mux.discard_notifications();
    }

    fn session(&self) -> &SessionRegistry {
        self.mux.session()
    }

    fn active_tab_id(&self, window_id: WindowId) -> Option<TabId> {
        self.mux.active_tab_id(window_id)
    }

    fn get_pane_entry(&self, pane_id: PaneId) -> Option<PaneEntry> {
        self.mux.get_pane_entry(pane_id).cloned()
    }

    fn is_last_pane(&self, pane_id: PaneId) -> bool {
        self.mux.is_last_pane(pane_id)
    }

    fn create_window(&mut self) -> io::Result<WindowId> {
        Ok(self.mux.create_window())
    }

    fn close_window(&mut self, window_id: WindowId) -> Vec<PaneId> {
        self.mux.close_window(window_id)
    }

    fn create_tab(
        &mut self,
        window_id: WindowId,
        config: &SpawnConfig,
        theme: Theme,
    ) -> io::Result<(TabId, PaneId)> {
        let (tab_id, pane_id, pane) =
            self.mux
                .create_tab(window_id, config, theme, &self.wakeup)?;
        self.panes.insert(pane_id, pane);
        Ok((tab_id, pane_id))
    }

    fn close_tab(&mut self, tab_id: TabId) -> Vec<PaneId> {
        self.mux.close_tab(tab_id)
    }

    fn switch_active_tab(&mut self, window_id: WindowId, tab_id: TabId) -> bool {
        self.mux.switch_active_tab(window_id, tab_id)
    }

    fn cycle_active_tab(&mut self, window_id: WindowId, delta: isize) -> Option<TabId> {
        self.mux.cycle_active_tab(window_id, delta)
    }

    fn reorder_tab(&mut self, window_id: WindowId, from: usize, to: usize) -> bool {
        self.mux.reorder_tab(window_id, from, to)
    }

    fn move_tab_to_window(&mut self, tab_id: TabId, dest: WindowId) -> bool {
        self.mux.move_tab_to_window(tab_id, dest)
    }

    fn move_tab_to_window_at(&mut self, tab_id: TabId, dest: WindowId, index: usize) -> bool {
        self.mux.move_tab_to_window_at(tab_id, dest, index)
    }

    fn split_pane(
        &mut self,
        tab_id: TabId,
        source: PaneId,
        dir: SplitDirection,
        config: &SpawnConfig,
        theme: Theme,
    ) -> io::Result<PaneId> {
        let (pane_id, pane) =
            self.mux
                .split_pane(tab_id, source, dir, config, theme, &self.wakeup)?;
        self.panes.insert(pane_id, pane);
        Ok(pane_id)
    }

    fn close_pane(&mut self, pane_id: PaneId) -> ClosePaneResult {
        self.mux.close_pane(pane_id)
    }

    fn set_active_pane(&mut self, tab_id: TabId, pane_id: PaneId) -> bool {
        self.mux.set_active_pane(tab_id, pane_id)
    }

    fn toggle_zoom(&mut self, tab_id: TabId) {
        self.mux.toggle_zoom(tab_id);
    }

    fn unzoom_silent(&mut self, tab_id: TabId) {
        self.mux.unzoom_silent(tab_id);
    }

    fn equalize_panes(&mut self, tab_id: TabId) {
        self.mux.equalize_panes(tab_id);
    }

    fn set_divider_ratio(&mut self, tab_id: TabId, before: PaneId, after: PaneId, ratio: f32) {
        self.mux.set_divider_ratio(tab_id, before, after, ratio);
    }

    fn resize_pane(
        &mut self,
        tab_id: TabId,
        pane_id: PaneId,
        axis: SplitDirection,
        first: bool,
        delta: f32,
    ) {
        self.mux.resize_pane(tab_id, pane_id, axis, first, delta);
    }

    fn undo_split(&mut self, tab_id: TabId, live: &HashSet<PaneId>) -> bool {
        self.mux.undo_split(tab_id, live)
    }

    fn redo_split(&mut self, tab_id: TabId, live: &HashSet<PaneId>) -> bool {
        self.mux.redo_split(tab_id, live)
    }

    fn spawn_floating_pane(
        &mut self,
        tab_id: TabId,
        config: &SpawnConfig,
        theme: Theme,
        available: &Rect,
    ) -> io::Result<PaneId> {
        let (pane_id, pane) =
            self.mux
                .spawn_floating_pane(tab_id, config, theme, &self.wakeup, available)?;
        self.panes.insert(pane_id, pane);
        Ok(pane_id)
    }

    fn move_pane_to_floating(&mut self, tab_id: TabId, pane_id: PaneId, available: &Rect) -> bool {
        self.mux.move_pane_to_floating(tab_id, pane_id, available)
    }

    fn move_pane_to_tiled(&mut self, tab_id: TabId, pane_id: PaneId) -> bool {
        self.mux.move_pane_to_tiled(tab_id, pane_id)
    }

    fn move_floating_pane(&mut self, tab_id: TabId, pane_id: PaneId, x: f32, y: f32) {
        self.mux.move_floating_pane(tab_id, pane_id, x, y);
    }

    fn resize_floating_pane(&mut self, tab_id: TabId, pane_id: PaneId, w: f32, h: f32) {
        self.mux.resize_floating_pane(tab_id, pane_id, w, h);
    }

    fn set_floating_pane_rect(&mut self, tab_id: TabId, pane_id: PaneId, rect: Rect) {
        self.mux.set_floating_pane_rect(tab_id, pane_id, rect);
    }

    fn raise_floating_pane(&mut self, tab_id: TabId, pane_id: PaneId) {
        self.mux.raise_floating_pane(tab_id, pane_id);
    }

    fn pane(&self, pane_id: PaneId) -> Option<&Pane> {
        self.panes.get(&pane_id)
    }

    fn pane_mut(&mut self, pane_id: PaneId) -> Option<&mut Pane> {
        self.panes.get_mut(&pane_id)
    }

    fn remove_pane(&mut self, pane_id: PaneId) -> Option<Pane> {
        self.panes.remove(&pane_id)
    }

    fn pane_ids(&self) -> Vec<PaneId> {
        self.panes.keys().copied().collect()
    }

    fn event_tx(&self) -> Option<&mpsc::Sender<MuxEvent>> {
        Some(self.mux.event_tx())
    }

    fn default_domain(&self) -> DomainId {
        self.mux.default_domain()
    }

    fn is_daemon_mode(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests;
