//! Mux backend abstraction.
//!
//! [`MuxBackend`] defines the interface between the client app and the
//! multiplexer state. Two implementations exist:
//!
//! - [`EmbeddedMux`] — in-process mux for single-process mode. Wraps
//!   [`InProcessMux`](crate::InProcessMux) and owns `Pane` structs directly.
//! - [`MuxClient`] — IPC client for daemon mode. Sends requests to a
//!   [`MuxServer`](crate::server::MuxServer) over a Unix socket / named pipe.

pub mod client;
pub mod embedded;

use std::io;
use std::sync::mpsc;

use oriterm_core::Theme;
use oriterm_core::selection::Selection;

use crate::PaneSnapshot;
use crate::domain::SpawnConfig;
use crate::in_process::ClosePaneResult;
use crate::mux_event::{MuxEvent, MuxNotification};
use crate::registry::PaneEntry;
use crate::{DomainId, PaneId};

pub use self::client::MuxClient;
pub use self::embedded::EmbeddedMux;

/// Abstraction over in-process and daemon-mode multiplexer access.
///
/// The App calls trait methods identically regardless of whether
/// terminal state lives in-process ([`EmbeddedMux`]) or in a remote
/// daemon ([`MuxClient`]). All methods are synchronous.
pub trait MuxBackend {
    // -- Event pump --

    /// Drain `MuxEvent`s from PTY reader threads and emit notifications.
    ///
    /// In embedded mode, this processes the mpsc channel. In client mode,
    /// this is a no-op (the reader thread pushes directly).
    fn poll_events(&mut self);

    /// Drain accumulated notifications into the caller's buffer.
    fn drain_notifications(&mut self, out: &mut Vec<MuxNotification>);

    /// Discard all pending notifications.
    fn discard_notifications(&mut self);

    /// Look up a pane's metadata entry.
    fn get_pane_entry(&self, pane_id: PaneId) -> Option<PaneEntry>;

    // -- Pane operations --

    /// Spawn a pane with a new PTY process.
    ///
    /// The client owns tab/window grouping — the mux creates the pane
    /// and manages its PTY lifecycle. Returns `PaneId` for the new pane.
    fn spawn_pane(&mut self, config: &SpawnConfig, theme: Theme) -> io::Result<PaneId>;

    /// Close a single pane.
    fn close_pane(&mut self, pane_id: PaneId) -> ClosePaneResult;

    // -- Grid operations --

    /// Resize a pane's terminal grid and PTY.
    ///
    /// In embedded mode, calls `Pane::resize_grid` + `Pane::resize_pty`.
    /// In daemon mode, sends a fire-and-forget `Resize` PDU.
    fn resize_pane_grid(&mut self, pane_id: PaneId, rows: u16, cols: u16);

    // -- Mode query --

    /// Terminal mode bits for a pane (raw `u32`).
    ///
    /// In embedded mode, reads the lock-free atomic cache.
    /// In daemon mode, reads from the cached snapshot.
    fn pane_mode(&self, pane_id: PaneId) -> Option<u32>;

    // -- Theme + palette + cursor operations --

    /// Apply a theme and palette to a pane's terminal.
    fn set_pane_theme(&mut self, pane_id: PaneId, theme: Theme, palette: oriterm_core::Palette);

    /// Change the cursor shape for a pane.
    fn set_cursor_shape(&mut self, pane_id: PaneId, shape: oriterm_core::CursorShape);

    /// Mark all lines in a pane as dirty (forces full re-render).
    fn mark_all_dirty(&mut self, pane_id: PaneId);

    // -- Scroll operations --

    /// Scroll the viewport by `delta` lines (positive = toward history).
    fn scroll_display(&mut self, pane_id: PaneId, delta: isize);

    /// Scroll to the live terminal position (bottom).
    fn scroll_to_bottom(&mut self, pane_id: PaneId);

    /// Scroll to the nearest prompt above the current viewport.
    fn scroll_to_previous_prompt(&mut self, pane_id: PaneId) -> bool;

    /// Scroll to the nearest prompt below the current viewport.
    fn scroll_to_next_prompt(&mut self, pane_id: PaneId) -> bool;

    // -- Search operations --

    /// Open search for a pane (initializes empty search state).
    fn open_search(&mut self, pane_id: PaneId);

    /// Close search and clear search state.
    fn close_search(&mut self, pane_id: PaneId);

    /// Update the search query. Recomputes matches against the full grid.
    fn search_set_query(&mut self, pane_id: PaneId, query: String);

    /// Navigate to the next search match.
    fn search_next_match(&mut self, pane_id: PaneId);

    /// Navigate to the previous search match.
    fn search_prev_match(&mut self, pane_id: PaneId);

    /// Whether search is currently active for a pane.
    fn is_search_active(&self, pane_id: PaneId) -> bool;

    // -- Clipboard text extraction --

    /// Extract plain text from a selection range.
    ///
    /// Returns `None` if the pane doesn't exist or the selection is empty.
    fn extract_text(&mut self, pane_id: PaneId, selection: &Selection) -> Option<String>;

    /// Extract HTML (with inline styles) and plain text from a selection.
    ///
    /// `font_family` and `font_size` are used for the HTML wrapper.
    /// Returns `None` if the pane doesn't exist or the selection is empty.
    fn extract_html(
        &mut self,
        pane_id: PaneId,
        selection: &Selection,
        font_family: &str,
        font_size: f32,
    ) -> Option<(String, String)>;

    // -- Input --

    /// Send raw bytes to a pane's PTY.
    ///
    /// In embedded mode, delegates to [`Pane::write_input`].
    /// In daemon mode, sends a fire-and-forget `Input` PDU to the daemon.
    fn send_input(&mut self, pane_id: PaneId, data: &[u8]);

    // -- Pane metadata --

    /// Current working directory of a pane (from OSC 7).
    ///
    /// Reads from the cached snapshot's `cwd` field.
    fn pane_cwd(&self, pane_id: PaneId) -> Option<String> {
        self.pane_snapshot(pane_id).and_then(|s| s.cwd.clone())
    }

    /// Mark the bell as active for a pane.
    ///
    /// In embedded mode, sets the pane's bell flag. In client mode this is
    /// a no-op — the bell state is driven by `MuxNotification::PaneBell`.
    fn set_bell(&mut self, _pane_id: PaneId) {}

    /// Clear the bell flag for a pane.
    ///
    /// In embedded mode, clears the pane's bell flag. In client mode this is
    /// a no-op — the client manages bell state locally.
    fn clear_bell(&mut self, _pane_id: PaneId) {}

    /// Clean up a closed pane's resources.
    ///
    /// In embedded mode, removes the pane from storage and drops it on a
    /// background thread (PTY kill, reader join, child reap). In client
    /// mode this is a no-op — the daemon owns pane resources.
    fn cleanup_closed_pane(&mut self, _pane_id: PaneId) {}

    /// Build a `Selection` covering the nearest command output zone.
    ///
    /// Uses shell integration markers to find the output region around
    /// the viewport center. Returns `None` if no zone is found or shell
    /// integration is not active.
    fn select_command_output(&self, _pane_id: PaneId) -> Option<Selection> {
        None
    }

    /// Build a `Selection` covering the nearest command input zone.
    ///
    /// Uses shell integration markers to find the input region around
    /// the viewport center. Returns `None` if no zone is found or shell
    /// integration is not active.
    fn select_command_input(&self, _pane_id: PaneId) -> Option<Selection> {
        None
    }

    /// All pane IDs currently stored in the backend.
    fn pane_ids(&self) -> Vec<PaneId>;

    // -- Event channel --

    /// Event sender for spawning new panes (embedded: mpsc; client: None).
    fn event_tx(&self) -> Option<&mpsc::Sender<MuxEvent>>;

    /// Default domain ID for spawning.
    fn default_domain(&self) -> DomainId;

    /// Whether the daemon connection is alive.
    ///
    /// Always `true` for embedded mode (no remote connection).
    /// In daemon mode, reflects the transport's liveness state.
    fn is_connected(&self) -> bool {
        true
    }

    /// Whether this backend is running in daemon (IPC client) mode.
    ///
    /// Embedded mode returns `false`. Client mode returns `true`.
    fn is_daemon_mode(&self) -> bool;

    // -- Snapshot access --

    /// Swap the cached [`RenderableContent`] for a pane into `target`.
    ///
    /// In embedded mode, [`refresh_pane_snapshot`](Self::refresh_pane_snapshot)
    /// captures the `RenderableContent` extracted from the terminal. This
    /// method swaps it directly into the caller's `FrameInput.content`,
    /// bypassing the `RenderableContent → WireCell → RenderableContent`
    /// round-trip that the snapshot path requires.
    ///
    /// Returns `true` if the swap succeeded (embedded mode). Returns `false`
    /// in daemon mode (caller must use `pane_snapshot()` + conversion).
    fn swap_renderable_content(
        &mut self,
        _pane_id: PaneId,
        _target: &mut oriterm_core::RenderableContent,
    ) -> bool {
        false
    }

    /// Cached snapshot for a pane.
    ///
    /// Returns the most recently cached snapshot, or `None` if no snapshot
    /// has been built/fetched yet.
    fn pane_snapshot(&self, pane_id: PaneId) -> Option<&PaneSnapshot>;

    /// Whether the cached snapshot for `pane_id` is stale.
    fn is_pane_snapshot_dirty(&self, pane_id: PaneId) -> bool;

    /// Build (embedded) or fetch (daemon) a fresh snapshot and cache it.
    fn refresh_pane_snapshot(&mut self, pane_id: PaneId) -> Option<&PaneSnapshot>;

    /// Clear the dirty flag for a pane's cached snapshot.
    fn clear_pane_snapshot_dirty(&mut self, pane_id: PaneId);
}
