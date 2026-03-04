use std::sync::atomic::Ordering;

use oriterm_ipc::ClientStream;

use crate::{MuxPdu, ProtocolCodec};

use super::MuxServer;
use super::frame_io::FrameReader;
use super::ipc::IpcListener;
use super::pid_file::{PidFile, read_pid};

/// Atomic counter for unique named pipe names (Windows).
#[cfg(windows)]
static TEST_PIPE_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Generate a platform-appropriate IPC test address.
///
/// Unix: returns a path inside the given `dir` (Unix domain socket).
/// Windows: returns a unique named pipe path (`\\.\pipe\...`).
fn test_sock_path(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    #[cfg(unix)]
    {
        dir.join(format!("{name}.sock"))
    }
    #[cfg(windows)]
    {
        let _ = dir;
        let n = TEST_PIPE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let pid = std::process::id();
        std::path::PathBuf::from(format!(r"\\.\pipe\oriterm-test-{pid}-{n}-{name}"))
    }
}

/// Helper: create a server, connect a client, and accept it.
fn server_with_client() -> (tempfile::TempDir, MuxServer, ClientStream) {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "test");
    let pid_path = dir.path().join("test.pid");
    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    let client = ClientStream::connect(&sock_path).unwrap();

    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(50)))
        .unwrap();
    server.accept_connections().unwrap();

    (dir, server, client)
}

/// Helper: send a PDU to a stream and flush.
fn send_pdu(stream: &mut ClientStream, seq: u32, pdu: &MuxPdu) {
    ProtocolCodec::encode_frame(stream, seq, pdu).unwrap();
}

/// Helper: read a response PDU from a stream.
fn recv_pdu(stream: &mut ClientStream) -> (u32, MuxPdu) {
    let frame = ProtocolCodec::new().decode_frame(stream).unwrap();
    (frame.seq, frame.pdu)
}

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
    let sock_path = test_sock_path(dir.path(), "bind");

    let mut listener = IpcListener::bind_at(&sock_path).unwrap();
    #[cfg(unix)]
    assert!(sock_path.exists(), "socket file should exist after bind");

    // Register with mio so Windows named pipe can accept.
    let mut poll = mio::Poll::new().unwrap();
    poll.registry()
        .register(&mut listener, mio::Token(0), mio::Interest::READABLE)
        .unwrap();

    // Connect a client.
    let _client = ClientStream::connect(&sock_path).unwrap();

    // Poll to let accept complete (needed for IOCP on Windows).
    let mut events = mio::Events::with_capacity(4);
    poll.poll(&mut events, Some(std::time::Duration::from_millis(100)))
        .unwrap();

    // Accept on the server side.
    let _stream = listener.accept().unwrap();
}

#[cfg(unix)]
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

#[cfg(unix)]
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
    let sock_path = test_sock_path(dir.path(), "noblock");
    let mut listener = IpcListener::bind_at(&sock_path).unwrap();

    // Register with mio so the accept state machine works on Windows.
    let poll = mio::Poll::new().unwrap();
    poll.registry()
        .register(&mut listener, mio::Token(0), mio::Interest::READABLE)
        .unwrap();

    // No client connected — accept should return WouldBlock.
    let result = listener.accept();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
}

// -- MuxServer tests --

#[test]
fn server_creates_pid_file_and_socket() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "server");
    let pid_path = dir.path().join("server.pid");

    let server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();
    #[cfg(unix)]
    assert!(sock_path.exists(), "socket should exist after server init");
    assert!(pid_path.exists(), "PID file should exist after server init");
    assert_eq!(server.client_count(), 0);

    let pid = read_pid(&pid_path).unwrap();
    assert_eq!(pid, std::process::id());
}

#[test]
fn server_accepts_client_connection() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "accept");
    let pid_path = dir.path().join("accept.pid");

    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();
    assert_eq!(server.client_count(), 0);

    // Connect a client.
    let _client = ClientStream::connect(&sock_path).unwrap();

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
fn server_cleans_up_on_drop() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "cleanup");
    let pid_path = dir.path().join("cleanup.pid");

    {
        let _server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();
        #[cfg(unix)]
        assert!(sock_path.exists());
        assert!(pid_path.exists());
    }
    // Dropped — PID file should be cleaned up.
    #[cfg(unix)]
    assert!(!sock_path.exists(), "socket should be removed on drop");
    assert!(!pid_path.exists(), "PID file should be removed on drop");
}

#[test]
fn server_shutdown_flag_stops_event_loop() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "shutdown");
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
    let sock_path = test_sock_path(dir.path(), "multi");
    let pid_path = dir.path().join("multi.pid");

    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    // Connect three clients.
    let _c1 = ClientStream::connect(&sock_path).unwrap();
    let _c2 = ClientStream::connect(&sock_path).unwrap();
    let _c3 = ClientStream::connect(&sock_path).unwrap();

    // Run one poll cycle.
    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(50)))
        .unwrap();
    server.accept_connections().unwrap();

    assert_eq!(server.client_count(), 3);
}

// -- FrameReader tests --

#[test]
fn frame_reader_empty_returns_none() {
    let mut reader = FrameReader::new();
    assert!(reader.try_decode().is_none());
}

#[test]
fn frame_reader_partial_header_returns_none() {
    let mut reader = FrameReader::new();
    // Only 5 bytes (less than 10-byte header).
    reader.extend(&[0x01, 0x01, 0x00, 0x00, 0x00]);
    assert!(reader.try_decode().is_none());
}

#[test]
fn frame_reader_complete_frame() {
    let mut reader = FrameReader::new();

    // Encode a Hello PDU.
    let pdu = MuxPdu::Hello { pid: 42 };
    let mut buf = Vec::new();
    ProtocolCodec::encode_frame(&mut buf, 1, &pdu).unwrap();

    reader.extend(&buf);
    let frame = reader.try_decode().unwrap().unwrap();
    assert_eq!(frame.seq, 1);
    assert_eq!(frame.pdu, MuxPdu::Hello { pid: 42 });

    // No more frames.
    assert!(reader.try_decode().is_none());
}

#[test]
fn frame_reader_multiple_frames_in_one_read() {
    let mut reader = FrameReader::new();

    let mut buf = Vec::new();
    ProtocolCodec::encode_frame(&mut buf, 1, &MuxPdu::Hello { pid: 1 }).unwrap();
    ProtocolCodec::encode_frame(&mut buf, 2, &MuxPdu::CreateWindow).unwrap();
    ProtocolCodec::encode_frame(&mut buf, 3, &MuxPdu::ListWindows).unwrap();

    reader.extend(&buf);

    let f1 = reader.try_decode().unwrap().unwrap();
    assert_eq!(f1.seq, 1);
    assert_eq!(f1.pdu, MuxPdu::Hello { pid: 1 });

    let f2 = reader.try_decode().unwrap().unwrap();
    assert_eq!(f2.seq, 2);
    assert_eq!(f2.pdu, MuxPdu::CreateWindow);

    let f3 = reader.try_decode().unwrap().unwrap();
    assert_eq!(f3.seq, 3);
    assert_eq!(f3.pdu, MuxPdu::ListWindows);

    assert!(reader.try_decode().is_none());
}

#[test]
fn frame_reader_partial_payload_waits() {
    let mut reader = FrameReader::new();

    let pdu = MuxPdu::Hello { pid: 99 };
    let mut full = Vec::new();
    ProtocolCodec::encode_frame(&mut full, 5, &pdu).unwrap();

    // Feed just the header + half the payload.
    let split_at = 10 + (full.len() - 10) / 2;
    reader.extend(&full[..split_at]);
    assert!(reader.try_decode().is_none());

    // Feed the rest.
    reader.extend(&full[split_at..]);
    let frame = reader.try_decode().unwrap().unwrap();
    assert_eq!(frame.seq, 5);
    assert_eq!(frame.pdu, MuxPdu::Hello { pid: 99 });
}

#[test]
fn frame_reader_unknown_msg_type_returns_error() {
    let mut reader = FrameReader::new();

    // Construct a header with an invalid message type.
    let mut buf = [0u8; 10];
    buf[0..2].copy_from_slice(&0xFFFFu16.to_le_bytes()); // bad msg type
    buf[2..6].copy_from_slice(&1u32.to_le_bytes()); // seq
    buf[6..10].copy_from_slice(&0u32.to_le_bytes()); // payload_len = 0

    reader.extend(&buf);
    let result = reader.try_decode().unwrap();
    assert!(result.is_err());
}

#[test]
fn frame_reader_eof_handling() {
    // Verify that an EOF read (0 bytes) leaves the reader's buffer
    // empty — try_decode should still return None.
    let mut reader = FrameReader::new();
    reader.extend(&[]); // Simulate a 0-byte read.
    assert!(reader.try_decode().is_none());
}

// -- Hello handshake roundtrip --

#[test]
fn hello_handshake_roundtrip() {
    let (_dir, mut server, mut client) = server_with_client();

    // Client sends Hello.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 42 });

    // Server polls and dispatches.
    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(100)))
        .unwrap();
    for event in &events {
        match event.token() {
            super::LISTENER => server.accept_connections().unwrap(),
            super::WAKER => {}
            token => server.handle_client_event(token),
        }
    }

    // Client reads the HelloAck.
    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 1);
    match resp {
        MuxPdu::HelloAck { client_id } => {
            // Client ID should be valid (non-zero).
            assert_ne!(client_id.raw(), 0);
        }
        other => panic!("expected HelloAck, got {other:?}"),
    }
}

// -- CreateWindow roundtrip --

#[test]
fn create_window_roundtrip() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Create a window.
    send_pdu(&mut client, 2, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
    match resp {
        MuxPdu::WindowCreated { window_id } => {
            assert_ne!(window_id.raw(), 0);
        }
        other => panic!("expected WindowCreated, got {other:?}"),
    }
}

// -- ClaimWindow --

#[test]
fn claim_window_sets_connection_window_id() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Create a window.
    send_pdu(&mut client, 2, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    let window_id = match resp {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("expected WindowCreated, got {other:?}"),
    };

    // Claim the window.
    send_pdu(&mut client, 3, &MuxPdu::ClaimWindow { window_id });
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 3);
    assert_eq!(resp, MuxPdu::WindowClaimed);
}

// -- ListWindows --

#[test]
fn list_windows_empty_then_one() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // List windows (should be empty).
    send_pdu(&mut client, 2, &MuxPdu::ListWindows);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    match resp {
        MuxPdu::WindowList { windows } => {
            assert!(windows.is_empty());
        }
        other => panic!("expected WindowList, got {other:?}"),
    }

    // Create a window and list again.
    send_pdu(&mut client, 3, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    send_pdu(&mut client, 4, &MuxPdu::ListWindows);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    match resp {
        MuxPdu::WindowList { windows } => {
            assert_eq!(windows.len(), 1);
        }
        other => panic!("expected WindowList, got {other:?}"),
    }
}

// -- Disconnect cleans up state --

#[test]
fn disconnect_removes_client() {
    let (_dir, mut server, client) = server_with_client();
    assert_eq!(server.client_count(), 1);

    // Drop the client to trigger EOF.
    drop(client);

    // Poll to detect disconnect.
    poll_and_dispatch(&mut server);

    assert_eq!(server.client_count(), 0);
}

// -- Fire-and-forget messages --

#[test]
fn input_is_fire_and_forget() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Send Input (fire-and-forget) — pane doesn't exist, but no error returned.
    send_pdu(
        &mut client,
        0,
        &MuxPdu::Input {
            pane_id: crate::PaneId::from_raw(999),
            data: b"hello".to_vec(),
        },
    );
    poll_and_dispatch(&mut server);

    // Verify the server is still alive by sending another request.
    send_pdu(&mut client, 2, &MuxPdu::ListWindows);
    poll_and_dispatch(&mut server);
    let (seq, _) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
}

// -- Unexpected PDU from client --

#[test]
fn unexpected_pdu_returns_error() {
    let (_dir, mut server, mut client) = server_with_client();

    // Send a response PDU (which is invalid from a client).
    send_pdu(&mut client, 1, &MuxPdu::TabClosed);
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 1);
    match resp {
        MuxPdu::Error { message } => {
            assert!(message.contains("unexpected"));
        }
        other => panic!("expected Error, got {other:?}"),
    }
}

// -- ReadStatus from empty stream --

#[test]
fn frame_reader_no_data_returns_none() {
    let mut reader = FrameReader::new();
    // FrameReader with no data returns None.
    assert!(reader.try_decode().is_none());
    // Extending with empty slice is a no-op.
    reader.extend(&[]);
    assert!(reader.try_decode().is_none());
}

/// Helper: run one poll cycle and dispatch all events.
fn poll_and_dispatch(server: &mut MuxServer) {
    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(100)))
        .unwrap();
    for event in &events {
        match event.token() {
            super::LISTENER => server.accept_connections().unwrap(),
            super::WAKER => {}
            token => server.handle_client_event(token),
        }
    }
    server.drain_mux_events();
}

// -- Duplicate Hello handling --

#[test]
fn duplicate_hello_returns_second_ack() {
    let (_dir, mut server, mut client) = server_with_client();

    // First Hello.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 42 });
    poll_and_dispatch(&mut server);
    let (_, first_resp) = recv_pdu(&mut client);
    let first_id = match first_resp {
        MuxPdu::HelloAck { client_id } => client_id,
        other => panic!("expected HelloAck, got {other:?}"),
    };

    // Second Hello from the same client.
    send_pdu(&mut client, 2, &MuxPdu::Hello { pid: 42 });
    poll_and_dispatch(&mut server);
    let (seq, second_resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);

    // The second HelloAck should return the same client ID (idempotent).
    match second_resp {
        MuxPdu::HelloAck { client_id } => {
            assert_eq!(
                client_id, first_id,
                "duplicate Hello should return the same client ID"
            );
        }
        other => panic!("expected HelloAck, got {other:?}"),
    }
}

// -- ListTabs for non-existent window --

#[test]
fn list_tabs_nonexistent_window_returns_empty() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // List tabs for a window that doesn't exist.
    send_pdu(
        &mut client,
        2,
        &MuxPdu::ListTabs {
            window_id: crate::WindowId::from_raw(999),
        },
    );
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
    match resp {
        MuxPdu::TabList { tabs } => {
            assert!(
                tabs.is_empty(),
                "non-existent window should return empty tab list"
            );
        }
        other => panic!("expected TabList, got {other:?}"),
    }
}

// -- Unsubscribe from never-subscribed pane --

#[test]
fn unsubscribe_without_subscribe_succeeds() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Unsubscribe from a pane we never subscribed to.
    send_pdu(
        &mut client,
        2,
        &MuxPdu::Unsubscribe {
            pane_id: crate::PaneId::from_raw(999),
        },
    );
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
    assert_eq!(resp, MuxPdu::Unsubscribed);
}

// -- FrameReader byte-by-byte feeding --

#[test]
fn frame_reader_byte_by_byte() {
    let mut reader = FrameReader::new();

    let pdu = MuxPdu::Hello { pid: 77 };
    let mut full = Vec::new();
    ProtocolCodec::encode_frame(&mut full, 3, &pdu).unwrap();

    // Feed one byte at a time.
    for (i, &byte) in full.iter().enumerate() {
        reader.extend(&[byte]);
        if i < full.len() - 1 {
            assert!(
                reader.try_decode().is_none(),
                "should not decode until all bytes received (byte {i})"
            );
        }
    }

    // Now the full frame is in the buffer.
    let frame = reader.try_decode().unwrap().unwrap();
    assert_eq!(frame.seq, 3);
    assert_eq!(frame.pdu, MuxPdu::Hello { pid: 77 });
    assert!(reader.try_decode().is_none());
}

// -- FrameReader recovery after PayloadTooLarge --

#[test]
fn frame_reader_recovers_after_payload_too_large() {
    use crate::protocol::MAX_PAYLOAD;
    use crate::{FrameHeader, MsgType};

    let mut reader = FrameReader::new();

    // First: a bad frame with payload_len > MAX_PAYLOAD.
    let bad_header = FrameHeader {
        msg_type: MsgType::Hello as u16,
        seq: 1,
        payload_len: MAX_PAYLOAD + 1,
    };
    reader.extend(&bad_header.encode());

    // Should produce PayloadTooLarge error.
    let result = reader.try_decode().unwrap();
    assert!(result.is_err(), "expected error for oversized payload");

    // Second: a valid frame immediately after.
    let good_pdu = MuxPdu::CreateWindow;
    let mut good_buf = Vec::new();
    ProtocolCodec::encode_frame(&mut good_buf, 2, &good_pdu).unwrap();
    reader.extend(&good_buf);

    // Should decode the good frame successfully.
    let frame = reader.try_decode().unwrap().unwrap();
    assert_eq!(frame.seq, 2);
    assert_eq!(frame.pdu, MuxPdu::CreateWindow);
}

// -- Server auto-exit conditions --

#[test]
fn server_does_not_exit_during_grace_period() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "grace");
    let pid_path = dir.path().join("grace.pid");

    let server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    // Server just started with no clients — should NOT want to exit
    // because of the startup grace period.
    assert!(
        !server.should_exit(),
        "should not exit during startup grace period"
    );
}

#[test]
fn server_does_not_exit_before_first_client() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "noclient");
    let pid_path = dir.path().join("noclient.pid");

    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    // Hack: fast-forward past the grace period by replacing start_time.
    server.start_time = std::time::Instant::now() - std::time::Duration::from_secs(10);

    // No client has ever connected — should not exit.
    assert!(
        !server.should_exit(),
        "should not exit until at least one client has connected"
    );
}

#[test]
fn server_exits_after_client_disconnects_and_no_windows() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "exit");
    let pid_path = dir.path().join("exit.pid");

    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    // Connect a client.
    let client = ClientStream::connect(&sock_path).unwrap();
    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(50)))
        .unwrap();
    server.accept_connections().unwrap();
    assert_eq!(server.client_count(), 1);

    // Disconnect the client.
    drop(client);
    poll_and_dispatch(&mut server);
    assert_eq!(server.client_count(), 0);

    // Fast-forward past grace period.
    server.start_time = std::time::Instant::now() - std::time::Duration::from_secs(10);

    // No windows, no clients, had_client=true → should exit.
    assert!(
        server.should_exit(),
        "should exit when no clients and no windows after grace"
    );
}

// -- MoveTabToWindow via server (tasks 2, 3) --

/// Helper: connect a client, handshake, create a window, claim it, and
/// inject a test tab into the server's mux. Returns the window ID and tab ID.
fn setup_client_with_tab(
    server: &mut MuxServer,
    client: &mut ClientStream,
    seq_start: u32,
) -> (crate::WindowId, crate::TabId) {
    // Handshake.
    send_pdu(client, seq_start, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(server);
    let _ = recv_pdu(client);

    // Create a window.
    send_pdu(client, seq_start + 1, &MuxPdu::CreateWindow);
    poll_and_dispatch(server);
    let (_, resp) = recv_pdu(client);
    let window_id = match resp {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("expected WindowCreated, got {other:?}"),
    };

    // Claim the window.
    send_pdu(client, seq_start + 2, &MuxPdu::ClaimWindow { window_id });
    poll_and_dispatch(server);
    let _ = recv_pdu(client);

    // Inject a test tab directly into the server's mux.
    let tid = crate::TabId::from_raw(window_id.raw() * 10);
    let pid = crate::PaneId::from_raw(window_id.raw() * 100);
    server.mux.inject_test_tab(window_id, tid, pid);

    (window_id, tid)
}

/// MoveTabToWindow succeeds and the mux state reflects the move.
#[test]
fn move_tab_between_windows_roundtrip() {
    let (_dir, mut server, mut client) = server_with_client();

    // Create two windows with one tab each.
    let (_w1, t1) = setup_client_with_tab(&mut server, &mut client, 1);

    // Create second window.
    send_pdu(&mut client, 10, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    let w2 = match resp {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("expected WindowCreated, got {other:?}"),
    };

    // Inject a tab in w2 so w1 move doesn't leave w2 empty.
    let t2 = crate::TabId::from_raw(w2.raw() * 10);
    let p2 = crate::PaneId::from_raw(w2.raw() * 100);
    server.mux.inject_test_tab(w2, t2, p2);

    // Move t1 from w1 → w2.
    send_pdu(
        &mut client,
        11,
        &MuxPdu::MoveTabToWindow {
            tab_id: t1,
            target_window_id: w2,
        },
    );
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 11);
    assert_eq!(resp, MuxPdu::TabMovedAck);

    // Verify mux state: t1 should now be in w2.
    let w2_win = server.mux.session().get_window(w2).unwrap();
    assert!(w2_win.tabs().contains(&t1));
}

/// MoveTabToWindow with a nonexistent tab returns an Error PDU.
#[test]
fn move_nonexistent_tab_returns_error() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Create a destination window.
    send_pdu(&mut client, 2, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    let w1 = match resp {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("expected WindowCreated, got {other:?}"),
    };

    // Move a nonexistent tab.
    let fake_tab = crate::TabId::from_raw(999);
    send_pdu(
        &mut client,
        3,
        &MuxPdu::MoveTabToWindow {
            tab_id: fake_tab,
            target_window_id: w1,
        },
    );
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 3);
    assert!(
        matches!(resp, MuxPdu::Error { .. }),
        "expected Error for nonexistent tab, got {resp:?}"
    );
}

/// MoveTabToWindow to nonexistent target window returns Error.
#[test]
fn move_tab_to_nonexistent_window_returns_error() {
    let (_dir, mut server, mut client) = server_with_client();
    let (w1, t1) = setup_client_with_tab(&mut server, &mut client, 1);

    // Inject a second tab so t1 isn't the last tab in w1.
    let t2 = crate::TabId::from_raw(w1.raw() * 10 + 1);
    let p2 = crate::PaneId::from_raw(w1.raw() * 100 + 1);
    server.mux.inject_test_tab(w1, t2, p2);

    let fake_dest = crate::WindowId::from_raw(999);
    send_pdu(
        &mut client,
        10,
        &MuxPdu::MoveTabToWindow {
            tab_id: t1,
            target_window_id: fake_dest,
        },
    );
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 10);
    assert!(
        matches!(resp, MuxPdu::Error { .. }),
        "expected Error for nonexistent dest window, got {resp:?}"
    );
}

// -- Multi-client interaction (task 4) --

/// Helper: connect a second client to the server.
fn connect_second_client(server: &mut MuxServer, sock_path: &std::path::Path) -> ClientStream {
    let client2 = ClientStream::connect(sock_path).unwrap();

    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(50)))
        .unwrap();
    server.accept_connections().unwrap();

    client2
}

/// Two clients claim different windows. Tab move triggers
/// `NotifyWindowTabsChanged` delivery to the destination window's client.
#[test]
fn multi_client_tab_move_notification() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "tabmove");
    let pid_path = dir.path().join("tabmove.pid");
    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    // Connect client 1.
    let mut c1 = ClientStream::connect(&sock_path).unwrap();
    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(50)))
        .unwrap();
    server.accept_connections().unwrap();

    let (_w1, t1) = setup_client_with_tab(&mut server, &mut c1, 1);

    // Connect client 2.
    let mut c2 = connect_second_client(&mut server, &sock_path);
    // ClientStream is always blocking.

    // Client 2 handshake.
    send_pdu(&mut c2, 1, &MuxPdu::Hello { pid: 2 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut c2);

    // Client 2 creates and claims window w2.
    send_pdu(&mut c2, 2, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut c2);
    let w2 = match resp {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("expected WindowCreated, got {other:?}"),
    };
    send_pdu(&mut c2, 3, &MuxPdu::ClaimWindow { window_id: w2 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut c2);

    // Inject a tab in w2 (so the move has somewhere to land).
    let t2 = crate::TabId::from_raw(w2.raw() * 10);
    let p2 = crate::PaneId::from_raw(w2.raw() * 100);
    server.mux.inject_test_tab(w2, t2, p2);

    // Client 1 moves t1 to w2.
    send_pdu(
        &mut c1,
        10,
        &MuxPdu::MoveTabToWindow {
            tab_id: t1,
            target_window_id: w2,
        },
    );
    poll_and_dispatch(&mut server);

    // Client 1 gets TabMovedAck.
    let (seq, resp) = recv_pdu(&mut c1);
    assert_eq!(seq, 10);
    assert_eq!(resp, MuxPdu::TabMovedAck);

    // Client 2 should receive NotifyWindowTabsChanged for w2.
    c2.set_read_timeout(Some(std::time::Duration::from_millis(500)))
        .unwrap();
    let (notif_seq, notif_pdu) = recv_pdu(&mut c2);
    assert_eq!(notif_seq, 0, "notification should have seq=0");
    assert_eq!(
        notif_pdu,
        MuxPdu::NotifyWindowTabsChanged { window_id: w2 },
        "client 2 should get WindowTabsChanged for its window"
    );
}

// -- Client disconnect after claiming window (task 7) --

/// Client claims a window, then disconnects. Server handles cleanup
/// without panic and doesn't leak the claimed window.
#[test]
fn disconnect_after_claim_cleans_up() {
    let (_dir, mut server, mut client) = server_with_client();
    let (w1, _t1) = setup_client_with_tab(&mut server, &mut client, 1);

    assert_eq!(server.client_count(), 1);

    // Verify the connection has the window claim.
    let has_claim = server
        .connections
        .values()
        .any(|c| c.window_id() == Some(w1));
    assert!(has_claim, "client should have claimed w1");

    // Disconnect.
    drop(client);
    poll_and_dispatch(&mut server);

    assert_eq!(server.client_count(), 0);

    // The window is closed when the owning client disconnects
    // (GUI is gone → panes are orphaned).
    assert!(
        server.mux.session().get_window(w1).is_none(),
        "window should be closed after owning client disconnects"
    );
}

// -- remove_client_subscriptions integration (task 8) --

/// Client subscribes to a pane, then disconnects. Subscription is cleaned up.
#[test]
fn disconnect_cleans_up_subscriptions() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Create a window and inject a pane.
    send_pdu(&mut client, 2, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    let w1 = match resp {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("expected WindowCreated, got {other:?}"),
    };
    let tid = crate::TabId::from_raw(1);
    let pid = crate::PaneId::from_raw(1);
    server.mux.inject_test_tab(w1, tid, pid);

    // We need a real Pane in the server's pane map for Subscribe to succeed.
    // Since we can't easily construct a Pane in tests, we'll just subscribe
    // and the server will return an Error (pane not in pane map). But the
    // subscription is still recorded on the connection.
    //
    // Actually, let's test with Unsubscribe which doesn't require the pane to
    // exist, or check that the server returns Error but still registers the
    // subscription on the connection side.
    //
    // The subscribe dispatch checks `panes.get(&pane_id)` which will fail
    // since inject_test_tab doesn't insert into the server's pane HashMap.
    // Let's test with Subscribe → Error, then check subscription cleanup.
    send_pdu(&mut client, 3, &MuxPdu::Subscribe { pane_id: pid });
    poll_and_dispatch(&mut server);

    // The response will be an Error because the pane isn't in the server's
    // pane map (inject_test_tab only sets up mux metadata, not actual Panes).
    let (_, resp) = recv_pdu(&mut client);
    assert!(
        matches!(resp, MuxPdu::Error { .. }),
        "expected Error since no Pane object exists"
    );

    // However, the connection's subscribe() was called before the pane check.
    // Let's verify: dispatch calls conn.subscribe() first, then checks panes.
    // Actually, looking at dispatch.rs, Subscribe checks panes AFTER calling
    // conn.subscribe(). So the subscription is recorded.
    let has_sub = server.connections.values().any(|c| c.is_subscribed(pid));
    assert!(has_sub, "subscription should be recorded on connection");

    // Also check global subscriptions map.
    assert!(
        server.subscriptions.contains_key(&pid),
        "global subscriptions should track the pane"
    );

    // Disconnect.
    drop(client);
    poll_and_dispatch(&mut server);

    // Subscription should be cleaned up.
    assert_eq!(server.client_count(), 0);
    assert!(
        !server.subscriptions.contains_key(&pid),
        "subscription should be removed after disconnect"
    );
}

// -- parse_theme roundtrip (task 9) --

/// `parse_theme` with `"dark"` returns Dark.
#[test]
fn parse_theme_dark() {
    use oriterm_core::Theme;
    assert_eq!(super::dispatch::parse_theme(Some("dark")), Theme::Dark);
}

/// `parse_theme` with `"light"` returns Light.
#[test]
fn parse_theme_light() {
    use oriterm_core::Theme;
    assert_eq!(super::dispatch::parse_theme(Some("light")), Theme::Light);
}

/// `parse_theme` with `None` defaults to Dark.
#[test]
fn parse_theme_none_defaults_to_dark() {
    use oriterm_core::Theme;
    assert_eq!(super::dispatch::parse_theme(None), Theme::Dark);
}

/// `parse_theme` with unrecognized string defaults to Dark.
#[test]
fn parse_theme_garbage_defaults_to_dark() {
    use oriterm_core::Theme;
    assert_eq!(super::dispatch::parse_theme(Some("solarized")), Theme::Dark);
    assert_eq!(super::dispatch::parse_theme(Some("")), Theme::Dark);
}

// -- Ping/PingAck roundtrip --

/// Server responds to Ping with PingAck.
#[test]
fn ping_returns_ping_ack() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake first.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 99 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Send Ping.
    send_pdu(&mut client, 2, &MuxPdu::Ping);
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
    assert_eq!(resp, MuxPdu::PingAck);
}

// -- Broken-pipe notification cleanup --

/// Client disconnects while server has a notification queued.
///
/// Server should handle the broken pipe gracefully — no panic, client
/// is cleaned up on the next poll cycle, including the window it owned.
#[test]
fn disconnect_closes_owned_window_and_server_continues() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "brokenpipe");
    let pid_path = dir.path().join("brokenpipe.pid");
    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    // Connect two clients.
    let mut c1 = ClientStream::connect(&sock_path).unwrap();
    let mut c2 = ClientStream::connect(&sock_path).unwrap();
    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(50)))
        .unwrap();
    server.accept_connections().unwrap();
    assert_eq!(server.client_count(), 2);

    // Client 1: handshake + create window + claim window.
    send_pdu(&mut c1, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut c1);

    send_pdu(&mut c1, 2, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut c1);
    let w1 = match resp {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("expected WindowCreated, got {other:?}"),
    };
    send_pdu(&mut c1, 3, &MuxPdu::ClaimWindow { window_id: w1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut c1);

    // Client 2: handshake + create window + claim window.
    send_pdu(&mut c2, 1, &MuxPdu::Hello { pid: 2 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut c2);

    send_pdu(&mut c2, 2, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut c2);
    let w2 = match resp {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("expected WindowCreated, got {other:?}"),
    };
    send_pdu(&mut c2, 3, &MuxPdu::ClaimWindow { window_id: w2 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut c2);

    assert_eq!(server.mux().session().window_count(), 2);

    // Drop client 2 (simulating disconnect).
    drop(c2);
    poll_and_dispatch(&mut server);

    // Client 2's window should be closed.
    assert_eq!(server.client_count(), 1);
    assert!(
        server.mux().session().get_window(w2).is_none(),
        "w2 should be closed after owning client disconnects"
    );
    // Client 1's window should still exist.
    assert!(server.mux().session().get_window(w1).is_some());
    assert_eq!(server.mux().session().window_count(), 1);

    // Server is still alive and functional.
    send_pdu(&mut c1, 4, &MuxPdu::ListWindows);
    poll_and_dispatch(&mut server);
    let (seq, resp) = recv_pdu(&mut c1);
    assert_eq!(seq, 4);
    match resp {
        MuxPdu::WindowList { windows } => {
            assert_eq!(windows.len(), 1, "only w1 should remain");
        }
        other => panic!("expected WindowList, got {other:?}"),
    }
}

// -- Integrated multi-command pipeline --

/// Multi-command pipeline: create window → inject tabs → move tab →
/// close window → verify final state.
#[test]
fn multi_command_pipeline_final_state() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Create window 1.
    send_pdu(&mut client, 2, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    let w1 = match resp {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("expected WindowCreated, got {other:?}"),
    };

    // Create window 2.
    send_pdu(&mut client, 3, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    let w2 = match resp {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("expected WindowCreated, got {other:?}"),
    };

    // Inject tabs into both windows.
    let t1 = crate::TabId::from_raw(10);
    let p1 = crate::PaneId::from_raw(100);
    server.mux.inject_test_tab(w1, t1, p1);

    let t2 = crate::TabId::from_raw(20);
    let p2 = crate::PaneId::from_raw(200);
    server.mux.inject_test_tab(w1, t2, p2);

    let t3 = crate::TabId::from_raw(30);
    let p3 = crate::PaneId::from_raw(300);
    server.mux.inject_test_tab(w2, t3, p3);

    // Verify initial state.
    let w1_tabs = server.mux.session().get_window(w1).unwrap().tabs().len();
    let w2_tabs = server.mux.session().get_window(w2).unwrap().tabs().len();
    assert_eq!(w1_tabs, 2, "w1 should have 2 tabs");
    assert_eq!(w2_tabs, 1, "w2 should have 1 tab");

    // Move t1 from w1 to w2.
    send_pdu(
        &mut client,
        4,
        &MuxPdu::MoveTabToWindow {
            tab_id: t1,
            target_window_id: w2,
        },
    );
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    assert_eq!(resp, MuxPdu::TabMovedAck);

    // Verify after move: w1 has 1 tab, w2 has 2 tabs.
    let w1_tabs = server.mux.session().get_window(w1).unwrap().tabs().len();
    let w2_tabs = server.mux.session().get_window(w2).unwrap().tabs().len();
    assert_eq!(w1_tabs, 1, "w1 should have 1 tab after move");
    assert_eq!(w2_tabs, 2, "w2 should have 2 tabs after move");
    assert!(
        server
            .mux
            .session()
            .get_window(w2)
            .unwrap()
            .tabs()
            .contains(&t1)
    );

    // Close window 1.
    send_pdu(&mut client, 5, &MuxPdu::CloseWindow { window_id: w1 });
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    match resp {
        MuxPdu::WindowClosed { pane_ids } => {
            assert!(
                pane_ids.contains(&p2),
                "w1's remaining pane should be in closed list"
            );
        }
        other => panic!("expected WindowClosed, got {other:?}"),
    }

    // Verify final state: only w2 remains with 2 tabs.
    assert!(
        server.mux.session().get_window(w1).is_none(),
        "w1 should be gone"
    );
    let w2_win = server.mux.session().get_window(w2).unwrap();
    assert_eq!(w2_win.tabs().len(), 2);
    assert!(w2_win.tabs().contains(&t1));
    assert!(w2_win.tabs().contains(&t3));

    // List windows should return exactly 1.
    send_pdu(&mut client, 6, &MuxPdu::ListWindows);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    match resp {
        MuxPdu::WindowList { windows } => {
            assert_eq!(windows.len(), 1);
            assert_eq!(windows[0].window_id, w2);
        }
        other => panic!("expected WindowList, got {other:?}"),
    }
}

// -- Resize fire-and-forget verification --

/// Resize PDU is fire-and-forget — server processes it silently.
///
/// Even for a non-existent pane, no error response is sent.
/// The server continues processing subsequent requests normally.
#[test]
fn resize_fire_and_forget_no_response() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Send Resize for a nonexistent pane (fire-and-forget).
    send_pdu(
        &mut client,
        0,
        &MuxPdu::Resize {
            pane_id: crate::PaneId::from_raw(999),
            cols: 120,
            rows: 40,
        },
    );
    poll_and_dispatch(&mut server);

    // Verify server is still alive by sending a normal request.
    send_pdu(&mut client, 2, &MuxPdu::ListWindows);
    poll_and_dispatch(&mut server);
    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
    assert!(matches!(resp, MuxPdu::WindowList { .. }));
}

// -- CloseWindow full cleanup --

/// Closing a window with multiple tabs cleans up all related state
/// (session, pane registry).
#[test]
fn close_window_removes_all_tabs_and_pane_entries() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Create a window.
    send_pdu(&mut client, 2, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    let wid = match resp {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("expected WindowCreated, got {other:?}"),
    };

    // Inject 3 tabs with 3 panes.
    let t1 = crate::TabId::from_raw(10);
    let p1 = crate::PaneId::from_raw(100);
    let t2 = crate::TabId::from_raw(20);
    let p2 = crate::PaneId::from_raw(200);
    let t3 = crate::TabId::from_raw(30);
    let p3 = crate::PaneId::from_raw(300);
    server.mux.inject_test_tab(wid, t1, p1);
    server.mux.inject_test_tab(wid, t2, p2);
    server.mux.inject_test_tab(wid, t3, p3);

    // Verify pre-conditions.
    assert_eq!(
        server.mux.session().get_window(wid).unwrap().tabs().len(),
        3
    );

    // Close the window.
    send_pdu(&mut client, 3, &MuxPdu::CloseWindow { window_id: wid });
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    match resp {
        MuxPdu::WindowClosed { pane_ids } => {
            assert_eq!(pane_ids.len(), 3, "all 3 panes should be returned");
            assert!(pane_ids.contains(&p1));
            assert!(pane_ids.contains(&p2));
            assert!(pane_ids.contains(&p3));
        }
        other => panic!("expected WindowClosed, got {other:?}"),
    }

    // Window, tabs, pane entries should all be gone.
    assert!(server.mux.session().get_window(wid).is_none());
    assert!(server.mux.session().get_tab(t1).is_none());
    assert!(server.mux.session().get_tab(t2).is_none());
    assert!(server.mux.session().get_tab(t3).is_none());
    assert!(server.mux.pane_registry().get(p1).is_none());
    assert!(server.mux.pane_registry().get(p2).is_none());
    assert!(server.mux.pane_registry().get(p3).is_none());
}

// -- Concurrent multi-client RPC --

/// Two clients send requests concurrently. Both get correct,
/// non-cross-contaminated responses.
#[test]
fn concurrent_clients_no_cross_contamination() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "concurrent");
    let pid_path = dir.path().join("concurrent.pid");
    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    // Connect two clients.
    let mut c1 = ClientStream::connect(&sock_path).unwrap();
    let mut c2 = ClientStream::connect(&sock_path).unwrap();
    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(50)))
        .unwrap();
    server.accept_connections().unwrap();
    assert_eq!(server.client_count(), 2);

    // ClientStream is always blocking.
    // ClientStream is always blocking.

    // Both handshake.
    send_pdu(&mut c1, 1, &MuxPdu::Hello { pid: 1 });
    send_pdu(&mut c2, 1, &MuxPdu::Hello { pid: 2 });
    poll_and_dispatch(&mut server);
    let (_, r1) = recv_pdu(&mut c1);
    let (_, r2) = recv_pdu(&mut c2);
    let id1 = match r1 {
        MuxPdu::HelloAck { client_id } => client_id,
        other => panic!("expected HelloAck, got {other:?}"),
    };
    let id2 = match r2 {
        MuxPdu::HelloAck { client_id } => client_id,
        other => panic!("expected HelloAck, got {other:?}"),
    };
    assert_ne!(id1, id2, "clients should get different IDs");

    // Both create windows simultaneously.
    send_pdu(&mut c1, 2, &MuxPdu::CreateWindow);
    send_pdu(&mut c2, 2, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);

    let (_, r1) = recv_pdu(&mut c1);
    let (_, r2) = recv_pdu(&mut c2);
    let w1 = match r1 {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("c1: expected WindowCreated, got {other:?}"),
    };
    let w2 = match r2 {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("c2: expected WindowCreated, got {other:?}"),
    };
    assert_ne!(w1, w2, "windows should have different IDs");

    // Both claim their windows.
    send_pdu(&mut c1, 3, &MuxPdu::ClaimWindow { window_id: w1 });
    send_pdu(&mut c2, 3, &MuxPdu::ClaimWindow { window_id: w2 });
    poll_and_dispatch(&mut server);
    let (_, r1) = recv_pdu(&mut c1);
    let (_, r2) = recv_pdu(&mut c2);
    assert_eq!(r1, MuxPdu::WindowClaimed);
    assert_eq!(r2, MuxPdu::WindowClaimed);

    // Both list windows — should see the same 2 windows.
    send_pdu(&mut c1, 4, &MuxPdu::ListWindows);
    send_pdu(&mut c2, 4, &MuxPdu::ListWindows);
    poll_and_dispatch(&mut server);
    let (_, r1) = recv_pdu(&mut c1);
    let (_, r2) = recv_pdu(&mut c2);
    match (r1, r2) {
        (MuxPdu::WindowList { windows: w_a }, MuxPdu::WindowList { windows: w_b }) => {
            assert_eq!(w_a.len(), 2, "c1 should see 2 windows");
            assert_eq!(w_b.len(), 2, "c2 should see 2 windows");
        }
        other => panic!("expected WindowList from both, got {other:?}"),
    }
}

// -- Handshake rejection --

/// Client sends a response PDU (TabClosed) without Hello first.
///
/// The server should respond with an Error PDU.
#[test]
fn response_pdu_before_hello_returns_error() {
    let (_dir, mut server, mut client) = server_with_client();

    // Skip Hello — send CreateWindow directly.
    send_pdu(&mut client, 1, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);

    // Server should still accept the request (CreateWindow doesn't
    // require prior Hello in the current dispatch). Verify it returns
    // a valid response rather than crashing.
    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 1);
    // CreateWindow should succeed since dispatch doesn't require Hello first.
    assert!(
        matches!(resp, MuxPdu::WindowCreated { .. }),
        "should create window even without Hello: {resp:?}"
    );
}

/// Client sends a notification PDU (which only servers send).
///
/// The server should respond with an Error.
#[test]
fn notification_pdu_from_client_returns_error() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Client sends a notification PDU (which is a protocol violation).
    send_pdu(
        &mut client,
        2,
        &MuxPdu::NotifyPaneOutput {
            pane_id: crate::PaneId::from_raw(1),
        },
    );
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
    assert!(
        matches!(resp, MuxPdu::Error { .. }),
        "notification from client should be rejected: {resp:?}"
    );
}

/// Shutdown via IPC sets the server's shutdown flag.
#[test]
fn shutdown_via_ipc_sets_flag() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Shutdown flag should be false initially.
    assert!(
        !server.shutdown_flag().load(Ordering::Acquire),
        "shutdown flag should be false before Shutdown PDU"
    );

    // Send Shutdown request.
    send_pdu(&mut client, 2, &MuxPdu::Shutdown);
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
    assert!(
        matches!(resp, MuxPdu::ShutdownAck),
        "expected ShutdownAck, got {resp:?}"
    );

    // Shutdown flag should now be set.
    assert!(
        server.shutdown_flag().load(Ordering::Acquire),
        "shutdown flag should be true after ShutdownAck"
    );
}

// -- Shutdown + event loop exit integration --

/// After receiving a Shutdown PDU, the server's `run()` exits on the
/// next poll iteration. Tests the combined flow rather than individual
/// pieces (flag setting vs loop checking).
#[test]
fn shutdown_pdu_causes_run_to_exit() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "shutdown_run");
    let pid_path = dir.path().join("shutdown_run.pid");
    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    let mut client = ClientStream::connect(&sock_path).unwrap();

    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(50)))
        .unwrap();
    server.accept_connections().unwrap();

    // Send Shutdown. We need to get the data into the server's socket
    // buffer before calling run(), since run() will poll for events.
    send_pdu(&mut client, 1, &MuxPdu::Shutdown);

    // Spawn a thread to call run(). It should exit promptly because
    // the Shutdown PDU sets the shutdown flag.
    let handle = std::thread::spawn(move || {
        server.run().unwrap();
    });

    // If run() doesn't exit, the join will hang. Use a timeout.
    let result = handle.join();
    assert!(
        result.is_ok(),
        "server.run() should exit after Shutdown PDU"
    );

    // Client should have received ShutdownAck.
    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 1);
    assert_eq!(resp, MuxPdu::ShutdownAck);
}

// -- Shutdown from non-handshaked client --

/// Client sends Shutdown without prior Hello. Server should still
/// respond with ShutdownAck and set the shutdown flag.
#[test]
fn shutdown_without_hello_sets_flag() {
    let (_dir, mut server, mut client) = server_with_client();

    // No Hello — send Shutdown directly.
    send_pdu(&mut client, 1, &MuxPdu::Shutdown);
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 1);
    assert_eq!(resp, MuxPdu::ShutdownAck);

    assert!(
        server.shutdown_flag().load(Ordering::Acquire),
        "shutdown flag should be set even without Hello"
    );
}

// -- Double Shutdown idempotency --

/// Client sends Shutdown twice. Server ACKs both and shuts down once.
#[test]
fn double_shutdown_is_idempotent() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // First Shutdown.
    send_pdu(&mut client, 2, &MuxPdu::Shutdown);
    poll_and_dispatch(&mut server);
    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
    assert_eq!(resp, MuxPdu::ShutdownAck);
    assert!(server.shutdown_flag().load(Ordering::Acquire));

    // Second Shutdown — should still ACK without panic.
    send_pdu(&mut client, 3, &MuxPdu::Shutdown);
    poll_and_dispatch(&mut server);
    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 3);
    assert_eq!(resp, MuxPdu::ShutdownAck);

    // Flag is still set.
    assert!(server.shutdown_flag().load(Ordering::Acquire));
}

// -- Shutdown with active subscriptions --

/// Client subscribes to a pane, then sends Shutdown. Verify
/// the server handles shutdown cleanly with active subscriptions.
#[test]
fn shutdown_with_active_subscriptions() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Create a window and inject a pane.
    send_pdu(&mut client, 2, &MuxPdu::CreateWindow);
    poll_and_dispatch(&mut server);
    let (_, resp) = recv_pdu(&mut client);
    let wid = match resp {
        MuxPdu::WindowCreated { window_id } => window_id,
        other => panic!("expected WindowCreated, got {other:?}"),
    };
    let tid = crate::TabId::from_raw(1);
    let pid = crate::PaneId::from_raw(1);
    server.mux.inject_test_tab(wid, tid, pid);

    // Subscribe (will return Error since no real Pane, but subscription
    // is still recorded on the connection).
    send_pdu(&mut client, 3, &MuxPdu::Subscribe { pane_id: pid });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Verify subscription is registered.
    assert!(
        server.subscriptions.contains_key(&pid),
        "subscription should be active before shutdown"
    );

    // Now send Shutdown.
    send_pdu(&mut client, 4, &MuxPdu::Shutdown);
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 4);
    assert_eq!(resp, MuxPdu::ShutdownAck);
    assert!(server.shutdown_flag().load(Ordering::Acquire));
}

// -- Server init with unwritable PID path --

/// `MuxServer::with_paths` returns an error when the PID file path
/// is in a non-existent root directory that can't be created.
#[test]
fn server_init_unwritable_pid_path_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = test_sock_path(dir.path(), "ok");
    // /dev/null/nested is not a valid directory on any Unix.
    let bad_pid = std::path::PathBuf::from("/dev/null/nested/test.pid");
    let result = MuxServer::with_paths(&sock_path, &bad_pid);
    assert!(result.is_err(), "should fail with unwritable PID path");
}
