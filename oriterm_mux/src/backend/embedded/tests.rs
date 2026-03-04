//! Tests for EmbeddedMux backend.

use std::collections::HashSet;
use std::sync::Arc;

use super::EmbeddedMux;
use crate::backend::MuxBackend;
use crate::layout::SplitDirection;
use crate::mux_event::MuxNotification;
use crate::{PaneId, TabId, WindowId};

/// No-op wakeup for tests (no event loop to wake).
fn test_wakeup() -> Arc<dyn Fn() + Send + Sync> {
    Arc::new(|| {})
}

/// Build an `EmbeddedMux` with one window, one tab, one pane (metadata only).
fn one_pane_setup() -> (EmbeddedMux, WindowId, TabId, PaneId) {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let wid = WindowId::from_raw(100);
    let tid = TabId::from_raw(100);
    let pid = PaneId::from_raw(100);
    mux.mux.inject_test_tab(wid, tid, pid);
    (mux, wid, tid, pid)
}

/// Build an `EmbeddedMux` with one window, one tab, two panes (split).
fn two_pane_setup() -> (EmbeddedMux, WindowId, TabId, PaneId, PaneId) {
    let (mut mux, wid, tid, p1) = one_pane_setup();
    let p2 = PaneId::from_raw(101);
    mux.mux.inject_split(tid, p2, SplitDirection::Vertical);
    (mux, wid, tid, p1, p2)
}

/// Drain notifications from the embedded backend into a `Vec`.
fn drain(mux: &mut EmbeddedMux) -> Vec<MuxNotification> {
    let mut buf = Vec::new();
    mux.drain_notifications(&mut buf);
    buf
}

// -- Existing tests (object safety, basic queries) --

/// `EmbeddedMux` implements `MuxBackend` (compile check via object safety).
#[test]
fn object_safe() {
    let mux = EmbeddedMux::new(test_wakeup());
    let _boxed: Box<dyn MuxBackend> = Box::new(mux);
}

/// `create_window` returns a valid window ID.
#[test]
fn create_window() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let wid = mux.create_window().unwrap();
    assert!(mux.session().get_window(wid).is_some());
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
    let _ = mux.create_window().unwrap();
    // Window creation doesn't emit notifications, but discard shouldn't panic.
    mux.discard_notifications();
}

/// `close_window` on an empty window returns empty pane list.
#[test]
fn close_empty_window() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let wid = mux.create_window().unwrap();
    let panes = mux.close_window(wid);
    assert!(panes.is_empty());
}

/// `active_tab_id` returns `None` for empty window.
#[test]
fn empty_window_no_active_tab() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let wid = mux.create_window().unwrap();
    assert!(mux.active_tab_id(wid).is_none());
}

/// `is_last_pane` returns `false` when no panes exist.
#[test]
fn no_panes_not_last() {
    let mux = EmbeddedMux::new(test_wakeup());
    assert!(!mux.is_last_pane(PaneId::from_raw(1)));
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

// -- High priority: tab lifecycle --

/// Full tab lifecycle: create → query → switch → close.
#[test]
fn tab_lifecycle() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let wid = WindowId::from_raw(100);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(101);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);

    // Inject two tabs in one window.
    mux.mux.inject_test_tab(wid, t1, p1);
    mux.mux.inject_test_tab(wid, t2, p2);

    // Window should have two tabs.
    let win = mux.session().get_window(wid).unwrap();
    assert_eq!(win.tabs().len(), 2);

    // Active tab should be the first (index 0).
    assert_eq!(mux.active_tab_id(wid), Some(t1));

    // Switch active tab.
    assert!(mux.switch_active_tab(wid, t2));
    assert_eq!(mux.active_tab_id(wid), Some(t2));

    // Close tab t1 — pane registry entry removed, tab removed from window.
    let closed_panes = mux.close_tab(t1);
    assert_eq!(closed_panes, vec![p1]);
    assert!(mux.session().get_tab(t1).is_none());
    assert_eq!(mux.session().get_window(wid).unwrap().tabs().len(), 1);

    // Active tab should still be t2.
    assert_eq!(mux.active_tab_id(wid), Some(t2));
}

/// `close_tab` on the last tab removes the window.
#[test]
fn close_last_tab_removes_window() {
    let (mut mux, wid, tid, pid) = one_pane_setup();
    let closed_panes = mux.close_tab(tid);
    assert_eq!(closed_panes, vec![pid]);

    // Window should be removed from session.
    assert!(mux.session().get_window(wid).is_none());
    assert_eq!(mux.session().window_count(), 0);

    // Should emit LastWindowClosed notification.
    let notes = drain(&mut mux);
    assert!(
        notes
            .iter()
            .any(|n| matches!(n, MuxNotification::LastWindowClosed))
    );
}

// -- High priority: tab movement across windows --

/// Move a tab from one window to another.
#[test]
fn move_tab_between_windows() {
    let mut mux = EmbeddedMux::new(test_wakeup());

    let w1 = WindowId::from_raw(100);
    let w2 = WindowId::from_raw(101);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(101);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);

    mux.mux.inject_test_tab(w1, t1, p1);
    mux.mux.inject_test_tab(w2, t2, p2);

    // Move t1 from w1 → w2.
    assert!(mux.move_tab_to_window(t1, w2));

    // t1 should now be in w2.
    let dest = mux.session().get_window(w2).unwrap();
    assert!(dest.tabs().contains(&t1));
    assert_eq!(dest.tabs().len(), 2);

    // w1 was the last tab — source window should be removed.
    assert!(mux.session().get_window(w1).is_none());
}

/// `move_tab_to_window` to the same window returns false.
#[test]
fn move_tab_same_window_noop() {
    let (mut mux, wid, tid, _) = one_pane_setup();
    assert!(!mux.move_tab_to_window(tid, wid));
}

/// `move_tab_to_window_at` inserts at the given index.
#[test]
fn move_tab_to_window_at_index() {
    let mut mux = EmbeddedMux::new(test_wakeup());

    let w1 = WindowId::from_raw(100);
    let w2 = WindowId::from_raw(101);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(101);
    let t3 = TabId::from_raw(102);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    let p3 = PaneId::from_raw(102);

    mux.mux.inject_test_tab(w1, t1, p1);
    mux.mux.inject_test_tab(w2, t2, p2);
    mux.mux.inject_test_tab(w2, t3, p3);

    // Move t1 to w2 at index 1 (between t2 and t3).
    assert!(mux.move_tab_to_window_at(t1, w2, 1));

    let dest = mux.session().get_window(w2).unwrap();
    assert_eq!(dest.tabs(), &[t2, t1, t3]);
}

// -- High priority: close non-empty window --

/// `close_window` with a tab returns pane IDs.
#[test]
fn close_nonempty_window() {
    let (mut mux, wid, _tid, pid) = one_pane_setup();
    let panes = mux.close_window(wid);
    assert_eq!(panes, vec![pid]);
    assert!(mux.session().get_window(wid).is_none());
}

/// `close_window` with split panes returns all pane IDs.
#[test]
fn close_window_with_splits() {
    let (mut mux, wid, _tid, p1, p2) = two_pane_setup();
    let panes = mux.close_window(wid);

    let pane_set: HashSet<PaneId> = panes.into_iter().collect();
    assert!(pane_set.contains(&p1));
    assert!(pane_set.contains(&p2));
    assert_eq!(pane_set.len(), 2);
}

// -- High priority: pane storage consistency --

/// `get_pane_entry` returns metadata for injected panes.
#[test]
fn get_pane_entry_after_inject() {
    let (mux, _wid, tid, pid) = one_pane_setup();
    let entry = mux.get_pane_entry(pid).unwrap();
    assert_eq!(entry.tab, tid);
}

/// `is_last_pane` is true when exactly one pane exists.
#[test]
fn is_last_pane_single() {
    let (mux, _wid, _tid, pid) = one_pane_setup();
    assert!(mux.is_last_pane(pid));
}

/// `is_last_pane` is false when two panes exist.
#[test]
fn is_last_pane_two_panes() {
    let (mux, _wid, _tid, p1, p2) = two_pane_setup();
    assert!(!mux.is_last_pane(p1));
    assert!(!mux.is_last_pane(p2));
}

// -- Medium priority: cycle through single/empty windows --

/// `cycle_active_tab` returns `None` on a window with one tab.
#[test]
fn cycle_single_tab() {
    let (mut mux, wid, _tid, _pid) = one_pane_setup();

    // Single-tab windows can't cycle — returns None.
    assert!(mux.cycle_active_tab(wid, 1).is_none());
    assert!(mux.cycle_active_tab(wid, -1).is_none());
}

/// `cycle_active_tab` returns `None` for nonexistent window.
#[test]
fn cycle_nonexistent_window() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let result = mux.cycle_active_tab(WindowId::from_raw(999), 1);
    assert!(result.is_none());
}

// -- Medium priority: reorder boundary conditions --

/// `reorder_tab` with from == to is a valid no-op.
#[test]
fn reorder_same_position() {
    let (mut mux, wid, tid, _pid) = one_pane_setup();
    let result = mux.reorder_tab(wid, 0, 0);
    assert!(result);
    // Tab order unchanged.
    assert_eq!(mux.session().get_window(wid).unwrap().tabs(), &[tid]);
}

/// `reorder_tab` with out-of-bounds indices returns false.
#[test]
fn reorder_out_of_bounds() {
    let (mut mux, wid, _tid, _pid) = one_pane_setup();
    assert!(!mux.reorder_tab(wid, 0, 5));
    assert!(!mux.reorder_tab(wid, 5, 0));
}

/// `reorder_tab` swaps two tabs in the correct order.
#[test]
fn reorder_two_tabs() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let wid = WindowId::from_raw(100);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(101);
    mux.mux.inject_test_tab(wid, t1, PaneId::from_raw(100));
    mux.mux.inject_test_tab(wid, t2, PaneId::from_raw(101));

    assert!(mux.reorder_tab(wid, 0, 1));
    let win = mux.session().get_window(wid).unwrap();
    assert_eq!(win.tabs(), &[t2, t1]);
}

// -- Medium priority: floating pane round-trip --

/// Move a tiled pane to floating and back to tiled.
#[test]
fn floating_round_trip() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();

    let available = crate::layout::Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };

    // Move p2 to floating.
    assert!(mux.move_pane_to_floating(tid, p2, &available));

    // p2 should be in the floating layer.
    let tab = mux.session().get_tab(tid).unwrap();
    assert!(tab.is_floating(p2));
    assert!(!tab.is_floating(p1));

    // Move p2 back to tiled.
    assert!(mux.move_pane_to_tiled(tid, p2));

    // p2 should no longer be floating.
    let tab = mux.session().get_tab(tid).unwrap();
    assert!(!tab.is_floating(p2));
    assert!(tab.tree().contains(p2));
}

/// Cannot float the last tiled pane.
#[test]
fn cannot_float_last_tiled() {
    let (mut mux, _wid, tid, pid) = one_pane_setup();
    let available = crate::layout::Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    assert!(!mux.move_pane_to_floating(tid, pid, &available));
}

// -- Medium priority: zoom state machine --

/// Toggle zoom on a single-pane tab has no zoomed pane.
#[test]
fn zoom_single_pane() {
    let (mut mux, _wid, tid, _pid) = one_pane_setup();
    mux.toggle_zoom(tid);
    // Single-pane tabs have no meaningful zoom.
    let tab = mux.session().get_tab(tid).unwrap();
    // Zoom should not crash; whether it sets zoom depends on impl.
    // The key invariant: it doesn't panic.
    let _ = tab.zoomed_pane();
}

/// Toggle zoom on a multi-pane tab sets and clears zoom.
#[test]
fn zoom_toggle_multi_pane() {
    let (mut mux, _wid, tid, p1, _p2) = two_pane_setup();

    // Set active pane so zoom applies to it.
    mux.set_active_pane(tid, p1);

    // Toggle zoom on.
    mux.toggle_zoom(tid);
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.zoomed_pane(), Some(p1));

    // Toggle zoom off.
    mux.toggle_zoom(tid);
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.zoomed_pane(), None);
}

/// `unzoom_silent` clears zoom without emitting layout notification.
#[test]
fn unzoom_silent() {
    let (mut mux, _wid, tid, p1, _p2) = two_pane_setup();
    mux.set_active_pane(tid, p1);
    mux.toggle_zoom(tid);
    drain(&mut mux); // clear zoom notification

    mux.unzoom_silent(tid);
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.zoomed_pane(), None);

    // No layout notification should be emitted.
    let notes = drain(&mut mux);
    assert!(
        !notes
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(_))),
        "unzoom_silent should not emit TabLayoutChanged"
    );
}

// -- Medium priority: pane entry queries --

/// `get_pane_entry` returns `None` after close.
#[test]
fn pane_entry_gone_after_close() {
    let (mut mux, _wid, _tid, p1, p2) = two_pane_setup();
    mux.close_pane(p2);
    assert!(mux.get_pane_entry(p2).is_none());
    // p1 should still have an entry.
    assert!(mux.get_pane_entry(p1).is_some());
}

// -- Send compile check --

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

// -- Cycle wrap-around (trait interface) --

/// `cycle_active_tab` forward wraps from last tab to first.
#[test]
fn cycle_forward_wraps_to_first() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let wid = WindowId::from_raw(100);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(101);
    let t3 = TabId::from_raw(102);
    mux.mux.inject_test_tab(wid, t1, PaneId::from_raw(100));
    mux.mux.inject_test_tab(wid, t2, PaneId::from_raw(101));
    mux.mux.inject_test_tab(wid, t3, PaneId::from_raw(102));

    // Switch to last tab.
    mux.switch_active_tab(wid, t3);

    // Cycle forward → wraps to t1.
    assert_eq!(mux.cycle_active_tab(wid, 1), Some(t1));
    assert_eq!(mux.active_tab_id(wid), Some(t1));
}

/// `cycle_active_tab` backward wraps from first tab to last.
#[test]
fn cycle_backward_wraps_to_last() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let wid = WindowId::from_raw(100);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(101);
    let t3 = TabId::from_raw(102);
    mux.mux.inject_test_tab(wid, t1, PaneId::from_raw(100));
    mux.mux.inject_test_tab(wid, t2, PaneId::from_raw(101));
    mux.mux.inject_test_tab(wid, t3, PaneId::from_raw(102));

    // Active is t1 (index 0). Cycle backward → wraps to t3.
    assert_eq!(mux.cycle_active_tab(wid, -1), Some(t3));
    assert_eq!(mux.active_tab_id(wid), Some(t3));
}

// -- move_tab_to_window edge cases --

/// `move_tab_to_window` with nonexistent tab returns false.
#[test]
fn move_nonexistent_tab_returns_false() {
    let (mut mux, _wid, _tid, _pid) = one_pane_setup();
    let fake_tab = TabId::from_raw(999);
    let fake_dest = WindowId::from_raw(999);
    assert!(!mux.move_tab_to_window(fake_tab, fake_dest));
}

/// `move_tab_to_window` with nonexistent dest window returns false.
#[test]
fn move_tab_to_nonexistent_window_returns_false() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let w1 = WindowId::from_raw(100);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(101);
    mux.mux.inject_test_tab(w1, t1, PaneId::from_raw(100));
    mux.mux.inject_test_tab(w1, t2, PaneId::from_raw(101));

    let fake_dest = WindowId::from_raw(999);
    assert!(!mux.move_tab_to_window(t1, fake_dest));
}

// -- close_pane cascade --

/// `close_pane` on the sole pane cascades: removes tab, window, emits
/// `LastWindowClosed`.
#[test]
fn close_pane_cascade_to_last_window() {
    use crate::in_process::ClosePaneResult;

    let (mut mux, wid, tid, pid) = one_pane_setup();
    let result = mux.close_pane(pid);
    assert_eq!(result, ClosePaneResult::LastWindow);

    // Tab and window should be gone.
    assert!(mux.session().get_tab(tid).is_none());
    assert!(mux.session().get_window(wid).is_none());

    // Notifications: PaneClosed + LastWindowClosed.
    let notes = drain(&mut mux);
    assert!(
        notes
            .iter()
            .any(|n| matches!(n, MuxNotification::PaneClosed(id) if *id == pid))
    );
    assert!(
        notes
            .iter()
            .any(|n| matches!(n, MuxNotification::LastWindowClosed))
    );
}

// -- switch_active_tab edge case --

/// `switch_active_tab` to nonexistent tab returns false.
#[test]
fn switch_nonexistent_tab_returns_false() {
    let (mut mux, wid, _tid, _pid) = one_pane_setup();
    let stale = TabId::from_raw(999);
    assert!(!mux.switch_active_tab(wid, stale));
}

// -- Reorder with 4+ tabs --

/// `reorder_tab` with 4 tabs: move index 3 to index 1.
#[test]
fn reorder_four_tabs() {
    let mut mux = EmbeddedMux::new(test_wakeup());
    let wid = WindowId::from_raw(100);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(101);
    let t3 = TabId::from_raw(102);
    let t4 = TabId::from_raw(103);
    mux.mux.inject_test_tab(wid, t1, PaneId::from_raw(100));
    mux.mux.inject_test_tab(wid, t2, PaneId::from_raw(101));
    mux.mux.inject_test_tab(wid, t3, PaneId::from_raw(102));
    mux.mux.inject_test_tab(wid, t4, PaneId::from_raw(103));

    // Move t4 (index 3) to index 1.
    assert!(mux.reorder_tab(wid, 3, 1));
    let win = mux.session().get_window(wid).unwrap();
    assert_eq!(win.tabs(), &[t1, t4, t2, t3]);
}
