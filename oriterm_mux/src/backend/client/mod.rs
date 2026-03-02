//! IPC client backend for daemon mode.
//!
//! [`MuxClient`] implements [`MuxBackend`] by sending requests to a
//! [`MuxServer`](crate::server::MuxServer) over an IPC socket. Pane data
//! is not available locally — `pane()`/`pane_mut()` return `None`.
//! Rendering in daemon mode uses `PaneSnapshot` (a later step).

#[cfg(unix)]
mod notification;
mod rpc_methods;
#[cfg(unix)]
mod transport;

#[cfg(unix)]
use std::io;
#[cfg(unix)]
use std::sync::Arc;

use crate::mux_event::MuxNotification;
use crate::registry::{PaneRegistry, SessionRegistry};

#[cfg(unix)]
use self::transport::ClientTransport;

/// IPC client backend for daemon mode.
///
/// Sends mux operations to the daemon over an IPC socket and blocks on
/// responses. Pane data is not stored locally — the daemon owns all
/// terminal state. A background reader thread receives push notifications
/// from the daemon and buffers them for [`drain_notifications`].
pub struct MuxClient {
    /// IPC transport (reader thread + socket). `None` when test-only stub.
    #[cfg(unix)]
    transport: Option<ClientTransport>,

    /// Mirrored session state, synced from daemon responses/notifications.
    local_session: SessionRegistry,

    /// Mirrored pane registry, synced from daemon responses.
    pane_registry: PaneRegistry,

    /// Buffered notifications from the background reader thread.
    notifications: Vec<MuxNotification>,
}

impl MuxClient {
    /// Connect to a running daemon at `socket_path`.
    ///
    /// Performs the Hello handshake and spawns the background reader thread.
    /// `wakeup` is called when push notifications arrive (wakes the event loop).
    #[cfg(unix)]
    pub fn connect(
        socket_path: &std::path::Path,
        wakeup: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        let transport = ClientTransport::connect(socket_path, wakeup)?;
        log::info!("MuxClient connected, client_id={}", transport.client_id());
        Ok(Self {
            transport: Some(transport),
            local_session: SessionRegistry::new(),
            pane_registry: PaneRegistry::new(),
            notifications: Vec::new(),
        })
    }

    /// Create an unconnected client stub for testing.
    ///
    /// All RPC methods will fail gracefully (return defaults or errors).
    #[cfg(test)]
    pub fn new() -> Self {
        Self {
            #[cfg(unix)]
            transport: None,
            local_session: SessionRegistry::new(),
            pane_registry: PaneRegistry::new(),
            notifications: Vec::new(),
        }
    }

    /// The client ID assigned by the daemon, if connected.
    #[cfg(unix)]
    pub fn client_id(&self) -> Option<crate::id::ClientId> {
        self.transport.as_ref().map(ClientTransport::client_id)
    }

    /// Whether the daemon connection is alive.
    #[cfg(unix)]
    pub fn is_connected(&self) -> bool {
        self.transport
            .as_ref()
            .is_some_and(ClientTransport::is_alive)
    }
}

#[cfg(test)]
impl Default for MuxClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
