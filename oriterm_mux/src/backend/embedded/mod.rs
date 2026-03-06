//! In-process backend wrapping [`InProcessMux`] with local pane ownership.
//!
//! [`EmbeddedMux`] stores `Pane` structs internally alongside the mux
//! orchestrator, presenting them through the [`MuxBackend`] trait. The
//! wakeup callback is captured at construction — individual methods never
//! need it as a parameter.

use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use oriterm_core::selection::{self, Selection};
use oriterm_core::{RenderableContent, Theme};

use super::MuxBackend;
use crate::domain::SpawnConfig;
use crate::in_process::{ClosePaneResult, InProcessMux};
use crate::mux_event::{MuxEvent, MuxNotification};
use crate::pane::Pane;
use crate::registry::PaneEntry;
use crate::server::snapshot::build_snapshot_into;
use crate::{DomainId, PaneId, PaneSnapshot};

/// In-process mux backend for single-process mode.
///
/// Owns the [`InProcessMux`] orchestrator, the `Pane` map, and the wakeup
/// callback. The App interacts exclusively through [`MuxBackend`] methods.
pub struct EmbeddedMux {
    mux: InProcessMux,
    panes: HashMap<PaneId, Pane>,
    /// Coalesced wakeup closure — wraps the raw wakeup with an [`AtomicBool`]
    /// guard so that only one `PostMessage` is issued per poll cycle.
    guarded_wakeup: Arc<dyn Fn() + Send + Sync>,
    /// Coalescing flag cleared in [`poll_events`](MuxBackend::poll_events).
    wakeup_pending: Arc<AtomicBool>,
    snapshot_cache: HashMap<PaneId, PaneSnapshot>,
    snapshot_dirty: HashSet<PaneId>,
    /// Per-pane [`RenderableContent`] cache, filled by
    /// [`refresh_pane_snapshot`](MuxBackend::refresh_pane_snapshot) and
    /// consumed by [`swap_renderable_content`](MuxBackend::swap_renderable_content).
    ///
    /// Bypasses the `RenderableContent → WireCell → RenderableContent` round-trip
    /// that the snapshot path requires for daemon mode IPC. Vec allocations are
    /// reused across frames via [`std::mem::swap`].
    renderable_cache: HashMap<PaneId, RenderableContent>,
}

impl EmbeddedMux {
    /// Create a new embedded backend.
    ///
    /// `wakeup` is called by PTY reader threads to wake the event loop.
    /// The closure is wrapped with an [`AtomicBool`] guard so that only
    /// one wakeup is posted per poll cycle during flood output.
    pub fn new(wakeup: Arc<dyn Fn() + Send + Sync>) -> Self {
        let wakeup_pending = Arc::new(AtomicBool::new(false));
        let guarded_wakeup = {
            let pending = wakeup_pending.clone();
            Arc::new(move || {
                if !pending.swap(true, Ordering::Release) {
                    (wakeup)();
                }
            }) as Arc<dyn Fn() + Send + Sync>
        };
        Self {
            mux: InProcessMux::new(),
            panes: HashMap::new(),
            guarded_wakeup,
            wakeup_pending,
            snapshot_cache: HashMap::new(),
            snapshot_dirty: HashSet::new(),
            renderable_cache: HashMap::new(),
        }
    }
}

impl MuxBackend for EmbeddedMux {
    fn poll_events(&mut self) {
        self.wakeup_pending.store(false, Ordering::Release);
        self.mux.poll_events(&mut self.panes);

        // Mark panes dirty when the PTY reader thread has set grid_dirty.
        for (&pane_id, pane) in &self.panes {
            if pane.grid_dirty() {
                self.snapshot_dirty.insert(pane_id);
                pane.clear_grid_dirty();
            }
        }
    }

    fn drain_notifications(&mut self, out: &mut Vec<MuxNotification>) {
        self.mux.drain_notifications(out);
    }

    fn discard_notifications(&mut self) {
        self.mux.discard_notifications();
    }

    fn get_pane_entry(&self, pane_id: PaneId) -> Option<PaneEntry> {
        self.mux.get_pane_entry(pane_id).cloned()
    }

    fn spawn_pane(&mut self, config: &SpawnConfig, theme: Theme) -> io::Result<PaneId> {
        let (pane_id, pane) =
            self.mux
                .spawn_standalone_pane(config, theme, &self.guarded_wakeup)?;
        self.panes.insert(pane_id, pane);
        Ok(pane_id)
    }

    fn close_pane(&mut self, pane_id: PaneId) -> ClosePaneResult {
        self.mux.close_pane(pane_id)
    }

    fn resize_pane_grid(&mut self, pane_id: PaneId, rows: u16, cols: u16) {
        if let Some(pane) = self.panes.get(&pane_id) {
            pane.resize_grid(rows, cols);
            pane.resize_pty(rows, cols);
        }
        self.snapshot_dirty.insert(pane_id);
    }

    fn pane_mode(&self, pane_id: PaneId) -> Option<u32> {
        self.panes.get(&pane_id).map(Pane::mode)
    }

    fn set_pane_theme(&mut self, pane_id: PaneId, theme: Theme, palette: oriterm_core::Palette) {
        if let Some(pane) = self.panes.get(&pane_id) {
            let mut term = pane.terminal().lock();
            term.set_theme(theme);
            *term.palette_mut() = palette;
            term.grid_mut().dirty_mut().mark_all();
        }
        self.snapshot_dirty.insert(pane_id);
    }

    fn set_cursor_shape(&mut self, pane_id: PaneId, shape: oriterm_core::CursorShape) {
        if let Some(pane) = self.panes.get(&pane_id) {
            pane.terminal().lock().set_cursor_shape(shape);
        }
        self.snapshot_dirty.insert(pane_id);
    }

    fn mark_all_dirty(&mut self, pane_id: PaneId) {
        if let Some(pane) = self.panes.get(&pane_id) {
            pane.terminal().lock().grid_mut().dirty_mut().mark_all();
        }
        self.snapshot_dirty.insert(pane_id);
    }

    fn open_search(&mut self, pane_id: PaneId) {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.open_search();
        }
        self.snapshot_dirty.insert(pane_id);
    }

    fn close_search(&mut self, pane_id: PaneId) {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.close_search();
        }
        self.snapshot_dirty.insert(pane_id);
    }

    fn search_set_query(&mut self, pane_id: PaneId, query: String) {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            let grid_ref = pane.terminal().clone();
            if let Some(search) = pane.search_mut() {
                let term = grid_ref.lock();
                search.set_query(query, term.grid());
            }
        }
        self.snapshot_dirty.insert(pane_id);
    }

    fn search_next_match(&mut self, pane_id: PaneId) {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            if let Some(search) = pane.search_mut() {
                search.next_match();
            }
        }
        self.snapshot_dirty.insert(pane_id);
    }

    fn search_prev_match(&mut self, pane_id: PaneId) {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            if let Some(search) = pane.search_mut() {
                search.prev_match();
            }
        }
        self.snapshot_dirty.insert(pane_id);
    }

    fn is_search_active(&self, pane_id: PaneId) -> bool {
        self.panes.get(&pane_id).is_some_and(Pane::is_search_active)
    }

    fn extract_text(&mut self, pane_id: PaneId, sel: &Selection) -> Option<String> {
        let pane = self.panes.get(&pane_id)?;
        let term = pane.terminal().lock();
        let text = selection::extract_text(term.grid(), sel);
        (!text.is_empty()).then_some(text)
    }

    fn extract_html(
        &mut self,
        pane_id: PaneId,
        sel: &Selection,
        font_family: &str,
        font_size: f32,
    ) -> Option<(String, String)> {
        let pane = self.panes.get(&pane_id)?;
        let term = pane.terminal().lock();
        let (html, text) = selection::extract_html_with_text(
            term.grid(),
            sel,
            term.palette(),
            font_family,
            font_size,
        );
        (!text.is_empty()).then_some((html, text))
    }

    fn scroll_display(&mut self, pane_id: PaneId, delta: isize) {
        if let Some(pane) = self.panes.get(&pane_id) {
            pane.scroll_display(delta);
        }
        self.snapshot_dirty.insert(pane_id);
    }

    fn scroll_to_bottom(&mut self, pane_id: PaneId) {
        if let Some(pane) = self.panes.get(&pane_id) {
            pane.scroll_to_bottom();
        }
        self.snapshot_dirty.insert(pane_id);
    }

    fn scroll_to_previous_prompt(&mut self, pane_id: PaneId) -> bool {
        let scrolled = self
            .panes
            .get(&pane_id)
            .is_some_and(Pane::scroll_to_previous_prompt);
        if scrolled {
            self.snapshot_dirty.insert(pane_id);
        }
        scrolled
    }

    fn scroll_to_next_prompt(&mut self, pane_id: PaneId) -> bool {
        let scrolled = self
            .panes
            .get(&pane_id)
            .is_some_and(Pane::scroll_to_next_prompt);
        if scrolled {
            self.snapshot_dirty.insert(pane_id);
        }
        scrolled
    }

    fn send_input(&mut self, pane_id: PaneId, data: &[u8]) {
        if let Some(pane) = self.panes.get(&pane_id) {
            pane.write_input(data);
        }
    }

    fn set_bell(&mut self, pane_id: PaneId) {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.set_bell();
        }
    }

    fn clear_bell(&mut self, pane_id: PaneId) {
        if let Some(pane) = self.panes.get_mut(&pane_id) {
            pane.clear_bell();
        }
    }

    fn cleanup_closed_pane(&mut self, pane_id: PaneId) {
        if let Some(pane) = self.panes.remove(&pane_id) {
            self.snapshot_cache.remove(&pane_id);
            self.snapshot_dirty.remove(&pane_id);
            self.renderable_cache.remove(&pane_id);
            // Drop on a background thread to avoid blocking the event loop.
            // Pane destruction involves PTY kill, reader thread join, and child reap.
            std::thread::spawn(move || drop(pane));
        }
    }

    fn select_command_output(&self, pane_id: PaneId) -> Option<Selection> {
        self.panes.get(&pane_id)?.command_output_selection()
    }

    fn select_command_input(&self, pane_id: PaneId) -> Option<Selection> {
        self.panes.get(&pane_id)?.command_input_selection()
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

    fn swap_renderable_content(&mut self, pane_id: PaneId, target: &mut RenderableContent) -> bool {
        let Some(cached) = self.renderable_cache.get_mut(&pane_id) else {
            return false;
        };
        // Swap Vec allocations for zero-allocation steady state: target
        // gets fresh data, cached receives the old allocation for next frame.
        std::mem::swap(&mut target.cells, &mut cached.cells);
        std::mem::swap(&mut target.damage, &mut cached.damage);
        target.cursor = cached.cursor;
        target.display_offset = cached.display_offset;
        target.stable_row_base = cached.stable_row_base;
        target.mode = cached.mode;
        target.all_dirty = cached.all_dirty;
        true
    }

    fn pane_snapshot(&self, pane_id: PaneId) -> Option<&PaneSnapshot> {
        self.snapshot_cache.get(&pane_id)
    }

    fn is_pane_snapshot_dirty(&self, pane_id: PaneId) -> bool {
        self.snapshot_dirty.contains(&pane_id)
    }

    fn refresh_pane_snapshot(&mut self, pane_id: PaneId) -> Option<&PaneSnapshot> {
        let pane = self.panes.get(&pane_id)?;
        let snapshot = self.snapshot_cache.entry(pane_id).or_default();
        let render_buf = self.renderable_cache.entry(pane_id).or_default();
        // Full snapshot build: fills cells (for tests, text extraction) AND
        // caches the RenderableContent in render_buf for swap_renderable_content().
        // The render path uses swap to bypass the WireCell → RenderableCell
        // conversion; other code reads snapshot.cells directly.
        build_snapshot_into(pane, snapshot, render_buf);
        self.snapshot_dirty.remove(&pane_id);
        self.snapshot_cache.get(&pane_id)
    }

    fn clear_pane_snapshot_dirty(&mut self, pane_id: PaneId) {
        self.snapshot_dirty.remove(&pane_id);
    }
}

#[cfg(test)]
mod tests;
