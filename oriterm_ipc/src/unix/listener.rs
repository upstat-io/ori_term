//! Unix domain socket listener implementation.

use std::io;
use std::path::{Path, PathBuf};

use mio::event::Source;
use mio::net::UnixListener;
use mio::{Interest, Registry, Token};

use super::stream::IpcStream;

/// IPC listener backed by a Unix domain socket.
///
/// Binds at a filesystem path and accepts non-blocking connections.
/// The socket file is removed on drop to avoid stale sockets.
pub struct IpcListener {
    listener: UnixListener,
    path: PathBuf,
}

impl IpcListener {
    /// Bind at a specific path.
    pub fn bind_at(path: &Path) -> io::Result<Self> {
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
        Ok(IpcStream::new(stream))
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
