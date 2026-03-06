//! Tests for the IPC wire protocol.

use std::io::Cursor;

use super::codec::{DecodeError, DecodedFrame, ProtocolCodec};
use super::messages::{MsgType, MuxPdu};
use super::snapshot::{PaneSnapshot, WireCell, WireCursor, WireCursorShape, WireRgb};
use super::{FrameHeader, HEADER_LEN, MAX_PAYLOAD};
use crate::id::{ClientId, DomainId, PaneId};

// -- FrameHeader tests --

#[test]
fn header_roundtrip() {
    let header = FrameHeader {
        msg_type: 0x0103,
        seq: 42,
        payload_len: 1024,
    };
    let encoded = header.encode();
    assert_eq!(encoded.len(), HEADER_LEN);
    let decoded = FrameHeader::decode(&encoded);
    assert_eq!(header, decoded);
}

#[test]
fn header_zero_values() {
    let header = FrameHeader {
        msg_type: 0,
        seq: 0,
        payload_len: 0,
    };
    let decoded = FrameHeader::decode(&header.encode());
    assert_eq!(header, decoded);
}

#[test]
fn header_max_values() {
    let header = FrameHeader {
        msg_type: u16::MAX,
        seq: u32::MAX,
        payload_len: u32::MAX,
    };
    let decoded = FrameHeader::decode(&header.encode());
    assert_eq!(header, decoded);
}

// -- MsgType tests --

#[test]
fn msg_type_roundtrip_all() {
    let types = [
        MsgType::Hello,
        MsgType::ClosePane,
        MsgType::Input,
        MsgType::Resize,
        MsgType::Subscribe,
        MsgType::Unsubscribe,
        MsgType::GetPaneSnapshot,
        MsgType::Ping,
        MsgType::Shutdown,
        MsgType::ScrollDisplay,
        MsgType::ScrollToBottom,
        MsgType::ScrollToPrompt,
        MsgType::SetTheme,
        MsgType::SetCursorShape,
        MsgType::MarkAllDirty,
        MsgType::OpenSearch,
        MsgType::CloseSearch,
        MsgType::SearchSetQuery,
        MsgType::SearchNextMatch,
        MsgType::SearchPrevMatch,
        MsgType::ExtractText,
        MsgType::ExtractHtml,
        MsgType::SetCapabilities,
        MsgType::SpawnPane,
        MsgType::ListPanes,
        MsgType::HelloAck,
        MsgType::PaneClosedAck,
        MsgType::Subscribed,
        MsgType::Unsubscribed,
        MsgType::PaneSnapshotResp,
        MsgType::PingAck,
        MsgType::ShutdownAck,
        MsgType::ScrollToPromptAck,
        MsgType::ExtractTextResp,
        MsgType::ExtractHtmlResp,
        MsgType::SpawnPaneResponse,
        MsgType::ListPanesResponse,
        MsgType::Error,
        MsgType::NotifyPaneOutput,
        MsgType::NotifyPaneExited,
        MsgType::NotifyPaneTitleChanged,
        MsgType::NotifyPaneBell,
        MsgType::NotifyPaneSnapshot,
    ];
    for t in types {
        let raw = t as u16;
        let back = MsgType::from_u16(raw)
            .unwrap_or_else(|| panic!("MsgType::from_u16(0x{raw:04X}) returned None for {t:?}"));
        assert_eq!(t, back);
    }
}

#[test]
fn msg_type_unknown_returns_none() {
    assert!(MsgType::from_u16(0x0000).is_none());
    assert!(MsgType::from_u16(0xFFFF).is_none());
    assert!(MsgType::from_u16(0x0400).is_none());
}

// -- Frame encode/decode roundtrip tests --

/// Encode a PDU, then decode it, asserting equality.
fn roundtrip(seq: u32, pdu: MuxPdu) -> DecodedFrame {
    let mut buf = Vec::new();
    ProtocolCodec::encode_frame(&mut buf, seq, &pdu).expect("encode");

    let mut reader = Cursor::new(buf);
    let frame = ProtocolCodec::new()
        .decode_frame(&mut reader)
        .expect("decode");

    assert_eq!(frame.seq, seq);
    assert_eq!(frame.pdu, pdu);
    frame
}

#[test]
fn roundtrip_hello() {
    roundtrip(1, MuxPdu::Hello { pid: 12345 });
}

#[test]
fn roundtrip_hello_ack() {
    roundtrip(
        1,
        MuxPdu::HelloAck {
            client_id: ClientId::from_raw(7),
        },
    );
}

#[test]
fn roundtrip_close_pane() {
    roundtrip(
        6,
        MuxPdu::ClosePane {
            pane_id: PaneId::from_raw(4),
        },
    );
}

#[test]
fn roundtrip_input_fire_and_forget() {
    let pdu = MuxPdu::Input {
        pane_id: PaneId::from_raw(1),
        data: b"hello world\r".to_vec(),
    };
    assert!(pdu.is_fire_and_forget());
    roundtrip(0, pdu);
}

#[test]
fn roundtrip_resize_fire_and_forget() {
    let pdu = MuxPdu::Resize {
        pane_id: PaneId::from_raw(1),
        cols: 120,
        rows: 40,
    };
    assert!(pdu.is_fire_and_forget());
    roundtrip(0, pdu);
}

#[test]
fn roundtrip_subscribe() {
    roundtrip(
        8,
        MuxPdu::Subscribe {
            pane_id: PaneId::from_raw(1),
        },
    );
}

#[test]
fn roundtrip_unsubscribe() {
    roundtrip(
        9,
        MuxPdu::Unsubscribe {
            pane_id: PaneId::from_raw(1),
        },
    );
}

#[test]
fn roundtrip_get_pane_snapshot() {
    roundtrip(
        12,
        MuxPdu::GetPaneSnapshot {
            pane_id: PaneId::from_raw(1),
        },
    );
}

#[test]
fn roundtrip_error_response() {
    roundtrip(
        99,
        MuxPdu::Error {
            message: "pane not found".into(),
        },
    );
}

#[test]
fn roundtrip_unit_responses() {
    roundtrip(20, MuxPdu::PaneClosedAck);
    roundtrip(22, MuxPdu::Unsubscribed);
}

#[test]
fn roundtrip_ping() {
    roundtrip(28, MuxPdu::Ping);
}

#[test]
fn roundtrip_ping_ack() {
    roundtrip(29, MuxPdu::PingAck);
}

#[test]
fn roundtrip_shutdown() {
    roundtrip(30, MuxPdu::Shutdown);
}

#[test]
fn roundtrip_shutdown_ack() {
    roundtrip(31, MuxPdu::ShutdownAck);
}

// -- SpawnPane / ListPanes roundtrips --

#[test]
fn roundtrip_spawn_pane() {
    roundtrip(
        32,
        MuxPdu::SpawnPane {
            shell: Some("/bin/bash".into()),
            cwd: Some("/home/user".into()),
            theme: Some("dark".into()),
        },
    );
}

#[test]
fn roundtrip_spawn_pane_defaults() {
    roundtrip(
        33,
        MuxPdu::SpawnPane {
            shell: None,
            cwd: None,
            theme: None,
        },
    );
}

#[test]
fn roundtrip_spawn_pane_response() {
    roundtrip(
        34,
        MuxPdu::SpawnPaneResponse {
            pane_id: PaneId::from_raw(9),
            domain_id: DomainId::from_raw(0),
        },
    );
}

#[test]
fn roundtrip_list_panes() {
    roundtrip(35, MuxPdu::ListPanes);
}

#[test]
fn roundtrip_list_panes_response() {
    roundtrip(
        36,
        MuxPdu::ListPanesResponse {
            pane_ids: vec![
                PaneId::from_raw(1),
                PaneId::from_raw(3),
                PaneId::from_raw(7),
            ],
        },
    );
}

#[test]
fn roundtrip_list_panes_response_empty() {
    roundtrip(37, MuxPdu::ListPanesResponse { pane_ids: vec![] });
}

// -- Notification roundtrips --

#[test]
fn roundtrip_notify_pane_output() {
    let pdu = MuxPdu::NotifyPaneOutput {
        pane_id: PaneId::from_raw(1),
    };
    assert!(pdu.is_notification());
    roundtrip(0, pdu);
}

#[test]
fn roundtrip_notify_pane_exited() {
    let pdu = MuxPdu::NotifyPaneExited {
        pane_id: PaneId::from_raw(2),
    };
    assert!(pdu.is_notification());
    roundtrip(0, pdu);
}

#[test]
fn roundtrip_notify_title_changed() {
    roundtrip(
        0,
        MuxPdu::NotifyPaneTitleChanged {
            pane_id: PaneId::from_raw(1),
            title: "vim main.rs".into(),
        },
    );
}

#[test]
fn roundtrip_notify_bell() {
    roundtrip(
        0,
        MuxPdu::NotifyPaneBell {
            pane_id: PaneId::from_raw(1),
        },
    );
}

// -- PaneSnapshot roundtrip --

fn sample_snapshot() -> PaneSnapshot {
    PaneSnapshot {
        cells: vec![
            vec![
                WireCell {
                    ch: 'A',
                    fg: WireRgb {
                        r: 211,
                        g: 215,
                        b: 207,
                    },
                    bg: WireRgb { r: 0, g: 0, b: 0 },
                    flags: 0,
                    underline_color: None,
                    hyperlink_uri: None,
                    zerowidth: vec![],
                },
                WireCell {
                    ch: '你',
                    fg: WireRgb {
                        r: 255,
                        g: 128,
                        b: 0,
                    },
                    bg: WireRgb {
                        r: 48,
                        g: 48,
                        b: 48,
                    },
                    flags: 0x0100, // WIDE_CHAR
                    underline_color: None,
                    hyperlink_uri: None,
                    zerowidth: vec![],
                },
            ],
            vec![WireCell {
                ch: 'e',
                fg: WireRgb {
                    r: 78,
                    g: 154,
                    b: 6,
                },
                bg: WireRgb { r: 0, g: 0, b: 0 },
                flags: 0x0001 | 0x0004, // BOLD | ITALIC
                underline_color: None,
                hyperlink_uri: None,
                zerowidth: vec!['\u{0301}'], // combining acute accent
            }],
        ],
        cursor: WireCursor {
            col: 5,
            row: 0,
            shape: WireCursorShape::Block,
            visible: true,
        },
        palette: (0..270).map(|i| [(i % 256) as u8, 0, 0]).collect(),
        title: "bash — ~/projects".into(),
        icon_name: None,
        cwd: None,
        modes: 0x0201, // SHOW_CURSOR | ALT_SCREEN
        scrollback_len: 1500,
        display_offset: 0,
        stable_row_base: 1500,
        cols: 2,
        search_active: false,
        search_query: String::new(),
        search_matches: Vec::new(),
        search_focused: None,
        search_total_matches: 0,
    }
}

#[test]
fn roundtrip_pane_snapshot() {
    let snapshot = sample_snapshot();
    roundtrip(
        30,
        MuxPdu::PaneSnapshotResp {
            snapshot: snapshot.clone(),
        },
    );
}

#[test]
fn roundtrip_subscribed_with_snapshot() {
    let snapshot = sample_snapshot();
    roundtrip(31, MuxPdu::Subscribed { snapshot });
}

#[test]
fn snapshot_with_cjk_emoji_combining() {
    let snapshot = PaneSnapshot {
        cells: vec![vec![
            // CJK wide char.
            WireCell {
                ch: '漢',
                fg: WireRgb {
                    r: 211,
                    g: 215,
                    b: 207,
                },
                bg: WireRgb { r: 0, g: 0, b: 0 },
                flags: 0x0100,
                underline_color: None,
                hyperlink_uri: None,
                zerowidth: vec![],
            },
            // Emoji (🦀).
            WireCell {
                ch: '🦀',
                fg: WireRgb {
                    r: 255,
                    g: 69,
                    b: 0,
                },
                bg: WireRgb { r: 0, g: 0, b: 0 },
                flags: 0x0100,
                underline_color: None,
                hyperlink_uri: None,
                zerowidth: vec![],
            },
            // Combining marks (e + combining acute + combining tilde).
            WireCell {
                ch: 'e',
                fg: WireRgb {
                    r: 211,
                    g: 215,
                    b: 207,
                },
                bg: WireRgb { r: 0, g: 0, b: 0 },
                flags: 0,
                underline_color: None,
                hyperlink_uri: None,
                zerowidth: vec!['\u{0301}', '\u{0303}'],
            },
        ]],
        cursor: WireCursor {
            col: 0,
            row: 0,
            shape: WireCursorShape::Bar,
            visible: true,
        },
        palette: vec![[0, 0, 0]; 270],
        title: "unicode test 🚀".into(),
        icon_name: None,
        cwd: None,
        modes: 0,
        scrollback_len: 0,
        display_offset: 0,
        stable_row_base: 0,
        cols: 3,
        search_active: false,
        search_query: String::new(),
        search_matches: Vec::new(),
        search_focused: None,
        search_total_matches: 0,
    };

    roundtrip(32, MuxPdu::PaneSnapshotResp { snapshot });
}

// -- Sequence correlation tests --

#[test]
fn sequence_correlation() {
    let mut buf = Vec::new();
    ProtocolCodec::encode_frame(&mut buf, 100, &MuxPdu::Hello { pid: 1 }).unwrap();
    ProtocolCodec::encode_frame(
        &mut buf,
        100,
        &MuxPdu::HelloAck {
            client_id: ClientId::from_raw(1),
        },
    )
    .unwrap();
    ProtocolCodec::encode_frame(
        &mut buf,
        101,
        &MuxPdu::SpawnPane {
            shell: None,
            cwd: None,
            theme: None,
        },
    )
    .unwrap();

    let mut reader = Cursor::new(buf);
    let mut codec = ProtocolCodec::new();
    let f1 = codec.decode_frame(&mut reader).unwrap();
    let f2 = codec.decode_frame(&mut reader).unwrap();
    let f3 = codec.decode_frame(&mut reader).unwrap();

    // Request and response share the same seq.
    assert_eq!(f1.seq, 100);
    assert_eq!(f2.seq, 100);
    // Different request has a different seq.
    assert_eq!(f3.seq, 101);
}

// -- Fire-and-forget tests --

#[test]
fn fire_and_forget_no_block() {
    let mut buf = Vec::new();
    // Fire-and-forget messages use seq=0.
    ProtocolCodec::encode_frame(
        &mut buf,
        0,
        &MuxPdu::Input {
            pane_id: PaneId::from_raw(1),
            data: b"ls\r".to_vec(),
        },
    )
    .unwrap();
    ProtocolCodec::encode_frame(
        &mut buf,
        0,
        &MuxPdu::Resize {
            pane_id: PaneId::from_raw(1),
            cols: 80,
            rows: 24,
        },
    )
    .unwrap();

    let mut reader = Cursor::new(buf);
    let mut codec = ProtocolCodec::new();
    let f1 = codec.decode_frame(&mut reader).unwrap();
    let f2 = codec.decode_frame(&mut reader).unwrap();

    assert_eq!(f1.seq, 0);
    assert_eq!(f2.seq, 0);
    assert!(f1.pdu.is_fire_and_forget());
    assert!(f2.pdu.is_fire_and_forget());
}

// -- Push notification delivery --

#[test]
fn notification_delivery() {
    let mut buf = Vec::new();
    let notifications = vec![
        MuxPdu::NotifyPaneOutput {
            pane_id: PaneId::from_raw(1),
        },
        MuxPdu::NotifyPaneExited {
            pane_id: PaneId::from_raw(2),
        },
        MuxPdu::NotifyPaneTitleChanged {
            pane_id: PaneId::from_raw(1),
            title: "new title".into(),
        },
    ];

    for n in &notifications {
        ProtocolCodec::encode_frame(&mut buf, 0, n).unwrap();
    }

    let mut reader = Cursor::new(buf);
    for expected in &notifications {
        let frame = ProtocolCodec::new().decode_frame(&mut reader).unwrap();
        assert_eq!(frame.seq, 0);
        assert!(frame.pdu.is_notification());
        assert_eq!(&frame.pdu, expected);
    }
}

// -- Error condition tests --

#[test]
fn decode_payload_too_large() {
    // Craft a header with payload_len > MAX_PAYLOAD.
    let header = FrameHeader {
        msg_type: MsgType::Hello as u16,
        seq: 1,
        payload_len: MAX_PAYLOAD + 1,
    };
    let mut buf = header.encode().to_vec();
    // Append some dummy payload bytes.
    buf.extend_from_slice(&[0u8; 64]);

    let mut reader = Cursor::new(buf);
    let err = ProtocolCodec::new().decode_frame(&mut reader).unwrap_err();
    assert!(matches!(err, DecodeError::PayloadTooLarge(_)));
}

#[test]
fn decode_unknown_msg_type() {
    let header = FrameHeader {
        msg_type: 0xFFFF,
        seq: 1,
        payload_len: 0,
    };
    let buf = header.encode().to_vec();

    let mut reader = Cursor::new(buf);
    let err = ProtocolCodec::new().decode_frame(&mut reader).unwrap_err();
    assert!(matches!(err, DecodeError::UnknownMsgType(0xFFFF)));
}

#[test]
fn decode_truncated_header() {
    let buf = vec![0u8; 5]; // Only 5 bytes, header needs 10.
    let mut reader = Cursor::new(buf);
    let err = ProtocolCodec::new().decode_frame(&mut reader).unwrap_err();
    assert!(matches!(err, DecodeError::Io(_)));
}

#[test]
fn decode_truncated_payload() {
    let header = FrameHeader {
        msg_type: MsgType::Hello as u16,
        seq: 1,
        payload_len: 100,
    };
    let mut buf = header.encode().to_vec();
    buf.extend_from_slice(&[0u8; 10]); // Only 10 bytes, claims 100.

    let mut reader = Cursor::new(buf);
    let err = ProtocolCodec::new().decode_frame(&mut reader).unwrap_err();
    assert!(matches!(err, DecodeError::Io(_)));
}

// -- Multiple frames in a stream --

#[test]
fn multiple_frames_sequential() {
    let mut buf = Vec::new();
    let pdus = vec![
        (1, MuxPdu::Hello { pid: 1000 }),
        (
            1,
            MuxPdu::HelloAck {
                client_id: ClientId::from_raw(1),
            },
        ),
        (
            2,
            MuxPdu::SpawnPane {
                shell: None,
                cwd: None,
                theme: None,
            },
        ),
        (
            2,
            MuxPdu::SpawnPaneResponse {
                pane_id: PaneId::from_raw(1),
                domain_id: DomainId::from_raw(0),
            },
        ),
        (3, MuxPdu::ListPanes),
        (
            3,
            MuxPdu::ListPanesResponse {
                pane_ids: vec![PaneId::from_raw(1)],
            },
        ),
    ];

    for (seq, pdu) in &pdus {
        ProtocolCodec::encode_frame(&mut buf, *seq, pdu).unwrap();
    }

    let mut reader = Cursor::new(buf);
    for (expected_seq, expected_pdu) in &pdus {
        let frame = ProtocolCodec::new().decode_frame(&mut reader).unwrap();
        assert_eq!(frame.seq, *expected_seq);
        assert_eq!(&frame.pdu, expected_pdu);
    }

    // Stream exhausted — next decode should fail with UnexpectedEof.
    let err = ProtocolCodec::new().decode_frame(&mut reader).unwrap_err();
    assert!(matches!(err, DecodeError::Io(_)));
}

// -- Payload boundary size tests --

#[test]
fn roundtrip_large_input_near_max_payload() {
    // A payload just under MAX_PAYLOAD should encode/decode successfully.
    // Use Input which carries a Vec<u8> — easy to inflate.
    // We can't test exactly MAX_PAYLOAD since bincode adds overhead for the
    // enum discriminant and PaneId, but we can test a large data payload.
    let large_data = vec![b'X'; 1024 * 1024]; // 1 MiB of data.
    let pdu = MuxPdu::Input {
        pane_id: PaneId::from_raw(1),
        data: large_data.clone(),
    };
    let frame = roundtrip(1, pdu);
    match frame.pdu {
        MuxPdu::Input { data, .. } => assert_eq!(data.len(), large_data.len()),
        other => panic!("expected Input, got {other:?}"),
    }
}

#[test]
fn encode_rejects_payload_exceeding_max() {
    // A payload that exceeds MAX_PAYLOAD should fail to encode.
    // MAX_PAYLOAD is 16 MiB. We need a PDU whose bincode encoding
    // exceeds that. Use Input with a data vec > 16 MiB.
    let huge_data = vec![0u8; MAX_PAYLOAD as usize + 1];
    let pdu = MuxPdu::Input {
        pane_id: PaneId::from_raw(1),
        data: huge_data,
    };
    let mut buf = Vec::new();
    let result = ProtocolCodec::encode_frame(&mut buf, 1, &pdu);
    assert!(result.is_err(), "encoding >MAX_PAYLOAD should fail");
}

#[test]
fn decode_payload_exactly_at_max() {
    // A header claiming exactly MAX_PAYLOAD bytes should be accepted
    // (the check is > MAX_PAYLOAD, not >=).
    let header = FrameHeader {
        msg_type: MsgType::Hello as u16,
        seq: 1,
        payload_len: MAX_PAYLOAD,
    };
    let encoded = header.encode();
    let decoded = FrameHeader::decode(&encoded);
    assert_eq!(decoded.payload_len, MAX_PAYLOAD);
    // Note: actual decode would fail because we can't provide MAX_PAYLOAD
    // bytes of valid bincode, but the header itself should parse fine.
}

// -- Malformed payload (valid header, garbage body) --

#[test]
fn decode_garbage_payload_returns_deserialize_error() {
    // Valid header with correct msg_type and payload_len, but the payload
    // bytes are random garbage that can't be deserialized.
    let garbage = vec![0xFF; 32];
    let header = FrameHeader {
        msg_type: MsgType::Hello as u16,
        seq: 1,
        payload_len: garbage.len() as u32,
    };
    let mut buf = header.encode().to_vec();
    buf.extend_from_slice(&garbage);

    let mut reader = Cursor::new(buf);
    let err = ProtocolCodec::new().decode_frame(&mut reader).unwrap_err();
    assert!(
        matches!(err, DecodeError::Deserialize(_)),
        "expected Deserialize error, got {err:?}"
    );
}

#[test]
fn decode_empty_payload_for_pdu_with_fields() {
    // A Hello PDU requires a pid field. Sending an empty payload (len=0)
    // should cause a deserialization error, not a panic.
    let header = FrameHeader {
        msg_type: MsgType::Hello as u16,
        seq: 1,
        payload_len: 0,
    };
    let buf = header.encode().to_vec();

    let mut reader = Cursor::new(buf);
    let err = ProtocolCodec::new().decode_frame(&mut reader).unwrap_err();
    assert!(
        matches!(err, DecodeError::Deserialize(_)),
        "expected Deserialize error for empty payload, got {err:?}"
    );
}

// -- Variable-length payload boundary tests --

#[test]
fn roundtrip_boundary_payload_sizes() {
    // Test at sizes that could trip length encoding boundaries.
    let sizes = [0, 1, 127, 128, 255, 256, 1024, 65535, 65536];
    for &size in &sizes {
        let data = vec![b'X'; size];
        let pdu = MuxPdu::Input {
            pane_id: PaneId::from_raw(1),
            data,
        };
        let frame = roundtrip(1, pdu);
        match frame.pdu {
            MuxPdu::Input { data, .. } => {
                assert_eq!(data.len(), size, "payload roundtrip failed at size {size}");
            }
            other => panic!("expected Input at size={size}, got {other:?}"),
        }
    }
}

// -- Wire byte pinning --

#[test]
fn wire_bytes_stable_for_hello() {
    // Pin the exact wire encoding for Hello { pid: 42 } at seq=1.
    // If the serialization format changes, this test catches it.
    let pdu = MuxPdu::Hello { pid: 42 };
    let mut buf = Vec::new();
    ProtocolCodec::encode_frame(&mut buf, 1, &pdu).unwrap();

    // Decode back to verify.
    let mut reader = Cursor::new(&buf);
    let frame = ProtocolCodec::new().decode_frame(&mut reader).unwrap();
    assert_eq!(frame.seq, 1);
    assert_eq!(frame.pdu, MuxPdu::Hello { pid: 42 });

    // Pin header bytes: msg_type=0x0101 LE, seq=1 LE, payload_len LE.
    assert_eq!(buf[0..2], [0x01, 0x01]); // MsgType::Hello
    assert_eq!(buf[2..6], [0x01, 0x00, 0x00, 0x00]); // seq=1
    // Payload len and content depend on bincode, but header is stable.
    let payload_len = u32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]);
    assert_eq!(buf.len(), HEADER_LEN + payload_len as usize);

    // Pin total frame size. bincode for Hello { pid: 42 }:
    // variant index (4 bytes LE for enum discriminant) + pid (4 bytes LE).
    // This is bincode's default encoding.
    let expected_payload = bincode::serialize(&pdu).unwrap();
    assert_eq!(&buf[HEADER_LEN..], &expected_payload);
}

// -- theme_to_wire roundtrip --

#[test]
fn theme_to_wire_dark() {
    use super::messages::theme_to_wire;
    use oriterm_core::Theme;

    assert_eq!(theme_to_wire(Theme::Dark), Some("dark"));
}

#[test]
fn theme_to_wire_light() {
    use super::messages::theme_to_wire;
    use oriterm_core::Theme;

    assert_eq!(theme_to_wire(Theme::Light), Some("light"));
}

#[test]
fn theme_to_wire_unknown() {
    use super::messages::theme_to_wire;
    use oriterm_core::Theme;

    assert_eq!(theme_to_wire(Theme::Unknown), None);
}

// -- Large PaneSnapshot stress test --

#[test]
fn roundtrip_large_pane_snapshot() {
    // 200 columns x 50 rows — a realistic full-screen terminal snapshot.
    let cols = 200;
    let rows = 50;
    let mut cells = Vec::with_capacity(rows);
    for r in 0..rows {
        let mut row = Vec::with_capacity(cols);
        for c in 0..cols {
            row.push(WireCell {
                ch: char::from(b'A' + ((r * cols + c) % 26) as u8),
                fg: WireRgb {
                    r: (r * 5) as u8,
                    g: (c * 2) as u8,
                    b: 128,
                },
                bg: WireRgb { r: 0, g: 0, b: 0 },
                flags: if c % 10 == 0 { 0x0001 } else { 0 }, // every 10th cell bold
                underline_color: None,
                hyperlink_uri: None,
                zerowidth: vec![],
            });
        }
        cells.push(row);
    }

    let snapshot = PaneSnapshot {
        cells,
        cursor: WireCursor {
            col: 42,
            row: 25,
            shape: WireCursorShape::Block,
            visible: true,
        },
        palette: (0..270).map(|i| [(i % 256) as u8, 0, 0]).collect(),
        title: "large snapshot test".into(),
        icon_name: None,
        cwd: None,
        modes: 0x0201,
        scrollback_len: 10_000,
        display_offset: 50,
        stable_row_base: 9_950,
        cols: cols as u16,
        search_active: false,
        search_query: String::new(),
        search_matches: Vec::new(),
        search_focused: None,
        search_total_matches: 0,
    };

    let frame = roundtrip(
        100,
        MuxPdu::PaneSnapshotResp {
            snapshot: snapshot.clone(),
        },
    );
    match frame.pdu {
        MuxPdu::PaneSnapshotResp { snapshot: got } => {
            assert_eq!(got.cells.len(), rows);
            assert_eq!(got.cells[0].len(), cols);
            assert_eq!(got.cursor.col, 42);
            assert_eq!(got.cursor.row, 25);
            assert_eq!(got.scrollback_len, 10_000);
            assert_eq!(got.display_offset, 50);
            // Spot-check a few cells.
            assert_eq!(got.cells[0][0].ch, 'A');
            assert_eq!(got.cells[0][0].flags, 0x0001); // bold
            assert_eq!(got.cells[0][1].flags, 0); // not bold
            assert_eq!(got.cells[49][199].ch, snapshot.cells[49][199].ch);
        }
        other => panic!("expected PaneSnapshotResp, got {other:?}"),
    }
}

// -- SetCapabilities roundtrip --

#[test]
fn roundtrip_set_capabilities() {
    use super::messages::CAP_SNAPSHOT_PUSH;
    let pdu = MuxPdu::SetCapabilities {
        flags: CAP_SNAPSHOT_PUSH,
    };
    assert!(pdu.is_fire_and_forget());
    roundtrip(2, pdu);
}

// -- NotifyPaneSnapshot roundtrip --

#[test]
fn roundtrip_notify_pane_snapshot() {
    let snapshot = sample_snapshot();
    let pdu = MuxPdu::NotifyPaneSnapshot {
        pane_id: PaneId::from_raw(3),
        snapshot,
    };
    assert!(pdu.is_notification());
    roundtrip(0, pdu);
}

// -- Forward-compat: ProtocolCodec skips unknown msg_type, stream stays aligned --

#[test]
fn forward_compat_codec_skips_unknown_and_stays_aligned() {
    use std::io::Write;

    let mut buf = Vec::new();

    // Frame 1: unknown msg_type 0xFFFF with 100-byte payload.
    let header1 = FrameHeader {
        msg_type: 0xFFFF,
        seq: 0,
        payload_len: 100,
    };
    buf.write_all(&header1.encode()).unwrap();
    buf.extend_from_slice(&[0xAB; 100]);

    // Frame 2: valid Ping.
    ProtocolCodec::encode_frame(&mut buf, 42, &MuxPdu::Ping).unwrap();

    let mut reader = Cursor::new(buf);
    let mut codec = ProtocolCodec::new();

    // First decode: UnknownMsgType (payload consumed).
    let err = codec.decode_frame(&mut reader).unwrap_err();
    assert!(matches!(err, DecodeError::UnknownMsgType(0xFFFF)));

    // Second decode: valid Ping (stream aligned).
    let frame = codec.decode_frame(&mut reader).unwrap();
    assert_eq!(frame.seq, 42);
    assert!(matches!(frame.pdu, MuxPdu::Ping));
}

// FrameReader forward-compat tests live in `server/tests.rs` where FrameReader
// is accessible (it's a private server submodule).
