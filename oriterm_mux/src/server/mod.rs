//! Mux daemon server.
//!
//! [`MuxServer`] owns an [`InProcessMux`] and runs a `mio`-based event loop
//! that accepts IPC connections from window processes, dispatches requests,
//! and pushes notifications to subscribed clients.
//!
//! The server is single-threaded: mio multiplexes the IPC listener, all
//! client streams, and a [`Waker`] that PTY reader threads use to signal
//! new [`MuxEvent`]s.

mod clients;
mod connection;
mod dispatch;
mod frame_io;
mod ipc;
mod notify;
mod pid_file;
mod push;
pub(crate) mod snapshot;

use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use mio::{Events, Interest, Poll, Token, Waker};

use crate::id::ClientId;
use crate::pane::Pane;
use crate::{IdAllocator, InProcessMux, MuxNotification, PaneId};

use self::notify::TargetClients;
use self::snapshot::SnapshotCache;

pub use connection::ClientConnection;
pub use ipc::{IpcListener, IpcStream, socket_path};
pub use pid_file::{PidFile, pid_file_path, read_pid};

/// mio token for the IPC listener.
const LISTENER: Token = Token(0);

/// mio token for the cross-thread waker.
const WAKER: Token = Token(1);

/// First token available for client connections.
const CLIENT_BASE: usize = 2;

/// Daemon server owning all PTY sessions and managing IPC clients.
///
/// Runs a single-threaded `mio`-based event loop: accepts connections from
/// window processes, dispatches mux operations, drains PTY events, and
/// pushes notifications to subscribed clients.
pub struct MuxServer {
    // Core state.
    /// In-process multiplexer owning all panes, tabs, windows.
    mux: InProcessMux,
    /// Platform-specific IPC listener.
    listener: IpcListener,
    /// Live pane instances, keyed by ID.
    panes: HashMap<PaneId, Pane>,

    // Connection tracking.
    /// Connected window processes keyed by client ID.
    connections: HashMap<ClientId, ClientConnection>,
    /// Pane → subscribed clients mapping.
    subscriptions: HashMap<PaneId, Vec<ClientId>>,
    /// mio token → client ID for O(1) event dispatch.
    token_to_client: HashMap<Token, ClientId>,
    /// Allocator for client IDs.
    client_alloc: IdAllocator<ClientId>,

    // Event loop infrastructure.
    /// mio poll instance.
    poll: Poll,
    /// Cross-thread waker for `MuxEvent` notifications.
    waker: Arc<Waker>,
    /// Closure that wakes the mio event loop from PTY reader threads.
    wakeup: Arc<dyn Fn() + Send + Sync>,
    /// Shutdown flag — set by signal handler or `--stop` command.
    shutdown: Arc<AtomicBool>,

    // Housekeeping.
    /// PID file handle (removed on drop).
    _pid_file: PidFile,
    /// Next mio token for client connections.
    next_token: usize,
    /// Server start time (for startup grace period).
    start_time: Instant,
    /// Set once at least one client has connected.
    had_client: bool,
    /// Reusable buffer for draining notifications.
    notification_buf: Vec<MuxNotification>,
    /// Reusable scratch buffer for collecting client IDs during dispatch.
    scratch_clients: Vec<ClientId>,
    /// Reusable scratch buffer for collecting pane IDs during dispatch.
    scratch_panes: Vec<PaneId>,
    /// Reusable scratch buffer for panes needing immediate snapshot push.
    scratch_immediate_push: Vec<PaneId>,

    // Server-push state.
    /// Per-pane timestamp of last snapshot push.
    last_snapshot_push: HashMap<PaneId, Instant>,
    /// Panes with deferred pushes (per-client tracking).
    pending_push: HashMap<PaneId, HashSet<ClientId>>,

    // Snapshot cache (allocation reuse for GetPaneSnapshot).
    /// Cached snapshots with shared render buffer — encapsulates
    /// `RenderableContent` so the server layer never touches it directly.
    snapshot_cache: SnapshotCache,
}

impl MuxServer {
    /// Create a new server, binding the IPC listener and writing the PID file.
    pub fn new() -> io::Result<Self> {
        Self::with_paths(&socket_path(), &pid_file_path())
    }

    /// Create with explicit paths (for testing).
    pub fn with_paths(
        socket_path: &std::path::Path,
        pid_path: &std::path::Path,
    ) -> io::Result<Self> {
        let pid_file = PidFile::create_at(pid_path)?;
        let poll = Poll::new()?;
        let waker = Arc::new(Waker::new(poll.registry(), WAKER)?);

        // Build the wakeup closure that PTY reader threads will call.
        let waker_ref = Arc::clone(&waker);
        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _ = waker_ref.wake();
        });

        let mut listener = IpcListener::bind_at(socket_path)?;
        poll.registry()
            .register(&mut listener, LISTENER, Interest::READABLE)?;

        Ok(Self {
            mux: InProcessMux::new(),
            listener,
            panes: HashMap::new(),
            connections: HashMap::new(),
            subscriptions: HashMap::new(),
            token_to_client: HashMap::new(),
            client_alloc: IdAllocator::new(),
            poll,
            waker,
            wakeup,
            shutdown: Arc::new(AtomicBool::new(false)),
            _pid_file: pid_file,
            next_token: CLIENT_BASE,
            start_time: Instant::now(),
            had_client: false,
            notification_buf: Vec::new(),
            scratch_clients: Vec::new(),
            scratch_panes: Vec::new(),
            scratch_immediate_push: Vec::new(),
            last_snapshot_push: HashMap::new(),
            pending_push: HashMap::new(),
            snapshot_cache: SnapshotCache::new(),
        })
    }

    /// Arc reference to the waker for cross-thread use.
    ///
    /// PTY reader threads call `waker.wake()` to notify the event loop
    /// that new [`MuxEvent`]s are available.
    pub fn waker(&self) -> Arc<Waker> {
        Arc::clone(&self.waker)
    }

    /// Arc reference to the shutdown flag.
    ///
    /// Signal handlers set this to `true` to trigger graceful shutdown.
    pub fn shutdown_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shutdown)
    }

    /// Immutable access to the inner mux.
    pub fn mux(&self) -> &InProcessMux {
        &self.mux
    }

    /// Number of currently connected clients.
    pub fn client_count(&self) -> usize {
        self.connections.len()
    }

    /// Run the server event loop until shutdown.
    pub fn run(&mut self) -> io::Result<()> {
        let mut events = Events::with_capacity(64);
        log::info!(
            "oriterm-mux daemon started (pid={}, socket={})",
            std::process::id(),
            self.listener.path().display(),
        );

        while !self.shutdown.load(Ordering::Acquire) {
            let timeout = if self.pending_push.is_empty() {
                Duration::from_millis(100)
            } else {
                push::SNAPSHOT_PUSH_INTERVAL // 16ms — retries fire promptly.
            };
            self.poll.poll(&mut events, Some(timeout))?;

            for event in &events {
                match event.token() {
                    LISTENER => self.accept_connections()?,
                    WAKER => { /* MuxEvent arrived — handled below */ }
                    token => self.handle_client_event(token),
                }
            }

            // Drain `MuxEvent`s from PTY reader threads.
            self.drain_mux_events();

            // Second pass: handle client requests that arrived during
            // drain_mux_events (snapshot building can take milliseconds).
            // Non-blocking poll with zero timeout — only picks up already-ready events.
            self.poll.poll(&mut events, Some(Duration::ZERO))?;
            for event in &events {
                match event.token() {
                    LISTENER => self.accept_connections()?,
                    WAKER => { /* Will be drained on next main iteration */ }
                    token => self.handle_client_event(token),
                }
            }

            // Check exit condition: all panes exited + no clients.
            if self.should_exit() {
                log::info!("all panes exited and no clients — shutting down");
                break;
            }
        }

        log::info!("oriterm-mux daemon shutting down");
        Ok(())
    }

    /// Drain `MuxEvent`s from PTY reader threads and push notifications.
    ///
    /// Three-phase processing:
    /// 1. Trailing-edge flush — retry deferred pushes from previous cycles.
    /// 2. Route new notifications — `PaneOutput` triggers snapshot push
    ///    (or deferral); other notifications use existing routing.
    /// 3. Update write interests for connections with pending data.
    fn drain_mux_events(&mut self) {
        self.mux.poll_events(&mut self.panes);
        self.mux.drain_notifications(&mut self.notification_buf);
        let now = Instant::now();

        // Phase 1: Trailing-edge flush — retry deferred pushes.
        {
            let mut push_ctx = push::PushContext {
                last_snapshot_push: &mut self.last_snapshot_push,
                subscriptions: &self.subscriptions,
                connections: &mut self.connections,
                panes: &self.panes,
                snapshot_cache: &mut self.snapshot_cache,
                pending_push: &mut self.pending_push,
                scratch: &mut self.scratch_clients,
            };
            push::trailing_edge_flush(&mut push_ctx, now);
        }

        // Phase 2: Route new notifications.
        for notif in &self.notification_buf {
            if let MuxNotification::PaneOutput(pane_id) = notif {
                let mut push_ctx = push::PushContext {
                    last_snapshot_push: &mut self.last_snapshot_push,
                    subscriptions: &self.subscriptions,
                    connections: &mut self.connections,
                    panes: &self.panes,
                    snapshot_cache: &mut self.snapshot_cache,
                    pending_push: &mut self.pending_push,
                    scratch: &mut self.scratch_clients,
                };
                push::push_or_defer_pane(&mut push_ctx, now, *pane_id);
            } else {
                let Some((target, pdu)) = notify::notification_to_pdu(notif, &self.panes) else {
                    continue;
                };
                match target {
                    TargetClients::PaneSubscribers(pane_id) => {
                        if let Some(subs) = self.subscriptions.get(&pane_id) {
                            self.scratch_clients.clear();
                            self.scratch_clients.extend_from_slice(subs);
                            for &cid in &self.scratch_clients {
                                if let Some(conn) = self.connections.get_mut(&cid) {
                                    let _ = conn.queue_frame(0, &pdu);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Post-pass: Clean up per-pane state for closed panes.
        // Collect IDs first to avoid borrowing `self` immutably (notification_buf)
        // and mutably (cleanup_pane_state) at the same time.
        let closed: Vec<PaneId> = self
            .notification_buf
            .iter()
            .filter_map(|n| match n {
                MuxNotification::PaneClosed(id) => Some(*id),
                _ => None,
            })
            .collect();
        for pane_id in closed {
            self.cleanup_pane_state(pane_id);
        }

        // Phase 3: Update write interests for connections with pending data.
        // Reuse scratch_clients (free after phases 1-2) to avoid per-cycle allocation.
        self.scratch_clients.clear();
        self.scratch_clients.extend(
            self.connections
                .values()
                .filter(|c| c.has_pending_writes())
                .map(ClientConnection::id),
        );
        for i in 0..self.scratch_clients.len() {
            self.update_write_interest(self.scratch_clients[i]);
        }
    }

    /// Remove all per-pane tracking state for a closed pane.
    ///
    /// Clears snapshot cache, push timestamps, pending pushes, subscription
    /// entries, and per-connection subscription sets. Centralizes cleanup
    /// that previously lived in three separate locations.
    pub(super) fn cleanup_pane_state(&mut self, pane_id: PaneId) {
        self.snapshot_cache.remove(pane_id);
        self.last_snapshot_push.remove(&pane_id);
        self.pending_push.remove(&pane_id);
        self.subscriptions.remove(&pane_id);
        for conn in self.connections.values_mut() {
            conn.unsubscribe(pane_id);
        }
    }

    /// Check if the server should auto-exit.
    ///
    /// Exits when all panes have exited AND no clients are connected,
    /// with a startup grace period so the server doesn't exit immediately
    /// before any client has connected.
    fn should_exit(&self) -> bool {
        // Grace period: don't exit within first 5 seconds of startup.
        let grace = Duration::from_secs(5);
        if self.start_time.elapsed() < grace {
            return false;
        }
        // Don't exit until at least one client has connected and left.
        if !self.had_client {
            return false;
        }
        self.connections.is_empty() && self.panes.is_empty()
    }
}

#[cfg(test)]
mod tests;
