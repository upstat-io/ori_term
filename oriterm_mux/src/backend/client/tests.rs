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

/// `pane()` returns `None` in client mode.
#[test]
fn pane_returns_none() {
    let client = MuxClient::new();
    assert!(client.pane(PaneId::from_raw(1)).is_none());
}

/// `pane_mut()` returns `None` in client mode.
#[test]
fn pane_mut_returns_none() {
    let mut client = MuxClient::new();
    assert!(client.pane_mut(PaneId::from_raw(1)).is_none());
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

    client.notifications.push(MuxNotification::PaneDirty(p1));
    client.notifications.push(MuxNotification::PaneClosed(p2));

    let mut buf = Vec::new();
    client.drain_notifications(&mut buf);

    assert_eq!(buf.len(), 2);
    assert!(matches!(buf[0], MuxNotification::PaneDirty(id) if id == p1));
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
        .push(MuxNotification::PaneDirty(PaneId::from_raw(1)));

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

// -- claim_window / refresh_window_tabs stubs --

/// `claim_window` on an unconnected stub returns an error (no panic).
#[test]
fn claim_window_stub_returns_error() {
    use crate::WindowId;

    let mut client = MuxClient::new();
    let result = client.claim_window(WindowId::from_raw(1));
    assert!(result.is_err());
}

/// `refresh_window_tabs` on an unconnected stub is a no-op (no panic).
#[test]
fn refresh_window_tabs_stub_noop() {
    use crate::WindowId;

    let mut client = MuxClient::new();
    // Should not panic — just logs an error internally.
    client.refresh_window_tabs(WindowId::from_raw(1));
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

    use crate::id::{ClientId, PaneId, WindowId};
    use crate::mux_event::MuxNotification;
    use crate::protocol::{MuxPdu, ProtocolCodec};

    use super::super::notification::pdu_to_notification;
    use super::super::transport::ClientTransport;

    // -- Notification conversion tests --

    /// `NotifyPaneOutput` converts to `PaneDirty`.
    #[test]
    fn notify_pane_output() {
        let pdu = MuxPdu::NotifyPaneOutput {
            pane_id: PaneId::from_raw(1),
        };
        let notif = pdu_to_notification(pdu).unwrap();
        assert!(matches!(notif, MuxNotification::PaneDirty(id) if id == PaneId::from_raw(1)));
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
        assert!(matches!(notif, MuxNotification::Alert(id) if id == PaneId::from_raw(4)));
    }

    /// `NotifyWindowTabsChanged` converts to `WindowTabsChanged`.
    #[test]
    fn notify_window_tabs() {
        let pdu = MuxPdu::NotifyWindowTabsChanged {
            window_id: WindowId::from_raw(5),
        };
        let notif = pdu_to_notification(pdu).unwrap();
        assert!(
            matches!(notif, MuxNotification::WindowTabsChanged(id) if id == WindowId::from_raw(5))
        );
    }

    /// `NotifyTabMoved` returns `None` (no direct equivalent).
    #[test]
    fn notify_tab_moved_none() {
        let pdu = MuxPdu::NotifyTabMoved {
            tab_id: crate::TabId::from_raw(1),
            from_window: WindowId::from_raw(1),
            to_window: WindowId::from_raw(2),
        };
        assert!(pdu_to_notification(pdu).is_none());
    }

    /// Non-notification PDUs return `None`.
    #[test]
    fn non_notification_returns_none() {
        let pdu = MuxPdu::WindowCreated {
            window_id: WindowId::from_raw(1),
        };
        assert!(pdu_to_notification(pdu).is_none());
    }

    // -- Codec roundtrip tests (transport layer uses these) --

    /// Write a frame, read it back — basic roundtrip.
    #[test]
    fn codec_roundtrip_over_socket() {
        let (mut a, mut b) = UnixStream::pair().unwrap();

        let pdu = MuxPdu::CreateWindow;
        ProtocolCodec::encode_frame(&mut a, 7, &pdu).unwrap();

        let frame = ProtocolCodec::decode_frame(&mut b).unwrap();
        assert_eq!(frame.seq, 7);
        assert!(matches!(frame.pdu, MuxPdu::CreateWindow));
    }

    /// Multiple frames round-trip in order.
    #[test]
    fn multiple_frames_in_order() {
        let (mut a, mut b) = UnixStream::pair().unwrap();

        ProtocolCodec::encode_frame(&mut a, 1, &MuxPdu::CreateWindow).unwrap();
        ProtocolCodec::encode_frame(
            &mut a,
            2,
            &MuxPdu::CreateTab {
                window_id: WindowId::from_raw(1),
                shell: None,
                cwd: None,
                theme: None,
            },
        )
        .unwrap();

        let f1 = ProtocolCodec::decode_frame(&mut b).unwrap();
        let f2 = ProtocolCodec::decode_frame(&mut b).unwrap();

        assert_eq!(f1.seq, 1);
        assert!(matches!(f1.pdu, MuxPdu::CreateWindow));
        assert_eq!(f2.seq, 2);
        assert!(matches!(f2.pdu, MuxPdu::CreateTab { .. }));
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
            let frame = ProtocolCodec::decode_frame(&mut stream).unwrap();
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

    /// RPC roundtrip: send CreateWindow, receive WindowCreated.
    #[test]
    fn rpc_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("rpc.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let server_handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake.
            let hello = ProtocolCodec::decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();

            // Read CreateWindow request.
            let req = ProtocolCodec::decode_frame(&mut stream).unwrap();
            assert!(matches!(req.pdu, MuxPdu::CreateWindow));

            // Reply with WindowCreated.
            ProtocolCodec::encode_frame(
                &mut stream,
                req.seq,
                &MuxPdu::WindowCreated {
                    window_id: WindowId::from_raw(10),
                },
            )
            .unwrap();

            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut transport = ClientTransport::connect(&sock, wakeup).unwrap();

        let resp = transport.rpc(MuxPdu::CreateWindow).unwrap();
        assert!(matches!(
            resp,
            MuxPdu::WindowCreated { window_id } if window_id == WindowId::from_raw(10)
        ));

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
            let hello = ProtocolCodec::decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();

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
            matches!(notifications[0], MuxNotification::PaneDirty(id) if id == PaneId::from_raw(7))
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
            let hello = ProtocolCodec::decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();

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
            let hello = ProtocolCodec::decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();

            // Read the request but never respond — let client timeout.
            let _req = ProtocolCodec::decode_frame(&mut stream).unwrap();
            // Keep connection alive while client waits (just past the 5s RPC timeout).
            std::thread::sleep(Duration::from_secs(6));
            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut transport = ClientTransport::connect(&sock, wakeup).unwrap();

        let result = transport.rpc(MuxPdu::CreateWindow);
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
            let hello = ProtocolCodec::decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();

            // Read 3 requests, record their seqs, respond to each.
            for _ in 0..3 {
                let frame = ProtocolCodec::decode_frame(&mut stream).unwrap();
                seqs.lock().unwrap().push(frame.seq);
                ProtocolCodec::encode_frame(
                    &mut stream,
                    frame.seq,
                    &MuxPdu::WindowCreated {
                        window_id: WindowId::from_raw(1),
                    },
                )
                .unwrap();
            }
            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut transport = ClientTransport::connect(&sock, wakeup).unwrap();

        // Set next_seq near wraparound point.
        transport.test_set_next_seq(u32::MAX - 1);

        // Three RPCs: seqs should be MAX-1, MAX, 1 (skipping 0).
        transport.rpc(MuxPdu::CreateWindow).unwrap();
        transport.rpc(MuxPdu::CreateWindow).unwrap();
        transport.rpc(MuxPdu::CreateWindow).unwrap();

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
            let hello = ProtocolCodec::decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();
            drop(stream);
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut transport = ClientTransport::connect(&sock, wakeup).unwrap();

        server.join().unwrap();

        // Wait for reader thread to detect EOF.
        std::thread::sleep(Duration::from_millis(100));

        assert!(!transport.is_alive());

        let result = transport.rpc(MuxPdu::CreateWindow);
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
            let hello = ProtocolCodec::decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();

            // Read the RPC request.
            let req = ProtocolCodec::decode_frame(&mut stream).unwrap();

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
            ProtocolCodec::encode_frame(
                &mut stream,
                req.seq,
                &MuxPdu::WindowCreated {
                    window_id: WindowId::from_raw(5),
                },
            )
            .unwrap();

            // Keep alive briefly.
            std::thread::sleep(Duration::from_millis(200));
            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            wc.fetch_add(1, Ordering::Relaxed);
        });
        let mut transport = ClientTransport::connect(&sock, wakeup).unwrap();

        // RPC should succeed despite interleaved notification.
        let resp = transport.rpc(MuxPdu::CreateWindow).unwrap();
        assert!(matches!(
            resp,
            MuxPdu::WindowCreated { window_id } if window_id == WindowId::from_raw(5)
        ));

        // Notification should be in the buffer.
        std::thread::sleep(Duration::from_millis(50));
        let mut notifications = Vec::new();
        transport.poll_notifications(&mut notifications);
        assert!(
            notifications.iter().any(
                |n| matches!(n, MuxNotification::PaneDirty(id) if *id == PaneId::from_raw(42))
            )
        );

        let _s = server.join().unwrap();
    }

    /// Unknown PDU from daemon kills the transport.
    #[test]
    fn unknown_pdu_kills_transport() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("bogus.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake.
            let hello = ProtocolCodec::decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();

            // Send a frame with an unknown msg type.
            let header = crate::protocol::FrameHeader {
                msg_type: 0xFFFF,
                seq: 0,
                payload_len: 0,
            };
            stream.write_all(&header.encode()).unwrap();

            // Keep alive briefly.
            std::thread::sleep(Duration::from_millis(200));
            stream
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let transport = ClientTransport::connect(&sock, wakeup).unwrap();

        // Wait for reader thread to hit the unknown PDU error.
        std::thread::sleep(Duration::from_millis(100));

        assert!(!transport.is_alive(), "transport should die on unknown PDU");

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
            let hello = ProtocolCodec::decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();

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
                matches!(notif, MuxNotification::PaneDirty(id) if *id == expected_id),
                "notification {i} should be PaneDirty({expected_id:?}), got {notif:?}"
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
            let hello = ProtocolCodec::decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();

            // Read request, respond with Error.
            let req = ProtocolCodec::decode_frame(&mut stream).unwrap();
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

        let result = transport.rpc(MuxPdu::CreateWindow);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("test error"));

        let _s = server_handle.join().unwrap();
    }

    // -- MuxClient-level transport tests --
    //
    // These use MuxClient::connect() to exercise the full MuxBackend
    // trait methods through the IPC transport.

    use super::super::MuxClient;
    use crate::backend::MuxBackend;
    use crate::protocol::MuxTabInfo;

    /// Helper: start a fake server that handles Hello, then calls `handler`
    /// for subsequent requests. Returns the server thread handle.
    fn fake_server<F>(sock_path: &std::path::Path, handler: F) -> std::thread::JoinHandle<()>
    where
        F: FnOnce(&mut UnixStream) + Send + 'static,
    {
        let listener = std::os::unix::net::UnixListener::bind(sock_path).unwrap();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Handshake.
            let hello = ProtocolCodec::decode_frame(&mut stream).unwrap();
            ProtocolCodec::encode_frame(
                &mut stream,
                hello.seq,
                &MuxPdu::HelloAck {
                    client_id: ClientId::from_raw(1),
                },
            )
            .unwrap();

            handler(&mut stream);
        })
    }

    /// `claim_window` sends ClaimWindow and receives WindowClaimed.
    #[test]
    fn mux_client_claim_window() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("claim.sock");

        let server = fake_server(&sock, |stream| {
            let req = ProtocolCodec::decode_frame(stream).unwrap();
            assert!(matches!(req.pdu, MuxPdu::ClaimWindow { .. }));
            ProtocolCodec::encode_frame(stream, req.seq, &MuxPdu::WindowClaimed).unwrap();

            // Keep alive briefly.
            std::thread::sleep(Duration::from_millis(100));
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut client = MuxClient::connect(&sock, wakeup).unwrap();

        // Need to add the window to local_session first (claim_window
        // doesn't create the window — it just tells the daemon).
        let wid = WindowId::from_raw(42);
        client
            .local_session
            .add_window(crate::session::MuxWindow::new(wid));

        let result = client.claim_window(wid);
        assert!(result.is_ok(), "claim_window should succeed: {result:?}");

        server.join().unwrap();
    }

    /// `refresh_window_tabs` sends ListTabs and replaces local tab list.
    #[test]
    fn mux_client_refresh_window_tabs() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("refresh.sock");

        let t1 = crate::TabId::from_raw(10);
        let t2 = crate::TabId::from_raw(20);

        let server = fake_server(&sock, move |stream| {
            let req = ProtocolCodec::decode_frame(stream).unwrap();
            assert!(matches!(req.pdu, MuxPdu::ListTabs { .. }));
            ProtocolCodec::encode_frame(
                stream,
                req.seq,
                &MuxPdu::TabList {
                    tabs: vec![
                        MuxTabInfo {
                            tab_id: t1,
                            active_pane_id: PaneId::from_raw(100),
                            pane_count: 1,
                            title: "tab1".into(),
                        },
                        MuxTabInfo {
                            tab_id: t2,
                            active_pane_id: PaneId::from_raw(200),
                            pane_count: 1,
                            title: "tab2".into(),
                        },
                    ],
                },
            )
            .unwrap();

            // Keep alive.
            std::thread::sleep(Duration::from_millis(100));
        });

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut client = MuxClient::connect(&sock, wakeup).unwrap();

        // Set up local state: a window with one stale tab.
        let wid = WindowId::from_raw(42);
        let mut win = crate::session::MuxWindow::new(wid);
        win.add_tab(crate::TabId::from_raw(999)); // stale tab
        client.local_session.add_window(win);

        // refresh_window_tabs should replace local tabs with server's.
        client.refresh_window_tabs(wid);

        let win = client.local_session.get_window(wid).unwrap();
        assert_eq!(win.tabs(), &[t1, t2], "local tabs should match server");

        server.join().unwrap();
    }
}
