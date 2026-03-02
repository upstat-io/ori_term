use std::os::unix::net::UnixStream;
use std::sync::atomic::Ordering;

use crate::{MuxPdu, ProtocolCodec};

use super::MuxServer;
use super::frame_io::FrameReader;
use super::ipc::IpcListener;
use super::pid_file::{PidFile, read_pid};

/// Helper: create a server, connect a client, and accept it.
fn server_with_client() -> (tempfile::TempDir, MuxServer, UnixStream) {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("test.sock");
    let pid_path = dir.path().join("test.pid");
    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    let client = UnixStream::connect(&sock_path).unwrap();

    let mut events = mio::Events::with_capacity(16);
    server
        .poll
        .poll(&mut events, Some(std::time::Duration::from_millis(50)))
        .unwrap();
    server.accept_connections().unwrap();

    (dir, server, client)
}

/// Helper: send a PDU to a stream and flush.
fn send_pdu(stream: &mut UnixStream, seq: u32, pdu: &MuxPdu) {
    ProtocolCodec::encode_frame(stream, seq, pdu).unwrap();
}

/// Helper: read a response PDU from a stream.
fn recv_pdu(stream: &mut UnixStream) -> (u32, MuxPdu) {
    let frame = ProtocolCodec::decode_frame(stream).unwrap();
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
    client.set_nonblocking(false).unwrap();

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
    client.set_nonblocking(false).unwrap();

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
    client.set_nonblocking(false).unwrap();

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
    client.set_nonblocking(false).unwrap();

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
    client.set_nonblocking(false).unwrap();

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
    client.set_nonblocking(false).unwrap();

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
    client.set_nonblocking(false).unwrap();

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
    client.set_nonblocking(false).unwrap();

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
    client.set_nonblocking(false).unwrap();

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
    let sock_path = dir.path().join("grace.sock");
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
    let sock_path = dir.path().join("noclient.sock");
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
    let sock_path = dir.path().join("exit.sock");
    let pid_path = dir.path().join("exit.pid");

    let mut server = MuxServer::with_paths(&sock_path, &pid_path).unwrap();

    // Connect a client.
    let client = UnixStream::connect(&sock_path).unwrap();
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
