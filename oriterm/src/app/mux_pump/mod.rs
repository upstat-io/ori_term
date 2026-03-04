//! Mux event pump — drains PTY events and handles mux notifications.
//!
//! Called once per event loop iteration in `about_to_wait`, before rendering.
//! Processes `MuxEvent`s from PTY reader threads via `MuxBackend::poll_events`,
//! then handles resulting `MuxNotification`s (dirty, close, clipboard, etc.).

use std::fmt::Write as _;
use std::time::Duration;

use oriterm_mux::{PaneId, WindowId as MuxWindowId};

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
        let poll_start = std::time::Instant::now();
        mux.poll_events();
        let poll_elapsed = poll_start.elapsed();
        if poll_elapsed.as_millis() > 2 {
            log::warn!("[DIAG] mux.poll_events() took {:?}", poll_elapsed);
        }

        // 2. Drain notifications into our reusable buffer.
        mux.drain_notifications(&mut self.notification_buf);
        if self.notification_buf.is_empty() {
            return;
        }

        log::info!(
            "[DIAG] pump: {} notifications drained",
            self.notification_buf.len()
        );

        // 3. Handle each notification.
        self.with_drained_notifications(Self::handle_mux_notification);
    }

    /// Process a single mux notification.
    fn handle_mux_notification(&mut self, notification: MuxNotification) {
        match notification {
            MuxNotification::PaneDirty(id) => {
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
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            MuxNotification::PaneClosed(id) => {
                // Clean up client-side state for this pane.
                self.pane_selections.remove(&id);
                self.mark_cursors.remove(&id);

                // Clean up backend-side resources (PTY kill + reader thread
                // join + child reap on a background thread in embedded mode).
                if let Some(mux) = self.mux.as_mut() {
                    mux.cleanup_closed_pane(id);
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.pane_cache.remove(id);
                    ctx.dirty = true;
                }
            }
            MuxNotification::TabLayoutChanged(_) => {
                // Layout changed (split/close) — pane positions shifted.
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.pane_cache.invalidate_all();
                    ctx.cached_dividers = None;
                }
                self.resize_all_panes();
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            MuxNotification::FloatingPaneChanged(_) => {
                // Floating pane moved/resized — positions shifted but
                // PTY dimensions unchanged. Skip resize_all_panes.
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.pane_cache.invalidate_all();
                    ctx.dirty = true;
                }
            }
            MuxNotification::PaneTitleChanged(_) => {
                self.sync_tab_bar_from_mux();
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            MuxNotification::WindowTabsChanged(window_id) => {
                // In daemon mode, another client may have moved a tab to/from
                // this window. Re-fetch the authoritative tab list before
                // rebuilding the tab bar.
                if let Some(mux) = &mut self.mux {
                    if mux.is_daemon_mode() {
                        mux.refresh_window_tabs(window_id);
                    }
                }
                self.sync_tab_bar_from_mux();
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            MuxNotification::CommandComplete { pane_id, duration } => {
                self.handle_command_complete(pane_id, duration);
            }
            MuxNotification::Alert(id) => {
                if let Some(mux) = self.mux.as_mut() {
                    mux.set_bell(id);
                }
                if let Some(idx) = self.tab_index_for_pane(id) {
                    if let Some(ctx) = self.focused_ctx_mut() {
                        ctx.tab_bar.ring_bell(idx);
                    }
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            MuxNotification::WindowClosed(mux_wid) => {
                self.handle_mux_window_closed(mux_wid);
            }
            MuxNotification::LastWindowClosed => {
                log::info!("last mux window closed, exiting");
                self.exit_app();
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
    /// Handle daemon disconnect: fall back to embedded mode.
    ///
    /// When the daemon connection is lost, swap the backend to
    /// `EmbeddedMux` so the window stays alive in single-process mode.
    /// Existing pane state is lost (daemon owned it), but the window
    /// remains usable for new tabs.
    fn handle_daemon_disconnect(&mut self) {
        log::warn!("falling back to embedded mode after daemon disconnect");

        // Drop the dead client backend and replace with embedded.
        let mux = oriterm_mux::EmbeddedMux::new(self.mux_wakeup.clone());
        self.mux = Some(Box::new(mux));
    }

    /// Handle a mux window being closed by removing its `WindowContext`.
    ///
    /// Scans `self.windows` for the winit ID matching the closed mux window,
    /// removes the context, and updates focus if needed.
    fn handle_mux_window_closed(&mut self, mux_wid: MuxWindowId) {
        let winit_id = self
            .windows
            .iter()
            .find(|(_, ctx)| ctx.window.mux_window_id() == mux_wid)
            .map(|(&id, _)| id);
        let Some(wid) = winit_id else { return };

        self.windows.remove(&wid);

        // Update focus if the closed window was focused.
        if self.focused_window_id == Some(wid) {
            self.focused_window_id = self.windows.keys().next().copied();
            self.active_window = self
                .focused_window_id
                .and_then(|id| self.windows.get(&id).map(|ctx| ctx.window.mux_window_id()));
        }

        log::info!(
            "mux window closed: {wid:?}, {} remaining",
            self.windows.len()
        );
    }
}

#[cfg(test)]
mod tests;
