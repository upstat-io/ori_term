//! Tests for the mux server: PID file, IPC listener, MuxServer lifecycle,
//! frame codec, and request dispatch roundtrips.

use std::sync::atomic::Ordering;

use oriterm_ipc::ClientStream;

use crate::MuxPdu;
use crate::protocol::{FrameHeader, ProtocolCodec};

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
        let n = TEST_PIPE_COUNTER.fetch_add(1, Ordering::Relaxed);
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
    let err = result.err().expect("accept should fail with no client");
    assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);
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
    ProtocolCodec::encode_frame(&mut buf, 2, &MuxPdu::Ping).unwrap();
    ProtocolCodec::encode_frame(&mut buf, 3, &MuxPdu::ListPanes).unwrap();

    reader.extend(&buf);

    let f1 = reader.try_decode().unwrap().unwrap();
    assert_eq!(f1.seq, 1);
    assert_eq!(f1.pdu, MuxPdu::Hello { pid: 1 });

    let f2 = reader.try_decode().unwrap().unwrap();
    assert_eq!(f2.seq, 2);
    assert_eq!(f2.pdu, MuxPdu::Ping);

    let f3 = reader.try_decode().unwrap().unwrap();
    assert_eq!(f3.seq, 3);
    assert_eq!(f3.pdu, MuxPdu::ListPanes);

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
    send_pdu(&mut client, 2, &MuxPdu::Ping);
    poll_and_dispatch(&mut server);
    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
    assert_eq!(resp, MuxPdu::PingAck);
}

// -- Unexpected PDU from client --

#[test]
fn unexpected_pdu_returns_error() {
    let (_dir, mut server, mut client) = server_with_client();

    // Send a response PDU (which is invalid from a client).
    send_pdu(&mut client, 1, &MuxPdu::PaneClosedAck);
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
    use crate::protocol::{FrameHeader, MsgType};

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
    let good_pdu = MuxPdu::Ping;
    let mut good_buf = Vec::new();
    ProtocolCodec::encode_frame(&mut good_buf, 2, &good_pdu).unwrap();
    reader.extend(&good_buf);

    // Should decode the good frame successfully.
    let frame = reader.try_decode().unwrap().unwrap();
    assert_eq!(frame.seq, 2);
    assert_eq!(frame.pdu, MuxPdu::Ping);
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
fn server_exits_after_client_disconnects_and_no_panes() {
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

    // No panes, no clients, had_client=true — should exit.
    assert!(
        server.should_exit(),
        "should exit when no clients and no panes after grace"
    );
}

// -- SpawnPane dispatch --

/// SpawnPane returns a `SpawnPaneResponse` with a valid pane ID.
///
/// Note: this test may fail in environments without a working PTY (e.g.
/// CI containers). If so, the server returns `MuxPdu::Error` which we
/// also accept.
#[test]
fn spawn_pane_roundtrip() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // Spawn a pane.
    send_pdu(
        &mut client,
        2,
        &MuxPdu::SpawnPane {
            shell: None,
            cwd: None,
            theme: None,
        },
    );
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
    // PTY spawn may fail in test environments, so accept either response.
    assert!(
        matches!(
            resp,
            MuxPdu::SpawnPaneResponse { .. } | MuxPdu::Error { .. }
        ),
        "expected SpawnPaneResponse or Error, got {resp:?}"
    );
}

// -- ListPanes dispatch --

/// ListPanes on an empty server returns an empty list.
#[test]
fn list_panes_empty() {
    let (_dir, mut server, mut client) = server_with_client();

    // Handshake.
    send_pdu(&mut client, 1, &MuxPdu::Hello { pid: 1 });
    poll_and_dispatch(&mut server);
    let _ = recv_pdu(&mut client);

    // List panes (should be empty).
    send_pdu(&mut client, 2, &MuxPdu::ListPanes);
    poll_and_dispatch(&mut server);

    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
    match resp {
        MuxPdu::ListPanesResponse { pane_ids } => {
            assert!(pane_ids.is_empty(), "no panes should exist yet");
        }
        other => panic!("expected ListPanesResponse, got {other:?}"),
    }
}

// -- parse_theme roundtrip --

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
    send_pdu(&mut client, 2, &MuxPdu::Ping);
    poll_and_dispatch(&mut server);
    let (seq, resp) = recv_pdu(&mut client);
    assert_eq!(seq, 2);
    assert_eq!(resp, MuxPdu::PingAck);
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

    // Both list panes simultaneously.
    send_pdu(&mut c1, 2, &MuxPdu::ListPanes);
    send_pdu(&mut c2, 2, &MuxPdu::ListPanes);
    poll_and_dispatch(&mut server);

    let (_, r1) = recv_pdu(&mut c1);
    let (_, r2) = recv_pdu(&mut c2);
    match (r1, r2) {
        (MuxPdu::ListPanesResponse { pane_ids: a }, MuxPdu::ListPanesResponse { pane_ids: b }) => {
            assert_eq!(a.len(), b.len(), "both clients should see same pane count");
        }
        other => panic!("expected ListPanesResponse from both, got {other:?}"),
    }
}

// -- Handshake rejection --

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

// -- FrameReader forward-compat tests --

/// Unknown msg_type with payload: full frame consumed, stream stays aligned.
#[test]
fn frame_reader_forward_compat_skips_unknown_and_stays_aligned() {
    let mut data = Vec::new();

    // Frame 1: unknown msg_type 0xFFFF with 8-byte payload.
    let header1 = FrameHeader {
        msg_type: 0xFFFF,
        seq: 0,
        payload_len: 8,
    };
    data.extend_from_slice(&header1.encode());
    data.extend_from_slice(&[0xDE; 8]);

    // Frame 2: valid Ping.
    ProtocolCodec::encode_frame(&mut data, 99, &MuxPdu::Ping).unwrap();

    let mut reader = FrameReader::new();
    reader.extend(&data);

    // First decode: UnknownMsgType.
    let err = reader.try_decode().unwrap().unwrap_err();
    assert!(matches!(
        err,
        crate::protocol::DecodeError::UnknownMsgType(0xFFFF)
    ));

    // Second decode: valid Ping (stream aligned).
    let frame = reader.try_decode().unwrap().unwrap();
    assert_eq!(frame.seq, 99);
    assert!(matches!(frame.pdu, MuxPdu::Ping));
}

/// FrameReader waits for full unknown frame before draining.
#[test]
fn frame_reader_forward_compat_waits_for_full_unknown_frame() {
    let header = FrameHeader {
        msg_type: 0xFFFF,
        seq: 0,
        payload_len: 20,
    };

    let mut reader = FrameReader::new();

    // Feed only the header.
    reader.extend(&header.encode());
    assert!(reader.try_decode().is_none(), "should wait for payload");

    // Feed partial payload (10 of 20 bytes).
    reader.extend(&[0u8; 10]);
    assert!(
        reader.try_decode().is_none(),
        "should wait for full payload"
    );

    // Feed remaining 10 bytes.
    reader.extend(&[0u8; 10]);
    let err = reader.try_decode().unwrap().unwrap_err();
    assert!(matches!(
        err,
        crate::protocol::DecodeError::UnknownMsgType(0xFFFF)
    ));
}
