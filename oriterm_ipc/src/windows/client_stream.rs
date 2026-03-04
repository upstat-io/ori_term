//! Client-side blocking IPC stream (named pipe) on Windows.
//!
//! Opens a named pipe with `CreateFileW` in synchronous mode for blocking
//! read/write operations. Timeout support via `CancelIo` from a timer thread.

use std::io;
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::path::Path;
use std::time::Duration;

use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING, ReadFile, WriteFile,
};
use windows_sys::Win32::System::IO::CancelIoEx;
use windows_sys::Win32::System::Pipes::{SetNamedPipeHandleState, WaitNamedPipeW};

use super::pipe_name::to_wide_string;

/// Maximum time (ms) to wait for a pipe instance on `ERROR_PIPE_BUSY`.
const PIPE_BUSY_WAIT_MS: u32 = 2000;

/// Client-side IPC stream for blocking RPC communication.
///
/// Wraps a named pipe handle opened with `CreateFileW`. The background
/// reader thread in `MuxClient` uses this for the Hello handshake and
/// subsequent read/write operations.
pub struct ClientStream {
    handle: OwnedHandle,
    /// Read timeout (applied via timer-based `CancelIo`).
    read_timeout: Option<Duration>,
}

impl ClientStream {
    /// Connect to a daemon named pipe at `path`.
    ///
    /// If all pipe instances are busy (server creating a new one after a
    /// recent accept), retries once after `WaitNamedPipeW`.
    pub fn connect(path: &Path) -> io::Result<Self> {
        let wide_name = to_wide_string(path);

        // SAFETY: `CreateFileW` is a well-documented Win32 API.
        let mut handle = unsafe {
            CreateFileW(
                wide_name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            )
        };

        // ERROR_PIPE_BUSY (231): all instances connected, server is creating
        // a new one. Wait for it and retry once.
        if handle == INVALID_HANDLE_VALUE {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(231) {
                // SAFETY: `WaitNamedPipeW` blocks until an instance is available.
                unsafe { WaitNamedPipeW(wide_name.as_ptr(), PIPE_BUSY_WAIT_MS) };

                handle = unsafe {
                    CreateFileW(
                        wide_name.as_ptr(),
                        GENERIC_READ | GENERIC_WRITE,
                        0,
                        std::ptr::null(),
                        OPEN_EXISTING,
                        FILE_ATTRIBUTE_NORMAL,
                        std::ptr::null_mut(),
                    )
                };
            }
        }

        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        // Set pipe to byte-read mode (matching server config).
        let mode: u32 = 0; // PIPE_READMODE_BYTE
        // SAFETY: valid handle, valid pointer to mode.
        let ok = unsafe {
            SetNamedPipeHandleState(
                handle,
                &raw const mode,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            // Close handle on failure.
            unsafe { CloseHandle(handle) };
            return Err(io::Error::last_os_error());
        }

        // SAFETY: we just confirmed the handle is valid.
        let owned = unsafe { OwnedHandle::from_raw_handle(handle.cast()) };

        Ok(Self {
            handle: owned,
            read_timeout: None,
        })
    }

    /// Set the read timeout for blocking reads.
    ///
    /// When set, reads that exceed the timeout return `TimedOut`.
    /// Implemented via a `CancelIo` timer thread.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "API parity with Unix ClientStream"
    )]
    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.read_timeout = timeout;
        Ok(())
    }

    /// Set the write timeout for blocking writes.
    ///
    /// Currently a no-op — writes to named pipes complete quickly in practice.
    #[expect(
        clippy::unnecessary_wraps,
        clippy::unused_self,
        clippy::needless_pass_by_ref_mut,
        reason = "API parity with Unix ClientStream"
    )]
    pub fn set_write_timeout(&mut self, _timeout: Option<Duration>) -> io::Result<()> {
        Ok(())
    }

    /// Raw handle for timer-based read cancellation.
    fn raw_handle(&self) -> windows_sys::Win32::Foundation::HANDLE {
        use std::os::windows::io::AsRawHandle;
        self.handle.as_raw_handle().cast()
    }
}

impl io::Read for ClientStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // If a read timeout is set, spawn a cancellation timer.
        // We send the handle as a usize because raw pointers aren't Send.
        // The handle remains valid for the duration of the blocking read.
        //
        // Uses `CancelIoEx` (not `CancelIo`) because the cancellation runs
        // on a DIFFERENT thread than the one that issued the `ReadFile`.
        // `CancelIo` only cancels I/O for the calling thread — useless here.
        // `CancelIoEx` with `lpOverlapped = NULL` cancels all pending I/O
        // on the handle regardless of which thread issued them.
        let cancel_handle = self.read_timeout.map(|timeout| {
            let h = self.raw_handle() as usize;
            std::thread::spawn(move || {
                std::thread::sleep(timeout);
                // SAFETY: `CancelIoEx` cancels pending I/O on the handle.
                // The handle is still valid because the read is blocking.
                unsafe { CancelIoEx(h as *mut std::ffi::c_void, std::ptr::null_mut()) };
            })
        });

        let mut bytes_read: u32 = 0;
        // SAFETY: `ReadFile` is a well-documented Win32 API.
        // `buf` is a valid buffer with sufficient length.
        let ok = unsafe {
            ReadFile(
                self.raw_handle(),
                buf.as_mut_ptr().cast(),
                buf.len() as u32,
                &raw mut bytes_read,
                std::ptr::null_mut(),
            )
        };

        // Clean up cancellation thread (ignore its result).
        drop(cancel_handle);

        if ok == 0 {
            let err = io::Error::last_os_error();
            // ERROR_OPERATION_ABORTED (995) from CancelIo → translate to TimedOut.
            if err.raw_os_error() == Some(995) {
                return Err(io::Error::from(io::ErrorKind::TimedOut));
            }
            // ERROR_BROKEN_PIPE (109) → EOF.
            if err.raw_os_error() == Some(109) {
                return Ok(0);
            }
            return Err(err);
        }

        Ok(bytes_read as usize)
    }
}

impl io::Write for ClientStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut bytes_written: u32 = 0;
        // SAFETY: `WriteFile` is a well-documented Win32 API.
        let ok = unsafe {
            WriteFile(
                self.raw_handle(),
                buf.as_ptr().cast(),
                buf.len() as u32,
                &raw mut bytes_written,
                std::ptr::null_mut(),
            )
        };

        if ok == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(bytes_written as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        // Named pipes in byte mode don't buffer — nothing to flush.
        Ok(())
    }
}
