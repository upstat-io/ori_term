//! Unix process validation and daemon probing.

use std::path::Path;

/// Try connecting to the daemon socket.
///
/// Returns `true` if the daemon is reachable at `path`.
pub fn probe_daemon(path: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(path).is_ok()
}

/// Check whether a PID refers to a running process.
///
/// Uses `kill(pid, 0)` — the POSIX existence check. Returns `true` if
/// the process is alive.
pub fn validate_pid(pid: u32) -> bool {
    // SAFETY: `kill` with signal 0 performs a permission check without
    // sending any signal. This is the standard POSIX process existence test.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

/// The daemon binary name on this platform.
pub fn daemon_binary_name() -> &'static str {
    "oriterm-mux"
}
