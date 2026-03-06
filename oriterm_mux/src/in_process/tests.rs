//! Tests for `InProcessMux` pane lifecycle operations.
//!
//! These tests exercise the mux's registry and notification logic without
//! spawning real PTYs. We construct the mux, then manually register panes
//! to test close_pane and event pump behaviour in isolation.

use crate::PaneId;

use super::{ClosePaneResult, InProcessMux, MuxNotification};
use crate::mux_event::MuxEvent;

/// Drain notifications into a fresh Vec (convenience for tests).
fn drain(mux: &mut InProcessMux) -> Vec<MuxNotification> {
    let mut out = Vec::new();
    mux.drain_notifications(&mut out);
    out
}

/// Build a mux with one registered pane.
fn one_pane_setup() -> (InProcessMux, PaneId) {
    let mut mux = InProcessMux::new();
    let pid = PaneId::from_raw(100);
    mux.inject_test_pane(pid);
    drain(&mut mux);
    (mux, pid)
}

/// Build a mux with two registered panes.
fn two_pane_setup() -> (InProcessMux, PaneId, PaneId) {
    let mut mux = InProcessMux::new();
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    mux.inject_test_pane(p1);
    mux.inject_test_pane(p2);
    drain(&mut mux);
    (mux, p1, p2)
}

// -- close_pane --

#[test]
fn close_pane_not_found() {
    let mut mux = InProcessMux::new();
    assert_eq!(
        mux.close_pane(PaneId::from_raw(999)),
        ClosePaneResult::NotFound
    );
}

#[test]
fn close_pane_emits_pane_closed() {
    let (mut mux, _p1, p2) = two_pane_setup();

    mux.close_pane(p2);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::PaneClosed(id) if *id == p2)),
        "PaneClosed notification missing"
    );
}

#[test]
fn close_pane_twice_returns_not_found_on_second_call() {
    let (mut mux, _p1, p2) = two_pane_setup();

    let first = mux.close_pane(p2);
    assert_eq!(first, ClosePaneResult::PaneRemoved);

    let second = mux.close_pane(p2);
    assert_eq!(second, ClosePaneResult::NotFound);
}

#[test]
fn close_pane_removes_from_registry() {
    let (mut mux, pid) = one_pane_setup();

    mux.close_pane(pid);
    assert!(mux.get_pane_entry(pid).is_none());
}

// -- event pump --

#[test]
fn poll_events_handles_title_change() {
    let (mut mux, pid) = one_pane_setup();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::PaneTitleChanged {
        pane_id: pid,
        title: "new title".to_string(),
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);
}

#[test]
fn poll_events_clipboard_store_emits_notification() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::ClipboardStore {
        pane_id: PaneId::from_raw(1),
        clipboard_type: oriterm_core::ClipboardType::Clipboard,
        text: "hello".to_string(),
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert!(notifs.iter().any(|n| matches!(
        n,
        MuxNotification::ClipboardStore { text, .. } if text == "hello"
    )));
}

#[test]
fn poll_events_bell_emits_alert() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    let pid = PaneId::from_raw(42);
    tx.send(MuxEvent::PaneBell(pid)).unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::PaneBell(id) if *id == pid))
    );
}

#[test]
fn drain_notifications_clears_queue() {
    let (mut mux, pid) = one_pane_setup();
    mux.close_pane(pid);

    let first = drain(&mut mux);
    assert!(!first.is_empty());

    let second = drain(&mut mux);
    assert!(second.is_empty());
}

// -- get_pane_entry --

#[test]
fn get_pane_entry_returns_correct_metadata() {
    let (mux, pid) = one_pane_setup();
    let entry = mux.get_pane_entry(pid).unwrap();
    assert_eq!(entry.pane, pid);
}

// -- event_tx cloning --

#[test]
fn event_tx_can_be_cloned_and_used() {
    let mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::PaneBell(PaneId::from_raw(1))).unwrap();

    let event = mux.event_rx.try_recv().unwrap();
    assert!(matches!(event, MuxEvent::PaneBell(_)));
}

// -- poll_events with disconnected sender --

#[test]
fn poll_events_with_empty_channel_is_noop() {
    let mut mux = InProcessMux::new();
    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);
    assert!(drain(&mut mux).is_empty());
}

// -- multiple event processing --

#[test]
fn poll_events_processes_multiple_events() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);

    tx.send(MuxEvent::PaneBell(p1)).unwrap();
    tx.send(MuxEvent::PaneBell(p2)).unwrap();
    tx.send(MuxEvent::PaneOutput(p1)).unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert_eq!(notifs.len(), 3);
}

// -- Batch pane exits --

#[test]
fn batch_pane_exits_emit_pane_closed_for_each() {
    let mut mux = InProcessMux::new();
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    let p3 = PaneId::from_raw(102);

    mux.inject_test_pane(p1);
    mux.inject_test_pane(p2);
    mux.inject_test_pane(p3);
    drain(&mut mux);

    let tx = mux.event_tx().clone();
    for &pid in &[p1, p2, p3] {
        tx.send(MuxEvent::PaneExited {
            pane_id: pid,
            exit_code: 0,
        })
        .unwrap();
    }

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);

    for &pid in &[p1, p2, p3] {
        let count = notifs
            .iter()
            .filter(|n| matches!(n, MuxNotification::PaneClosed(id) if *id == pid))
            .count();
        assert_eq!(count, 1, "PaneClosed({pid:?}) emitted {count} times");
    }
}

// -- CWD change with missing pane --

#[test]
fn poll_events_cwd_change_missing_pane_no_panic() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::PaneCwdChanged {
        pane_id: PaneId::from_raw(999),
        cwd: "/tmp/nonexistent".to_string(),
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);
}

// -- PtyWrite with missing pane --

#[test]
fn poll_events_pty_write_missing_pane_no_panic() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::PtyWrite {
        pane_id: PaneId::from_raw(999),
        data: "hello".to_string(),
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);
}

// -- Notification ordering --

#[test]
fn drain_notifications_preserves_insertion_order() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);

    tx.send(MuxEvent::PaneBell(p1)).unwrap();
    tx.send(MuxEvent::PaneOutput(p2)).unwrap();
    tx.send(MuxEvent::PaneBell(p3)).unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert_eq!(notifs.len(), 3);

    assert!(matches!(&notifs[0], MuxNotification::PaneBell(id) if *id == p1));
    assert!(matches!(&notifs[1], MuxNotification::PaneOutput(id) if *id == p2));
    assert!(matches!(&notifs[2], MuxNotification::PaneBell(id) if *id == p3));
}

// -- Send trait bound --

/// Compile-time assertion that key mux types are `Send`.
#[test]
fn mux_types_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<InProcessMux>();
    assert_send::<ClosePaneResult>();
    assert_send::<MuxNotification>();
}

// -- Stale pane events --

#[test]
fn stale_pane_map_during_event_dispatch() {
    let (mut mux, p1, p2) = two_pane_setup();
    let tx = mux.event_tx().clone();

    // p1 exits, then stale events arrive for p1.
    tx.send(MuxEvent::PaneExited {
        pane_id: p1,
        exit_code: 0,
    })
    .unwrap();
    tx.send(MuxEvent::PaneOutput(p1)).unwrap();
    tx.send(MuxEvent::PaneBell(p1)).unwrap();
    tx.send(MuxEvent::PaneTitleChanged {
        pane_id: p1,
        title: "stale".to_string(),
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    assert!(mux.get_pane_entry(p1).is_none());
    assert!(mux.get_pane_entry(p2).is_some());
}

// -- PaneOutput after close --

#[test]
fn pane_output_after_pane_closed_is_noop() {
    let (mut mux, pid) = one_pane_setup();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::PaneExited {
        pane_id: pid,
        exit_code: 0,
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);
    drain(&mut mux);

    // Stale output event.
    tx.send(MuxEvent::PaneOutput(pid)).unwrap();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::PaneOutput(id) if *id == pid))
    );
}

// -- PaneClosed notification ID --

#[test]
fn pane_closed_notification_carries_correct_id() {
    let (mut mux, _p1, p2) = two_pane_setup();
    drain(&mut mux);

    mux.close_pane(p2);

    let notifs = drain(&mut mux);
    let closed: Vec<PaneId> = notifs
        .iter()
        .filter_map(|n| match n {
            MuxNotification::PaneClosed(id) => Some(*id),
            _ => None,
        })
        .collect();

    assert_eq!(
        closed,
        vec![p2],
        "PaneClosed should carry the exact pane ID"
    );
}

// -- Domain allocator --

#[test]
fn domain_alloc_persisted_in_struct() {
    let mut mux = InProcessMux::new();
    let local_id = mux.default_domain();
    let second_id = mux.domain_alloc.alloc();
    assert_ne!(local_id, second_id);

    let third_id = mux.domain_alloc.alloc();
    assert_ne!(second_id, third_id);
    assert_ne!(local_id, third_id);
}

// -- Sender drops --

#[test]
fn sender_dropped_during_poll_drains_remaining() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::PaneBell(PaneId::from_raw(1))).unwrap();
    tx.send(MuxEvent::PaneOutput(PaneId::from_raw(2))).unwrap();
    drop(tx);

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert_eq!(notifs.len(), 2);

    mux.poll_events(&mut panes);
    assert!(drain(&mut mux).is_empty());
}

// -- Clipboard data preservation --

#[test]
fn drain_notifications_preserves_clipboard_data() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::ClipboardStore {
        pane_id: PaneId::from_raw(42),
        clipboard_type: oriterm_core::ClipboardType::Clipboard,
        text: "important data".to_string(),
    })
    .unwrap();

    tx.send(MuxEvent::ClipboardLoad {
        pane_id: PaneId::from_raw(42),
        clipboard_type: oriterm_core::ClipboardType::Selection,
        formatter: std::sync::Arc::new(|s: &str| format!("\x1b]52;s;{s}\x07")),
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert_eq!(notifs.len(), 2);

    assert!(matches!(
        &notifs[0],
        MuxNotification::ClipboardStore { text, pane_id, clipboard_type }
            if text == "important data"
            && *pane_id == PaneId::from_raw(42)
            && *clipboard_type == oriterm_core::ClipboardType::Clipboard
    ));

    if let MuxNotification::ClipboardLoad { formatter, .. } = &notifs[1] {
        assert_eq!(formatter("test"), "\x1b]52;s;test\x07");
    } else {
        panic!("expected ClipboardLoad notification");
    }
}

// -- PaneOutput for absent pane --

#[test]
fn pane_dirty_produced_for_absent_pane() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    let unknown = PaneId::from_raw(999);
    tx.send(MuxEvent::PaneOutput(unknown)).unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert_eq!(notifs.len(), 1);
    assert!(matches!(
        &notifs[0],
        MuxNotification::PaneOutput(id) if *id == unknown
    ));
}

// -- ClipboardLoad for unknown pane --

#[test]
fn clipboard_load_unknown_pane_produces_notification() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    let unknown = PaneId::from_raw(999);
    tx.send(MuxEvent::ClipboardLoad {
        pane_id: unknown,
        clipboard_type: oriterm_core::ClipboardType::Clipboard,
        formatter: std::sync::Arc::new(|s: &str| format!("\x1b]52;c;{s}\x07")),
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert_eq!(notifs.len(), 1);
    if let MuxNotification::ClipboardLoad {
        pane_id, formatter, ..
    } = &notifs[0]
    {
        assert_eq!(*pane_id, unknown);
        assert_eq!(formatter("hello"), "\x1b]52;c;hello\x07");
    } else {
        panic!("expected ClipboardLoad notification");
    }
}

// -- Empty notification buffer --

#[test]
fn empty_notification_buffer_short_circuits() {
    let mut mux = InProcessMux::new();
    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let mut buf = Vec::new();
    mux.drain_notifications(&mut buf);
    assert!(
        buf.is_empty(),
        "expected empty buffer when no events arrive"
    );
}

// -- Double-buffer pattern --

#[test]
fn drain_double_buffer_no_cross_cycle_accumulation() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::PaneBell(PaneId::from_raw(1))).unwrap();
    tx.send(MuxEvent::PaneOutput(PaneId::from_raw(2))).unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let mut buf = Vec::new();
    mux.drain_notifications(&mut buf);
    assert_eq!(buf.len(), 2, "cycle 1 should have 2 notifications");

    mux.poll_events(&mut panes);

    mux.drain_notifications(&mut buf);
    assert!(
        buf.is_empty(),
        "cycle 2 should be empty — stale notifications must not accumulate"
    );
}

// -- CommandComplete --

#[test]
fn poll_events_command_complete_emits_notification() {
    let (mut mux, pid) = one_pane_setup();
    let tx = mux.event_tx().clone();

    let dur = std::time::Duration::from_secs(42);
    tx.send(MuxEvent::CommandComplete {
        pane_id: pid,
        duration: dur,
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert!(
        notifs.iter().any(
            |n| matches!(n, MuxNotification::CommandComplete { pane_id, duration } if *pane_id == pid && *duration == dur)
        ),
        "expected CommandComplete notification with correct pane_id and duration"
    );
}

#[test]
fn poll_events_command_complete_missing_pane_no_panic() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::CommandComplete {
        pane_id: PaneId::from_raw(999),
        duration: std::time::Duration::from_secs(5),
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert!(notifs.iter().any(
        |n| matches!(n, MuxNotification::CommandComplete { pane_id, .. } if *pane_id == PaneId::from_raw(999))
    ));
}

// -- PaneIconChanged --

#[test]
fn poll_events_icon_changed_emits_title_notification() {
    let (mut mux, pid) = one_pane_setup();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::PaneIconChanged {
        pane_id: pid,
        icon_name: "\u{1f40d}python".to_string(),
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::PaneTitleChanged(id) if *id == pid)),
        "expected PaneTitleChanged notification for icon change"
    );
}

#[test]
fn poll_events_icon_changed_missing_pane_no_panic() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::PaneIconChanged {
        pane_id: PaneId::from_raw(999),
        icon_name: "icon".to_string(),
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert!(
        notifs.iter().any(
            |n| matches!(n, MuxNotification::PaneTitleChanged(id) if *id == PaneId::from_raw(999))
        ),
        "expected PaneTitleChanged even when pane is missing from map"
    );
}
