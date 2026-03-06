//! Mux event pump — drains PTY events and handles mux notifications.
//!
//! Called once per event loop iteration in `about_to_wait`, before rendering.
//! Processes `MuxEvent`s from PTY reader threads via `MuxBackend::poll_events`,
//! then handles resulting `MuxNotification`s (dirty, close, clipboard, etc.).

use std::fmt::Write as _;
use std::time::Duration;

use oriterm_mux::PaneId;

use crate::config::NotifyOnCommandFinish;
use crate::platform::notify;
use oriterm_mux::mux_event::MuxNotification;

use super::App;

impl App {
    /// Pump mux events and process resulting notifications.
    ///
    /// Drains PTY reader thread messages via the mux, then handles each
    /// notification (dirty, close, clipboard, etc.).
    pub(super) fn pump_mux_events(&mut self) {
        let Some(mux) = &mut self.mux else { return };

        // Check daemon connectivity.
        if mux.is_daemon_mode() && !mux.is_connected() {
            log::warn!("daemon connection lost");
            self.handle_daemon_disconnect();
            return;
        }

        // 1. Process incoming MuxEvents from PTY reader threads.
        mux.poll_events();

        // 2. Drain notifications into our reusable buffer.
        mux.drain_notifications(&mut self.notification_buf);
        if self.notification_buf.is_empty() {
            return;
        }

        // 3. Handle each notification.
        self.with_drained_notifications(Self::handle_mux_notification);
    }

    /// Process a single mux notification.
    fn handle_mux_notification(&mut self, notification: MuxNotification) {
        match notification {
            MuxNotification::PaneOutput(id) => {
                // Invalidate client-side selection when terminal content changes.
                // New output can shift scrollback, making selection coordinates stale.
                self.clear_pane_selection(id);

                // Only invalidate URL hover when the dirty pane is focused.
                // Background shell output in other panes shouldn't kill the
                // URL highlight under the cursor.
                if self.active_pane_id() == Some(id) {
                    if let Some(ctx) = self.focused_ctx_mut() {
                        ctx.url_cache.invalidate();
                        ctx.hovered_url = None;
                    }
                }
                // Mark all windows dirty — the pane may be in any window.
                self.mark_all_windows_dirty();
            }
            MuxNotification::PaneClosed(id) => {
                self.handle_pane_closed(id);
            }
            MuxNotification::PaneTitleChanged(_) => {
                self.sync_tab_bar_from_mux();
                self.mark_all_windows_dirty();
            }
            MuxNotification::CommandComplete { pane_id, duration } => {
                self.handle_command_complete(pane_id, duration);
            }
            MuxNotification::PaneBell(id) => {
                if let Some(mux) = self.mux.as_mut() {
                    mux.set_bell(id);
                }
                if let Some(idx) = self.tab_index_for_pane(id) {
                    if let Some(ctx) = self.focused_ctx_mut() {
                        ctx.tab_bar.ring_bell(idx);
                    }
                }
                self.mark_all_windows_dirty();
            }
            MuxNotification::ClipboardStore {
                clipboard_type,
                text,
                ..
            } => {
                self.clipboard.store(clipboard_type, &text);
            }
            MuxNotification::ClipboardLoad {
                pane_id,
                clipboard_type,
                formatter,
            } => {
                let text = self.clipboard.load(clipboard_type);
                let response = formatter(&text);
                self.write_pane_input(pane_id, response.as_bytes());
            }
        }
    }

    /// Handle a command completing in a pane.
    ///
    /// Checks config threshold and focus state to decide whether to flash
    /// the tab bar (bell pulse) and/or log the completion.
    fn handle_command_complete(&mut self, pane_id: PaneId, duration: Duration) {
        let behavior = &self.config.behavior;
        let threshold = Duration::from_secs(behavior.notify_command_threshold_secs);
        if duration < threshold {
            return;
        }

        let mode = behavior.notify_on_command_finish;
        if mode == NotifyOnCommandFinish::Never {
            return;
        }

        let is_focused = self.active_pane_id() == Some(pane_id);
        if mode == NotifyOnCommandFinish::Unfocused && is_focused {
            return;
        }

        log::info!(
            "command completed in {pane_id} after {:.1}s",
            duration.as_secs_f64()
        );

        // Flash the tab bar (reuse bell pulse) if configured.
        if behavior.notify_command_bell {
            if let Some(idx) = self.tab_index_for_pane(pane_id) {
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.tab_bar.ring_bell(idx);
                    ctx.dirty = true;
                }
            }
        }

        // Build and dispatch OS notification.
        let title = self
            .mux
            .as_ref()
            .and_then(|m| m.pane_snapshot(pane_id))
            .map_or_else(
                || "Command finished".to_owned(),
                |s| {
                    if s.title.is_empty() {
                        "Command finished".to_owned()
                    } else {
                        s.title.clone()
                    }
                },
            );
        let body = format_duration_body(duration);
        notify::send(&title, &body);
    }
}

/// Format a human-readable duration string for notification body.
///
/// Examples: `"Completed in 12s"`, `"Completed in 2m 30s"`, `"Completed in 1h 5m"`.
fn format_duration_body(duration: Duration) -> String {
    let secs = duration.as_secs();
    let mut buf = String::from("Completed in ");
    if secs >= 3600 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let _ = write!(buf, "{h}h {m}m");
    } else if secs >= 60 {
        let m = secs / 60;
        let s = secs % 60;
        let _ = write!(buf, "{m}m {s}s");
    } else {
        let _ = write!(buf, "{secs}s");
    }
    buf
}

impl App {
    /// Handle a pane being closed (shell exit, PTY EOF, or explicit close).
    ///
    /// Cleans up client-side state, backend resources, and removes the pane
    /// from the local session (tree/floating). If the tab becomes empty,
    /// removes the tab; if the window becomes empty, closes the window.
    fn handle_pane_closed(&mut self, id: PaneId) {
        // Clean up client-side state.
        self.pane_selections.remove(&id);
        self.mark_cursors.remove(&id);

        // Clean up backend-side resources.
        if let Some(mux) = self.mux.as_mut() {
            mux.cleanup_closed_pane(id);
        }
        for ctx in self.windows.values_mut() {
            ctx.pane_cache.remove(id);
            ctx.dirty = true;
        }

        // Remove pane from local session (tab tree/floating).
        let tab_id = self.session.tab_for_pane(id);
        let Some(tab_id) = tab_id else { return };

        let tab_empty = if let Some(tab) = self.session.get_tab_mut(tab_id) {
            if tab.is_floating(id) {
                let new_layer = tab.floating().remove(id);
                tab.set_floating(new_layer);
            } else if let Some(new_tree) = tab.tree().remove(id) {
                tab.replace_layout(new_tree);
            } else {
                // Pane already removed from tree/floating.
            }
            if tab.active_pane() == id {
                tab.set_active_pane(tab.tree().first_pane());
            }
            tab.all_panes().is_empty()
        } else {
            false
        };

        if tab_empty {
            let win_id = self.session.window_for_tab(tab_id);
            self.session.remove_tab(tab_id);
            if let Some(wid) = win_id {
                if let Some(win) = self.session.get_window_mut(wid) {
                    win.remove_tab(tab_id);
                }
                let window_empty = self
                    .session
                    .get_window(wid)
                    .is_some_and(|w| w.tabs().is_empty());
                if window_empty {
                    self.close_empty_session_window(wid);
                    return;
                }
            }
        }

        self.sync_tab_bar_from_mux();
        self.resize_all_panes();
    }

    /// Handle daemon disconnect by closing the window.
    ///
    /// When the daemon connection is lost the terminal state is gone —
    /// the daemon owned all panes. Closing is the honest response; it
    /// matches how `tmux` clients exit when the server dies.
    fn handle_daemon_disconnect(&self) {
        log::warn!("daemon connection lost, closing window");
        self.exit_app();
    }
}

#[cfg(test)]
mod tests;
