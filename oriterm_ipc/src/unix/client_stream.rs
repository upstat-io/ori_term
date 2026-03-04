//! Client-side blocking IPC stream (Unix domain socket).

use std::io;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

/// Client-side IPC stream for blocking RPC communication.
///
/// Wraps a standard library `UnixStream` (blocking mode). The background
/// reader thread in `MuxClient` uses this for the Hello handshake and
/// subsequent read/write operations.
pub struct ClientStream(UnixStream);

impl ClientStream {
    /// Connect to a daemon at `path`.
    pub fn connect(path: &Path) -> io::Result<Self> {
        let stream = UnixStream::connect(path)?;
        Ok(Self(stream))
    }

    /// Set the read timeout for blocking reads.
    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "API parity with Windows ClientStream"
    )]
    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(timeout)
    }

    /// Set the write timeout for blocking writes.
    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "API parity with Windows ClientStream"
    )]
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_write_timeout(timeout)
    }
}

impl io::Read for ClientStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl io::Write for ClientStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
