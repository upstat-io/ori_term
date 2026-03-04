//! Named pipe listener for the mux daemon on Windows.
//!
//! Uses `CreateNamedPipeW` to create pipe instances and `ConnectNamedPipe`
//! for the accept cycle. Integrates with mio via `mio::windows::NamedPipe`.
//!
//! Unlike Unix sockets (where the listener fd never changes), Windows named
//! pipes require a fresh handle per connection. After each accept, the new
//! pending instance must be registered with mio's IOCP for event delivery.
//! The listener stores a cloned [`Registry`] handle to do this automatically.

use std::io;
use std::os::windows::io::{FromRawHandle, RawHandle};
use std::path::{Path, PathBuf};

use mio::event::Source;
use mio::windows::NamedPipe;
use mio::{Interest, Registry, Token};
use windows_sys::Win32::Foundation::{ERROR_IO_PENDING, ERROR_PIPE_CONNECTED};
use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX};
use windows_sys::Win32::System::Pipes::{
    CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use super::stream::IpcStream;

/// Maximum buffer size for named pipe read/write operations.
const PIPE_BUF_SIZE: u32 = 65536;

/// IPC listener backed by a Windows named pipe.
///
/// Manages the pending-instance accept cycle. One named pipe instance is
/// always waiting for a client via `ConnectNamedPipe`. When a client
/// connects, the instance is yielded as an `IpcStream` and a new pending
/// instance is created and registered with mio.
pub struct IpcListener {
    /// The pending named pipe instance waiting for a connection.
    pending: NamedPipe,
    /// The pipe name (e.g. `\\.\pipe\oriterm-mux-user`).
    path: PathBuf,
    /// mio token assigned during `register()`.
    token: Option<Token>,
    /// Cloned registry handle for re-registering new pending instances.
    ///
    /// Populated during `register()` via `Registry::try_clone()`.
    registry: Option<Registry>,
}

impl IpcListener {
    /// Bind at a specific path (named pipe name).
    ///
    /// The accept cycle is NOT started here — overlapped I/O requires the
    /// handle to be associated with an IOCP port first. `start_accept()` is
    /// called automatically during [`Source::register`].
    pub fn bind_at(path: &Path) -> io::Result<Self> {
        let pending = create_pipe_instance(path)?;
        Ok(Self {
            pending,
            path: path.to_owned(),
            token: None,
            registry: None,
        })
    }

    /// Accept a new client connection.
    ///
    /// Returns `WouldBlock` when no client has connected yet.
    pub fn accept(&mut self) -> io::Result<IpcStream> {
        match self.pending.connect() {
            Ok(()) => self.yield_connected(),
            Err(ref e) if e.raw_os_error() == Some(ERROR_PIPE_CONNECTED as i32) => {
                self.yield_connected()
            }
            Err(ref e) if e.raw_os_error() == Some(ERROR_IO_PENDING as i32) => {
                Err(io::Error::from(io::ErrorKind::WouldBlock))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                Err(io::Error::from(io::ErrorKind::WouldBlock))
            }
            Err(e) => Err(e),
        }
    }

    /// Pipe name path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Yield the connected instance and create a new pending one.
    ///
    /// The new pending instance is registered with mio via the stored
    /// registry handle so that subsequent client connections trigger events.
    fn yield_connected(&mut self) -> io::Result<IpcStream> {
        let new_pending = create_pipe_instance(&self.path)?;
        let connected = std::mem::replace(&mut self.pending, new_pending);

        // Register the new pending instance with mio's IOCP. Without this,
        // the event loop would never be notified of new client connections.
        if let (Some(registry), Some(token)) = (&self.registry, self.token) {
            self.pending.register(registry, token, Interest::READABLE)?;
        }

        self.start_accept();
        Ok(IpcStream::new(connected))
    }

    /// Initiate an overlapped `ConnectNamedPipe` on the pending instance.
    fn start_accept(&self) {
        let _ = self.pending.connect();
    }
}

impl Source for IpcListener {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.token = Some(token);
        self.registry = Some(registry.try_clone()?);
        self.pending.register(registry, token, interests)?;
        // Start the overlapped ConnectNamedPipe now that the handle is
        // associated with the IOCP port (required for completion delivery).
        self.start_accept();
        Ok(())
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        self.token = Some(token);
        self.pending.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        self.registry = None;
        self.pending.deregister(registry)
    }
}

/// Create a new named pipe instance with overlapped I/O.
fn create_pipe_instance(path: &Path) -> io::Result<NamedPipe> {
    let wide_name = super::pipe_name::to_wide_string(path);

    // SAFETY: `CreateNamedPipeW` is a well-documented Win32 API.
    // The wide string is valid for the duration of the call.
    let handle = unsafe {
        CreateNamedPipeW(
            wide_name.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            PIPE_BUF_SIZE,
            PIPE_BUF_SIZE,
            0, // default timeout
            std::ptr::null(),
        )
    };

    if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: we just created a valid handle above.
    Ok(unsafe { NamedPipe::from_raw_handle(handle as RawHandle) })
}
