//! Event pump and notification drain for `InProcessMux`.
//!
//! Separated from the main CRUD operations to keep `mod.rs` under the
//! 500-line file size limit.

use std::collections::HashMap;

use oriterm_mux::domain::Domain;
use oriterm_mux::{DomainId, PaneId, SessionRegistry, TabId, WindowId};

use super::{ClosePaneResult, InProcessMux};
use crate::mux_event::{MuxEvent, MuxNotification};
use crate::pane::Pane;

impl InProcessMux {
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

    /// Drain accumulated notifications into the caller's buffer.
    ///
    /// Swaps the internal and caller buffers so both retain their heap
    /// allocations across frames (double-buffer pattern). The caller's
    /// buffer is cleared before receiving the new notifications.
    pub(crate) fn drain_notifications(&mut self, out: &mut Vec<MuxNotification>) {
        out.clear();
        std::mem::swap(&mut self.notifications, out);
    }

    // -- Accessors --

    /// Active tab ID for a given window.
    pub(crate) fn active_tab_id(&self, window_id: WindowId) -> Option<TabId> {
        self.session.get_window(window_id)?.active_tab()
    }

    /// Change the focused pane within a tab.
    ///
    /// Returns `true` if the active pane was changed, `false` if the tab
    /// was not found.
    pub(crate) fn set_active_pane(&mut self, tab_id: TabId, pane_id: PaneId) -> bool {
        if let Some(tab) = self.session.get_tab_mut(tab_id) {
            tab.set_active_pane(pane_id);
            true
        } else {
            false
        }
    }

    /// Immutable access to the session registry.
    pub(crate) fn session(&self) -> &SessionRegistry {
        &self.session
    }

    // -- Tab switching and reordering --

    /// Switch the active tab in a window to a specific tab ID.
    ///
    /// Returns `true` if the switch was performed, `false` if the window
    /// or tab was not found.
    pub(crate) fn switch_active_tab(&mut self, window_id: WindowId, tab_id: TabId) -> bool {
        let Some(win) = self.session.get_window_mut(window_id) else {
            return false;
        };
        let Some(idx) = win.tabs().iter().position(|&t| t == tab_id) else {
            return false;
        };
        win.set_active_tab_idx(idx);
        true
    }

    /// Cycle to the next or previous tab in a window.
    ///
    /// `delta` is typically +1 (next) or -1 (previous); wraps around.
    /// Returns the newly active `TabId`, or `None` if the window was not
    /// found or has fewer than 2 tabs.
    pub(crate) fn cycle_active_tab(&mut self, window_id: WindowId, delta: isize) -> Option<TabId> {
        let win = self.session.get_window_mut(window_id)?;
        let count = win.tabs().len();
        if count <= 1 {
            return None;
        }
        let current = win.active_tab_idx();
        let next = (current as isize + delta).rem_euclid(count as isize) as usize;
        win.set_active_tab_idx(next);
        win.active_tab()
    }

    /// Reorder a tab within a window.
    ///
    /// Returns `true` if the move was performed.
    pub(crate) fn reorder_tab(&mut self, window_id: WindowId, from: usize, to: usize) -> bool {
        let Some(win) = self.session.get_window_mut(window_id) else {
            return false;
        };
        win.reorder_tab(from, to)
    }

    /// Immutable access to the pane registry.
    #[allow(dead_code, reason = "used when pane registry queries are wired to App")]
    pub(crate) fn pane_registry(&self) -> &oriterm_mux::registry::PaneRegistry {
        &self.pane_registry
    }

    /// Clone of the event sender for spawning new panes.
    #[allow(dead_code, reason = "used when dynamic pane spawning is wired to App")]
    pub(crate) fn event_tx(&self) -> &std::sync::mpsc::Sender<MuxEvent> {
        &self.event_tx
    }

    /// Default domain ID for spawning.
    #[allow(dead_code, reason = "used when multi-domain spawning is wired to App")]
    pub(crate) fn default_domain(&self) -> DomainId {
        self.local_domain.id()
    }
}
