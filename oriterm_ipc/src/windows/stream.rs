//! Server-side non-blocking IPC stream (named pipe) on Windows.

use std::io;

use mio::event::Source;
use mio::windows::NamedPipe;
use mio::{Interest, Registry, Token};

/// Server-side IPC stream wrapping a connected named pipe.
///
/// Implements `Read + Write + mio::Source` for non-blocking server use.
///
/// Streams yielded by [`super::listener::IpcListener::accept`] are already
/// associated with mio's IOCP port (they were the listener's pending
/// instance). The first `register()` call transparently uses `reregister()`
/// to update the token without re-associating the handle.
pub struct IpcStream {
    inner: NamedPipe,
    /// `true` when this stream was accepted from an `IpcListener`.
    ///
    /// IOCP handle associations are permanent — you can't deregister and
    /// re-register the same handle. The first `register()` call uses
    /// `reregister()` instead to update the mio token.
    pre_registered: bool,
}

impl IpcStream {
    /// Wrap a connected `NamedPipe` that is already registered with IOCP.
    pub(super) fn new(pipe: NamedPipe) -> Self {
        Self {
            inner: pipe,
            pre_registered: true,
        }
    }
}

impl io::Read for IpcStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl io::Write for IpcStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl Source for IpcStream {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        if self.pre_registered {
            // The underlying NamedPipe handle is already associated with the
            // IOCP port from the listener. Use `reregister` to update the
            // mio token and interests without re-associating.
            self.pre_registered = false;
            self.inner.reregister(registry, token, interests)
        } else {
            self.inner.register(registry, token, interests)
        }
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.inner.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        self.inner.deregister(registry)
    }
}
