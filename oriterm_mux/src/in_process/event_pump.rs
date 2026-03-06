//! Event pump and notification drain for `InProcessMux`.
//!
//! Separated from the main CRUD operations to keep `mod.rs` under the
//! 500-line file size limit.

use std::collections::HashMap;

use crate::domain::Domain;
use crate::{DomainId, PaneId};

use super::InProcessMux;
use crate::mux_event::{MuxEvent, MuxNotification};
use crate::pane::Pane;

impl InProcessMux {
    // -- Event pump --

    /// Drain `MuxEvent`s from pane reader threads and emit `MuxNotification`s.
    ///
    /// Called from the App's event loop every iteration. The `panes` map is
    /// passed so the mux can update pane metadata (title, CWD) and write
    /// PTY responses without the App needing to know event internals.
    pub fn poll_events(&mut self, panes: &mut HashMap<PaneId, Pane>) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                MuxEvent::PaneOutput(id) => {
                    if let Some(pane) = panes.get(&id) {
                        pane.clear_wakeup();
                    }
                    self.notifications.push(MuxNotification::PaneOutput(id));
                }
                MuxEvent::PaneExited { pane_id, .. } => {
                    self.close_pane(pane_id);
                }
                MuxEvent::PaneTitleChanged { pane_id, title } => {
                    if let Some(pane) = panes.get_mut(&pane_id) {
                        pane.set_title(title);
                    }
                    self.notifications
                        .push(MuxNotification::PaneTitleChanged(pane_id));
                }
                MuxEvent::PaneIconChanged { pane_id, icon_name } => {
                    if let Some(pane) = panes.get_mut(&pane_id) {
                        pane.set_icon_name(icon_name);
                    }
                    self.notifications
                        .push(MuxNotification::PaneTitleChanged(pane_id));
                }
                MuxEvent::PaneCwdChanged { pane_id, cwd } => {
                    if let Some(pane) = panes.get_mut(&pane_id) {
                        pane.set_cwd(cwd);
                    }
                    self.notifications
                        .push(MuxNotification::PaneTitleChanged(pane_id));
                }
                MuxEvent::CommandComplete { pane_id, duration } => {
                    if let Some(pane) = panes.get_mut(&pane_id) {
                        pane.set_last_command_duration(duration);
                    }
                    self.notifications
                        .push(MuxNotification::CommandComplete { pane_id, duration });
                }
                MuxEvent::PaneBell(id) => {
                    self.notifications.push(MuxNotification::PaneBell(id));
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

    /// Drain accumulated notifications into the caller's buffer.
    ///
    /// Swaps the internal and caller buffers so both retain their heap
    /// allocations across frames (double-buffer pattern). The caller's
    /// buffer is cleared before receiving the new notifications.
    pub fn drain_notifications(&mut self, out: &mut Vec<MuxNotification>) {
        out.clear();
        std::mem::swap(&mut self.notifications, out);
    }

    /// Discard all pending notifications without draining to an external buffer.
    pub fn discard_notifications(&mut self) {
        self.notifications.clear();
    }

    // -- Accessors --

    /// Immutable access to the pane registry.
    pub fn pane_registry(&self) -> &crate::registry::PaneRegistry {
        &self.pane_registry
    }

    /// Clone of the event sender for spawning new panes.
    pub fn event_tx(&self) -> &std::sync::mpsc::Sender<MuxEvent> {
        &self.event_tx
    }

    /// Default domain ID for spawning.
    pub fn default_domain(&self) -> DomainId {
        self.local_domain.id()
    }
}
