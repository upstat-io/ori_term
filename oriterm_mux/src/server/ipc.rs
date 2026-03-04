//! IPC types re-exported from `oriterm_ipc`.
//!
//! The actual platform-specific implementations (Unix domain sockets,
//! Windows named pipes) live in the `oriterm_ipc` crate. This module
//! re-exports the types under names compatible with existing server code.

pub use oriterm_ipc::{IpcListener, IpcStream};

/// Compute the default IPC address.
///
/// Re-exported from `oriterm_ipc::ipc_addr` as `socket_path` for
/// backward compatibility with existing call sites.
pub fn socket_path() -> std::path::PathBuf {
    oriterm_ipc::ipc_addr()
}
