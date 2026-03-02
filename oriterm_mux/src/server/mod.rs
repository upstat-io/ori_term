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
mod ipc;
mod pid_file;

use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use mio::{Events, Interest, Poll, Token, Waker};

use crate::id::ClientId;
use crate::{IdAllocator, InProcessMux, PaneId};

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

    // Connection tracking.
    /// Connected window processes keyed by client ID.
    connections: HashMap<ClientId, ClientConnection>,
    /// Pane → subscribed clients mapping.
    #[allow(dead_code, reason = "subscriptions wired in Section 44.2 IPC protocol")]
    subscriptions: HashMap<PaneId, Vec<ClientId>>,
    /// Allocator for client IDs.
    client_alloc: IdAllocator<ClientId>,

    // Event loop infrastructure.
    /// mio poll instance.
    poll: Poll,
    /// Cross-thread waker for `MuxEvent` notifications.
    waker: Arc<Waker>,
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
        let mut listener = IpcListener::bind_at(socket_path)?;
        poll.registry()
            .register(&mut listener, LISTENER, Interest::READABLE)?;

        Ok(Self {
            mux: InProcessMux::new(),
            listener,
            connections: HashMap::new(),
            subscriptions: HashMap::new(),
            client_alloc: IdAllocator::new(),
            poll,
            waker,
            shutdown: Arc::new(AtomicBool::new(false)),
            _pid_file: pid_file,
            next_token: CLIENT_BASE,
            start_time: Instant::now(),
            had_client: false,
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
    fn handle_client_event(&self, token: Token) {
        // Find the client with this mio token.
        let _client_id = self
            .connections
            .values()
            .find(|c| c.token() == token)
            .map(ClientConnection::id);

        // TODO (Section 44.2): Read IPC requests and dispatch to mux.
        // For now, this is a skeleton — the IPC protocol codec and
        // request/response types are defined in Section 44.2.
    }

    /// Drain `MuxEvent`s from PTY reader threads and generate notifications.
    #[expect(
        clippy::unused_self,
        reason = "will use self in Section 44.2 when IPC protocol is wired"
    )]
    fn drain_mux_events(&self) {
        // TODO (Section 44.2): Process `MuxEvent`s through `InProcessMux` and
        // push notifications to subscribed clients via IPC streams.
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

#[cfg(test)]
mod tests;
