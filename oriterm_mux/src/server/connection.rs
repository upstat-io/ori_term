//! Client connection tracking.
//!
//! Each window process that connects to the daemon is tracked as a
//! [`ClientConnection`] with a unique [`ClientId`] and the mio [`Token`]
//! used for event dispatching.

use mio::Token;

use crate::WindowId;
use crate::id::ClientId;

use super::ipc::IpcStream;

/// A connected window process.
pub struct ClientConnection {
    /// Unique connection identifier.
    id: ClientId,
    /// IPC stream to the client.
    stream: IpcStream,
    /// mio token for event routing.
    token: Token,
    /// Which mux window this client renders (set after handshake).
    window_id: Option<WindowId>,
}

impl ClientConnection {
    /// Create a new connection with the given ID and stream.
    pub fn new(id: ClientId, stream: IpcStream, token: Token) -> Self {
        Self {
            id,
            stream,
            window_id: None,
            token,
        }
    }

    /// Connection identifier.
    pub fn id(&self) -> ClientId {
        self.id
    }

    /// Mutable access to the IPC stream.
    pub fn stream_mut(&mut self) -> &mut IpcStream {
        &mut self.stream
    }

    /// mio token assigned to this connection.
    pub fn token(&self) -> Token {
        self.token
    }

    /// Which mux window this client is rendering.
    pub fn window_id(&self) -> Option<WindowId> {
        self.window_id
    }

    /// Set the window this client renders (after handshake).
    pub fn set_window_id(&mut self, id: WindowId) {
        self.window_id = Some(id);
    }
}
