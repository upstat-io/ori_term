//! Mux daemon server.
//!
//! [`MuxServer`] owns an [`InProcessMux`] and runs a `mio`-based event loop
//! that accepts IPC connections from window processes, dispatches requests,
//! and pushes notifications to subscribed clients.
//!
//! The server is single-threaded: mio multiplexes the IPC listener, all
//! client streams, and a [`Waker`] that PTY reader threads use to signal
//! new [`MuxEvent`]s.

mod connection;
mod dispatch;
mod frame_io;
mod ipc;
mod notify;
mod pid_file;
mod snapshot;

use std::collections::HashMap;
use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use mio::{Events, Interest, Poll, Token, Waker};

use crate::id::ClientId;
use crate::pane::Pane;
use crate::{IdAllocator, InProcessMux, MuxNotification, PaneId};

use self::frame_io::{ReadStatus, send_frame};
use self::notify::TargetClients;

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
            self.poll
                .poll(&mut events, Some(Duration::from_millis(100)))?;

            for event in &events {
                match event.token() {
                    LISTENER => self.accept_connections()?,
                    WAKER => { /* MuxEvent arrived — handled below */ }
                    token => self.handle_client_event(token),
                }
            }

            // Drain `MuxEvent`s from PTY reader threads.
            self.drain_mux_events();

            // Check exit condition: all panes exited + no clients.
            if self.should_exit() {
                log::info!("all panes exited and no clients — shutting down");
                break;
            }
        }

        log::info!("oriterm-mux daemon shutting down");
        Ok(())
    }

    /// Accept pending connections from the IPC listener.
    fn accept_connections(&mut self) -> io::Result<()> {
        loop {
            match self.listener.accept() {
                Ok(mut stream) => {
                    let id = self.client_alloc.alloc();
                    let token = Token(self.next_token);
                    self.next_token += 1;

                    self.poll
                        .registry()
                        .register(&mut stream, token, Interest::READABLE)?;

                    let conn = ClientConnection::new(id, stream, token);
                    self.connections.insert(id, conn);
                    self.token_to_client.insert(token, id);
                    self.had_client = true;
                    log::info!("client {id} connected");
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Handle a readable event from a connected client.
    fn handle_client_event(&mut self, token: Token) {
        let Some(&client_id) = self.token_to_client.get(&token) else {
            log::warn!("readable event for unknown token {}", token.0);
            return;
        };

        // Read available bytes from the stream into the frame reader.
        let read_status = {
            let Some(conn) = self.connections.get_mut(&client_id) else {
                return;
            };
            let mut tmp = [0u8; 4096];
            match conn.stream_mut().read(&mut tmp) {
                Ok(0) => ReadStatus::Closed,
                Ok(n) => {
                    conn.frame_reader_mut().extend(&tmp[..n]);
                    ReadStatus::GotData
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => ReadStatus::WouldBlock,
                Err(e) => {
                    log::warn!("read error from client {client_id}: {e}");
                    self.disconnect_client(client_id);
                    return;
                }
            }
        };

        if read_status == ReadStatus::Closed {
            log::info!("client {client_id} disconnected (EOF)");
            self.disconnect_client(client_id);
            return;
        }

        // Decode and dispatch all complete frames.
        loop {
            let frame = {
                let Some(conn) = self.connections.get_mut(&client_id) else {
                    return;
                };
                conn.frame_reader_mut().try_decode()
            };

            let Some(decode_result) = frame else {
                break; // No more complete frames.
            };

            match decode_result {
                Ok(decoded) => {
                    let seq = decoded.seq;
                    let Some(conn) = self.connections.get_mut(&client_id) else {
                        return;
                    };
                    let response = dispatch::dispatch_request(
                        &mut self.mux,
                        &mut self.panes,
                        conn,
                        decoded.pdu,
                        &self.wakeup,
                    );

                    // Sync subscription tracking to the global map.
                    self.sync_subscriptions(client_id);

                    if let Some(resp_pdu) = response {
                        let Some(conn) = self.connections.get_mut(&client_id) else {
                            return;
                        };
                        if let Err(e) = send_frame(conn.stream_mut(), seq, &resp_pdu) {
                            log::warn!("write error to client {client_id}: {e}");
                            self.disconnect_client(client_id);
                            return;
                        }
                    }
                }
                Err(e) => {
                    log::warn!("decode error from client {client_id}: {e}");
                    // Try to send an error response. If we don't know the seq,
                    // use 0.
                    let err_pdu = crate::MuxPdu::Error {
                        message: format!("decode error: {e}"),
                    };
                    if let Some(conn) = self.connections.get_mut(&client_id) {
                        let _ = send_frame(conn.stream_mut(), 0, &err_pdu);
                    }
                    // Fatal decode errors disconnect the client.
                    if is_fatal_decode_error(&e) {
                        self.disconnect_client(client_id);
                        return;
                    }
                }
            }
        }
    }

    /// Drain `MuxEvent`s from PTY reader threads and push notifications.
    fn drain_mux_events(&mut self) {
        self.mux.poll_events(&mut self.panes);
        self.mux.drain_notifications(&mut self.notification_buf);

        for notif in &self.notification_buf {
            let Some((target, pdu)) = notify::notification_to_pdu(notif, &self.panes) else {
                continue;
            };

            match target {
                TargetClients::PaneSubscribers(pane_id) => {
                    let Some(subs) = self.subscriptions.get(&pane_id) else {
                        continue;
                    };
                    self.scratch_clients.clear();
                    self.scratch_clients.extend_from_slice(subs);
                    for &cid in &self.scratch_clients {
                        if let Some(conn) = self.connections.get_mut(&cid) {
                            if let Err(e) = send_frame(conn.stream_mut(), 0, &pdu) {
                                log::warn!("notification write error to {cid}: {e}");
                            }
                        }
                    }
                }
                TargetClients::WindowClient(window_id) => {
                    // Find the client that owns this window.
                    let cid = self
                        .connections
                        .values()
                        .find(|c| c.window_id() == Some(window_id))
                        .map(ClientConnection::id);
                    if let Some(cid) = cid {
                        if let Some(conn) = self.connections.get_mut(&cid) {
                            if let Err(e) = send_frame(conn.stream_mut(), 0, &pdu) {
                                log::warn!("notification write error to {cid}: {e}");
                            }
                        }
                    }
                }
            }
        }
    }

    /// Disconnect a client, cleaning up all associated state.
    fn disconnect_client(&mut self, client_id: ClientId) {
        let Some(mut conn) = self.connections.remove(&client_id) else {
            return;
        };

        // Deregister from mio.
        let _ = self.poll.registry().deregister(conn.stream_mut());

        // Remove token mapping.
        self.token_to_client.remove(&conn.token());

        // Clean up subscription state.
        dispatch::remove_client_subscriptions(
            &mut self.subscriptions,
            client_id,
            conn.subscribed_panes(),
        );

        log::info!("client {client_id} fully disconnected");
    }

    /// Sync per-connection subscription state to the global subscriptions map.
    ///
    /// Called after dispatch to ensure the global map stays in sync with
    /// per-connection tracking.
    fn sync_subscriptions(&mut self, client_id: ClientId) {
        let Some(conn) = self.connections.get(&client_id) else {
            return;
        };
        for &pane_id in conn.subscribed_panes() {
            let subs = self.subscriptions.entry(pane_id).or_default();
            if !subs.contains(&client_id) {
                subs.push(client_id);
            }
        }
        // Remove entries where the client unsubscribed.
        self.scratch_panes.clear();
        self.scratch_panes
            .extend(conn.subscribed_panes().iter().copied());
        self.subscriptions.retain(|pane_id, subs| {
            if !self.scratch_panes.contains(pane_id) {
                subs.retain(|&c| c != client_id);
            }
            !subs.is_empty()
        });
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
        self.connections.is_empty() && self.mux.session().window_count() == 0
    }
}

/// Whether a decode error should cause the client to be disconnected.
fn is_fatal_decode_error(err: &crate::DecodeError) -> bool {
    matches!(
        err,
        crate::DecodeError::PayloadTooLarge(_) | crate::DecodeError::UnknownMsgType(_)
    )
}

#[cfg(test)]
mod tests;
