//! Mux backend abstraction.
//!
//! [`MuxBackend`] defines the interface between the GUI app and the
//! multiplexer state. Two implementations exist:
//!
//! - [`EmbeddedMux`] — in-process mux for single-process mode. Wraps
//!   [`InProcessMux`](crate::InProcessMux) and owns `Pane` structs directly.
//! - [`MuxClient`] — IPC client for daemon mode. Sends requests to a
//!   [`MuxServer`](crate::server::MuxServer) over a Unix socket / named pipe.

pub mod client;
pub mod embedded;

use std::collections::HashSet;
use std::io;
use std::sync::mpsc;

use oriterm_core::Theme;

use crate::domain::SpawnConfig;
use crate::in_process::ClosePaneResult;
use crate::layout::{Rect, SplitDirection};
use crate::mux_event::{MuxEvent, MuxNotification};
use crate::pane::Pane;
use crate::registry::{PaneEntry, SessionRegistry};
use crate::{DomainId, PaneId, TabId, WindowId};

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

    // -- Session queries --

    /// Immutable access to the session registry.
    fn session(&self) -> &SessionRegistry;

    /// Active tab ID for a given window.
    fn active_tab_id(&self, window_id: WindowId) -> Option<TabId>;

    /// Look up a pane's metadata entry.
    fn get_pane_entry(&self, pane_id: PaneId) -> Option<PaneEntry>;

    /// True when this pane is the only pane in the entire session.
    fn is_last_pane(&self, pane_id: PaneId) -> bool;

    // -- Window operations --

    /// Create a new empty mux window.
    fn create_window(&mut self) -> io::Result<WindowId>;

    /// Close a window and all its tabs/panes.
    ///
    /// Returns the list of `PaneId`s whose panes were removed.
    fn close_window(&mut self, window_id: WindowId) -> Vec<PaneId>;

    // -- Tab operations --

    /// Create a new tab with a single pane in the given window.
    ///
    /// Returns `(TabId, PaneId)` — the pane is stored internally.
    fn create_tab(
        &mut self,
        window_id: WindowId,
        config: &SpawnConfig,
        theme: Theme,
    ) -> io::Result<(TabId, PaneId)>;

    /// Close a tab and all its panes.
    ///
    /// Returns the list of `PaneId`s whose panes were removed.
    fn close_tab(&mut self, tab_id: TabId) -> Vec<PaneId>;

    /// Switch the active tab in a window.
    fn switch_active_tab(&mut self, window_id: WindowId, tab_id: TabId) -> bool;

    /// Cycle to the next or previous tab.
    fn cycle_active_tab(&mut self, window_id: WindowId, delta: isize) -> Option<TabId>;

    /// Reorder a tab within a window.
    fn reorder_tab(&mut self, window_id: WindowId, from: usize, to: usize) -> bool;

    /// Move a tab to a different window (appended).
    fn move_tab_to_window(&mut self, tab_id: TabId, dest: WindowId) -> bool;

    /// Move a tab to a specific index in the destination window.
    fn move_tab_to_window_at(&mut self, tab_id: TabId, dest: WindowId, index: usize) -> bool;

    // -- Pane operations --

    /// Split an existing pane, creating a new sibling.
    ///
    /// Returns `PaneId` of the newly created pane.
    #[expect(
        clippy::too_many_arguments,
        reason = "split requires source pane + direction on top of spawn params"
    )]
    fn split_pane(
        &mut self,
        tab_id: TabId,
        source: PaneId,
        dir: SplitDirection,
        config: &SpawnConfig,
        theme: Theme,
    ) -> io::Result<PaneId>;

    /// Close a single pane.
    fn close_pane(&mut self, pane_id: PaneId) -> ClosePaneResult;

    /// Change the focused pane within a tab.
    fn set_active_pane(&mut self, tab_id: TabId, pane_id: PaneId) -> bool;

    // -- Layout operations --

    /// Toggle zoom on the active pane in a tab.
    fn toggle_zoom(&mut self, tab_id: TabId);

    /// Clear zoom without emitting a notification.
    fn unzoom_silent(&mut self, tab_id: TabId);

    /// Reset all split ratios to 0.5.
    fn equalize_panes(&mut self, tab_id: TabId);

    /// Set the ratio of a specific divider.
    fn set_divider_ratio(&mut self, tab_id: TabId, before: PaneId, after: PaneId, ratio: f32);

    /// Resize a pane by adjusting the nearest qualifying split border.
    #[expect(
        clippy::too_many_arguments,
        reason = "resize requires tab + pane + axis + side + delta"
    )]
    fn resize_pane(
        &mut self,
        tab_id: TabId,
        pane_id: PaneId,
        axis: SplitDirection,
        first: bool,
        delta: f32,
    );

    /// Undo the last split tree mutation.
    fn undo_split(&mut self, tab_id: TabId, live: &HashSet<PaneId>) -> bool;

    /// Redo the last undone split tree mutation.
    fn redo_split(&mut self, tab_id: TabId, live: &HashSet<PaneId>) -> bool;

    // -- Floating pane operations --

    /// Spawn a new floating pane.
    fn spawn_floating_pane(
        &mut self,
        tab_id: TabId,
        config: &SpawnConfig,
        theme: Theme,
        available: &Rect,
    ) -> io::Result<PaneId>;

    /// Move a tiled pane into the floating layer.
    fn move_pane_to_floating(&mut self, tab_id: TabId, pane_id: PaneId, available: &Rect) -> bool;

    /// Move a floating pane back into the tiled split tree.
    fn move_pane_to_tiled(&mut self, tab_id: TabId, pane_id: PaneId) -> bool;

    /// Move a floating pane to a new position.
    fn move_floating_pane(&mut self, tab_id: TabId, pane_id: PaneId, x: f32, y: f32);

    /// Resize a floating pane.
    fn resize_floating_pane(&mut self, tab_id: TabId, pane_id: PaneId, w: f32, h: f32);

    /// Set a floating pane's rect (position + size) in one call.
    fn set_floating_pane_rect(&mut self, tab_id: TabId, pane_id: PaneId, rect: Rect);

    /// Bring a floating pane to the front.
    fn raise_floating_pane(&mut self, tab_id: TabId, pane_id: PaneId);

    // -- Pane data access --

    /// Immutable reference to a pane.
    ///
    /// Returns `Some` in embedded mode, `None` in client mode (daemon owns panes).
    fn pane(&self, pane_id: PaneId) -> Option<&Pane>;

    /// Mutable reference to a pane.
    ///
    /// Returns `Some` in embedded mode, `None` in client mode.
    fn pane_mut(&mut self, pane_id: PaneId) -> Option<&mut Pane>;

    /// Remove a pane from the backend's storage.
    ///
    /// Returns the removed pane in embedded mode, `None` in client mode.
    fn remove_pane(&mut self, pane_id: PaneId) -> Option<Pane>;

    /// All pane IDs currently stored in the backend.
    fn pane_ids(&self) -> Vec<PaneId>;

    // -- Event channel --

    /// Event sender for spawning new panes (embedded: mpsc; client: None).
    fn event_tx(&self) -> Option<&mpsc::Sender<MuxEvent>>;

    /// Default domain ID for spawning.
    fn default_domain(&self) -> DomainId;

    /// Tell the daemon which mux window this client renders.
    ///
    /// In embedded mode this is a no-op (the process owns its own state).
    /// In daemon mode this sends a `ClaimWindow` RPC so the server can
    /// route `WindowTabsChanged` notifications to this client.
    fn claim_window(&mut self, _window_id: WindowId) -> io::Result<()> {
        Ok(())
    }

    /// Re-fetch the tab list for `window_id` from the daemon.
    ///
    /// Called in daemon mode when a `WindowTabsChanged` notification
    /// arrives — another client may have moved a tab to this window.
    /// In embedded mode this is a no-op (local state is authoritative).
    fn refresh_window_tabs(&mut self, _window_id: WindowId) {}

    /// Whether this backend is running in daemon (IPC client) mode.
    ///
    /// Embedded mode returns `false`. Client mode returns `true`.
    /// The App uses this to choose between in-process window creation
    /// and cross-process tab migration.
    fn is_daemon_mode(&self) -> bool;
}
