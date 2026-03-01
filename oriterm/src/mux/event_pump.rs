//! Event pump and notification drain for `InProcessMux`.
//!
//! Separated from the main CRUD operations to keep `mod.rs` under the
//! 500-line file size limit.

use std::collections::HashMap;

use oriterm_mux::domain::Domain;
use oriterm_mux::{DomainId, PaneId, SessionRegistry, TabId, WindowId};

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
                    self.close_pane(pane_id);
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

    // -- Cross-window operations --

    /// Move a tab from its current window to a different window.
    ///
    /// The tab's panes, split tree, and floating layer are preserved — only
    /// window ownership changes. The tab becomes the active tab in the
    /// destination window.
    ///
    /// If the source window becomes empty after the move, it is removed and
    /// a `WindowClosed` (or `LastWindowClosed`) notification is emitted.
    ///
    /// Returns `true` if the move was performed.
    pub(crate) fn move_tab_to_window(&mut self, tab_id: TabId, dest_window_id: WindowId) -> bool {
        // Find source window.
        let Some(source_window_id) = self.session.window_for_tab(tab_id) else {
            return false;
        };

        // No-op if source == dest.
        if source_window_id == dest_window_id {
            return false;
        }

        // Verify destination window exists.
        if self.session.get_window(dest_window_id).is_none() {
            return false;
        }

        // Remove from source window.
        let Some(source) = self.session.get_window_mut(source_window_id) else {
            return false;
        };
        source.remove_tab(tab_id);
        let source_empty = source.tabs().is_empty();

        // Add to destination window and make it active.
        let Some(dest) = self.session.get_window_mut(dest_window_id) else {
            return false;
        };
        dest.add_tab(tab_id);
        let dest_idx = dest.tabs().len() - 1;
        dest.set_active_tab_idx(dest_idx);

        // Emit notifications for both windows.
        self.notifications
            .push(MuxNotification::WindowTabsChanged(dest_window_id));

        // Handle empty source window.
        if source_empty {
            self.session.remove_window(source_window_id);
            if self.session.window_count() == 0 {
                self.notifications.push(MuxNotification::LastWindowClosed);
            } else {
                self.notifications
                    .push(MuxNotification::WindowClosed(source_window_id));
            }
        } else {
            self.notifications
                .push(MuxNotification::WindowTabsChanged(source_window_id));
        }

        // Trigger layout recomputation for the moved tab in its new window.
        self.notifications
            .push(MuxNotification::TabLayoutChanged(tab_id));

        true
    }

    /// Move a tab from its current window to a specific index in the
    /// destination window.
    ///
    /// Like [`move_tab_to_window`] but inserts at `dest_index` instead of
    /// appending. The tab becomes the active tab in the destination window.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn move_tab_to_window_at(
        &mut self,
        tab_id: TabId,
        dest_window_id: WindowId,
        dest_index: usize,
    ) -> bool {
        let Some(source_window_id) = self.session.window_for_tab(tab_id) else {
            return false;
        };
        if source_window_id == dest_window_id {
            return false;
        }
        if self.session.get_window(dest_window_id).is_none() {
            return false;
        }

        let Some(source) = self.session.get_window_mut(source_window_id) else {
            return false;
        };
        source.remove_tab(tab_id);
        let source_empty = source.tabs().is_empty();

        let Some(dest) = self.session.get_window_mut(dest_window_id) else {
            return false;
        };
        dest.insert_tab_at(dest_index, tab_id);
        let actual_idx = dest.tabs().iter().position(|&t| t == tab_id).unwrap_or(0);
        dest.set_active_tab_idx(actual_idx);

        self.notifications
            .push(MuxNotification::WindowTabsChanged(dest_window_id));

        if source_empty {
            self.session.remove_window(source_window_id);
            if self.session.window_count() == 0 {
                self.notifications.push(MuxNotification::LastWindowClosed);
            } else {
                self.notifications
                    .push(MuxNotification::WindowClosed(source_window_id));
            }
        } else {
            self.notifications
                .push(MuxNotification::WindowTabsChanged(source_window_id));
        }

        self.notifications
            .push(MuxNotification::TabLayoutChanged(tab_id));

        true
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
