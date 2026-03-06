//! Tests for MuxClient.

use super::MuxClient;
use crate::PaneId;
use crate::backend::MuxBackend;
use crate::mux_event::MuxNotification;

// -- Existing tests (stub behavior) --

/// `MuxClient` implements `MuxBackend` (compile check via object safety).
#[test]
fn object_safe() {
    let client = MuxClient::new();
    let _boxed: Box<dyn MuxBackend> = Box::new(client);
}

/// `drain_notifications` returns empty initially.
#[test]
fn drain_empty() {
    let mut client = MuxClient::new();
    let mut buf = Vec::new();
    client.drain_notifications(&mut buf);
    assert!(buf.is_empty());
}

/// `poll_events` is a no-op without transport and doesn't panic.
#[test]
fn poll_events_noop() {
    let mut client = MuxClient::new();
    client.poll_events();
}

/// `drain_notifications` returns injected notifications in order.
#[test]
fn drain_returns_injected_notifications() {
    let mut client = MuxClient::new();
    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);

    client.notifications.push(MuxNotification::PaneOutput(p1));
    client.notifications.push(MuxNotification::PaneClosed(p2));

    let mut buf = Vec::new();
    client.drain_notifications(&mut buf);

    assert_eq!(buf.len(), 2);
    assert!(matches!(buf[0], MuxNotification::PaneOutput(id) if id == p1));
    assert!(matches!(buf[1], MuxNotification::PaneClosed(id) if id == p2));

    // Buffer should be empty after drain.
    let mut buf2 = Vec::new();
    client.drain_notifications(&mut buf2);
    assert!(buf2.is_empty());
}

/// `discard_notifications` clears injected notifications.
#[test]
fn discard_clears_notifications() {
    let mut client = MuxClient::new();
    client
        .notifications
        .push(MuxNotification::PaneOutput(PaneId::from_raw(1)));

    client.discard_notifications();

    let mut buf = Vec::new();
    client.drain_notifications(&mut buf);
    assert!(buf.is_empty());
}

/// `is_daemon_mode` returns true for MuxClient.
#[test]
fn is_daemon_mode() {
    let client = MuxClient::new();
    assert!(client.is_daemon_mode());
}

/// `event_tx` returns `None` in client mode.
#[test]
fn event_tx_none() {
    let client = MuxClient::new();
    assert!(client.event_tx().is_none());
}

/// `pane_ids` returns empty in client mode.
#[test]
fn pane_ids_empty() {
    let client = MuxClient::new();
    assert!(client.pane_ids().is_empty());
}

// -- Snapshot cache tests --

/// Helper: build a minimal `PaneSnapshot` for cache tests.
fn test_snapshot(title: &str) -> crate::PaneSnapshot {
    use crate::protocol::{WireCell, WireCursor, WireCursorShape, WireRgb};

    crate::PaneSnapshot {
        cells: vec![vec![WireCell {
            ch: 'X',
            fg: WireRgb {
                r: 200,
                g: 200,
                b: 200,
            },
            bg: WireRgb { r: 0, g: 0, b: 0 },
            flags: 0,
            underline_color: None,
            hyperlink_uri: None,
            zerowidth: vec![],
        }]],
        cursor: WireCursor {
            col: 0,
            row: 0,
            shape: WireCursorShape::Block,
            visible: true,
        },
        palette: vec![[0, 0, 0]; 270],
        title: title.into(),
        icon_name: None,
        cwd: None,
        modes: 0,
        scrollback_len: 0,
        display_offset: 0,
        stable_row_base: 0,
        cols: 1,
        search_active: false,
        search_query: String::new(),
        search_matches: Vec::new(),
        search_focused: None,
        search_total_matches: 0,
    }
}

/// `cache_snapshot` makes a snapshot retrievable via `pane_snapshot`.
#[test]
fn cache_snapshot_then_retrieve() {
    let mut client = MuxClient::new();
    let p = PaneId::from_raw(1);
    let snap = test_snapshot("cached");

    client.cache_snapshot(p, snap);

    let got = client.pane_snapshot(p);
    assert!(got.is_some());
    assert_eq!(got.unwrap().title, "cached");
}

/// `cache_snapshot` overwrites a previously cached snapshot for the same pane.
#[test]
fn cache_snapshot_overwrites() {
    let mut client = MuxClient::new();
    let p = PaneId::from_raw(1);

    client.cache_snapshot(p, test_snapshot("first"));
    client.cache_snapshot(p, test_snapshot("second"));

    assert_eq!(client.pane_snapshot(p).unwrap().title, "second");
}

/// `pane_snapshot` returns `None` for an uncached pane.
#[test]
fn pane_snapshot_returns_none_for_unknown() {
    let client = MuxClient::new();
    assert!(client.pane_snapshot(PaneId::from_raw(999)).is_none());
}

/// `remove_snapshot` evicts the cached snapshot and clears dirty.
#[test]
fn remove_snapshot_evicts_and_clears_dirty() {
    let mut client = MuxClient::new();
    let p = PaneId::from_raw(1);

    client.cache_snapshot(p, test_snapshot("doomed"));
    client.dirty_panes.insert(p);

    client.remove_snapshot(p);

    assert!(client.pane_snapshot(p).is_none());
    assert!(!client.is_pane_snapshot_dirty(p));
}

/// `remove_snapshot` on an unknown pane is a no-op.
#[test]
fn remove_snapshot_noop_for_unknown() {
    let mut client = MuxClient::new();
    // Should not panic.
    client.remove_snapshot(PaneId::from_raw(999));
}

// -- Dirty tracking tests --

/// `is_pane_snapshot_dirty` returns `false` initially.
#[test]
fn dirty_initially_false() {
    let client = MuxClient::new();
    assert!(!client.is_pane_snapshot_dirty(PaneId::from_raw(1)));
}

/// Dirty flag set → cleared lifecycle.
#[test]
fn dirty_set_then_clear() {
    let mut client = MuxClient::new();
    let p = PaneId::from_raw(1);

    client.dirty_panes.insert(p);
    assert!(client.is_pane_snapshot_dirty(p));

    client.clear_pane_snapshot_dirty(p);
    assert!(!client.is_pane_snapshot_dirty(p));
}

/// `clear_pane_snapshot_dirty` on an already-clean pane is a no-op.
#[test]
fn clear_dirty_idempotent() {
    let mut client = MuxClient::new();
    let p = PaneId::from_raw(1);

    // Clear without ever setting — should not panic.
    client.clear_pane_snapshot_dirty(p);
    assert!(!client.is_pane_snapshot_dirty(p));
}

/// `poll_events` marks panes dirty from `PaneOutput` notifications.
#[test]
fn poll_events_marks_dirty() {
    let mut client = MuxClient::new();
    let p = PaneId::from_raw(5);

    client.notifications.push(MuxNotification::PaneOutput(p));
    client.poll_events();

    assert!(client.is_pane_snapshot_dirty(p));
}

/// Multiple dirty notifications for the same pane don't corrupt state.
#[test]
fn duplicate_dirty_notifications() {
    let mut client = MuxClient::new();
    let p = PaneId::from_raw(1);

    client.notifications.push(MuxNotification::PaneOutput(p));
    client.notifications.push(MuxNotification::PaneOutput(p));
    client.notifications.push(MuxNotification::PaneOutput(p));
    client.poll_events();

    assert!(client.is_pane_snapshot_dirty(p));

    // A single clear removes the flag.
    client.clear_pane_snapshot_dirty(p);
    assert!(!client.is_pane_snapshot_dirty(p));
}

/// Non-PaneOutput notifications don't set dirty flags.
#[test]
fn non_dirty_notifications_ignored() {
    let mut client = MuxClient::new();
    let p = PaneId::from_raw(1);

    client.notifications.push(MuxNotification::PaneClosed(p));
    client
        .notifications
        .push(MuxNotification::PaneTitleChanged(p));
    client.poll_events();

    assert!(!client.is_pane_snapshot_dirty(p));
}

/// `refresh_pane_snapshot` on unconnected stub returns `None`.
#[test]
fn refresh_snapshot_stub_returns_none() {
    let mut client = MuxClient::new();
    assert!(client.refresh_pane_snapshot(PaneId::from_raw(1)).is_none());
}

/// Double refresh without dirty notification is safe (idempotent).
#[test]
fn refresh_snapshot_idempotent() {
    let mut client = MuxClient::new();
    let p = PaneId::from_raw(1);

    // Both return None on stub, but neither panics.
    assert!(client.refresh_pane_snapshot(p).is_none());
    assert!(client.refresh_pane_snapshot(p).is_none());
}

// -- MuxBackend default trait method tests --

/// Default `pane_snapshot` returns `None` (embedded mode behavior).
#[test]
fn trait_default_pane_snapshot_none() {
    use std::sync::Arc;

    use crate::backend::embedded::EmbeddedMux;

    let mux = EmbeddedMux::new(Arc::new(|| {}));
    let backend: &dyn MuxBackend = &mux;
    assert!(backend.pane_snapshot(PaneId::from_raw(1)).is_none());
}

/// Default `is_pane_snapshot_dirty` returns `false`.
#[test]
fn trait_default_is_dirty_false() {
    use std::sync::Arc;

    use crate::backend::embedded::EmbeddedMux;

    let mux = EmbeddedMux::new(Arc::new(|| {}));
    let backend: &dyn MuxBackend = &mux;
    assert!(!backend.is_pane_snapshot_dirty(PaneId::from_raw(1)));
}

/// Default `clear_pane_snapshot_dirty` is a no-op (no panic).
#[test]
fn trait_default_clear_dirty_noop() {
    use std::sync::Arc;

    use crate::backend::embedded::EmbeddedMux;

    let mut mux = EmbeddedMux::new(Arc::new(|| {}));
    let backend: &mut dyn MuxBackend = &mut mux;
    backend.clear_pane_snapshot_dirty(PaneId::from_raw(1));
}

// -- Send compile check --

/// `MuxClient` satisfies `Send` (prevents accidental `Rc`/`Cell` additions).
#[test]
fn mux_client_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<MuxClient>();
}

// -- Transport tests (Unix only, using UnixStream::pair) --

#[cfg(unix)]
mod transport_tests {
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use crate::id::{ClientId, PaneId};
    use crate::mux_event::MuxNotification;
    use crate::protocol::{MuxPdu, ProtocolCodec};

    use super::super::notification::pdu_to_notification;
    use super::super::transport::ClientTransport;

    /// Consume the `SetCapabilities` frame that the client sends after Hello.
    ///
    /// Test servers must call this after writing `HelloAck` to stay in sync
    /// with the client's handshake sequence.
    fn consume_set_capabilities(stream: &mut UnixStream) {
        let _ = ProtocolCodec::new().decode_frame(stream);
    }

    // -- Notification conversion tests --

    /// `NotifyPaneOutput` is handled in the reader loop, not `pdu_to_notification`.
    #[test]
    fn notify_pane_output_handled_in_reader_loop() {
        let pdu = MuxPdu::NotifyPaneOutput {
            pane_id: PaneId::from_raw(1),
        };
        // NotifyPaneOutput is intercepted in the reader loop (dispatch_notification),
        // so pdu_to_notification returns None for it.
        assert!(pdu_to_notification(pdu).is_none());
    }

    /// `NotifyPaneExited` converts to `PaneClosed`.
    #[test]
    fn notify_pane_exited() {
        let pdu = MuxPdu::NotifyPaneExited {
            pane_id: PaneId::from_raw(2),
        };
        let notif = pdu_to_notification(pdu).unwrap();
        assert!(matches!(notif, MuxNotification::PaneClosed(id) if id == PaneId::from_raw(2)));
    }

    /// `NotifyPaneTitleChanged` converts to `PaneTitleChanged`.
    #[test]
    fn notify_pane_title() {
        let pdu = MuxPdu::NotifyPaneTitleChanged {
            pane_id: PaneId::from_raw(3),
            title: "hello".into(),
        };
        let notif = pdu_to_notification(pdu).unwrap();
        assert!(
            matches!(notif, MuxNotification::PaneTitleChanged(id) if id == PaneId::from_raw(3))
        );
    }

    /// `NotifyPaneBell` converts to `Alert`.
    #[test]
    fn notify_bell() {
        let pdu = MuxPdu::NotifyPaneBell {
            pane_id: PaneId::from_raw(4),
        };
        let notif = pdu_to_notification(pdu).unwrap();
        assert!(matches!(notif, MuxNotification::PaneBell(id) if id == PaneId::from_raw(4)));
    }

    /// Non-notification PDUs return `None`.
    #[test]
    fn non_notification_returns_none() {
        let pdu = MuxPdu::PingAck;
        assert!(pdu_to_notification(pdu).is_none());
    }

    // -- Codec roundtrip tests (transport layer uses these) --

    /// Write a frame, read it back — basic roundtrip.
    #[test]
    fn codec_roundtrip_over_socket() {
        let (mut a, mut b) = UnixStream::pair().unwrap();

        let pdu = MuxPdu::Ping;
        ProtocolCodec::encode_frame(&mut a, 7, &pdu).unwrap();

        let frame = ProtocolCodec::new().decode_frame(&mut b).unwrap();
        assert_eq!(frame.seq, 7);
        assert!(matches!(frame.pdu, MuxPdu::Ping));
    }

    /// Multiple frames round-trip in order.
    #[test]
    fn multiple_frames_in_order() {
        let (mut a, mut b) = UnixStream::pair().unwrap();

        ProtocolCodec::encode_frame(&mut a, 1, &MuxPdu::Ping).unwrap();
        ProtocolCodec::encode_frame(&mut a, 2, &MuxPdu::ListPanes).unwrap();

        let f1 = ProtocolCodec::new().decode_frame(&mut b).unwrap();
        let f2 = ProtocolCodec::new().decode_frame(&mut b).unwrap();

        assert_eq!(f1.seq, 1);
        assert!(matches!(f1.pdu, MuxPdu::Ping));
        assert_eq!(f2.seq, 2);
        assert!(matches!(f2.pdu, MuxPdu::ListPanes));
    }

    // -- Integration tests using real ClientTransport::connect --

    /// Full connect → handshake over a temp socket.
    #[test]
    fn connect_handshake() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");

        // Start a fake server on the socket.
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let wakeup_count = Arc::new(AtomicUsize::new(0));
        let wc = wakeup_count.clone();

        let server_handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Read Hello.
            let frame = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            assert!(matches!(frame.pdu, MuxPdu::Hello { .. }));
            // Write HelloAck.
            ProtocolCodec::encode_frame(
                &mut stream,
                frame.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(99),
                },
            )
            .unwrap();
            consume_set_capabilities(&mut stream);
            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            wc.fetch_add(1, Ordering::Relaxed);
        });

        let transport = ClientTransport::connect(&sock, wakeup).unwrap();
        assert_eq!(transport.client_id(), ClientId::from_raw(99));
        assert!(transport.is_alive());

        let _server_stream = server_handle.join().unwrap();
    }

    /// RPC roundtrip: send Ping, receive PingAck.
    #[test]
    fn rpc_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("rpc.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let server_handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake.
            let hello = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();
            consume_set_capabilities(&mut stream);

            // Read Ping request.
            let req = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            assert!(matches!(req.pdu, MuxPdu::Ping));

            // Reply with PingAck.
            ProtocolCodec::encode_frame(&mut stream, req.seq, &MuxPdu::PingAck).unwrap();

            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut transport = ClientTransport::connect(&sock, wakeup).unwrap();

        let resp = transport.rpc(MuxPdu::Ping).unwrap();
        assert!(matches!(resp, MuxPdu::PingAck));

        let _s = server_handle.join().unwrap();
    }

    /// Notification delivery: server pushes NotifyPaneOutput with seq=0.
    #[test]
    fn notification_delivery() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("notif.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let wakeup_count = Arc::new(AtomicUsize::new(0));
        let wc = wakeup_count.clone();

        let server_handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake.
            let hello = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();
            consume_set_capabilities(&mut stream);

            // Push a notification (seq=0).
            ProtocolCodec::encode_frame(
                &mut stream,
                0,
                &MuxPdu::NotifyPaneOutput {
                    pane_id: PaneId::from_raw(7),
                },
            )
            .unwrap();

            // Keep connection alive briefly.
            std::thread::sleep(Duration::from_millis(200));
            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            wc.fetch_add(1, Ordering::Relaxed);
        });
        let transport = ClientTransport::connect(&sock, wakeup).unwrap();

        // Wait a bit for the reader thread to receive the notification.
        std::thread::sleep(Duration::from_millis(100));

        let mut notifications = Vec::new();
        transport.poll_notifications(&mut notifications);

        assert!(
            !notifications.is_empty(),
            "expected at least one notification"
        );
        assert!(
            matches!(notifications[0], MuxNotification::PaneOutput(id) if id == PaneId::from_raw(7))
        );
        assert!(wakeup_count.load(Ordering::Relaxed) > 0);

        let _s = server_handle.join().unwrap();
    }

    /// Disconnect detection: when server drops the connection, transport detects it.
    #[test]
    fn disconnect_detection() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("disconnect.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let server_handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake.
            let hello = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();
            consume_set_capabilities(&mut stream);

            // Drop the stream immediately to simulate disconnect.
            drop(stream);
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let transport = ClientTransport::connect(&sock, wakeup).unwrap();

        server_handle.join().unwrap();

        // Wait for reader thread to detect EOF.
        std::thread::sleep(Duration::from_millis(100));

        assert!(!transport.is_alive(), "transport should detect disconnect");
    }

    /// RPC timeout: server never responds, client times out.
    #[test]
    fn rpc_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("timeout.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let server_handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake.
            let hello = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();
            consume_set_capabilities(&mut stream);

            // Read the request but never respond — let client timeout.
            let _req = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            // Keep connection alive while client waits (just past the 5s RPC timeout).
            std::thread::sleep(Duration::from_secs(6));
            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut transport = ClientTransport::connect(&sock, wakeup).unwrap();

        let result = transport.rpc(MuxPdu::Ping);
        assert!(result.is_err(), "should timeout");
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);

        // Clean up: server thread will exit when stream drops.
        drop(transport);
        let _ = server_handle.join();
    }

    /// Sequence number wraps from u32::MAX to 1, skipping 0.
    #[test]
    fn seq_wraparound_skips_zero() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("wrap.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let received_seqs = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seqs = received_seqs.clone();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake.
            let hello = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();
            consume_set_capabilities(&mut stream);

            // Read 3 requests, record their seqs, respond to each.
            for _ in 0..3 {
                let frame = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
                seqs.lock().unwrap().push(frame.seq);
                ProtocolCodec::encode_frame(&mut stream, frame.seq, &MuxPdu::PingAck).unwrap();
            }
            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut transport = ClientTransport::connect(&sock, wakeup).unwrap();

        // Set next_seq near wraparound point.
        transport.test_set_next_seq(u32::MAX - 1);

        // Three RPCs: seqs should be MAX-1, MAX, 1 (skipping 0).
        transport.rpc(MuxPdu::Ping).unwrap();
        transport.rpc(MuxPdu::Ping).unwrap();
        transport.rpc(MuxPdu::Ping).unwrap();

        let _s = server.join().unwrap();

        let seqs = received_seqs.lock().unwrap();
        assert_eq!(*seqs, vec![u32::MAX - 1, u32::MAX, 1]);
    }

    /// RPC on a dead transport returns `NotConnected` immediately.
    #[test]
    fn rpc_on_dead_transport() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("dead.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake only, then drop.
            let hello = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();
            consume_set_capabilities(&mut stream);
            drop(stream);
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut transport = ClientTransport::connect(&sock, wakeup).unwrap();

        server.join().unwrap();

        // Wait for reader thread to detect EOF.
        std::thread::sleep(Duration::from_millis(100));

        assert!(!transport.is_alive());

        let result = transport.rpc(MuxPdu::Ping);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotConnected);
    }

    /// Notification arrives during an active RPC — both are correctly routed.
    #[test]
    fn notification_during_rpc() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("interleave.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let wakeup_count = Arc::new(AtomicUsize::new(0));
        let wc = wakeup_count.clone();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake.
            let hello = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();
            consume_set_capabilities(&mut stream);

            // Read the RPC request.
            let req = ProtocolCodec::new().decode_frame(&mut stream).unwrap();

            // Send a notification BEFORE the RPC response.
            ProtocolCodec::encode_frame(
                &mut stream,
                0,
                &MuxPdu::NotifyPaneOutput {
                    pane_id: PaneId::from_raw(42),
                },
            )
            .unwrap();

            // Now send the RPC response.
            ProtocolCodec::encode_frame(&mut stream, req.seq, &MuxPdu::PingAck).unwrap();

            // Keep alive briefly.
            std::thread::sleep(Duration::from_millis(200));
            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            wc.fetch_add(1, Ordering::Relaxed);
        });
        let mut transport = ClientTransport::connect(&sock, wakeup).unwrap();

        // RPC should succeed despite interleaved notification.
        let resp = transport.rpc(MuxPdu::Ping).unwrap();
        assert!(matches!(resp, MuxPdu::PingAck));

        // Notification should be in the buffer.
        std::thread::sleep(Duration::from_millis(50));
        let mut notifications = Vec::new();
        transport.poll_notifications(&mut notifications);
        assert!(
            notifications.iter().any(
                |n| matches!(n, MuxNotification::PaneOutput(id) if *id == PaneId::from_raw(42))
            )
        );

        let _s = server.join().unwrap();
    }

    /// Unknown PDU from daemon is skipped (forward-compat), transport stays alive.
    #[test]
    fn unknown_pdu_skipped() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("bogus.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake.
            let hello = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();
            consume_set_capabilities(&mut stream);

            // Send a frame with an unknown msg type + 4-byte payload.
            let header = crate::protocol::FrameHeader {
                msg_type: 0xFFFF,
                seq: 0,
                payload_len: 4,
            };
            stream.write_all(&header.encode()).unwrap();
            stream.write_all(&[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();

            // Keep alive briefly.
            std::thread::sleep(Duration::from_millis(200));
            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let transport = ClientTransport::connect(&sock, wakeup).unwrap();

        // Wait for reader thread to skip the unknown PDU.
        std::thread::sleep(Duration::from_millis(100));

        assert!(transport.is_alive(), "transport should survive unknown PDU");

        let _s = server.join().unwrap();
    }

    /// Burst of 10 notifications arrive in FIFO order.
    #[test]
    fn notification_burst_ordering() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("burst.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake.
            let hello = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();
            consume_set_capabilities(&mut stream);

            // Send 10 notifications in rapid succession.
            for i in 1..=10u64 {
                ProtocolCodec::encode_frame(
                    &mut stream,
                    0,
                    &MuxPdu::NotifyPaneOutput {
                        pane_id: PaneId::from_raw(i),
                    },
                )
                .unwrap();
            }

            // Keep alive.
            std::thread::sleep(Duration::from_millis(200));
            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let transport = ClientTransport::connect(&sock, wakeup).unwrap();

        // Wait for all notifications to arrive.
        std::thread::sleep(Duration::from_millis(150));

        let mut notifications = Vec::new();
        transport.poll_notifications(&mut notifications);
        assert_eq!(notifications.len(), 10, "expected 10 notifications");

        // Verify FIFO ordering.
        for (i, notif) in notifications.iter().enumerate() {
            let expected_id = PaneId::from_raw((i + 1) as u64);
            assert!(
                matches!(notif, MuxNotification::PaneOutput(id) if *id == expected_id),
                "notification {i} should be PaneOutput({expected_id:?}), got {notif:?}"
            );
        }

        let _s = server.join().unwrap();
    }

    /// Error response from daemon is surfaced as io::Error.
    #[test]
    fn error_response() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("error.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let server_handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake.
            let hello = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();
            consume_set_capabilities(&mut stream);

            // Read request, respond with Error.
            let req = ProtocolCodec::new().decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                req.seq,
                &MuxPdu::Error {
                    message: "test error".into(),
                },
            )
            .unwrap();

            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut transport = ClientTransport::connect(&sock, wakeup).unwrap();

        let result = transport.rpc(MuxPdu::Ping);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("test error"));

        let _s = server_handle.join().unwrap();
    }
}
