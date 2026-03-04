//! Platform-specific IPC abstraction for oriterm daemon mode.
//!
//! Provides safe types for inter-process communication between the
//! `oriterm` window binary and the `oriterm-mux` daemon:
//!
//! - [`IpcListener`] — server-side: bind, accept, `impl mio::Source`
//! - [`IpcStream`] — server-side: `Read + Write + mio::Source`
//! - [`ClientStream`] — client-side: `Read + Write + set_read_timeout`
//! - [`ipc_addr`] — platform-appropriate default IPC address
//! - [`pid_file_path`] — platform-appropriate PID file location
//! - [`probe_daemon`] — try connecting, return reachability
//! - [`validate_pid`] — check if a PID is alive
//! - [`daemon_binary_name`] — `"oriterm-mux"` vs `"oriterm-mux.exe"`
//!
//! # Platform implementations
//!
//! | Type | Unix | Windows |
//! |------|------|---------|
//! | `IpcListener` | Unix domain socket | Named pipe (`CreateNamedPipeW`) |
//! | `IpcStream` | `mio::net::UnixStream` | `mio::windows::NamedPipe` |
//! | `ClientStream` | `std::os::unix::net::UnixStream` | `CreateFileW` + `ReadFile/WriteFile` |

mod addr;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

// Re-export platform types under unified names.
#[cfg(unix)]
pub use self::unix::client_stream::ClientStream;
#[cfg(unix)]
pub use self::unix::listener::IpcListener;
#[cfg(unix)]
pub use self::unix::process::{daemon_binary_name, probe_daemon, validate_pid};
#[cfg(unix)]
pub use self::unix::stream::IpcStream;

#[cfg(windows)]
pub use self::windows::client_stream::ClientStream;
#[cfg(windows)]
pub use self::windows::listener::IpcListener;
#[cfg(windows)]
pub use self::windows::process::{daemon_binary_name, probe_daemon, validate_pid};
#[cfg(windows)]
pub use self::windows::stream::IpcStream;

pub use addr::{ipc_addr, pid_file_path};
