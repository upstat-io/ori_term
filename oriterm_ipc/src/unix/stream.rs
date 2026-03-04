//! Server-side non-blocking IPC stream (Unix domain socket).

use std::io;

use mio::event::Source;
use mio::net::UnixStream;
use mio::{Interest, Registry, Token};

/// Server-side IPC stream wrapping a mio Unix domain socket.
///
/// Implements `Read + Write + mio::Source` for non-blocking server use.
#[derive(Debug)]
pub struct IpcStream(UnixStream);

impl IpcStream {
    /// Wrap a mio `UnixStream` accepted from the listener.
    pub(super) fn new(stream: UnixStream) -> Self {
        Self(stream)
    }
}

impl io::Read for IpcStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl io::Write for IpcStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

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
