//! Daemon auto-start and discovery.
//!
//! Provides the glue between `oriterm` (window binary) and `oriterm-mux`
//! (daemon binary). On first launch, [`ensure_daemon`] spawns the daemon
//! if it isn't already running, waits for the socket to appear, and
//! returns the socket path for [`MuxClient::connect`].

use std::io;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::server::{pid_file_path, read_pid, socket_path};

/// Maximum total wait time when polling for the daemon socket to appear.
const MAX_WAIT: Duration = Duration::from_millis(2550);

/// Initial backoff interval for socket polling.
const INITIAL_BACKOFF: Duration = Duration::from_millis(10);

/// Try connecting to the daemon socket.
///
/// Returns `true` if the daemon is reachable at `path`.
pub fn probe_daemon(path: &Path) -> bool {
    UnixStream::connect(path).is_ok()
}

/// Check whether the PID file points to a live process.
///
/// Returns `true` if the PID file exists and the process is still running.
/// Cleans up stale PID and socket files when the process is dead.
pub fn validate_pid_file(pid_path: &Path, sock_path: &Path) -> bool {
    let pid = match read_pid(pid_path) {
        Ok(pid) => pid,
        Err(_) => return false,
    };

    // `kill(pid, 0)` checks process existence without sending a signal.
    #[allow(
        unsafe_code,
        reason = "kill(2) with signal 0 is the POSIX existence check"
    )]
    let alive = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
    if alive {
        return true;
    }

    // Dead process — clean up stale files.
    log::info!("stale PID file (pid={pid}), cleaning up");
    let _ = std::fs::remove_file(pid_path);
    let _ = std::fs::remove_file(sock_path);
    false
}

/// Spawn `oriterm-mux --daemon` as a detached background process.
///
/// Locates the daemon binary as a sibling of the current executable.
fn start_daemon() -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "cannot determine executable directory",
        )
    })?;
    let daemon_bin = dir.join("oriterm-mux");

    if !daemon_bin.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("daemon binary not found: {}", daemon_bin.display()),
        ));
    }

    log::info!("starting daemon: {}", daemon_bin.display());

    Command::new(&daemon_bin)
        .arg("--daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| io::Error::new(e.kind(), format!("failed to spawn daemon: {e}")))?;

    Ok(())
}

/// Poll for the daemon socket to appear and become connectable.
///
/// Uses exponential backoff starting at 10ms, doubling each iteration,
/// up to `max_wait` total elapsed time.
fn wait_for_socket(sock_path: &Path, max_wait: Duration) -> io::Result<()> {
    let start = Instant::now();
    let mut backoff = INITIAL_BACKOFF;

    loop {
        if sock_path.exists() && probe_daemon(sock_path) {
            return Ok(());
        }

        if start.elapsed() >= max_wait {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("daemon did not start within {}ms", max_wait.as_millis()),
            ));
        }

        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(max_wait.saturating_sub(start.elapsed()));
    }
}

/// Ensure a daemon is running and return the socket path.
///
/// 1. Cleans up stale PID files.
/// 2. Probes for an existing daemon — returns immediately if reachable.
/// 3. Spawns `oriterm-mux --daemon` and waits for the socket.
pub fn ensure_daemon() -> io::Result<PathBuf> {
    let sock = socket_path();
    let pid = pid_file_path();

    // Clean up stale state.
    validate_pid_file(&pid, &sock);

    // Already running?
    if probe_daemon(&sock) {
        log::info!("daemon already running at {}", sock.display());
        return Ok(sock);
    }

    // Start the daemon.
    start_daemon()?;

    // Wait for the socket to appear.
    wait_for_socket(&sock, MAX_WAIT)?;

    log::info!("daemon started, socket at {}", sock.display());
    Ok(sock)
}

#[cfg(test)]
mod tests;
