//! Tests for `InProcessMux` logic operations.
//!
//! These tests exercise the mux's registry/tree/notification logic without
//! spawning real PTYs. We construct the mux, then manually register panes
//! and build tab/window state to test close, split-tree, and event pump
//! behaviour in isolation.

use oriterm_mux::layout::SplitDirection;
use oriterm_mux::registry::PaneEntry;
use oriterm_mux::session::{MuxTab, MuxWindow};
use oriterm_mux::{PaneId, TabId, WindowId};

use super::{ClosePaneResult, InProcessMux, MuxNotification};
use crate::mux_event::MuxEvent;

/// Build a mux with pre-wired state: one window, one tab, one pane.
///
/// Returns `(mux, window_id, tab_id, pane_id)`. IDs use raw values starting
/// at 100 to avoid collision with the mux's own allocator (which starts at 1).
fn one_pane_setup() -> (InProcessMux, WindowId, TabId, PaneId) {
    let mut mux = InProcessMux::new();

    let wid = WindowId::from_raw(100);
    let tid = TabId::from_raw(100);
    let pid = PaneId::from_raw(100);
    let did = mux.default_domain();

    mux.session.add_window(MuxWindow::new(wid));
    mux.session.get_window_mut(wid).unwrap().add_tab(tid);

    let tab = MuxTab::new(tid, pid);
    mux.session.add_tab(tab);

    mux.pane_registry.register(PaneEntry {
        pane: pid,
        tab: tid,
        domain: did,
    });

    // Drain any notifications from setup.
    mux.drain_notifications();

    (mux, wid, tid, pid)
}

/// Build a mux with two panes split in one tab.
fn two_pane_setup() -> (InProcessMux, WindowId, TabId, PaneId, PaneId) {
    let mut mux = InProcessMux::new();

    let wid = WindowId::from_raw(100);
    let tid = TabId::from_raw(100);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    let did = mux.default_domain();

    mux.session.add_window(MuxWindow::new(wid));
    mux.session.get_window_mut(wid).unwrap().add_tab(tid);

    let mut tab = MuxTab::new(tid, p1);
    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree);
    mux.session.add_tab(tab);

    mux.pane_registry.register(PaneEntry {
        pane: p1,
        tab: tid,
        domain: did,
    });
    mux.pane_registry.register(PaneEntry {
        pane: p2,
        tab: tid,
        domain: did,
    });

    mux.drain_notifications();

    (mux, wid, tid, p1, p2)
}

// -- Constructor --

#[test]
fn new_creates_empty_mux() {
    let mux = InProcessMux::new();
    assert!(mux.pane_registry().is_empty());
    assert_eq!(mux.session().tab_count(), 0);
    assert_eq!(mux.session().window_count(), 0);
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
fn close_one_of_two_panes_returns_pane_removed() {
    let (mut mux, _wid, tid, _p1, p2) = two_pane_setup();

    let result = mux.close_pane(p2);
    assert_eq!(result, ClosePaneResult::PaneRemoved);

    // p2 should be gone from registry.
    assert!(mux.get_pane_entry(p2).is_none());

    // Tab should still exist with one pane.
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.all_panes().len(), 1);

    // Notifications should include PaneClosed and TabLayoutChanged.
    let notifs = mux.drain_notifications();
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::PaneClosed(id) if *id == p2))
    );
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid))
    );
}

#[test]
fn close_active_pane_reassigns_active() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();

    // Set p1 as active, then close it.
    mux.session_mut()
        .get_tab_mut(tid)
        .unwrap()
        .set_active_pane(p1);

    let result = mux.close_pane(p1);
    assert_eq!(result, ClosePaneResult::PaneRemoved);

    // Active pane should now be p2.
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.active_pane(), p2);
}

#[test]
fn close_last_pane_in_last_window_returns_last_window() {
    let (mut mux, wid, _tid, pid) = one_pane_setup();

    let result = mux.close_pane(pid);
    assert_eq!(result, ClosePaneResult::LastWindow);

    // Window and tab should both be removed.
    assert!(mux.session().get_window(wid).is_none());
    assert_eq!(mux.session().window_count(), 0);
}

#[test]
fn close_last_pane_in_tab_with_other_tabs_remaining() {
    // Two tabs in one window: closing the sole pane in tab 1 should
    // return TabClosed (not LastWindow) because tab 2 still exists.
    let mut mux = InProcessMux::new();
    let wid = WindowId::from_raw(100);
    let did = mux.default_domain();

    let tid1 = TabId::from_raw(100);
    let tid2 = TabId::from_raw(101);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);

    let mut win = MuxWindow::new(wid);
    win.add_tab(tid1);
    win.add_tab(tid2);
    mux.session.add_window(win);

    mux.session.add_tab(MuxTab::new(tid1, p1));
    mux.session.add_tab(MuxTab::new(tid2, p2));

    mux.pane_registry.register(PaneEntry {
        pane: p1,
        tab: tid1,
        domain: did,
    });
    mux.pane_registry.register(PaneEntry {
        pane: p2,
        tab: tid2,
        domain: did,
    });
    mux.drain_notifications();

    let result = mux.close_pane(p1);
    assert_eq!(result, ClosePaneResult::TabClosed { tab_id: tid1 });

    // Window should still exist with one tab.
    let win = mux.session().get_window(wid).unwrap();
    assert_eq!(win.tabs(), &[tid2]);

    // WindowTabsChanged notification emitted.
    let notifs = mux.drain_notifications();
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::WindowTabsChanged(id) if *id == wid))
    );
}

// -- close_tab --

#[test]
fn close_tab_removes_all_panes() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();

    let pane_ids = mux.close_tab(tid);
    assert_eq!(pane_ids.len(), 2);
    assert!(pane_ids.contains(&p1));
    assert!(pane_ids.contains(&p2));

    // Tab and panes should be gone.
    assert!(mux.session().get_tab(tid).is_none());
    assert!(mux.get_pane_entry(p1).is_none());
    assert!(mux.get_pane_entry(p2).is_none());
}

#[test]
fn close_nonexistent_tab_returns_empty() {
    let mut mux = InProcessMux::new();
    let panes = mux.close_tab(TabId::from_raw(999));
    assert!(panes.is_empty());
}

// -- close_window --

#[test]
fn close_window_removes_all_tabs_and_panes() {
    let (mut mux, wid, tid, p1, p2) = two_pane_setup();

    let pane_ids = mux.close_window(wid);
    assert_eq!(pane_ids.len(), 2);
    assert!(pane_ids.contains(&p1));
    assert!(pane_ids.contains(&p2));

    assert!(mux.session().get_window(wid).is_none());
    assert!(mux.session().get_tab(tid).is_none());
}

#[test]
fn close_nonexistent_window_returns_empty() {
    let mut mux = InProcessMux::new();
    let panes = mux.close_window(WindowId::from_raw(999));
    assert!(panes.is_empty());
}

// -- create_window --

#[test]
fn create_window_allocates_unique_ids() {
    let mut mux = InProcessMux::new();
    let w1 = mux.create_window();
    let w2 = mux.create_window();
    assert_ne!(w1, w2);
    assert_eq!(mux.session().window_count(), 2);
}

// -- event pump --

#[test]
fn poll_events_handles_pane_exited() {
    let (mut mux, _wid, tid, _p1, p2) = two_pane_setup();
    let tx = mux.event_tx().clone();

    // Simulate a PaneExited event.
    tx.send(MuxEvent::PaneExited {
        pane_id: p2,
        exit_code: 0,
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    // p2 should be gone from registry.
    assert!(mux.get_pane_entry(p2).is_none());

    // Tab should still exist with one pane.
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.all_panes().len(), 1);
}

#[test]
fn poll_events_handles_title_change() {
    let (mut mux, _wid, _tid, pid) = one_pane_setup();
    let tx = mux.event_tx().clone();

    // We need a real Pane to test title changes, but we can verify
    // the event is processed without panic even if pane is absent.
    tx.send(MuxEvent::PaneTitleChanged {
        pane_id: pid,
        title: "new title".to_string(),
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    // No pane in the map — should not panic.
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

    let notifs = mux.drain_notifications();
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

    let notifs = mux.drain_notifications();
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::Alert(id) if *id == pid))
    );
}

#[test]
fn drain_notifications_clears_queue() {
    let (mut mux, _wid, _tid, pid) = one_pane_setup();
    mux.close_pane(pid);

    let first = mux.drain_notifications();
    assert!(!first.is_empty());

    let second = mux.drain_notifications();
    assert!(second.is_empty());
}

// -- get_pane_entry --

#[test]
fn get_pane_entry_returns_correct_metadata() {
    let (mux, _wid, tid, pid) = one_pane_setup();
    let entry = mux.get_pane_entry(pid).unwrap();
    assert_eq!(entry.pane, pid);
    assert_eq!(entry.tab, tid);
}

// -- event_tx cloning --

#[test]
fn event_tx_can_be_cloned_and_used() {
    let mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    // Should be able to send events through the cloned sender.
    tx.send(MuxEvent::PaneBell(PaneId::from_raw(1))).unwrap();

    // Verify event is received internally.
    let event = mux.event_rx.try_recv().unwrap();
    assert!(matches!(event, MuxEvent::PaneBell(_)));
}

// -- poll_events with disconnected sender --

#[test]
fn poll_events_with_empty_channel_is_noop() {
    let mut mux = InProcessMux::new();
    let mut panes = std::collections::HashMap::new();
    // No events sent — should not panic.
    mux.poll_events(&mut panes);
    assert!(mux.drain_notifications().is_empty());
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

    let notifs = mux.drain_notifications();
    assert_eq!(notifs.len(), 3);
}

// -- Pane exit cascading to last-window detection --

#[test]
fn pane_exited_event_triggers_last_window_close() {
    let (mut mux, _wid, _tid, pid) = one_pane_setup();
    let tx = mux.event_tx().clone();

    tx.send(MuxEvent::PaneExited {
        pane_id: pid,
        exit_code: 0,
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    // All registries should be empty after the last pane exits.
    assert!(mux.pane_registry().is_empty());
    assert_eq!(mux.session().tab_count(), 0);
    assert_eq!(mux.session().window_count(), 0);
}

// -- High priority: multiple simultaneous pane exits --

#[test]
fn multiple_pane_exits_cascade_cleanly() {
    // Window with one tab containing 3 panes. All three exit in one
    // poll_events batch. Registries must be fully clean afterward.
    let mut mux = InProcessMux::new();
    let wid = WindowId::from_raw(100);
    let tid = TabId::from_raw(100);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    let p3 = PaneId::from_raw(102);
    let did = mux.default_domain();

    mux.session.add_window(MuxWindow::new(wid));
    mux.session.get_window_mut(wid).unwrap().add_tab(tid);

    let mut tab = MuxTab::new(tid, p1);
    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree);
    let tree = tab.tree().split_at(p2, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree);
    mux.session.add_tab(tab);

    for &pid in &[p1, p2, p3] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    mux.drain_notifications();

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

    // All registries should be empty — no orphaned entries.
    assert!(mux.pane_registry().is_empty());
    assert_eq!(mux.session().tab_count(), 0);
    assert_eq!(mux.session().window_count(), 0);
}

// -- High priority: close window with multiple tabs --

#[test]
fn close_window_with_multiple_tabs_varying_splits() {
    // Window with 3 tabs: tab1 has 1 pane, tab2 has 2 panes, tab3 has 3 panes.
    // close_window should return all 6 pane IDs.
    let mut mux = InProcessMux::new();
    let wid = WindowId::from_raw(100);
    let did = mux.default_domain();

    let tid1 = TabId::from_raw(100);
    let tid2 = TabId::from_raw(101);
    let tid3 = TabId::from_raw(102);

    let p1 = PaneId::from_raw(100);
    let p2a = PaneId::from_raw(200);
    let p2b = PaneId::from_raw(201);
    let p3a = PaneId::from_raw(300);
    let p3b = PaneId::from_raw(301);
    let p3c = PaneId::from_raw(302);

    let mut win = MuxWindow::new(wid);
    win.add_tab(tid1);
    win.add_tab(tid2);
    win.add_tab(tid3);
    mux.session.add_window(win);

    // Tab 1: single pane.
    mux.session.add_tab(MuxTab::new(tid1, p1));

    // Tab 2: two panes.
    let mut tab2 = MuxTab::new(tid2, p2a);
    let tree = tab2
        .tree()
        .split_at(p2a, SplitDirection::Vertical, p2b, 0.5);
    tab2.set_tree(tree);
    mux.session.add_tab(tab2);

    // Tab 3: three panes.
    let mut tab3 = MuxTab::new(tid3, p3a);
    let tree = tab3
        .tree()
        .split_at(p3a, SplitDirection::Vertical, p3b, 0.5);
    tab3.set_tree(tree);
    let tree = tab3
        .tree()
        .split_at(p3b, SplitDirection::Horizontal, p3c, 0.5);
    tab3.set_tree(tree);
    mux.session.add_tab(tab3);

    // Register all 6 panes.
    for &(pid, tid) in &[
        (p1, tid1),
        (p2a, tid2),
        (p2b, tid2),
        (p3a, tid3),
        (p3b, tid3),
        (p3c, tid3),
    ] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    mux.drain_notifications();

    let pane_ids = mux.close_window(wid);
    assert_eq!(pane_ids.len(), 6);
    for &pid in &[p1, p2a, p2b, p3a, p3b, p3c] {
        assert!(pane_ids.contains(&pid));
    }

    // Everything should be gone.
    assert!(mux.session().get_window(wid).is_none());
    assert!(mux.pane_registry().is_empty());
    assert_eq!(mux.session().tab_count(), 0);
    assert_eq!(mux.session().window_count(), 0);
}

// -- High priority: close_tab emits WindowTabsChanged --

#[test]
fn close_tab_emits_window_tabs_changed() {
    let (mut mux, wid, tid, _p1, _p2) = two_pane_setup();
    mux.drain_notifications();

    mux.close_tab(tid);

    let notifs = mux.drain_notifications();
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::WindowTabsChanged(id) if *id == wid))
    );
    // Should also emit PaneClosed for each pane.
    assert_eq!(
        notifs
            .iter()
            .filter(|n| matches!(n, MuxNotification::PaneClosed(_)))
            .count(),
        2
    );
}

// -- High priority: poll_events CWD change with missing pane --

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
    // Pane not in map — should not panic.
    mux.poll_events(&mut panes);
}

// -- Medium priority: notification ordering after close_pane --

#[test]
fn close_pane_notification_order_pane_closed_before_layout() {
    let (mut mux, _wid, tid, _p1, p2) = two_pane_setup();
    mux.drain_notifications();

    mux.close_pane(p2);

    let notifs = mux.drain_notifications();
    let closed_idx = notifs
        .iter()
        .position(|n| matches!(n, MuxNotification::PaneClosed(id) if *id == p2))
        .expect("PaneClosed notification missing");
    let layout_idx = notifs
        .iter()
        .position(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid))
        .expect("TabLayoutChanged notification missing");

    // PaneClosed must come before TabLayoutChanged so the GUI can
    // clean up pane references before re-laying out the tab.
    assert!(
        closed_idx < layout_idx,
        "PaneClosed (idx {closed_idx}) should precede TabLayoutChanged (idx {layout_idx})"
    );
}

// -- Medium priority: double close_pane --

#[test]
fn close_pane_twice_returns_not_found_on_second_call() {
    let (mut mux, _wid, _tid, _p1, p2) = two_pane_setup();

    let first = mux.close_pane(p2);
    assert_eq!(first, ClosePaneResult::PaneRemoved);

    let second = mux.close_pane(p2);
    assert_eq!(second, ClosePaneResult::NotFound);
}

// -- Medium priority: poll_events PtyWrite with missing pane --

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
    // Pane not in map — should not panic.
    mux.poll_events(&mut panes);
}

// -- Medium priority: close_tab on orphaned tab --

#[test]
fn close_tab_orphaned_no_window() {
    // Tab exists in session but no window references it.
    let mut mux = InProcessMux::new();
    let tid = TabId::from_raw(100);
    let pid = PaneId::from_raw(100);
    let did = mux.default_domain();

    mux.session.add_tab(MuxTab::new(tid, pid));
    mux.pane_registry.register(PaneEntry {
        pane: pid,
        tab: tid,
        domain: did,
    });
    mux.drain_notifications();

    // close_tab should still work — removes tab and panes, no panic.
    let pane_ids = mux.close_tab(tid);
    assert_eq!(pane_ids, vec![pid]);
    assert!(mux.session().get_tab(tid).is_none());
    assert!(mux.get_pane_entry(pid).is_none());
}

// -- Low priority: drain_notifications preserves insertion order --

#[test]
fn drain_notifications_preserves_insertion_order() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);

    // Send events in a known order.
    tx.send(MuxEvent::PaneBell(p1)).unwrap();
    tx.send(MuxEvent::PaneOutput(p2)).unwrap();
    tx.send(MuxEvent::PaneBell(p3)).unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = mux.drain_notifications();
    assert_eq!(notifs.len(), 3);

    // Verify FIFO order.
    assert!(matches!(&notifs[0], MuxNotification::Alert(id) if *id == p1));
    assert!(matches!(&notifs[1], MuxNotification::PaneDirty(id) if *id == p2));
    assert!(matches!(&notifs[2], MuxNotification::Alert(id) if *id == p3));
}

// -- Low priority: Send trait bound --

/// Compile-time assertion that key mux types are `Send`.
///
/// Prevents accidental introduction of non-Send fields (e.g., `Rc`).
#[test]
fn mux_types_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<InProcessMux>();
    assert_send::<ClosePaneResult>();
    assert_send::<MuxNotification>();
}
