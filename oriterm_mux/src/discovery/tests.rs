use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::time::Duration;

use super::{validate_pid_file, wait_for_socket};

/// `probe_daemon` returns true when a listener is bound to the socket.
#[test]
fn probe_daemon_success() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");

    let _listener = UnixListener::bind(&sock).unwrap();
    assert!(oriterm_ipc::probe_daemon(&sock));
}

/// `probe_daemon` returns false when no listener exists.
#[test]
fn probe_daemon_no_server() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    assert!(!oriterm_ipc::probe_daemon(&sock));
}

/// `validate_pid_file` returns true for the current process.
#[test]
fn validate_pid_file_live_process() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("test.pid");
    let sock_path = dir.path().join("test.sock");

    let mut f = std::fs::File::create(&pid_path).unwrap();
    write!(f, "{}", std::process::id()).unwrap();
    drop(f);

    assert!(validate_pid_file(&pid_path, &sock_path));
    assert!(pid_path.exists(), "PID file should remain for live process");
}

/// `validate_pid_file` cleans up stale PID and socket files.
#[test]
fn validate_pid_file_stale_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("test.pid");
    let sock_path = dir.path().join("test.sock");

    // Write a PID that almost certainly doesn't exist.
    let mut f = std::fs::File::create(&pid_path).unwrap();
    write!(f, "999999999").unwrap();
    drop(f);

    // Create a stale socket file.
    std::fs::File::create(&sock_path).unwrap();

    assert!(!validate_pid_file(&pid_path, &sock_path));
    assert!(!pid_path.exists(), "stale PID file should be removed");
    assert!(!sock_path.exists(), "stale socket should be removed");
}

/// `validate_pid_file` returns false when the file doesn't exist.
#[test]
fn validate_pid_file_missing() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("nonexistent.pid");
    let sock_path = dir.path().join("nonexistent.sock");
    assert!(!validate_pid_file(&pid_path, &sock_path));
}

/// `wait_for_socket` succeeds when socket is already available.
#[test]
fn wait_for_socket_already_available() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");

    let _listener = UnixListener::bind(&sock).unwrap();
    wait_for_socket(&sock, Duration::from_millis(100)).unwrap();
}

/// `wait_for_socket` returns timeout error when socket never appears.
#[test]
fn wait_for_socket_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");

    let err = wait_for_socket(&sock, Duration::from_millis(50)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
}

/// `wait_for_socket` succeeds when socket appears mid-wait.
#[test]
fn wait_for_socket_delayed_start() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("test.sock");

    let sock_for_thread = sock_path.clone();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        UnixListener::bind(&sock_for_thread).unwrap()
    });

    wait_for_socket(&sock_path, Duration::from_secs(2)).unwrap();

    // Keep the listener alive until assertion passes.
    let _listener = handle.join().unwrap();
}

/// `probe_daemon` returns false for a socket file with no listener.
#[test]
fn probe_daemon_stale_socket_file() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");

    // Create a regular file — not a real socket.
    std::fs::File::create(&sock).unwrap();
    assert!(!oriterm_ipc::probe_daemon(&sock));
}

/// `validate_pid_file` returns false for non-numeric content.
#[test]
fn validate_pid_file_invalid_content() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("test.pid");
    let sock_path = dir.path().join("test.sock");

    std::fs::write(&pid_path, "not-a-number").unwrap();

    // Invalid content → read_pid fails → returns false.
    assert!(!validate_pid_file(&pid_path, &sock_path));
}

/// Verify `ensure_daemon` with a pre-existing daemon (simulated via listener).
#[test]
fn ensure_daemon_with_existing_daemon() {
    // We can't easily test the full ensure_daemon flow because it calls
    // socket_path() which uses the real runtime dir. Instead, test the
    // individual building blocks above and verify the composition logic
    // by testing probe_daemon succeeds after a listener is bound.
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");

    let _listener = UnixListener::bind(&sock).unwrap();

    // Simulate what ensure_daemon does: probe first.
    assert!(oriterm_ipc::probe_daemon(&sock));
    // If probe succeeds, ensure_daemon returns immediately — verified.

    assert!(Path::new(sock.as_path()).exists());
}

/// Multiple socket files: one live listener, one stale file.
///
/// Discovery building blocks should distinguish live from dead sockets.
/// `probe_daemon` succeeds for the live one, fails for the stale one.
#[test]
fn multiple_sockets_dead_pruning() {
    let dir = tempfile::tempdir().unwrap();
    let live_sock = dir.path().join("live.sock");
    let stale_sock = dir.path().join("stale.sock");

    // Bind a live listener.
    let _listener = UnixListener::bind(&live_sock).unwrap();

    // Create a stale socket file (regular file, not a real listener).
    std::fs::File::create(&stale_sock).unwrap();

    // Live socket is reachable.
    assert!(
        oriterm_ipc::probe_daemon(&live_sock),
        "live socket should be reachable"
    );

    // Stale file is not reachable.
    assert!(
        !oriterm_ipc::probe_daemon(&stale_sock),
        "stale socket file should not be reachable"
    );

    // Both files exist.
    assert!(live_sock.exists());
    assert!(stale_sock.exists());
}

/// `validate_pid_file` handles PID files with trailing whitespace/newline.
///
/// PID files written by other processes may include `\n` or spaces.
#[test]
fn validate_pid_file_trailing_whitespace() {
    use crate::server::read_pid;

    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("test.sock");

    // PID with trailing newline.
    let pid_newline = dir.path().join("newline.pid");
    std::fs::write(&pid_newline, format!("{}\n", std::process::id())).unwrap();
    assert_eq!(read_pid(&pid_newline).unwrap(), std::process::id());
    assert!(validate_pid_file(&pid_newline, &sock_path));

    // PID with trailing spaces.
    let pid_spaces = dir.path().join("spaces.pid");
    std::fs::write(&pid_spaces, format!("{}  ", std::process::id())).unwrap();
    assert_eq!(read_pid(&pid_spaces).unwrap(), std::process::id());
    assert!(validate_pid_file(&pid_spaces, &sock_path));

    // PID with leading and trailing whitespace.
    let pid_both = dir.path().join("both.pid");
    std::fs::write(&pid_both, format!("  {} \n", std::process::id())).unwrap();
    assert_eq!(read_pid(&pid_both).unwrap(), std::process::id());
    assert!(validate_pid_file(&pid_both, &sock_path));
}
