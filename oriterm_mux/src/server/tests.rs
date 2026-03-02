use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::atomic::Ordering;

use super::MuxServer;
use super::ipc::IpcListener;
use super::pid_file::{PidFile, read_pid};

// -- PID file tests --

#[test]
fn pid_file_creates_and_removes_on_drop() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.pid");

    {
        let pf = PidFile::create_at(&path).unwrap();
        assert!(path.exists(), "PID file should exist after creation");

        let content = std::fs::read_to_string(pf.path()).unwrap();
        let pid: u32 = content.trim().parse().unwrap();
        assert_eq!(pid, std::process::id());
    }
    // Dropped — file should be removed.
    assert!(!path.exists(), "PID file should be removed on drop");
}

#[test]
fn pid_file_read_pid() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("read.pid");

    let _pf = PidFile::create_at(&path).unwrap();
    let pid = read_pid(&path).unwrap();
    assert_eq!(pid, std::process::id());
}

#[test]
fn pid_file_read_invalid_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.pid");
    std::fs::write(&path, "not-a-number").unwrap();

    let result = read_pid(&path);
    assert!(result.is_err());
}

#[test]
fn pid_file_read_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.pid");
    let result = read_pid(&path);
    assert!(result.is_err());
}

#[test]
fn pid_file_creates_parent_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("deep").join("test.pid");
    let _pf = PidFile::create_at(&path).unwrap();
    assert!(path.exists());
}

// -- IPC listener tests --

#[test]
fn ipc_listener_bind_and_accept() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("test.sock");

    let listener = IpcListener::bind_at(&sock_path).unwrap();
    assert!(sock_path.exists(), "socket file should exist after bind");

    // Connect a client using std UnixStream.
    let _client = UnixStream::connect(&sock_path).unwrap();

    // Accept on the server side.
    let _stream = listener.accept().unwrap();
}

#[test]
fn ipc_listener_removes_stale_socket() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("stale.sock");

    // Create a stale socket file.
    std::fs::write(&sock_path, "stale").unwrap();
    assert!(sock_path.exists());

    // Binding should succeed despite the stale file.
    let _listener = IpcListener::bind_at(&sock_path).unwrap();
    assert!(sock_path.exists());
}

#[test]
fn ipc_listener_cleans_up_on_drop() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("drop.sock");

    {
        let _listener = IpcListener::bind_at(&sock_path).unwrap();
        assert!(sock_path.exists());
    }
    // Dropped — socket should be removed.
    assert!(!sock_path.exists(), "socket should be removed on drop");
}

#[test]
fn ipc_listener_accept_would_block() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("noblock.sock");
    let listener = IpcListener::bind_at(&sock_path).unwrap();

    // No client connected — accept should return WouldBlock.
    let result = listener.accept();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
}

// -- MuxServer tests --

#[test]
fn server_creates_pid_file_and_socket() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("server.sock");
    let pid_path = dir.path().join("server.pid");

    let server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();
    assert!(sock_path.exists(), "socket should exist after server init");
    assert!(pid_path.exists(), "PID file should exist after server init");
    assert_eq!(server.client_count(), 0);

    let pid = read_pid(&pid_path).unwrap();
    assert_eq!(pid, std::process::id());
}

#[test]
fn server_accepts_client_connection() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("accept.sock");
    let pid_path = dir.path().join("accept.pid");

    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();
    assert_eq!(server.client_count(), 0);

    // Connect a client.
    let _client = UnixStream::connect(&sock_path).unwrap();

    // Run one poll cycle to accept the connection.
    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(50)))
        .unwrap();
    server.accept_connections().unwrap();

    assert_eq!(server.client_count(), 1);
}

#[test]
fn server_version_handshake_placeholder() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("handshake.sock");
    let pid_path = dir.path().join("handshake.pid");

    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    // Connect and send a greeting.
    let mut client = UnixStream::connect(&sock_path).unwrap();
    client.write_all(b"hello").unwrap();

    // Run one poll cycle.
    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(50)))
        .unwrap();
    server.accept_connections().unwrap();

    assert_eq!(server.client_count(), 1);
}

#[test]
fn server_cleans_up_on_drop() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("cleanup.sock");
    let pid_path = dir.path().join("cleanup.pid");

    {
        let _server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();
        assert!(sock_path.exists());
        assert!(pid_path.exists());
    }
    // Dropped — both should be cleaned up.
    assert!(!sock_path.exists(), "socket should be removed on drop");
    assert!(!pid_path.exists(), "PID file should be removed on drop");
}

#[test]
fn server_shutdown_flag_stops_event_loop() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("shutdown.sock");
    let pid_path = dir.path().join("shutdown.pid");

    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    // Set the shutdown flag before running.
    server.shutdown_flag().store(true, Ordering::Release);

    // Run should return immediately.
    server.run().unwrap();
}

#[test]
fn server_multiple_clients() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("multi.sock");
    let pid_path = dir.path().join("multi.pid");

    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    // Connect three clients.
    let _c1 = UnixStream::connect(&sock_path).unwrap();
    let _c2 = UnixStream::connect(&sock_path).unwrap();
    let _c3 = UnixStream::connect(&sock_path).unwrap();

    // Run one poll cycle.
    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(50)))
        .unwrap();
    server.accept_connections().unwrap();

    assert_eq!(server.client_count(), 3);
}
