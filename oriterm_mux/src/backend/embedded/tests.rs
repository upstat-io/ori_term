//! Tests for EmbeddedMux backend.

use std::sync::Arc;

use super::EmbeddedMux;
use crate::PaneId;
use crate::backend::MuxBackend;
use crate::mux_event::MuxNotification;

/// No-op wakeup for tests (no event loop to wake).
fn test_wakeup() -> Arc<dyn Fn() + Send + Sync> {
    Arc::new(|| {})
}

/// Drain notifications from the embedded backend into a `Vec`.
fn drain(mux: &mut EmbeddedMux) -> Vec<MuxNotification> {
    let mut buf = Vec::new();
    mux.drain_notifications(&mut buf);
    buf
}

// -- Object safety and basic queries --

/// `EmbeddedMux` implements `MuxBackend` (compile check via object safety).
#[test]
fn object_safe() {
    let mux = EmbeddedMux::new(test_wakeup());
    let _boxed: Box<dyn MuxBackend> = Box::new(mux);
}

/// `drain_notifications` returns empty when nothing has happened.
#[test]
fn drain_empty() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let mut buf = Vec::new();
    mux.drain_notifications(&mut buf);
    assert!(buf.is_empty());
}

/// `discard_notifications` clears pending notifications.
#[test]
fn discard_notifications() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    mux.discard_notifications();
}

/// `poll_events` with no pending events doesn't panic.
#[test]
fn poll_events_empty() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    mux.poll_events();
}

/// `event_tx` returns `Some` in embedded mode.
#[test]
fn event_tx_available() {
    let mux = EmbeddedMux::new(test_wakeup());
    assert!(mux.event_tx().is_some());
}

/// `pane_ids` returns empty initially.
#[test]
fn pane_ids_empty() {
    let mux = EmbeddedMux::new(test_wakeup());
    assert!(mux.pane_ids().is_empty());
}

// -- Pane entry queries (via inject_test_pane helper) --

/// `get_pane_entry` returns metadata for injected panes.
#[test]
fn get_pane_entry_after_inject() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let pid = PaneId::from_raw(100);
    mux.mux.inject_test_pane(pid);

    let entry = mux.get_pane_entry(pid).unwrap();
    assert_eq!(entry.pane, pid);
}

/// `get_pane_entry` returns `None` after close.
#[test]
fn pane_entry_gone_after_close() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    mux.mux.inject_test_pane(p1);
    mux.mux.inject_test_pane(p2);

    mux.close_pane(p2);
    assert!(mux.get_pane_entry(p2).is_none());
    assert!(mux.get_pane_entry(p1).is_some());
}

// -- close_pane --

/// `close_pane` emits `PaneClosed` notification.
#[test]
fn close_pane_emits_notification() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    mux.mux.inject_test_pane(p1);
    mux.mux.inject_test_pane(p2);

    mux.close_pane(p2);
    let notes = drain(&mut mux);
    assert!(
        notes
            .iter()
            .any(|n| matches!(n, MuxNotification::PaneClosed(id) if *id == p2))
    );
}

// -- Send + daemon mode --

/// `EmbeddedMux` satisfies `Send` (prevents accidental `Rc`/`Cell` additions).
#[test]
fn embedded_mux_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<EmbeddedMux>();
}

/// `is_daemon_mode` returns false for embedded backend.
#[test]
fn is_not_daemon_mode() {
    let mux = EmbeddedMux::new(test_wakeup());
    assert!(!mux.is_daemon_mode());
}
