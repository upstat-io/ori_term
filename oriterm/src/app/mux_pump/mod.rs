//! Mux event pump — drains PTY events and handles mux notifications.
//!
//! Called once per event loop iteration in `about_to_wait`, before rendering.
//! Processes `MuxEvent`s from PTY reader threads via `InProcessMux::poll_events`,
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

        // 1. Process incoming MuxEvents from PTY reader threads.
        mux.poll_events(&mut self.panes);

        // 2. Drain notifications into our reusable buffer.
        mux.drain_notifications(&mut self.notification_buf);
        if self.notification_buf.is_empty() {
            return;
        }

        // 3. Handle each notification.
        //    Take the buffer to avoid borrow conflicts with `self`, then
        //    restore it after iteration to preserve Vec capacity across frames.
        let mut notifications = std::mem::take(&mut self.notification_buf);
        #[allow(
            clippy::iter_with_drain,
            reason = "drain preserves Vec capacity; into_iter drops it"
        )]
        for notification in notifications.drain(..) {
            self.handle_mux_notification(notification);
        }
        self.notification_buf = notifications;
    }

    /// Process a single mux notification.
    fn handle_mux_notification(&mut self, notification: MuxNotification) {
        match notification {
            MuxNotification::PaneDirty(id) => {
                if let Some(pane) = self.panes.get_mut(&id) {
                    pane.check_selection_invalidation();
                }
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
                // Remove the pane from the map. Drop (PTY kill + reader
                // thread join + child reap) runs on a background thread
                // to avoid blocking the event loop.
                if let Some(pane) = self.panes.remove(&id) {
                    std::thread::spawn(move || drop(pane));
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
            MuxNotification::PaneTitleChanged(_) | MuxNotification::WindowTabsChanged(_) => {
                self.sync_tab_bar_from_mux();
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            MuxNotification::CommandComplete { pane_id, duration } => {
                self.handle_command_complete(pane_id, duration);
            }
            MuxNotification::Alert(id) => {
                if let Some(pane) = self.panes.get_mut(&id) {
                    pane.set_bell();
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
                if let Some(pane) = self.panes.get(&pane_id) {
                    pane.write_input(response.as_bytes());
                }
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
            .panes
            .get(&pane_id)
            .map_or("Command finished", |p| {
                let t = p.effective_title();
                if t.is_empty() { "Command finished" } else { t }
            })
            .to_owned();
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
