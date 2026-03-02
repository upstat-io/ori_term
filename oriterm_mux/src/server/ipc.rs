//! IPC listener for the mux daemon.
//!
//! Uses Unix domain sockets. The socket is created at a well-known path
//! under `$XDG_RUNTIME_DIR` (or `/tmp/oriterm-$USER/` as fallback).
//! The listener is removed on drop to avoid stale sockets.

use std::io;
use std::path::{Path, PathBuf};

use mio::event::Source;
use mio::net::{UnixListener, UnixStream};
use mio::{Interest, Registry, Token};

/// Unix domain socket listener for daemon IPC connections.
pub struct IpcListener {
    /// Inner mio listener.
    listener: UnixListener,
    /// Socket file path (removed on drop).
    path: PathBuf,
}

impl IpcListener {
    /// Bind a new IPC listener at the default socket path.
    pub fn bind() -> io::Result<Self> {
        Self::bind_at(&socket_path())
    }

    /// Bind at a specific path (for testing).
    pub fn bind_at(path: &Path) -> io::Result<Self> {
        // Ensure parent directory exists.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Remove stale socket from a previous run.
        let _ = std::fs::remove_file(path);

        let listener = UnixListener::bind(path)?;
        Ok(Self {
            listener,
            path: path.to_owned(),
        })
    }

    /// Accept a new client connection.
    ///
    /// Returns `WouldBlock` when no pending connections are available.
    pub fn accept(&self) -> io::Result<IpcStream> {
        let (stream, _addr) = self.listener.accept()?;
        Ok(IpcStream(stream))
    }

    /// Socket file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Source for IpcListener {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.listener.register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.listener.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        self.listener.deregister(registry)
    }
}

impl Drop for IpcListener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Client-side IPC stream wrapping a Unix domain socket.
#[derive(Debug)]
pub struct IpcStream(UnixStream);

impl Source for IpcStream {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.0.register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.0.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        self.0.deregister(registry)
    }
}

/// Compute the default socket path.
///
/// Prefers `$XDG_RUNTIME_DIR/oriterm-mux.sock` (standard on systemd-based
/// systems). Falls back to `/tmp/oriterm-$USER/mux.sock`.
pub fn socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir).join("oriterm-mux.sock")
    } else {
        let user = std::env::var("USER").unwrap_or_else(|_| String::from("unknown"));
        PathBuf::from(format!("/tmp/oriterm-{user}")).join("mux.sock")
    }
}
