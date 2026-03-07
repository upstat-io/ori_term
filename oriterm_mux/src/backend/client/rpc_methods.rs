//! IPC roundtrip implementations for [`MuxBackend`] methods.
//!
//! Each method sends a PDU to the daemon via [`MuxClient::rpc`],
//! extracts the response, and marks snapshots dirty as needed.

use std::io;
use std::sync::mpsc;

use oriterm_core::Theme;
use oriterm_core::selection::Selection;

use crate::PaneSnapshot;
use crate::backend::{ImageConfig, MuxBackend};
use crate::domain::SpawnConfig;
use crate::in_process::ClosePaneResult;
use crate::mux_event::{MuxEvent, MuxNotification};
use crate::registry::PaneEntry;

use crate::protocol::messages::theme_to_wire;
use crate::protocol::{MuxPdu, WireSelection};
use crate::{DomainId, PaneId};

use super::MuxClient;

impl MuxBackend for MuxClient {
    fn poll_events(&mut self) {
        if let Some(transport) = &self.transport {
            transport.clear_wakeup_pending();
            transport.poll_notifications(&mut self.notifications);
        }

        // Scan buffered notifications to mark panes dirty for rendering.
        for notif in &self.notifications {
            if let MuxNotification::PaneOutput(pane_id) = notif {
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

    fn get_pane_entry(&self, _pane_id: PaneId) -> Option<PaneEntry> {
        // Daemon mode: no local pane registry.
        None
    }

    fn spawn_pane(&mut self, config: &SpawnConfig, theme: Theme) -> io::Result<PaneId> {
        let pdu = MuxPdu::SpawnPane {
            shell: config.shell.clone(),
            cwd: config.cwd.as_ref().map(|p| p.display().to_string()),
            theme: theme_to_wire(theme).map(str::to_owned),
        };

        match self.rpc(pdu)? {
            MuxPdu::SpawnPaneResponse { pane_id } => {
                // Subscribe to the new pane and cache its initial snapshot.
                self.subscribe_pane(pane_id);
                log::info!("daemon spawned pane {pane_id}");
                Ok(pane_id)
            }
            other => Err(io::Error::other(format!(
                "spawn_pane: unexpected response: {other:?}"
            ))),
        }
    }

    fn close_pane(&mut self, pane_id: PaneId) -> ClosePaneResult {
        match self.rpc(MuxPdu::ClosePane { pane_id }) {
            Ok(MuxPdu::PaneClosedAck) => {
                self.remove_snapshot(pane_id);
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

    fn resize_pane_grid(&mut self, pane_id: PaneId, rows: u16, cols: u16) {
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::Resize {
                pane_id,
                cols,
                rows,
            });
            transport.invalidate_pushed_snapshot(pane_id);
        }
        self.dirty_panes.insert(pane_id);
    }

    fn pane_mode(&self, pane_id: PaneId) -> Option<u32> {
        self.pane_snapshots.get(&pane_id).map(|s| s.modes)
    }

    fn set_pane_theme(&mut self, pane_id: PaneId, theme: Theme, palette: oriterm_core::Palette) {
        let theme_str = theme_to_wire(theme).unwrap_or("dark").to_owned();
        let palette_rgb: Vec<[u8; 3]> = (0..270)
            .map(|i| {
                let rgb = palette.color(i);
                [rgb.r, rgb.g, rgb.b]
            })
            .collect();
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::SetTheme {
                pane_id,
                theme: theme_str,
                palette_rgb,
            });
            transport.invalidate_pushed_snapshot(pane_id);
        }
        self.dirty_panes.insert(pane_id);
    }

    fn set_cursor_shape(&mut self, pane_id: PaneId, shape: oriterm_core::CursorShape) {
        let wire = crate::WireCursorShape::from(shape) as u8;
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::SetCursorShape {
                pane_id,
                shape: wire,
            });
            transport.invalidate_pushed_snapshot(pane_id);
        }
        self.dirty_panes.insert(pane_id);
    }

    fn mark_all_dirty(&mut self, pane_id: PaneId) {
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::MarkAllDirty { pane_id });
            transport.invalidate_pushed_snapshot(pane_id);
        }
        self.dirty_panes.insert(pane_id);
    }

    fn set_image_config(&mut self, pane_id: PaneId, config: ImageConfig) {
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::SetImageConfig {
                pane_id,
                enabled: config.enabled,
                memory_limit: config.memory_limit as u64,
                max_single: config.max_single as u64,
                animation_enabled: config.animation_enabled,
            });
            transport.invalidate_pushed_snapshot(pane_id);
        }
        self.dirty_panes.insert(pane_id);
    }

    fn open_search(&mut self, pane_id: PaneId) {
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::OpenSearch { pane_id });
            transport.invalidate_pushed_snapshot(pane_id);
        }
        self.dirty_panes.insert(pane_id);
    }

    fn close_search(&mut self, pane_id: PaneId) {
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::CloseSearch { pane_id });
            transport.invalidate_pushed_snapshot(pane_id);
        }
        self.dirty_panes.insert(pane_id);
    }

    fn search_set_query(&mut self, pane_id: PaneId, query: String) {
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::SearchSetQuery { pane_id, query });
            transport.invalidate_pushed_snapshot(pane_id);
        }
        self.dirty_panes.insert(pane_id);
    }

    fn search_next_match(&mut self, pane_id: PaneId) {
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::SearchNextMatch { pane_id });
            transport.invalidate_pushed_snapshot(pane_id);
        }
        self.dirty_panes.insert(pane_id);
    }

    fn search_prev_match(&mut self, pane_id: PaneId) {
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::SearchPrevMatch { pane_id });
            transport.invalidate_pushed_snapshot(pane_id);
        }
        self.dirty_panes.insert(pane_id);
    }

    fn is_search_active(&self, pane_id: PaneId) -> bool {
        self.pane_snapshots
            .get(&pane_id)
            .is_some_and(|s| s.search_active)
    }

    fn extract_text(&mut self, pane_id: PaneId, selection: &Selection) -> Option<String> {
        let pdu = MuxPdu::ExtractText {
            pane_id,
            selection: WireSelection::from_selection(selection),
        };
        match self.rpc(pdu) {
            Ok(MuxPdu::ExtractTextResp { text }) => (!text.is_empty()).then_some(text),
            Ok(other) => {
                log::error!("extract_text: unexpected response: {other:?}");
                None
            }
            Err(e) => {
                log::error!("extract_text: RPC failed: {e}");
                None
            }
        }
    }

    fn extract_html(
        &mut self,
        pane_id: PaneId,
        selection: &Selection,
        font_family: &str,
        font_size: f32,
    ) -> Option<(String, String)> {
        let pdu = MuxPdu::ExtractHtml {
            pane_id,
            selection: WireSelection::from_selection(selection),
            font_family: font_family.to_string(),
            font_size_x100: (font_size * 100.0) as u16,
        };
        match self.rpc(pdu) {
            Ok(MuxPdu::ExtractHtmlResp { html, text }) => {
                (!text.is_empty()).then_some((html, text))
            }
            Ok(other) => {
                log::error!("extract_html: unexpected response: {other:?}");
                None
            }
            Err(e) => {
                log::error!("extract_html: RPC failed: {e}");
                None
            }
        }
    }

    fn scroll_display(&mut self, pane_id: PaneId, delta: isize) {
        let wire_delta = delta.clamp(i32::MIN as isize, i32::MAX as isize) as i32;
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::ScrollDisplay {
                pane_id,
                delta: wire_delta,
            });
            transport.invalidate_pushed_snapshot(pane_id);
        }
        self.dirty_panes.insert(pane_id);
    }

    fn scroll_to_bottom(&mut self, pane_id: PaneId) {
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::ScrollToBottom { pane_id });
            transport.invalidate_pushed_snapshot(pane_id);
        }
        self.dirty_panes.insert(pane_id);
    }

    fn scroll_to_previous_prompt(&mut self, pane_id: PaneId) -> bool {
        match self.rpc(MuxPdu::ScrollToPrompt {
            pane_id,
            direction: -1,
        }) {
            Ok(MuxPdu::ScrollToPromptAck { scrolled }) => {
                if scrolled {
                    self.dirty_panes.insert(pane_id);
                    if let Some(transport) = &self.transport {
                        transport.invalidate_pushed_snapshot(pane_id);
                    }
                }
                scrolled
            }
            Ok(other) => {
                log::error!("scroll_to_previous_prompt: unexpected response: {other:?}");
                false
            }
            Err(e) => {
                log::error!("scroll_to_previous_prompt: RPC failed: {e}");
                false
            }
        }
    }

    fn scroll_to_next_prompt(&mut self, pane_id: PaneId) -> bool {
        match self.rpc(MuxPdu::ScrollToPrompt {
            pane_id,
            direction: 1,
        }) {
            Ok(MuxPdu::ScrollToPromptAck { scrolled }) => {
                if scrolled {
                    self.dirty_panes.insert(pane_id);
                    if let Some(transport) = &self.transport {
                        transport.invalidate_pushed_snapshot(pane_id);
                    }
                }
                scrolled
            }
            Ok(other) => {
                log::error!("scroll_to_next_prompt: unexpected response: {other:?}");
                false
            }
            Err(e) => {
                log::error!("scroll_to_next_prompt: RPC failed: {e}");
                false
            }
        }
    }

    fn send_input(&mut self, pane_id: PaneId, data: &[u8]) {
        if let Some(transport) = &mut self.transport {
            transport.fire_and_forget(MuxPdu::Input {
                pane_id,
                data: data.to_vec(),
            });
        }
    }

    fn pane_ids(&self) -> Vec<PaneId> {
        self.pane_snapshots.keys().copied().collect()
    }

    fn event_tx(&self) -> Option<&mpsc::Sender<MuxEvent>> {
        // No local event channel in daemon mode.
        None
    }

    fn default_domain(&self) -> DomainId {
        DomainId::from_raw(0)
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
        // Try server-pushed snapshot first (checked at render time, not poll
        // time, so bare-dirty invalidations between poll and render are respected).
        let pushed = self
            .transport
            .as_ref()
            .and_then(|t| t.take_pushed_snapshot(pane_id));
        if let Some(snapshot) = pushed {
            self.pane_snapshots.insert(pane_id, snapshot);
            return self.pane_snapshots.get(&pane_id);
        }

        // Fallback: synchronous RPC (no pushed snapshot available).
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
