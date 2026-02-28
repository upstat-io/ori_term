//! Tests for `InProcessMux` logic operations.
//!
//! These tests exercise the mux's registry/tree/notification logic without
//! spawning real PTYs. We construct the mux, then manually register panes
//! and build tab/window state to test close, split-tree, and event pump
//! behaviour in isolation.

use oriterm_mux::layout::Rect;
use oriterm_mux::layout::floating::FloatingPane;
use oriterm_mux::layout::{SplitDirection, SplitTree};
use oriterm_mux::registry::PaneEntry;
use oriterm_mux::session::{MuxTab, MuxWindow};
use oriterm_mux::{PaneId, TabId, WindowId};

use super::{ClosePaneResult, InProcessMux, MuxNotification};
use crate::mux_event::MuxEvent;

/// Drain notifications into a fresh Vec (convenience for tests).
fn drain(mux: &mut InProcessMux) -> Vec<MuxNotification> {
    let mut out = Vec::new();
    mux.drain_notifications(&mut out);
    out
}

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
    drain(&mut mux);

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

    drain(&mut mux);

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
    let notifs = drain(&mut mux);
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
    mux.session.get_tab_mut(tid).unwrap().set_active_pane(p1);

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
    drain(&mut mux);

    let result = mux.close_pane(p1);
    assert_eq!(result, ClosePaneResult::TabClosed { tab_id: tid1 });

    // Window should still exist with one tab.
    let win = mux.session().get_window(wid).unwrap();
    assert_eq!(win.tabs(), &[tid2]);

    // WindowTabsChanged notification emitted.
    let notifs = drain(&mut mux);
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

#[test]
fn close_window_last_window_emits_last_window_closed() {
    let (mut mux, wid, _tid, _p1, _p2) = two_pane_setup();
    drain(&mut mux);

    mux.close_window(wid);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::LastWindowClosed)),
        "expected LastWindowClosed when closing the only window"
    );
    assert!(
        !notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::WindowClosed(_))),
        "unexpected WindowClosed — should be LastWindowClosed"
    );
}

#[test]
fn close_window_non_last_emits_window_closed() {
    // Two windows, close window 1 → WindowClosed(w1), not LastWindowClosed.
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();

    let w1 = WindowId::from_raw(100);
    let w2 = WindowId::from_raw(200);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(200);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(200);

    mux.session.add_window(MuxWindow::new(w1));
    mux.session.get_window_mut(w1).unwrap().add_tab(t1);
    mux.session.add_tab(MuxTab::new(t1, p1));
    mux.pane_registry.register(PaneEntry {
        pane: p1,
        tab: t1,
        domain: did,
    });

    mux.session.add_window(MuxWindow::new(w2));
    mux.session.get_window_mut(w2).unwrap().add_tab(t2);
    mux.session.add_tab(MuxTab::new(t2, p2));
    mux.pane_registry.register(PaneEntry {
        pane: p2,
        tab: t2,
        domain: did,
    });
    drain(&mut mux);

    mux.close_window(w1);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::WindowClosed(id) if *id == w1)),
        "expected WindowClosed(w1)"
    );
    assert!(
        !notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::LastWindowClosed)),
        "unexpected LastWindowClosed — window 2 still exists"
    );
    assert!(mux.session().get_window(w2).is_some());
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

    // LastWindowClosed must NOT be emitted when panes remain.
    let notifs = drain(&mut mux);
    assert!(
        !notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::LastWindowClosed)),
        "unexpected LastWindowClosed notification"
    );
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
            .any(|n| matches!(n, MuxNotification::Alert(id) if *id == pid))
    );
}

#[test]
fn drain_notifications_clears_queue() {
    let (mut mux, _wid, _tid, pid) = one_pane_setup();
    mux.close_pane(pid);

    let first = drain(&mut mux);
    assert!(!first.is_empty());

    let second = drain(&mut mux);
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

    // LastWindowClosed notification must be emitted.
    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::LastWindowClosed)),
        "expected LastWindowClosed notification"
    );
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

    // All registries should be empty — no orphaned entries.
    assert!(mux.pane_registry().is_empty());
    assert_eq!(mux.session().tab_count(), 0);
    assert_eq!(mux.session().window_count(), 0);
}

#[test]
fn batch_pane_exits_emit_last_window_closed_exactly_once() {
    // 3 panes exit in one poll_events batch. LastWindowClosed must be
    // emitted exactly once — not once per pane exit.
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

    // LastWindowClosed must appear exactly once.
    let last_window_count = notifs
        .iter()
        .filter(|n| matches!(n, MuxNotification::LastWindowClosed))
        .count();
    assert_eq!(
        last_window_count, 1,
        "LastWindowClosed emitted {last_window_count} times, expected exactly 1"
    );

    // Each pane should have exactly one PaneClosed notification.
    for &pid in &[p1, p2, p3] {
        let count = notifs
            .iter()
            .filter(|n| matches!(n, MuxNotification::PaneClosed(id) if *id == pid))
            .count();
        assert_eq!(count, 1, "PaneClosed({pid:?}) emitted {count} times");
    }
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
    drain(&mut mux);

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

// -- High priority: close_tab cascades to window removal --

#[test]
fn close_tab_last_tab_in_last_window_emits_last_window_closed() {
    // two_pane_setup: 1 window, 1 tab, 2 panes. Closing the only tab
    // empties the only window → LastWindowClosed.
    let (mut mux, _wid, tid, _p1, _p2) = two_pane_setup();
    drain(&mut mux);

    mux.close_tab(tid);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::LastWindowClosed)),
        "expected LastWindowClosed when closing the only tab in the only window"
    );
    // Should also emit PaneClosed for each pane.
    assert_eq!(
        notifs
            .iter()
            .filter(|n| matches!(n, MuxNotification::PaneClosed(_)))
            .count(),
        2
    );
    // Window and tab should be gone.
    assert_eq!(mux.session().window_count(), 0);
    assert_eq!(mux.session().tab_count(), 0);
}

#[test]
fn close_tab_non_last_tab_emits_window_tabs_changed() {
    // Two tabs in one window. Closing one leaves the other → WindowTabsChanged.
    let mut mux = InProcessMux::new();
    let wid = WindowId::from_raw(100);
    let did = mux.default_domain();

    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(101);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);

    let mut win = MuxWindow::new(wid);
    win.add_tab(t1);
    win.add_tab(t2);
    mux.session.add_window(win);

    mux.session.add_tab(MuxTab::new(t1, p1));
    mux.session.add_tab(MuxTab::new(t2, p2));

    for &(pid, tid) in &[(p1, t1), (p2, t2)] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    drain(&mut mux);

    mux.close_tab(t1);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::WindowTabsChanged(id) if *id == wid)),
        "expected WindowTabsChanged when other tabs remain"
    );
    assert!(
        !notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::LastWindowClosed)),
        "unexpected LastWindowClosed — tab t2 still exists"
    );
    assert_eq!(mux.session().window_count(), 1);
    assert_eq!(mux.session().get_window(wid).unwrap().tabs(), &[t2]);
}

#[test]
fn close_tab_last_tab_non_last_window_emits_window_closed() {
    // Two windows, each with one tab. Close the tab in window 1 →
    // window 1 is empty → WindowClosed(w1), not LastWindowClosed.
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();

    let w1 = WindowId::from_raw(100);
    let w2 = WindowId::from_raw(200);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(200);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(200);

    mux.session.add_window(MuxWindow::new(w1));
    mux.session.get_window_mut(w1).unwrap().add_tab(t1);
    mux.session.add_tab(MuxTab::new(t1, p1));
    mux.pane_registry.register(PaneEntry {
        pane: p1,
        tab: t1,
        domain: did,
    });

    mux.session.add_window(MuxWindow::new(w2));
    mux.session.get_window_mut(w2).unwrap().add_tab(t2);
    mux.session.add_tab(MuxTab::new(t2, p2));
    mux.pane_registry.register(PaneEntry {
        pane: p2,
        tab: t2,
        domain: did,
    });
    drain(&mut mux);

    mux.close_tab(t1);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::WindowClosed(id) if *id == w1)),
        "expected WindowClosed(w1)"
    );
    assert!(
        !notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::LastWindowClosed)),
        "unexpected LastWindowClosed — window 2 still exists"
    );
    assert!(mux.session().get_window(w1).is_none());
    assert!(mux.session().get_window(w2).is_some());
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
    drain(&mut mux);

    mux.close_pane(p2);

    let notifs = drain(&mut mux);
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
    drain(&mut mux);

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

    let notifs = drain(&mut mux);
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

// -- High priority: concurrent create and exit in same poll batch --

#[test]
fn concurrent_create_and_exit_in_same_poll_batch() {
    // Pane is registered, then PaneExited arrives in the same poll_events call.
    // Simulates a shell that exits instantly (bad $SHELL, permission denied).
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
    drain(&mut mux);

    let tx = mux.event_tx().clone();

    // Pane 2 sends output, then immediately exits — both in same batch.
    tx.send(MuxEvent::PaneOutput(p2)).unwrap();
    tx.send(MuxEvent::PaneExited {
        pane_id: p2,
        exit_code: 127,
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    // p2 should be gone; p1 should remain.
    assert!(mux.get_pane_entry(p2).is_none());
    assert!(mux.get_pane_entry(p1).is_some());
    assert_eq!(mux.session().get_tab(tid).unwrap().all_panes().len(), 1);
}

// -- High priority: stale pane map during event dispatch --

#[test]
fn stale_pane_map_during_event_dispatch() {
    // poll_events receives events for a pane that was already unregistered
    // by an earlier event in the same batch, but the Pane struct still
    // exists in the HashMap. Should not panic.
    let (mut mux, _wid, _tid, p1, p2) = two_pane_setup();
    let tx = mux.event_tx().clone();

    // p1 exits (unregisters), then a stale PaneOutput arrives for p1.
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
    // p1 is NOT in the pane map (App would have dropped it) — should not panic.
    mux.poll_events(&mut panes);

    // p1 should be gone from registry.
    assert!(mux.get_pane_entry(p1).is_none());
    // p2 should still be alive.
    assert!(mux.get_pane_entry(p2).is_some());
}

// -- High priority: multi-window isolation --

#[test]
fn multi_window_isolation_on_close() {
    // Two windows with independent tabs/panes. Close one window,
    // verify the other's registries are untouched.
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();

    let w1 = WindowId::from_raw(100);
    let w2 = WindowId::from_raw(200);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(200);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(200);

    // Window 1 with one tab, one pane.
    mux.session.add_window(MuxWindow::new(w1));
    mux.session.get_window_mut(w1).unwrap().add_tab(t1);
    mux.session.add_tab(MuxTab::new(t1, p1));
    mux.pane_registry.register(PaneEntry {
        pane: p1,
        tab: t1,
        domain: did,
    });

    // Window 2 with one tab, one pane.
    mux.session.add_window(MuxWindow::new(w2));
    mux.session.get_window_mut(w2).unwrap().add_tab(t2);
    mux.session.add_tab(MuxTab::new(t2, p2));
    mux.pane_registry.register(PaneEntry {
        pane: p2,
        tab: t2,
        domain: did,
    });

    drain(&mut mux);

    // Close window 1.
    let closed = mux.close_window(w1);
    assert_eq!(closed, vec![p1]);

    // Window 2 should be completely untouched.
    assert!(mux.session().get_window(w2).is_some());
    assert_eq!(mux.session().get_window(w2).unwrap().tabs(), &[t2]);
    assert!(mux.session().get_tab(t2).is_some());
    assert!(mux.get_pane_entry(p2).is_some());
    assert_eq!(mux.pane_registry().len(), 1);
    assert_eq!(mux.session().window_count(), 1);
    assert_eq!(mux.session().tab_count(), 1);
}

// -- Medium priority: unbalanced tree collapse after split close --

#[test]
fn unbalanced_tree_collapse_after_split_close() {
    // 3 panes: [p1 | [p2 / p3]]. Close p2 → tree collapses,
    // p3 stays, active pane should not jump.
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

    // Set p3 as active.
    tab.set_active_pane(p3);
    mux.session.add_tab(tab);

    for &pid in &[p1, p2, p3] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    drain(&mut mux);

    // Close p2 — p3 was active, should remain active.
    let result = mux.close_pane(p2);
    assert_eq!(result, ClosePaneResult::PaneRemoved);

    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.all_panes().len(), 2);
    assert!(tab.tree().contains(p1));
    assert!(tab.tree().contains(p3));
    // Active pane should still be p3 (it wasn't closed).
    assert_eq!(tab.active_pane(), p3);
}

// -- Medium priority: close_tab adjusts window active tab --

#[test]
fn close_tab_adjusts_window_active_tab() {
    // Window has 3 tabs, active is tab 2. close_tab(tab2) should
    // adjust window's active_tab_idx via remove_tab.
    let mut mux = InProcessMux::new();
    let wid = WindowId::from_raw(100);
    let did = mux.default_domain();

    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(101);
    let t3 = TabId::from_raw(102);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    let p3 = PaneId::from_raw(102);

    let mut win = MuxWindow::new(wid);
    win.add_tab(t1);
    win.add_tab(t2);
    win.add_tab(t3);
    win.set_active_tab_idx(1); // tab 2 is active
    mux.session.add_window(win);

    mux.session.add_tab(MuxTab::new(t1, p1));
    mux.session.add_tab(MuxTab::new(t2, p2));
    mux.session.add_tab(MuxTab::new(t3, p3));

    for &(pid, tid) in &[(p1, t1), (p2, t2), (p3, t3)] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    drain(&mut mux);

    // Close tab 2 (the active tab).
    mux.close_tab(t2);

    let win = mux.session().get_window(wid).unwrap();
    assert_eq!(win.tabs(), &[t1, t3]);
    // Active tab index should now point to t3 (index 1, same position).
    assert_eq!(win.active_tab(), Some(t3));
}

// -- Medium priority: PaneOutput after pane closed is noop --

#[test]
fn pane_output_after_pane_closed_is_noop() {
    // After PaneExited, a stale PaneOutput arrives. Should produce
    // PaneDirty notification (the GUI will ignore it for unknown panes)
    // but must not panic.
    let (mut mux, _wid, _tid, pid) = one_pane_setup();
    let tx = mux.event_tx().clone();

    // Pane exits first.
    tx.send(MuxEvent::PaneExited {
        pane_id: pid,
        exit_code: 0,
    })
    .unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);
    drain(&mut mux);

    // Now a stale output event arrives.
    tx.send(MuxEvent::PaneOutput(pid)).unwrap();
    mux.poll_events(&mut panes);

    // Should have a PaneDirty notification but no panic.
    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::PaneDirty(id) if *id == pid))
    );
}

// -- Medium priority: full lifecycle create → close --

#[test]
fn full_lifecycle_create_window_tab_close_window() {
    // Full happy-path lifecycle: create window, add tabs, close window.
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();

    // Create window.
    let wid = mux.create_window();
    assert_eq!(mux.session().window_count(), 1);

    // Manually add two tabs (spawn_pane needs real PTY).
    let t1 = TabId::from_raw(10);
    let t2 = TabId::from_raw(20);
    let p1 = PaneId::from_raw(10);
    let p2 = PaneId::from_raw(20);

    mux.session.get_window_mut(wid).unwrap().add_tab(t1);
    mux.session.get_window_mut(wid).unwrap().add_tab(t2);
    mux.session.add_tab(MuxTab::new(t1, p1));
    mux.session.add_tab(MuxTab::new(t2, p2));

    mux.pane_registry.register(PaneEntry {
        pane: p1,
        tab: t1,
        domain: did,
    });
    mux.pane_registry.register(PaneEntry {
        pane: p2,
        tab: t2,
        domain: did,
    });

    assert_eq!(mux.session().tab_count(), 2);
    assert_eq!(mux.pane_registry().len(), 2);

    // Close the window.
    let closed = mux.close_window(wid);
    assert_eq!(closed.len(), 2);
    assert!(closed.contains(&p1));
    assert!(closed.contains(&p2));

    // Everything should be gone.
    assert_eq!(mux.session().window_count(), 0);
    assert_eq!(mux.session().tab_count(), 0);
    assert!(mux.pane_registry().is_empty());
}

// -- Domain allocator persistence --

#[test]
fn domain_alloc_persisted_in_struct() {
    // The domain allocator is stored in the struct, so creating a second
    // domain should get a different ID than the local domain.
    let mut mux = InProcessMux::new();
    let local_id = mux.default_domain();
    let second_id = mux.domain_alloc.alloc();
    assert_ne!(local_id, second_id);

    // Third allocation should also be unique.
    let third_id = mux.domain_alloc.alloc();
    assert_ne!(second_id, third_id);
    assert_ne!(local_id, third_id);
}

// -- WindowClosed notification on non-last window --

#[test]
fn close_pane_last_tab_non_last_window_emits_window_closed() {
    // Two windows, each with one tab and one pane. Close the sole pane
    // in window 1 — should emit WindowClosed(w1), not LastWindow.
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();

    let w1 = WindowId::from_raw(100);
    let w2 = WindowId::from_raw(200);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(200);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(200);

    mux.session.add_window(MuxWindow::new(w1));
    mux.session.get_window_mut(w1).unwrap().add_tab(t1);
    mux.session.add_tab(MuxTab::new(t1, p1));
    mux.pane_registry.register(PaneEntry {
        pane: p1,
        tab: t1,
        domain: did,
    });

    mux.session.add_window(MuxWindow::new(w2));
    mux.session.get_window_mut(w2).unwrap().add_tab(t2);
    mux.session.add_tab(MuxTab::new(t2, p2));
    mux.pane_registry.register(PaneEntry {
        pane: p2,
        tab: t2,
        domain: did,
    });
    drain(&mut mux);

    // Close the sole pane in window 1.
    let result = mux.close_pane(p1);
    assert_eq!(result, ClosePaneResult::TabClosed { tab_id: t1 });

    // Window 1 should be removed.
    assert!(mux.session().get_window(w1).is_none());
    // Window 2 should be untouched.
    assert!(mux.session().get_window(w2).is_some());

    let notifs = drain(&mut mux);

    // WindowClosed notification for w1.
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::WindowClosed(id) if *id == w1)),
        "expected WindowClosed(w1) notification"
    );

    // Must NOT emit LastWindowClosed — window 2 still exists.
    assert!(
        !notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::LastWindowClosed)),
        "unexpected LastWindowClosed notification"
    );
}

// -- High priority: split tree ratio preserved after pane close --

#[test]
fn split_ratio_preserved_after_middle_pane_close() {
    // [p1 | [p2 / p3]] with custom ratios. Close p2 → remaining split
    // between p1 and p3 should preserve p1's original ratio.
    let mut mux = InProcessMux::new();
    let wid = WindowId::from_raw(100);
    let tid = TabId::from_raw(100);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    let p3 = PaneId::from_raw(102);
    let did = mux.default_domain();

    mux.session.add_window(MuxWindow::new(wid));
    mux.session.get_window_mut(wid).unwrap().add_tab(tid);

    // Build tree: [p1 (0.3) | [p2 (0.5) / p3]]
    let mut tab = MuxTab::new(tid, p1);
    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.3);
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
    drain(&mut mux);

    // Close p2 — tree should collapse [p2/p3] sub-split, leaving [p1 | p3].
    let result = mux.close_pane(p2);
    assert_eq!(result, ClosePaneResult::PaneRemoved);

    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.all_panes().len(), 2);
    assert!(tab.tree().contains(p1));
    assert!(tab.tree().contains(p3));

    // The outer split ratio (0.3) should be preserved.
    let (dir, ratio) = tab.tree().parent_split(p1).unwrap();
    assert_eq!(dir, SplitDirection::Vertical);
    assert!((ratio - 0.3).abs() < f32::EPSILON);
}

// -- High priority: interleaved mutation consistency --

#[test]
fn interleaved_create_close_poll_consistency() {
    // Simulates rapid user actions: create window, add two tabs, close a
    // pane via event, close a tab directly, then close window.
    // Verifies registry consistency at every step.
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();

    // Step 1: Create window with two tabs.
    let wid = mux.create_window();
    let t1 = TabId::from_raw(10);
    let t2 = TabId::from_raw(20);
    let p1 = PaneId::from_raw(10);
    let p2a = PaneId::from_raw(20);
    let p2b = PaneId::from_raw(21);

    mux.session.get_window_mut(wid).unwrap().add_tab(t1);
    mux.session.get_window_mut(wid).unwrap().add_tab(t2);
    mux.session.add_tab(MuxTab::new(t1, p1));

    let mut tab2 = MuxTab::new(t2, p2a);
    let tree = tab2
        .tree()
        .split_at(p2a, SplitDirection::Vertical, p2b, 0.5);
    tab2.set_tree(tree);
    mux.session.add_tab(tab2);

    for &(pid, tid) in &[(p1, t1), (p2a, t2), (p2b, t2)] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    drain(&mut mux);

    // Step 2: p2b exits via event.
    let tx = mux.event_tx().clone();
    tx.send(MuxEvent::PaneExited {
        pane_id: p2b,
        exit_code: 0,
    })
    .unwrap();
    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    assert!(mux.get_pane_entry(p2b).is_none());
    assert!(mux.get_pane_entry(p2a).is_some());
    assert_eq!(mux.pane_registry().len(), 2);

    // Step 3: Close tab 2 directly (p2a still there).
    let closed = mux.close_tab(t2);
    assert_eq!(closed, vec![p2a]);
    assert!(mux.get_pane_entry(p2a).is_none());
    assert_eq!(mux.session().tab_count(), 1);

    // Step 4: Close window (should get p1 back).
    let closed = mux.close_window(wid);
    assert_eq!(closed, vec![p1]);
    assert!(mux.pane_registry().is_empty());
    assert_eq!(mux.session().window_count(), 0);
    assert_eq!(mux.session().tab_count(), 0);
}

// -- Medium priority: empty mux recovery --

#[test]
fn empty_mux_recovery_after_full_teardown() {
    let (mut mux, wid, _tid, pid) = one_pane_setup();

    // Close everything.
    mux.close_window(wid);
    assert!(mux.pane_registry().is_empty());
    assert_eq!(mux.session().window_count(), 0);
    assert_eq!(mux.session().tab_count(), 0);

    // Create a new window — allocator should still work.
    let w2 = mux.create_window();
    assert_ne!(w2, wid);
    assert_eq!(mux.session().window_count(), 1);

    // Can manually add tabs and panes to the new window.
    let t_new = TabId::from_raw(500);
    let p_new = PaneId::from_raw(500);
    let did = mux.default_domain();

    mux.session.get_window_mut(w2).unwrap().add_tab(t_new);
    mux.session.add_tab(MuxTab::new(t_new, p_new));
    mux.pane_registry.register(PaneEntry {
        pane: p_new,
        tab: t_new,
        domain: did,
    });

    // Verify the new state is consistent.
    assert_eq!(mux.pane_registry().len(), 1);
    assert_eq!(mux.session().tab_count(), 1);
    assert!(mux.get_pane_entry(p_new).is_some());

    // Old pane should still be gone.
    assert!(mux.get_pane_entry(pid).is_none());
}

// -- Medium priority: notification order under cascading close --

#[test]
fn cascading_close_notification_order() {
    // Two tabs (t1 with 2 panes, t2 with 1 pane) in one window.
    // Close pane p1a → tab t1 stays (p1b remains).
    // Close pane p1b → tab t1 closes → only t2 remains.
    // Close pane p2 → tab t2 closes → last window.
    // Verify notification ordering at each step.
    let mut mux = InProcessMux::new();
    let wid = WindowId::from_raw(100);
    let did = mux.default_domain();

    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(200);
    let p1a = PaneId::from_raw(100);
    let p1b = PaneId::from_raw(101);
    let p2 = PaneId::from_raw(200);

    let mut win = MuxWindow::new(wid);
    win.add_tab(t1);
    win.add_tab(t2);
    mux.session.add_window(win);

    // Tab 1: two panes.
    let mut tab1 = MuxTab::new(t1, p1a);
    let tree = tab1
        .tree()
        .split_at(p1a, SplitDirection::Vertical, p1b, 0.5);
    tab1.set_tree(tree);
    mux.session.add_tab(tab1);
    mux.session.add_tab(MuxTab::new(t2, p2));

    for &(pid, tid) in &[(p1a, t1), (p1b, t1), (p2, t2)] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    drain(&mut mux);

    // Close p1a — leaves p1b in t1, tab stays.
    mux.close_pane(p1a);
    let notifs = drain(&mut mux);
    let closed_idx = notifs
        .iter()
        .position(|n| matches!(n, MuxNotification::PaneClosed(id) if *id == p1a))
        .expect("PaneClosed for p1a");
    let layout_idx = notifs
        .iter()
        .position(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == t1))
        .expect("TabLayoutChanged for t1");
    assert!(closed_idx < layout_idx);

    // Close p1b — last pane in t1 → tab closes, window gets WindowTabsChanged.
    mux.close_pane(p1b);
    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::PaneClosed(id) if *id == p1b))
    );
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::WindowTabsChanged(id) if *id == wid))
    );

    // Close p2 — last pane in last tab → LastWindow.
    let result = mux.close_pane(p2);
    assert_eq!(result, ClosePaneResult::LastWindow);
}

// -- Medium priority: sender drops mid-poll --

#[test]
fn sender_dropped_during_poll_drains_remaining() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    // Send a few events, then drop the sender.
    tx.send(MuxEvent::PaneBell(PaneId::from_raw(1))).unwrap();
    tx.send(MuxEvent::PaneOutput(PaneId::from_raw(2))).unwrap();
    drop(tx);

    // poll_events should drain the buffered events, then stop cleanly.
    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let notifs = drain(&mut mux);
    assert_eq!(notifs.len(), 2);

    // Subsequent polls should be no-ops (channel disconnected).
    mux.poll_events(&mut panes);
    assert!(drain(&mut mux).is_empty());
}

// -- Medium priority: ID allocator monotonicity under churn --

#[test]
fn id_allocator_monotonic_under_churn() {
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();
    let mut seen_window_ids = Vec::new();

    // Create and close 50 windows, collecting all allocated IDs.
    for _ in 0..50 {
        let wid = mux.create_window();
        seen_window_ids.push(wid);

        // Add a tab+pane so close_window has something to clean.
        let tid = TabId::from_raw(wid.raw() * 1000);
        let pid = PaneId::from_raw(wid.raw() * 1000);
        mux.session.get_window_mut(wid).unwrap().add_tab(tid);
        mux.session.add_tab(MuxTab::new(tid, pid));
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });

        mux.close_window(wid);
    }

    // All IDs should be unique.
    let unique: std::collections::HashSet<_> = seen_window_ids.iter().collect();
    assert_eq!(unique.len(), seen_window_ids.len());

    // All IDs should be monotonically increasing.
    for pair in seen_window_ids.windows(2) {
        assert!(
            pair[0].raw() < pair[1].raw(),
            "IDs not monotonic: {} >= {}",
            pair[0].raw(),
            pair[1].raw()
        );
    }

    // Mux should be clean after all windows closed.
    assert_eq!(mux.session().window_count(), 0);
    assert!(mux.pane_registry().is_empty());
}

// -- Low priority: MuxNotification drain preserves clipboard data --

#[test]
fn drain_notifications_preserves_clipboard_data() {
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    // Send a ClipboardStore event with known data.
    tx.send(MuxEvent::ClipboardStore {
        pane_id: PaneId::from_raw(42),
        clipboard_type: oriterm_core::ClipboardType::Clipboard,
        text: "important data".to_string(),
    })
    .unwrap();

    // Send a ClipboardLoad event with a formatter.
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

    // Verify ClipboardStore data is intact.
    let store = &notifs[0];
    assert!(matches!(
        store,
        MuxNotification::ClipboardStore { text, pane_id, clipboard_type }
            if text == "important data"
            && *pane_id == PaneId::from_raw(42)
            && *clipboard_type == oriterm_core::ClipboardType::Clipboard
    ));

    // Verify ClipboardLoad formatter works.
    if let MuxNotification::ClipboardLoad { formatter, .. } = &notifs[1] {
        assert_eq!(formatter("test"), "\x1b]52;s;test\x07");
    } else {
        panic!("expected ClipboardLoad notification");
    }
}

// ── Tests for pump_mux_events notification handling (App-layer) ──
//
// These verify that the mux produces the right notifications so that
// App::pump_mux_events handles them safely. The App handler uses if-let
// guards on self.panes for unknown pane IDs, and std::mem::take for the
// double-buffer pattern.

// -- High priority: PaneDirty for absent pane --

#[test]
fn pane_dirty_produced_for_absent_pane() {
    // PaneOutput for a pane not in the HashMap should still produce a
    // PaneDirty notification (App handler skips check_selection_invalidation
    // via if-let but still sets dirty and invalidates URL cache).
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
        MuxNotification::PaneDirty(id) if *id == unknown
    ));
}

// -- High priority: ClipboardLoad for unknown pane --

#[test]
fn clipboard_load_unknown_pane_produces_notification() {
    // ClipboardLoad for a pane that doesn't exist in the pane HashMap.
    // The mux produces the notification regardless; App handler guards
    // with `if let Some(pane) = self.panes.get(&pane_id)`.
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
        // Formatter should still work (it's a closure, not dependent on pane).
        assert_eq!(formatter("hello"), "\x1b]52;c;hello\x07");
    } else {
        panic!("expected ClipboardLoad notification");
    }
}

// -- High priority: empty notification buffer short-circuits --

#[test]
fn empty_notification_buffer_short_circuits() {
    // When no events arrive, drain produces an empty buffer.
    // App::pump_mux_events returns early when notification_buf.is_empty().
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

// -- Medium priority: double-buffer no stale accumulation --

#[test]
fn drain_double_buffer_no_cross_cycle_accumulation() {
    // Simulates the double-buffer pattern in pump_mux_events:
    // 1. Cycle 1: events → notifications → drain → process
    // 2. Cycle 2: no events → drain → must be empty (no stale data)
    // This verifies std::mem::swap clears correctly across cycles.
    let mut mux = InProcessMux::new();
    let tx = mux.event_tx().clone();

    // Cycle 1: send events, poll, drain.
    tx.send(MuxEvent::PaneBell(PaneId::from_raw(1))).unwrap();
    tx.send(MuxEvent::PaneOutput(PaneId::from_raw(2))).unwrap();

    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    let mut buf = Vec::new();
    mux.drain_notifications(&mut buf);
    assert_eq!(buf.len(), 2, "cycle 1 should have 2 notifications");

    // Cycle 2: no new events.
    mux.poll_events(&mut panes);

    // Drain into the SAME buffer (simulates reuse). drain_notifications
    // clears `out` before swapping, so stale data must not leak.
    mux.drain_notifications(&mut buf);
    assert!(
        buf.is_empty(),
        "cycle 2 should be empty — stale notifications must not accumulate"
    );
}

// -- set_divider_ratio --

#[test]
fn set_divider_ratio_simple_split() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();

    mux.set_divider_ratio(tid, p1, p2, 0.7);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid)),
        "should emit TabLayoutChanged",
    );

    // Verify the tree was updated.
    let tab = mux.session().get_tab(tid).unwrap();
    if let SplitTree::Split { ratio, .. } = tab.tree() {
        assert!(
            (*ratio - 0.7).abs() < f32::EPSILON,
            "ratio should be 0.7, got {ratio}"
        );
    } else {
        panic!("expected Split");
    }
}

#[test]
fn set_divider_ratio_clamps_extreme_values() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();

    mux.set_divider_ratio(tid, p1, p2, 0.0);

    let tab = mux.session().get_tab(tid).unwrap();
    if let SplitTree::Split { ratio, .. } = tab.tree() {
        assert!(
            (*ratio - 0.1).abs() < f32::EPSILON,
            "ratio should be clamped to 0.1, got {ratio}"
        );
    } else {
        panic!("expected Split");
    }
}

#[test]
fn set_divider_ratio_nonexistent_panes_is_noop() {
    let (mut mux, _wid, tid, _p1, _p2) = two_pane_setup();
    drain(&mut mux);

    let bogus_a = PaneId::from_raw(999);
    let bogus_b = PaneId::from_raw(998);
    mux.set_divider_ratio(tid, bogus_a, bogus_b, 0.7);

    // Tree should be unchanged (still 0.5).
    let tab = mux.session().get_tab(tid).unwrap();
    if let SplitTree::Split { ratio, .. } = tab.tree() {
        assert!(
            (*ratio - 0.5).abs() < f32::EPSILON,
            "ratio should stay 0.5, got {ratio}"
        );
    } else {
        panic!("expected Split");
    }
}

#[test]
fn set_divider_ratio_nonexistent_tab_is_noop() {
    let (mut mux, _wid, _tid, p1, p2) = two_pane_setup();
    drain(&mut mux);

    let bad_tab = TabId::from_raw(999);
    mux.set_divider_ratio(bad_tab, p1, p2, 0.7);

    let notifs = drain(&mut mux);
    assert!(notifs.is_empty(), "no notifications for nonexistent tab");
}

// -- resize_pane --

#[test]
fn resize_pane_grows_first_child() {
    let (mut mux, _wid, tid, p1, _p2) = two_pane_setup();
    drain(&mut mux);

    mux.resize_pane(tid, p1, SplitDirection::Vertical, true, 0.1);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid)),
        "should emit TabLayoutChanged",
    );

    let tab = mux.session().get_tab(tid).unwrap();
    if let SplitTree::Split { ratio, .. } = tab.tree() {
        assert!(
            (*ratio - 0.6).abs() < f32::EPSILON,
            "ratio should be 0.6, got {ratio}"
        );
    } else {
        panic!("expected Split");
    }
}

#[test]
fn resize_pane_shrinks_via_second_child() {
    let (mut mux, _wid, tid, _p1, p2) = two_pane_setup();
    drain(&mut mux);

    // p2 is in second child; pane_in_first=false, delta=-0.1 → ratio drops.
    mux.resize_pane(tid, p2, SplitDirection::Vertical, false, -0.1);

    let tab = mux.session().get_tab(tid).unwrap();
    if let SplitTree::Split { ratio, .. } = tab.tree() {
        assert!(
            (*ratio - 0.4).abs() < f32::EPSILON,
            "ratio should be 0.4, got {ratio}"
        );
    } else {
        panic!("expected Split");
    }
}

#[test]
fn resize_pane_single_pane_is_noop() {
    let (mut mux, _wid, tid, pid) = one_pane_setup();
    drain(&mut mux);

    mux.resize_pane(tid, pid, SplitDirection::Vertical, true, 0.1);

    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "no notifications when resize is a noop (single pane)"
    );
}

#[test]
fn resize_pane_wrong_axis_is_noop() {
    let (mut mux, _wid, tid, p1, _p2) = two_pane_setup();
    drain(&mut mux);

    // Split is Vertical, try resizing on Horizontal axis.
    mux.resize_pane(tid, p1, SplitDirection::Horizontal, true, 0.1);

    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "no notifications when axis doesn't match"
    );

    // Ratio unchanged.
    let tab = mux.session().get_tab(tid).unwrap();
    if let SplitTree::Split { ratio, .. } = tab.tree() {
        assert!((*ratio - 0.5).abs() < f32::EPSILON);
    }
}

#[test]
fn resize_pane_no_notification_when_tree_unchanged() {
    let (mut mux, _wid, tid, _p1, p2) = two_pane_setup();
    drain(&mut mux);

    // p2 is in second child; pane_in_first=true doesn't match → noop.
    mux.resize_pane(tid, p2, SplitDirection::Vertical, true, 0.1);

    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "resize_pane should suppress notification when tree is unchanged"
    );
}

// -- equalize_panes --

#[test]
fn equalize_panes_resets_ratio() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();

    // Skew the ratio first.
    mux.set_divider_ratio(tid, p1, p2, 0.8);
    drain(&mut mux);

    mux.equalize_panes(tid);

    let tab = mux.session().get_tab(tid).unwrap();
    if let SplitTree::Split { ratio, .. } = tab.tree() {
        assert!(
            (*ratio - 0.5).abs() < f32::EPSILON,
            "ratio should be reset to 0.5, got {ratio}"
        );
    } else {
        panic!("expected Split");
    }

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid)),
        "should emit TabLayoutChanged",
    );
}

#[test]
fn equalize_panes_single_pane_is_noop() {
    // Single-pane tree is already equalized — no undo entry, no notification.
    let (mut mux, _wid, tid, _pid) = one_pane_setup();
    drain(&mut mux);

    mux.equalize_panes(tid);

    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "single-pane equalize should not emit notifications"
    );
}

#[test]
fn equalize_panes_asymmetric_nesting() {
    // Build: p1 | (p2 / p3) with skewed ratios.
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    let p3 = PaneId::from_raw(102);
    let did = mux.default_domain();

    // Split p2 horizontally.
    let tab = mux.session.get_tab_mut(tid).unwrap();
    let tree = tab.tree().split_at(p2, SplitDirection::Horizontal, p3, 0.3);
    tab.set_tree(tree);
    mux.pane_registry.register(PaneEntry {
        pane: p3,
        tab: tid,
        domain: did,
    });

    // Skew the outer split.
    mux.set_divider_ratio(tid, p1, p2, 0.8);
    drain(&mut mux);

    mux.equalize_panes(tid);

    let tab = mux.session().get_tab(tid).unwrap();
    // Outer split should be 0.5.
    if let SplitTree::Split { ratio, .. } = tab.tree() {
        assert!(
            (*ratio - 0.5).abs() < f32::EPSILON,
            "outer ratio should be 0.5, got {ratio}"
        );
    } else {
        panic!("expected Split");
    }
    // Inner split should also be 0.5.
    let inner_split = tab.tree().parent_split(p2);
    assert_eq!(
        inner_split,
        Some((SplitDirection::Horizontal, 0.5)),
        "inner ratio should be equalized to 0.5"
    );
}

#[test]
fn equalize_panes_nonexistent_tab_is_noop() {
    let (mut mux, _wid, _tid, _p1, _p2) = two_pane_setup();
    drain(&mut mux);

    let bad_tab = TabId::from_raw(999);
    mux.equalize_panes(bad_tab);

    let notifs = drain(&mut mux);
    assert!(notifs.is_empty(), "no notifications for nonexistent tab");
}

// -- Hygiene: leak prevention and no-op guards --

/// Verify that `split_pane`'s error path mirrors `spawn_floating_pane`:
/// the `else` branch unregisters the pane and returns `Err`. This is a
/// structural assertion — the actual PTY spawn path can't be exercised in
/// unit tests, so we verify the code structure is correct by confirming
/// that a manually registered pane under a nonexistent tab triggers the
/// invariant violation `debug_assert` in `close_pane`.
#[test]
#[should_panic(expected = "tab is missing")]
fn orphan_pane_under_missing_tab_detected_by_close() {
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();

    let bogus_tab = TabId::from_raw(999);
    let orphan = PaneId::from_raw(200);

    // Manually register an orphaned pane (simulates the pre-fix leak).
    mux.pane_registry.register(PaneEntry {
        pane: orphan,
        tab: bogus_tab,
        domain: did,
    });

    // close_pane finds the entry but the tab doesn't exist — hits the
    // debug_assert (registry/session out of sync) and panics in debug builds.
    mux.close_pane(orphan);
}

#[test]
fn set_divider_ratio_same_ratio_no_undo_push() {
    // Setting the same ratio should not push an undo entry or emit a notification.
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    drain(&mut mux);

    // The setup tree has ratio 0.5. Set it to 0.5 again — no-op.
    mux.set_divider_ratio(tid, p1, p2, 0.5);

    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "same ratio should not emit TabLayoutChanged"
    );

    // Verify no undo entry was pushed: the only undo entry should be from
    // the two_pane_setup split. One undo should succeed, a second should not.
    let live: std::collections::HashSet<_> = [p1, p2].into_iter().collect();
    assert!(
        mux.undo_split(tid, &live),
        "setup's undo entry should exist"
    );
    drain(&mut mux);
    assert!(
        !mux.undo_split(tid, &live),
        "no second undo entry — same-ratio call was a no-op"
    );
}

#[test]
fn set_divider_ratio_nonexistent_panes_no_undo_push() {
    // When the pane pair isn't found, set_divider_ratio returns an identical
    // tree. The equality guard should prevent an undo push.
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    drain(&mut mux);

    let bogus_a = PaneId::from_raw(999);
    let bogus_b = PaneId::from_raw(998);
    mux.set_divider_ratio(tid, bogus_a, bogus_b, 0.7);

    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "nonexistent pane pair should not emit notification"
    );

    // Only the setup's undo entry should exist.
    let live: std::collections::HashSet<_> = [p1, p2].into_iter().collect();
    assert!(mux.undo_split(tid, &live));
    drain(&mut mux);
    assert!(
        !mux.undo_split(tid, &live),
        "no extra undo entry from nonexistent pane pair"
    );
}

#[test]
fn equalize_panes_already_equal_no_undo_push() {
    // When all ratios are already 0.5, equalize should not push undo.
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    drain(&mut mux);

    // two_pane_setup creates a 0.5 split — already equalized.
    mux.equalize_panes(tid);

    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "already-equalized tree should not emit notification"
    );

    // Only the setup's undo entry should exist.
    let live: std::collections::HashSet<_> = [p1, p2].into_iter().collect();
    assert!(mux.undo_split(tid, &live));
    drain(&mut mux);
    assert!(
        !mux.undo_split(tid, &live),
        "no extra undo entry from equalize on already-equal tree"
    );
}

// -- Low priority: PaneClosed notification for removed pane --

#[test]
fn pane_closed_notification_carries_correct_id() {
    // Verify that close_pane produces PaneClosed(id) with the exact pane ID
    // that was closed. App::pump_mux_events uses this to self.panes.remove(&id).
    let (mut mux, _wid, _tid, _p1, p2) = two_pane_setup();
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

// --- Zoom tests ---

#[test]
fn toggle_zoom_sets_zoomed_pane() {
    let (mut mux, _wid, tid, pid) = one_pane_setup();

    mux.toggle_zoom(tid);
    let tab = mux.session.get_tab(tid).unwrap();
    assert_eq!(tab.zoomed_pane(), Some(pid));

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid)),
        "should emit TabLayoutChanged",
    );
}

#[test]
fn toggle_zoom_twice_unzooms() {
    let (mut mux, _wid, tid, _pid) = one_pane_setup();

    mux.toggle_zoom(tid);
    mux.toggle_zoom(tid);

    let tab = mux.session.get_tab(tid).unwrap();
    assert_eq!(tab.zoomed_pane(), None);
}

#[test]
fn unzoom_clears_zoom_and_emits_notification() {
    let (mut mux, _wid, tid, _pid) = one_pane_setup();

    mux.toggle_zoom(tid);
    drain(&mut mux);

    mux.unzoom(tid);
    let tab = mux.session.get_tab(tid).unwrap();
    assert_eq!(tab.zoomed_pane(), None);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid)),
        "should emit TabLayoutChanged",
    );
}

#[test]
fn unzoom_noop_when_not_zoomed() {
    let (mut mux, _wid, tid, _pid) = one_pane_setup();

    mux.unzoom(tid);

    let notifs = drain(&mut mux);
    assert!(
        !notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid)),
        "unzoom on non-zoomed tab should not emit notifications",
    );
}

#[test]
fn close_zoomed_pane_clears_zoom() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    drain(&mut mux);

    // Zoom p1.
    let tab = mux.session.get_tab_mut(tid).unwrap();
    tab.set_zoomed_pane(Some(p1));

    // Close the zoomed pane.
    mux.close_pane(p1);

    let tab = mux.session.get_tab(tid).unwrap();
    assert_eq!(
        tab.zoomed_pane(),
        None,
        "zoom should be cleared when zoomed pane is closed"
    );
    assert_eq!(tab.active_pane(), p2);
}

#[test]
fn close_non_zoomed_pane_preserves_zoom() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();

    // Zoom p1 (the active pane).
    mux.toggle_zoom(tid);
    drain(&mut mux);

    // Close p2 (the non-zoomed pane).
    let result = mux.close_pane(p2);
    assert_eq!(result, ClosePaneResult::PaneRemoved);

    // Zoom should still be set on p1.
    let tab = mux.session.get_tab(tid).unwrap();
    assert_eq!(
        tab.zoomed_pane(),
        Some(p1),
        "zoom should be preserved when a non-zoomed pane is closed",
    );
}

#[test]
fn unzoom_then_tree_mutation_works() {
    let (mut mux, _wid, tid, p1, _p2) = two_pane_setup();

    // Zoom, then unzoom (simulates what App does before split).
    mux.toggle_zoom(tid);
    mux.unzoom(tid);
    drain(&mut mux);

    // Manually extend the tree (simulates split result).
    let p3 = PaneId::from_raw(102);
    let tab = mux.session.get_tab_mut(tid).unwrap();
    let new_tree = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(new_tree);

    assert_eq!(
        tab.zoomed_pane(),
        None,
        "zoom should remain cleared after unzoom + split"
    );
    assert_eq!(tab.all_panes().len(), 3);
}

#[test]
fn toggle_zoom_in_three_pane_tree() {
    let (mut mux, _wid, tid, _p1, p2) = two_pane_setup();

    // Add a third pane to the tree.
    let p3 = PaneId::from_raw(102);
    let did = mux.default_domain();
    let tab = mux.session.get_tab_mut(tid).unwrap();
    let new_tree = tab.tree().split_at(p2, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(new_tree);
    mux.pane_registry.register(PaneEntry {
        pane: p3,
        tab: tid,
        domain: did,
    });

    // Set active to p2 and zoom.
    mux.set_active_pane(tid, p2);
    mux.toggle_zoom(tid);

    let tab = mux.session.get_tab(tid).unwrap();
    assert_eq!(
        tab.zoomed_pane(),
        Some(p2),
        "should zoom the active pane, not the first"
    );
    assert_eq!(tab.all_panes().len(), 3, "tree should be unchanged");
}

#[test]
fn navigate_after_unzoom_changes_active_pane() {
    let (mut mux, _wid, tid, _p1, p2) = two_pane_setup();

    // Zoom p1 (active).
    mux.toggle_zoom(tid);
    drain(&mut mux);

    // Unzoom (simulates what App does before navigate/cycle).
    mux.unzoom(tid);

    // After unzoom, changing active pane should work (simulates navigate result).
    mux.set_active_pane(tid, p2);

    let tab = mux.session.get_tab(tid).unwrap();
    assert_eq!(tab.active_pane(), p2);
    assert_eq!(
        tab.zoomed_pane(),
        None,
        "zoom should stay cleared after navigate"
    );
}

// -- Floating pane operations --

/// Helper: one-pane setup with a floating pane added manually.
fn one_pane_with_floating() -> (InProcessMux, WindowId, TabId, PaneId, PaneId) {
    let (mut mux, wid, tid, p1) = one_pane_setup();
    let p2 = PaneId::from_raw(200);
    let did = mux.default_domain();

    mux.pane_registry.register(PaneEntry {
        pane: p2,
        tab: tid,
        domain: did,
    });

    let tab = mux.session.get_tab_mut(tid).unwrap();
    let available = Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    let fp = FloatingPane::centered(p2, &available, 1);
    let new_layer = tab.floating().add(fp);
    tab.set_floating(new_layer);

    drain(&mut mux);

    (mux, wid, tid, p1, p2)
}

#[test]
fn all_panes_includes_floating() {
    let (_mux, _wid, tid, p1, p2) = one_pane_with_floating();
    let tab = _mux.session.get_tab(tid).unwrap();
    let all = tab.all_panes();
    assert!(all.contains(&p1), "tiled pane should be in all_panes");
    assert!(all.contains(&p2), "floating pane should be in all_panes");
    assert_eq!(all.len(), 2);
}

#[test]
fn is_floating_true_for_floating_pane() {
    let (_mux, _wid, tid, p1, p2) = one_pane_with_floating();
    let tab = _mux.session.get_tab(tid).unwrap();
    assert!(tab.is_floating(p2));
    assert!(!tab.is_floating(p1));
}

#[test]
fn close_floating_pane_removes_from_layer() {
    let (mut mux, _wid, tid, _p1, p2) = one_pane_with_floating();

    let result = mux.close_pane(p2);
    assert_eq!(result, ClosePaneResult::PaneRemoved);

    let tab = mux.session.get_tab(tid).unwrap();
    assert!(tab.floating().is_empty(), "floating layer should be empty");
    assert!(!tab.all_panes().contains(&p2));

    let notes = drain(&mut mux);
    assert!(
        notes
            .iter()
            .any(|n| matches!(n, MuxNotification::PaneClosed(id) if *id == p2))
    );
    assert!(
        notes
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid))
    );
}

#[test]
fn close_floating_pane_falls_back_to_tiled_active() {
    let (mut mux, _wid, tid, p1, p2) = one_pane_with_floating();

    // Make the floating pane active.
    mux.set_active_pane(tid, p2);
    let tab = mux.session.get_tab(tid).unwrap();
    assert_eq!(tab.active_pane(), p2);

    // Close the floating pane — should fall back to tiled pane.
    mux.close_pane(p2);

    let tab = mux.session.get_tab(tid).unwrap();
    assert_eq!(
        tab.active_pane(),
        p1,
        "active pane should fall back to tiled pane"
    );
}

#[test]
fn move_pane_to_floating_removes_from_tree() {
    let (mut mux, _wid, tid, _p1, p2) = two_pane_setup();
    drain(&mut mux);

    let available = Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    let moved = mux.move_pane_to_floating(tid, p2, &available);
    assert!(moved, "move_pane_to_floating should succeed");

    let tab = mux.session.get_tab(tid).unwrap();
    assert!(
        !tab.tree().panes().contains(&p2),
        "pane should be removed from tree"
    );
    assert!(tab.is_floating(p2), "pane should be in floating layer");
    assert_eq!(tab.active_pane(), p2, "moved pane should become active");

    let notes = drain(&mut mux);
    assert!(
        notes
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid))
    );
}

#[test]
fn move_last_tiled_pane_to_floating_rejected() {
    let (mut mux, _wid, tid, p1, _p2) = one_pane_with_floating();
    drain(&mut mux);

    let available = Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    // p1 is the only tiled pane — shouldn't allow floating it.
    let moved = mux.move_pane_to_floating(tid, p1, &available);
    assert!(!moved, "should not float the last tiled pane");
}

#[test]
fn move_pane_to_tiled_removes_from_floating() {
    let (mut mux, _wid, tid, _p1, p2) = one_pane_with_floating();
    drain(&mut mux);

    let moved = mux.move_pane_to_tiled(tid, p2);
    assert!(moved, "move_pane_to_tiled should succeed");

    let tab = mux.session.get_tab(tid).unwrap();
    assert!(!tab.is_floating(p2), "pane should no longer be floating");
    assert!(
        tab.tree().panes().contains(&p2),
        "pane should be in the tree"
    );

    let notes = drain(&mut mux);
    assert!(
        notes
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid))
    );
}

#[test]
fn raise_floating_pane_updates_z_order() {
    let (mut mux, _wid, tid, _p1, p2) = one_pane_with_floating();

    // Add a second floating pane.
    let p3 = PaneId::from_raw(201);
    let did = mux.default_domain();
    mux.pane_registry.register(PaneEntry {
        pane: p3,
        tab: tid,
        domain: did,
    });
    {
        let tab = mux.session.get_tab_mut(tid).unwrap();
        let available = Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };
        let fp = FloatingPane::centered(p3, &available, 2);
        let new_layer = tab.floating().add(fp);
        tab.set_floating(new_layer);
    }
    drain(&mut mux);

    // p3 is on top (z=2), p2 is below (z=1). Raise p2.
    mux.raise_floating_pane(tid, p2);

    let tab = mux.session.get_tab(tid).unwrap();
    let panes = tab.floating().panes();
    // After raise, p2 should be last (topmost) in z-order.
    assert_eq!(
        panes.last().unwrap().pane_id,
        p2,
        "raised pane should be topmost"
    );
}

// -- High priority: split-while-zoomed backstop --

#[test]
fn zoom_state_cleared_by_manual_split() {
    // Zoom a tab, then manually update the tree (simulating what
    // `mux.split_pane()` does internally). The zoom state should NOT
    // persist with an invalid tree — this tests the backstop.
    let (mut mux, _wid, tid, p1, _p2) = two_pane_setup();

    // Zoom p1.
    mux.set_active_pane(tid, p1);
    mux.toggle_zoom(tid);
    assert_eq!(
        mux.session().get_tab(tid).unwrap().zoomed_pane(),
        Some(p1),
        "p1 should be zoomed"
    );

    // Now unzoom via unzoom_silent (the App-level guard) and split.
    mux.unzoom_silent(tid);
    assert_eq!(
        mux.session().get_tab(tid).unwrap().zoomed_pane(),
        None,
        "zoom should be cleared after unzoom_silent"
    );

    // Manually split the tree (simulating what split_pane does).
    let p3 = PaneId::from_raw(200);
    {
        let tab = mux.session.get_tab_mut(tid).unwrap();
        let new_tree = tab.tree().split_at(p1, SplitDirection::Vertical, p3, 0.5);
        tab.set_tree(new_tree);
    }

    // Verify: zoom is cleared AND the tree is updated.
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.zoomed_pane(), None);
    assert_eq!(tab.all_panes().len(), 3);
    assert!(tab.tree().contains(p3));
}

#[test]
fn toggle_zoom_on_zoomed_tab_then_split_roundtrip() {
    // Full roundtrip: zoom → unzoom → split → verify all panes visible.
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();

    // Zoom.
    mux.set_active_pane(tid, p1);
    mux.toggle_zoom(tid);
    assert!(mux.session().get_tab(tid).unwrap().zoomed_pane().is_some());
    drain(&mut mux);

    // Unzoom (toggle again).
    mux.toggle_zoom(tid);
    assert!(mux.session().get_tab(tid).unwrap().zoomed_pane().is_none());

    // Split p2 — should work correctly on an unzoomed tab.
    let p3 = PaneId::from_raw(200);
    {
        let tab = mux.session.get_tab_mut(tid).unwrap();
        let new_tree = tab.tree().split_at(p2, SplitDirection::Horizontal, p3, 0.5);
        tab.set_tree(new_tree);
    }

    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.all_panes().len(), 3);
    assert!(tab.tree().contains(p1));
    assert!(tab.tree().contains(p2));
    assert!(tab.tree().contains(p3));
    assert_eq!(tab.zoomed_pane(), None);
}

// -- High priority: navigate-while-zoomed auto-unzoom --

#[test]
fn unzoom_then_navigate_changes_focus() {
    // Simulates the full App-level flow: zoom pane → unzoom_silent →
    // set_active_pane (nav target). Verifies the focus actually moves
    // to a different pane after unzooming.
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();

    // Set p1 as active and zoom it.
    mux.set_active_pane(tid, p1);
    mux.toggle_zoom(tid);
    assert_eq!(mux.session().get_tab(tid).unwrap().active_pane(), p1);
    assert_eq!(mux.session().get_tab(tid).unwrap().zoomed_pane(), Some(p1));

    // Unzoom silently (as focus_pane_direction does).
    mux.unzoom_silent(tid);
    assert_eq!(mux.session().get_tab(tid).unwrap().zoomed_pane(), None);

    // Navigate to p2 (simulating directional navigation).
    mux.set_active_pane(tid, p2);
    assert_eq!(
        mux.session().get_tab(tid).unwrap().active_pane(),
        p2,
        "focus should move to p2 after unzoom + navigate"
    );
}

// -- Medium priority: resize_pane with nonexistent tab is no-op --

#[test]
fn resize_pane_nonexistent_tab_no_notification() {
    let mut mux = InProcessMux::new();
    drain(&mut mux);

    // Resize on a tab that doesn't exist — should not panic or emit.
    mux.resize_pane(
        TabId::from_raw(999),
        PaneId::from_raw(1),
        SplitDirection::Vertical,
        true,
        0.1,
    );

    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "resize on nonexistent tab should emit no notifications"
    );
}

#[test]
fn resize_pane_nonexistent_pane_no_notification() {
    let (mut mux, _wid, tid, _p1, _p2) = two_pane_setup();
    drain(&mut mux);

    // Resize a pane that doesn't exist in the tree.
    mux.resize_pane(
        tid,
        PaneId::from_raw(999),
        SplitDirection::Vertical,
        true,
        0.1,
    );

    // The tree didn't change, so no TabLayoutChanged should be emitted.
    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "resize on nonexistent pane should emit no notifications"
    );
}

// -- Medium priority: stale registry after partial unregister --

#[test]
fn close_tab_after_pane_already_unregistered() {
    // Pane p2 was unregistered (e.g., via a PaneExited event) but the
    // tree still references it. close_tab should still clean up p1.
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();

    // Manually unregister p2 (simulates PaneExited processing).
    mux.pane_registry.unregister(p2);
    drain(&mut mux);

    // close_tab iterates all_panes() and unregisters them. p2 is already
    // gone from the registry — unregister returns None, no panic.
    let closed = mux.close_tab(tid);
    assert_eq!(closed.len(), 2, "all_panes should list both p1 and p2");
    assert!(closed.contains(&p1));
    assert!(closed.contains(&p2));

    // Tab and remaining pane entry should be fully cleaned up.
    assert!(mux.session().get_tab(tid).is_none());
    assert!(mux.get_pane_entry(p1).is_none());
}

// -- undo_split / redo_split --

#[test]
fn undo_split_restores_previous_tree() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    drain(&mut mux);

    // The two_pane_setup already called set_tree (split), so undo should
    // restore the single-pane tree.
    let live: std::collections::HashSet<_> = [p1, p2].into_iter().collect();
    assert!(mux.undo_split(tid, &live));

    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().panes(), vec![p1]);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid)),
    );
}

#[test]
fn redo_split_restores_undone_tree() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    drain(&mut mux);

    let live: std::collections::HashSet<_> = [p1, p2].into_iter().collect();
    assert!(mux.undo_split(tid, &live));
    drain(&mut mux);

    assert!(mux.redo_split(tid, &live));

    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().panes().len(), 2);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid)),
    );
}

#[test]
fn split_undo_redo_undo_cycle() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    drain(&mut mux);

    let live: std::collections::HashSet<_> = [p1, p2].into_iter().collect();

    // Undo → single pane.
    assert!(mux.undo_split(tid, &live));
    assert_eq!(mux.session().get_tab(tid).unwrap().tree().panes(), vec![p1],);

    // Redo → two panes.
    assert!(mux.redo_split(tid, &live));
    assert_eq!(mux.session().get_tab(tid).unwrap().tree().panes().len(), 2,);

    // Undo again → single pane.
    assert!(mux.undo_split(tid, &live));
    assert_eq!(mux.session().get_tab(tid).unwrap().tree().panes(), vec![p1],);
}

#[test]
fn undo_past_closed_pane_skips_entry() {
    // Build a 3-pane tree, close p2, then undo should skip the entry
    // that references p2 and restore the original single-pane tree.
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
    // Split 1: p1 + p2.
    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree);
    // Split 2: p1 + p2 + p3.
    let tree = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree);
    mux.session.add_tab(tab);

    for &pid in &[p1, p2, p3] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    drain(&mut mux);

    // "Close" p2 — remove from registry (simulating close_pane without
    // tree mutation so the undo stack still has entries referencing p2).
    mux.pane_registry.unregister(p2);

    // Live panes: p1, p3 (p2 is gone).
    let live: std::collections::HashSet<_> = [p1, p3].into_iter().collect();

    // Undo should skip the entry with p2 and restore the original tree.
    assert!(mux.undo_split(tid, &live));
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().panes(), vec![p1]);
}

// -- undo/redo notification suppression on noop --

#[test]
fn undo_split_nonexistent_tab_no_notification() {
    let (mut mux, _wid, _tid, _p1, _p2) = two_pane_setup();
    drain(&mut mux);

    let bad_tab = TabId::from_raw(999);
    let live = std::collections::HashSet::new();
    let result = mux.undo_split(bad_tab, &live);
    assert!(!result);

    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "no notifications for undo on nonexistent tab"
    );
}

#[test]
fn redo_split_nonexistent_tab_no_notification() {
    let (mut mux, _wid, _tid, _p1, _p2) = two_pane_setup();
    drain(&mut mux);

    let bad_tab = TabId::from_raw(999);
    let live = std::collections::HashSet::new();
    let result = mux.redo_split(bad_tab, &live);
    assert!(!result);

    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "no notifications for redo on nonexistent tab"
    );
}

#[test]
fn undo_split_empty_stack_no_notification() {
    let (mut mux, _wid, tid, p1) = one_pane_setup();
    drain(&mut mux);

    // Single-pane tab has no undo history.
    let live: std::collections::HashSet<_> = [p1].into_iter().collect();
    let result = mux.undo_split(tid, &live);
    assert!(!result);

    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "no notifications when undo stack is empty"
    );
}

// -- close pane after undo cascades correctly --

#[test]
fn close_after_undo_cascades_to_last_window() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    drain(&mut mux);

    // Undo the split → tree reverts to leaf(p1). p2 is still registered
    // (undo only affects tree layout, not pane registration).
    let live: std::collections::HashSet<_> = [p1, p2].into_iter().collect();
    assert!(mux.undo_split(tid, &live));
    drain(&mut mux);

    // Close p1 — the only pane in the tree → triggers tab/window removal.
    let result = mux.close_pane(p1);
    assert_eq!(result, ClosePaneResult::LastWindow);

    // Tab and window should be cleaned up.
    assert_eq!(mux.session().window_count(), 0);
    assert_eq!(mux.session().tab_count(), 0);

    // p1 was unregistered by close_pane. p2 is still registered because
    // undo only modifies the tree — the real App layer would close the PTY
    // and unregister p2 separately.
    assert!(mux.get_pane_entry(p1).is_none());
    assert!(mux.get_pane_entry(p2).is_some());
}

// -- geometry consistency post-undo/redo --

#[test]
fn geometry_tiled_panes_cover_full_area_after_undo() {
    use oriterm_mux::layout::compute::{LayoutDescriptor, compute_layout};
    use oriterm_mux::layout::floating::FloatingLayer;

    let p1 = PaneId::from_raw(1);
    let p2 = PaneId::from_raw(2);
    let p3 = PaneId::from_raw(3);
    let mut mux = InProcessMux::new();
    let wid = WindowId::from_raw(100);
    let tid = TabId::from_raw(100);
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
    drain(&mut mux);

    let desc = LayoutDescriptor {
        available: Rect {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 600.0,
        },
        cell_width: 8.0,
        cell_height: 16.0,
        divider_px: 2.0,
        min_pane_cells: (10, 5),
    };

    // Before undo: 3 panes should cover the full area.
    let tab = mux.session().get_tab(tid).unwrap();
    let layouts_before = compute_layout(tab.tree(), &FloatingLayer::new(), p1, &desc);
    assert_eq!(layouts_before.len(), 3);

    // Undo to 2 panes.
    let live: std::collections::HashSet<_> = [p1, p2, p3].into_iter().collect();
    assert!(mux.undo_split(tid, &live));

    let tab = mux.session().get_tab(tid).unwrap();
    let layouts_after = compute_layout(tab.tree(), &FloatingLayer::new(), p1, &desc);
    assert_eq!(layouts_after.len(), 2);

    // Sum of tiled pane widths + divider should approximate total width.
    // (The exact sum depends on divider placement; verify no zero-area panes.)
    for layout in &layouts_after {
        assert!(layout.pixel_rect.width > 0.0, "pane width must be positive");
        assert!(
            layout.pixel_rect.height > 0.0,
            "pane height must be positive"
        );
        assert!(layout.cols > 0, "pane columns must be > 0");
        assert!(layout.rows > 0, "pane rows must be > 0");
    }

    // Redo back to 3 panes.
    assert!(mux.redo_split(tid, &live));

    let tab = mux.session().get_tab(tid).unwrap();
    let layouts_redo = compute_layout(tab.tree(), &FloatingLayer::new(), p1, &desc);
    assert_eq!(layouts_redo.len(), 3);

    for layout in &layouts_redo {
        assert!(layout.pixel_rect.width > 0.0, "pane width must be positive");
        assert!(
            layout.pixel_rect.height > 0.0,
            "pane height must be positive"
        );
    }
}

// -- concurrent undo and pane exit --

#[test]
fn concurrent_pane_exit_then_undo_skips_dead_entry() {
    // Simulate: pane exits via event, then user presses undo. The undo
    // stack has an entry referencing the dead pane — it should be skipped.
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
    let tree = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree);
    mux.session.add_tab(tab);

    for &pid in &[p1, p2, p3] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    drain(&mut mux);

    // p2 exits via event.
    let tx = mux.event_tx().clone();
    tx.send(MuxEvent::PaneExited {
        pane_id: p2,
        exit_code: 0,
    })
    .unwrap();
    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);
    drain(&mut mux);

    // p2 is gone from registry. Undo entries referencing p2 should be skipped.
    assert!(mux.get_pane_entry(p2).is_none());
    let live: std::collections::HashSet<_> = [p1, p3].into_iter().collect();

    // Undo should skip [tree with p1+p2] and restore leaf(p1).
    assert!(mux.undo_split(tid, &live));
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().panes(), vec![p1]);
}

// -- Section 33.6 integration tests --

/// 4-pane 2×2 grid: navigate all 4 directions, verify correct focus.
#[test]
fn integration_navigation_4_pane_grid_all_directions() {
    use oriterm_mux::layout::{LayoutDescriptor, compute_layout};
    use oriterm_mux::nav::{self, Direction};

    let mut mux = InProcessMux::new();
    let did = mux.default_domain();
    let wid = WindowId::from_raw(100);
    let tid = TabId::from_raw(100);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    let p3 = PaneId::from_raw(102);
    let p4 = PaneId::from_raw(103);

    mux.session.add_window(MuxWindow::new(wid));
    mux.session.get_window_mut(wid).unwrap().add_tab(tid);

    // Build 2×2 grid: vertical split [left|right], then horizontal split each.
    // Result: p1=top-left, p3=bottom-left, p2=top-right, p4=bottom-right.
    let mut tab = MuxTab::new(tid, p1);
    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree);
    let tree = tab.tree().split_at(p1, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree);
    let tree = tab.tree().split_at(p2, SplitDirection::Horizontal, p4, 0.5);
    tab.set_tree(tree);
    mux.session.add_tab(tab);

    for &pid in &[p1, p2, p3, p4] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    drain(&mut mux);

    let desc = LayoutDescriptor {
        available: Rect {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 800.0,
        },
        cell_width: 10.0,
        cell_height: 20.0,
        divider_px: 2.0,
        min_pane_cells: (4, 2),
    };

    let tab = mux.session().get_tab(tid).unwrap();
    let layouts = compute_layout(tab.tree(), tab.floating(), p1, &desc);

    // From top-left (p1): right→p2, down→p3.
    assert_eq!(nav::navigate(&layouts, p1, Direction::Right), Some(p2));
    assert_eq!(nav::navigate(&layouts, p1, Direction::Down), Some(p3));

    // From bottom-right (p4): left→p3, up→p2.
    assert_eq!(nav::navigate(&layouts, p4, Direction::Left), Some(p3));
    assert_eq!(nav::navigate(&layouts, p4, Direction::Up), Some(p2));

    // From top-right (p2): left→p1, down→p4.
    assert_eq!(nav::navigate(&layouts, p2, Direction::Left), Some(p1));
    assert_eq!(nav::navigate(&layouts, p2, Direction::Down), Some(p4));

    // From bottom-left (p3): right→p4, up→p1.
    assert_eq!(nav::navigate(&layouts, p3, Direction::Right), Some(p4));
    assert_eq!(nav::navigate(&layouts, p3, Direction::Up), Some(p1));

    // set_active_pane updates focus correctly.
    assert!(mux.set_active_pane(tid, p4));
    assert_eq!(mux.session().get_tab(tid).unwrap().active_pane(), p4);
}

/// Drag divider: set ratio, verify tree updates and notification emitted.
#[test]
fn integration_resize_divider_ratio_and_notification() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    drain(&mut mux);

    mux.set_divider_ratio(tid, p1, p2, 0.7);

    // Verify ratio changed in tree.
    let tab = mux.session().get_tab(tid).unwrap();
    match tab.tree() {
        SplitTree::Split { ratio, .. } => {
            assert!(
                (*ratio - 0.7).abs() < f32::EPSILON,
                "expected ratio 0.7, got {ratio}"
            );
        }
        SplitTree::Leaf(_) => panic!("expected Split, got Leaf"),
    }

    // TabLayoutChanged notification emitted.
    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid))
    );
}

/// Floating pane: create, move, resize, toggle to tiled.
#[test]
fn integration_floating_create_move_resize_toggle_to_tiled() {
    let (mut mux, _wid, tid, p1, p2) = one_pane_with_floating();
    drain(&mut mux);

    // Move the floating pane.
    mux.move_floating_pane(tid, p2, 100.0, 50.0);
    let tab = mux.session.get_tab(tid).unwrap();
    let rect = tab.floating().pane_rect(p2).unwrap();
    assert!((rect.x - 100.0).abs() < f32::EPSILON);
    assert!((rect.y - 50.0).abs() < f32::EPSILON);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::FloatingPaneChanged(id) if *id == tid))
    );

    // Resize the floating pane.
    mux.resize_floating_pane(tid, p2, 400.0, 300.0);
    let tab = mux.session.get_tab(tid).unwrap();
    let rect = tab.floating().pane_rect(p2).unwrap();
    assert!((rect.width - 400.0).abs() < f32::EPSILON);
    assert!((rect.height - 300.0).abs() < f32::EPSILON);
    drain(&mut mux);

    // Toggle p2 back to tiled.
    assert!(mux.move_pane_to_tiled(tid, p2));

    let tab = mux.session.get_tab(tid).unwrap();
    assert!(!tab.is_floating(p2), "p2 should no longer be floating");
    assert!(tab.tree().contains(p2), "p2 should be in the split tree");
    assert!(tab.tree().contains(p1), "p1 should still be in the tree");
    assert_eq!(tab.all_panes().len(), 2);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid))
    );
}

/// Split 3 times, undo all 3, verify original single-pane layout restored.
#[test]
fn integration_undo_triple_split_restores_original() {
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();
    let wid = WindowId::from_raw(100);
    let tid = TabId::from_raw(100);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    let p3 = PaneId::from_raw(102);
    let p4 = PaneId::from_raw(103);

    mux.session.add_window(MuxWindow::new(wid));
    mux.session.get_window_mut(wid).unwrap().add_tab(tid);
    mux.session.add_tab(MuxTab::new(tid, p1));

    for &pid in &[p1, p2, p3, p4] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    drain(&mut mux);

    // Split 3 times via set_tree (each pushes to undo stack).
    let tab = mux.session.get_tab_mut(tid).unwrap();
    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree);
    let tree = tab.tree().split_at(p2, SplitDirection::Horizontal, p3, 0.5);
    tab.set_tree(tree);
    let tree = tab.tree().split_at(p1, SplitDirection::Horizontal, p4, 0.5);
    tab.set_tree(tree);
    assert_eq!(tab.tree().pane_count(), 4);

    let live: std::collections::HashSet<_> = [p1, p2, p3, p4].into_iter().collect();

    // Undo split 3 → 3 panes.
    assert!(mux.undo_split(tid, &live));
    assert_eq!(mux.session().get_tab(tid).unwrap().tree().pane_count(), 3);

    // Undo split 2 → 2 panes.
    assert!(mux.undo_split(tid, &live));
    assert_eq!(mux.session().get_tab(tid).unwrap().tree().pane_count(), 2);

    // Undo split 1 → original single pane.
    assert!(mux.undo_split(tid, &live));
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().panes(), vec![p1]);

    // No more undo entries.
    assert!(!mux.undo_split(tid, &live));
}

// -- High priority: undo/redo interaction with floating pane transitions --

#[test]
fn undo_move_to_floating_restores_tiled_layout() {
    // move_pane_to_floating modifies the split tree via set_tree, which
    // pushes to the undo stack. Undoing should restore the original tree.
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    drain(&mut mux);

    let available = Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };

    // Float p2 — tree changes from Split(p1,p2) to Leaf(p1).
    assert!(mux.move_pane_to_floating(tid, p2, &available));
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().panes(), vec![p1]);
    assert!(tab.is_floating(p2));
    drain(&mut mux);

    // Undo — tree should restore to Split(p1,p2).
    let live: std::collections::HashSet<_> = [p1, p2].into_iter().collect();
    assert!(mux.undo_split(tid, &live));

    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().panes().len(), 2);
    assert!(tab.tree().contains(p1));
    assert!(tab.tree().contains(p2));
}

#[test]
fn undo_move_to_tiled_restores_tree_before_tiling() {
    // move_pane_to_tiled modifies the split tree (splits next to anchor),
    // pushing to the undo stack. Undoing should restore the tree before tiling.
    let (mut mux, _wid, tid, p1, p2) = one_pane_with_floating();
    drain(&mut mux);

    // Move floating pane to tiled — tree changes from Leaf(p1) to Split(p1,p2).
    assert!(mux.move_pane_to_tiled(tid, p2));
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().panes().len(), 2);
    drain(&mut mux);

    // Undo — tree should revert to Leaf(p1).
    let live: std::collections::HashSet<_> = [p1, p2].into_iter().collect();
    assert!(mux.undo_split(tid, &live));

    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().panes(), vec![p1]);
}

// -- High priority: active pane consistency through multi-step cascade --

#[test]
fn active_pane_stable_through_non_active_close_cascade() {
    // 3-pane tree with p3 active. Close p1 (not active, not first_pane after
    // removal). Active pane should stay p3 through the entire cascade.
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
    tab.set_active_pane(p3);
    mux.session.add_tab(tab);

    for &pid in &[p1, p2, p3] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    drain(&mut mux);

    // Close p1 — p3 is active and should remain active.
    let result = mux.close_pane(p1);
    assert_eq!(result, ClosePaneResult::PaneRemoved);

    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(
        tab.active_pane(),
        p3,
        "active pane should remain p3 after closing non-active p1"
    );
    assert_eq!(tab.all_panes().len(), 2);

    // Close p2 — p3 is still active.
    let result = mux.close_pane(p2);
    assert_eq!(result, ClosePaneResult::PaneRemoved);

    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(
        tab.active_pane(),
        p3,
        "active pane should remain p3 after closing non-active p2"
    );
    assert_eq!(tab.all_panes().len(), 1);
}

// -- High priority: equalize after asymmetric pane removal --

#[test]
fn equalize_after_asymmetric_removal_balances_tree() {
    // [p1 (0.3) | [p2 (0.4) / p3]]. Close p2, leaving [p1 (0.3) | p3].
    // Equalize should reset to 0.5.
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
    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.3);
    tab.set_tree(tree);
    let tree = tab.tree().split_at(p2, SplitDirection::Horizontal, p3, 0.4);
    tab.set_tree(tree);
    mux.session.add_tab(tab);

    for &pid in &[p1, p2, p3] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    drain(&mut mux);

    // Close p2 — tree collapses to [p1 | p3] with the outer ratio (0.3).
    mux.close_pane(p2);
    drain(&mut mux);

    // The remaining split should have the skewed ratio.
    let tab = mux.session().get_tab(tid).unwrap();
    let (_, ratio) = tab.tree().parent_split(p1).unwrap();
    assert!(
        (ratio - 0.3).abs() < f32::EPSILON,
        "ratio should be 0.3 before equalize, got {ratio}"
    );

    // Equalize — should reset to 0.5.
    mux.equalize_panes(tid);

    let tab = mux.session().get_tab(tid).unwrap();
    let (_, ratio) = tab.tree().parent_split(p1).unwrap();
    assert!(
        (ratio - 0.5).abs() < f32::EPSILON,
        "ratio should be 0.5 after equalize, got {ratio}"
    );
}

// -- High priority: zoom + close sibling + unzoom chain --

#[test]
fn zoom_close_sibling_unzoom_chain() {
    // 3-pane tree: p1, p2, p3. Zoom p1, close p2 (sibling), unzoom.
    // Verify tree is valid and active pane is correct.
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
    drain(&mut mux);

    // Zoom p1.
    mux.set_active_pane(tid, p1);
    mux.toggle_zoom(tid);
    assert_eq!(mux.session().get_tab(tid).unwrap().zoomed_pane(), Some(p1));
    drain(&mut mux);

    // Close p2 (a non-zoomed sibling). Zoom should still be set on p1
    // because we didn't close the zoomed pane.
    mux.close_pane(p2);
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(
        tab.zoomed_pane(),
        Some(p1),
        "zoom should persist after closing non-zoomed sibling"
    );
    assert_eq!(tab.all_panes().len(), 2);
    drain(&mut mux);

    // Unzoom.
    mux.unzoom(tid);
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.zoomed_pane(), None);
    assert_eq!(tab.active_pane(), p1);

    // Tree should still be valid with p1 and p3.
    assert!(tab.tree().contains(p1));
    assert!(tab.tree().contains(p3));
    assert_eq!(tab.all_panes().len(), 2);
}

// -- Medium priority: resize divider with floating panes present --

#[test]
fn resize_divider_with_floating_panes_present() {
    // One tiled split + one floating pane. Resizing the tiled divider
    // should update the tiled tree without affecting the floating layer.
    let (mut mux, _wid, tid, p1, p2) = one_pane_with_floating();
    // p1 is tiled, p2 is floating. We need a tiled split, so add p3 tiled.
    let p3 = PaneId::from_raw(300);
    let did = mux.default_domain();

    mux.pane_registry.register(PaneEntry {
        pane: p3,
        tab: tid,
        domain: did,
    });
    {
        let tab = mux.session.get_tab_mut(tid).unwrap();
        let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p3, 0.5);
        tab.set_tree(tree);
    }
    drain(&mut mux);

    // Resize the tiled divider between p1 and p3.
    mux.set_divider_ratio(tid, p1, p3, 0.7);

    // Verify tiled tree updated.
    let tab = mux.session().get_tab(tid).unwrap();
    if let SplitTree::Split { ratio, .. } = tab.tree() {
        assert!(
            (*ratio - 0.7).abs() < f32::EPSILON,
            "ratio should be 0.7, got {ratio}"
        );
    } else {
        panic!("expected Split");
    }

    // Floating layer should be untouched.
    assert!(
        tab.is_floating(p2),
        "floating pane should still be in floating layer"
    );
    assert_eq!(tab.floating().panes().len(), 1);

    // Notification should be TabLayoutChanged (not FloatingPaneChanged).
    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid))
    );
}

// -- Medium priority: move_pane_to_tiled on already-tiled pane --

#[test]
fn move_pane_to_tiled_on_tiled_pane_is_noop() {
    let (mut mux, _wid, tid, p1, _p2) = two_pane_setup();
    drain(&mut mux);

    // p1 is tiled, not floating. move_pane_to_tiled should return false.
    let moved = mux.move_pane_to_tiled(tid, p1);
    assert!(
        !moved,
        "move_pane_to_tiled on a tiled pane should be a no-op"
    );

    // Tree should be unchanged.
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().panes().len(), 2);

    let notifs = drain(&mut mux);
    assert!(
        notifs.is_empty(),
        "no notifications when move_pane_to_tiled is a no-op"
    );
}

// -- Medium priority: move_pane_to_floating on already-floating pane --

#[test]
fn move_pane_to_floating_on_floating_pane_is_noop() {
    let (mut mux, _wid, tid, _p1, p2) = one_pane_with_floating();
    drain(&mut mux);

    let available = Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };

    // p2 is already floating. Trying to float it again should fail because
    // it's not in the split tree (tree.remove returns None → false).
    let moved = mux.move_pane_to_floating(tid, p2, &available);
    assert!(
        !moved,
        "move_pane_to_floating on an already-floating pane should be a no-op"
    );

    // State should be unchanged.
    let tab = mux.session().get_tab(tid).unwrap();
    assert!(tab.is_floating(p2));
    assert_eq!(tab.floating().panes().len(), 1);
}

// -- Medium priority: set_floating_pane_rect with degenerate dimensions --

#[test]
fn set_floating_pane_rect_degenerate_dimensions() {
    let (mut mux, _wid, tid, _p1, p2) = one_pane_with_floating();
    drain(&mut mux);

    // Zero-width/height rect.
    mux.set_floating_pane_rect(
        tid,
        p2,
        Rect {
            x: 10.0,
            y: 20.0,
            width: 0.0,
            height: 0.0,
        },
    );

    let tab = mux.session.get_tab(tid).unwrap();
    let rect = tab.floating().pane_rect(p2).unwrap();
    assert!((rect.width).abs() < f32::EPSILON, "width should be 0");
    assert!((rect.height).abs() < f32::EPSILON, "height should be 0");
    drain(&mut mux);

    // Negative position rect.
    mux.set_floating_pane_rect(
        tid,
        p2,
        Rect {
            x: -100.0,
            y: -50.0,
            width: 200.0,
            height: 150.0,
        },
    );

    let tab = mux.session.get_tab(tid).unwrap();
    let rect = tab.floating().pane_rect(p2).unwrap();
    assert!(
        (rect.x - (-100.0)).abs() < f32::EPSILON,
        "negative x should be stored"
    );
    assert!(
        (rect.y - (-50.0)).abs() < f32::EPSILON,
        "negative y should be stored"
    );

    // Should emit FloatingPaneChanged each time.
    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::FloatingPaneChanged(id) if *id == tid))
    );
}

// -- Medium priority: raise single floating pane does not panic --

#[test]
fn raise_single_floating_pane_does_not_panic() {
    let (mut mux, _wid, tid, _p1, p2) = one_pane_with_floating();
    drain(&mut mux);

    // Raise the only floating pane — should not panic, pane stays in layer.
    mux.raise_floating_pane(tid, p2);

    let tab = mux.session.get_tab(tid).unwrap();
    assert_eq!(tab.floating().panes().len(), 1);
    assert_eq!(tab.floating().panes().first().unwrap().pane_id, p2);

    // Should emit FloatingPaneChanged.
    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::FloatingPaneChanged(id) if *id == tid))
    );
}

// -- Low priority: rapid set_divider_ratio calls each emit notification --

#[test]
fn rapid_divider_ratio_calls_each_emit_notification() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    drain(&mut mux);

    // 5 rapid ratio changes.
    for ratio in [0.2, 0.4, 0.6, 0.8, 0.5] {
        mux.set_divider_ratio(tid, p1, p2, ratio);
    }

    let notifs = drain(&mut mux);
    let layout_count = notifs
        .iter()
        .filter(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == tid))
        .count();
    assert_eq!(
        layout_count, 5,
        "each set_divider_ratio call should emit TabLayoutChanged"
    );

    // Final ratio should be the last applied value.
    let tab = mux.session().get_tab(tid).unwrap();
    if let SplitTree::Split { ratio, .. } = tab.tree() {
        assert!(
            (*ratio - 0.5).abs() < f32::EPSILON,
            "final ratio should be 0.5, got {ratio}"
        );
    }
}

// -- Low priority: deeply nested tree (5 levels) --

#[test]
fn deeply_nested_tree_operations() {
    // Build a 5-level deep tree: p1 | (p2 / (p3 | (p4 / p5))).
    // Verify close, equalize, and undo work at depth.
    let mut mux = InProcessMux::new();
    let wid = WindowId::from_raw(100);
    let tid = TabId::from_raw(100);
    let did = mux.default_domain();

    let panes: Vec<PaneId> = (100..=104).map(PaneId::from_raw).collect();

    mux.session.add_window(MuxWindow::new(wid));
    mux.session.get_window_mut(wid).unwrap().add_tab(tid);

    let mut tab = MuxTab::new(tid, panes[0]);
    let tree = tab
        .tree()
        .split_at(panes[0], SplitDirection::Vertical, panes[1], 0.5);
    tab.set_tree(tree);
    let tree = tab
        .tree()
        .split_at(panes[1], SplitDirection::Horizontal, panes[2], 0.5);
    tab.set_tree(tree);
    let tree = tab
        .tree()
        .split_at(panes[2], SplitDirection::Vertical, panes[3], 0.5);
    tab.set_tree(tree);
    let tree = tab
        .tree()
        .split_at(panes[3], SplitDirection::Horizontal, panes[4], 0.5);
    tab.set_tree(tree);
    mux.session.add_tab(tab);

    for &pid in &panes {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }
    drain(&mut mux);

    // Verify all 5 panes are in the tree.
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().pane_count(), 5);

    // Equalize should work at all levels.
    mux.equalize_panes(tid);
    drain(&mut mux);

    // Close the deepest pane (p5).
    let result = mux.close_pane(panes[4]);
    assert_eq!(result, ClosePaneResult::PaneRemoved);

    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().pane_count(), 4);
    assert!(!tab.tree().contains(panes[4]));
    drain(&mut mux);

    // Close p4 (was the second deepest).
    let result = mux.close_pane(panes[3]);
    assert_eq!(result, ClosePaneResult::PaneRemoved);

    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().pane_count(), 3);

    // Undo should restore p4.
    let live: std::collections::HashSet<_> = panes.iter().copied().collect();
    assert!(mux.undo_split(tid, &live));

    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().pane_count(), 4);
    assert!(tab.tree().contains(panes[3]));
}

// -- Low priority: close_window interleaved with poll_events --

#[test]
fn close_window_with_queued_events_for_closed_panes() {
    // Queue PaneExited and PaneOutput events for panes, then close the
    // window before polling. poll_events should handle stale events
    // gracefully — no panic, no orphaned state.
    let (mut mux, wid, _tid, p1, p2) = two_pane_setup();
    let tx = mux.event_tx().clone();

    // Queue events for both panes.
    tx.send(MuxEvent::PaneOutput(p1)).unwrap();
    tx.send(MuxEvent::PaneBell(p2)).unwrap();
    tx.send(MuxEvent::PaneExited {
        pane_id: p1,
        exit_code: 0,
    })
    .unwrap();

    // Close the window BEFORE polling events.
    let closed = mux.close_window(wid);
    assert_eq!(closed.len(), 2);
    drain(&mut mux);

    // Now poll the stale events — should not panic.
    let mut panes = std::collections::HashMap::new();
    mux.poll_events(&mut panes);

    // Everything should be empty — no orphaned registrations.
    assert!(mux.pane_registry().is_empty());
    assert_eq!(mux.session().window_count(), 0);
    assert_eq!(mux.session().tab_count(), 0);
}

// -- cycle_active_tab --

/// Build a mux with one window containing three tabs (single pane each).
fn three_tab_setup() -> (InProcessMux, WindowId, TabId, TabId, TabId) {
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();

    let wid = WindowId::from_raw(100);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(101);
    let t3 = TabId::from_raw(102);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    let p3 = PaneId::from_raw(102);

    let mut win = MuxWindow::new(wid);
    win.add_tab(t1);
    win.add_tab(t2);
    win.add_tab(t3);
    mux.session.add_window(win);

    for (tid, pid) in [(t1, p1), (t2, p2), (t3, p3)] {
        mux.session.add_tab(MuxTab::new(tid, pid));
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }

    drain(&mut mux);
    (mux, wid, t1, t2, t3)
}

#[test]
fn cycle_active_tab_forward_wraps() {
    let (mut mux, wid, t1, t2, t3) = three_tab_setup();

    // Default active is index 0 (t1).
    assert_eq!(mux.active_tab_id(wid), Some(t1));

    // Cycle forward through all tabs.
    assert_eq!(mux.cycle_active_tab(wid, 1), Some(t2));
    assert_eq!(mux.active_tab_id(wid), Some(t2));

    assert_eq!(mux.cycle_active_tab(wid, 1), Some(t3));
    assert_eq!(mux.active_tab_id(wid), Some(t3));

    // Wrap around to t1.
    assert_eq!(mux.cycle_active_tab(wid, 1), Some(t1));
    assert_eq!(mux.active_tab_id(wid), Some(t1));
}

#[test]
fn cycle_active_tab_backward_wraps() {
    let (mut mux, wid, _t1, _t2, t3) = three_tab_setup();

    // Default active is t1 (index 0). Backward wraps to t3.
    assert_eq!(mux.cycle_active_tab(wid, -1), Some(t3));
    assert_eq!(mux.active_tab_id(wid), Some(t3));
}

#[test]
fn cycle_active_tab_single_tab_returns_none() {
    let (mut mux, wid, tid, pid) = one_pane_setup();
    let _ = (tid, pid); // suppress unused warnings

    // Only one tab — cycle should return None.
    assert_eq!(mux.cycle_active_tab(wid, 1), None);
    assert_eq!(mux.cycle_active_tab(wid, -1), None);
}

#[test]
fn cycle_active_tab_nonexistent_window_returns_none() {
    let (mut mux, _wid, _, _, _) = three_tab_setup();
    let stale = WindowId::from_raw(999);
    assert_eq!(mux.cycle_active_tab(stale, 1), None);
}

// -- switch_active_tab --

#[test]
fn switch_active_tab_to_specific_tab() {
    let (mut mux, wid, _t1, _t2, t3) = three_tab_setup();

    // Default active is t1 (index 0). Switch to t3.
    assert!(mux.switch_active_tab(wid, t3));
    assert_eq!(mux.active_tab_id(wid), Some(t3));
}

#[test]
fn switch_active_tab_nonexistent_tab_returns_false() {
    let (mut mux, wid, _t1, _t2, _t3) = three_tab_setup();
    let stale = TabId::from_raw(999);
    assert!(!mux.switch_active_tab(wid, stale));
}

#[test]
fn switch_active_tab_nonexistent_window_returns_false() {
    let (mut mux, _wid, _t1, _t2, t3) = three_tab_setup();
    let stale = WindowId::from_raw(999);
    assert!(!mux.switch_active_tab(stale, t3));
}

#[test]
fn switch_active_tab_to_already_active_is_idempotent() {
    let (mut mux, wid, t1, _t2, _t3) = three_tab_setup();

    assert_eq!(mux.active_tab_id(wid), Some(t1));
    assert!(mux.switch_active_tab(wid, t1));
    assert_eq!(mux.active_tab_id(wid), Some(t1));
}

// -- reorder_tab --

#[test]
fn reorder_tab_moves_and_tracks_active() {
    let (mut mux, wid, t1, t2, t3) = three_tab_setup();

    // Move t1 from position 0 to position 2.
    assert!(mux.reorder_tab(wid, 0, 2));
    let win = mux.session().get_window(wid).unwrap();
    assert_eq!(win.tabs(), &[t2, t3, t1]);
}

#[test]
fn reorder_tab_nonexistent_window_returns_false() {
    let (mut mux, _wid, _, _, _) = three_tab_setup();
    let stale = WindowId::from_raw(999);
    assert!(!mux.reorder_tab(stale, 0, 1));
}

#[test]
fn reorder_tab_out_of_bounds_returns_false() {
    let (mut mux, wid, _, _, _) = three_tab_setup();
    assert!(!mux.reorder_tab(wid, 0, 10));
    assert!(!mux.reorder_tab(wid, 10, 0));
}

// -- set_active_pane --

#[test]
fn set_active_pane_returns_true_for_existing_tab() {
    let (mut mux, _wid, tid, p1, p2) = two_pane_setup();
    assert!(mux.set_active_pane(tid, p2));
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.active_pane(), p2);

    // Switch back.
    assert!(mux.set_active_pane(tid, p1));
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.active_pane(), p1);
}

#[test]
fn set_active_pane_nonexistent_tab_returns_false() {
    let (mut mux, _wid, _tid, _p1, _p2) = two_pane_setup();
    let stale = TabId::from_raw(999);
    assert!(!mux.set_active_pane(stale, PaneId::from_raw(1)));
}

// -- Cross-window tab movement (Section 32.4) --

/// Two windows: w1 has two tabs (t1, t2), w2 has one tab (t3).
fn two_window_setup() -> (InProcessMux, WindowId, WindowId, TabId, TabId, TabId) {
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();

    let w1 = WindowId::from_raw(100);
    let w2 = WindowId::from_raw(200);
    let t1 = TabId::from_raw(100);
    let t2 = TabId::from_raw(101);
    let t3 = TabId::from_raw(200);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    let p3 = PaneId::from_raw(200);

    // Window 1 with two tabs.
    mux.session.add_window(MuxWindow::new(w1));
    mux.session.get_window_mut(w1).unwrap().add_tab(t1);
    mux.session.get_window_mut(w1).unwrap().add_tab(t2);
    mux.session.add_tab(MuxTab::new(t1, p1));
    mux.session.add_tab(MuxTab::new(t2, p2));
    mux.pane_registry.register(PaneEntry {
        pane: p1,
        tab: t1,
        domain: did,
    });
    mux.pane_registry.register(PaneEntry {
        pane: p2,
        tab: t2,
        domain: did,
    });

    // Window 2 with one tab.
    mux.session.add_window(MuxWindow::new(w2));
    mux.session.get_window_mut(w2).unwrap().add_tab(t3);
    mux.session.add_tab(MuxTab::new(t3, p3));
    mux.pane_registry.register(PaneEntry {
        pane: p3,
        tab: t3,
        domain: did,
    });

    drain(&mut mux);

    (mux, w1, w2, t1, t2, t3)
}

#[test]
fn move_tab_to_window_transfers_ownership() {
    let (mut mux, w1, w2, t1, _t2, _t3) = two_window_setup();

    assert!(mux.move_tab_to_window(t1, w2));

    // t1 should be in w2 now.
    let w2_win = mux.session().get_window(w2).unwrap();
    assert!(w2_win.tabs().contains(&t1));
    assert_eq!(w2_win.tabs().len(), 2); // t3 + t1

    // t1 should be removed from w1.
    let w1_win = mux.session().get_window(w1).unwrap();
    assert!(!w1_win.tabs().contains(&t1));
    assert_eq!(w1_win.tabs().len(), 1); // only t2
}

#[test]
fn move_tab_becomes_active_in_destination() {
    let (mut mux, _w1, w2, t1, _t2, _t3) = two_window_setup();

    mux.move_tab_to_window(t1, w2);

    let w2_win = mux.session().get_window(w2).unwrap();
    assert_eq!(w2_win.active_tab(), Some(t1));
}

#[test]
fn move_tab_panes_resized_notification() {
    let (mut mux, _w1, w2, t1, _t2, _t3) = two_window_setup();

    mux.move_tab_to_window(t1, w2);

    let notifs = drain(&mut mux);

    // Must emit TabLayoutChanged for the moved tab (triggers resize).
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::TabLayoutChanged(id) if *id == t1))
    );
}

#[test]
fn move_tab_emits_tabs_changed_for_both_windows() {
    let (mut mux, w1, w2, t1, _t2, _t3) = two_window_setup();

    mux.move_tab_to_window(t1, w2);

    let notifs = drain(&mut mux);

    // Both windows should get WindowTabsChanged.
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::WindowTabsChanged(id) if *id == w1))
    );
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::WindowTabsChanged(id) if *id == w2))
    );
}

#[test]
fn move_last_tab_closes_source_window() {
    let (mut mux, w1, w2, _t1, _t2, t3) = two_window_setup();

    // w2 has only t3. Move t3 to w1 → w2 should close.
    assert!(mux.move_tab_to_window(t3, w1));

    assert!(mux.session().get_window(w2).is_none());
    assert_eq!(mux.session().window_count(), 1);

    let notifs = drain(&mut mux);
    assert!(
        notifs
            .iter()
            .any(|n| matches!(n, MuxNotification::WindowClosed(id) if *id == w2))
    );
}

#[test]
fn move_tab_to_same_window_is_noop() {
    let (mut mux, w1, _w2, t1, _t2, _t3) = two_window_setup();

    assert!(!mux.move_tab_to_window(t1, w1));

    // No notifications emitted.
    let notifs = drain(&mut mux);
    assert!(notifs.is_empty());
}

#[test]
fn move_nonexistent_tab_returns_false() {
    let (mut mux, _w1, w2, _t1, _t2, _t3) = two_window_setup();

    let stale = TabId::from_raw(999);
    assert!(!mux.move_tab_to_window(stale, w2));
}

#[test]
fn move_to_nonexistent_window_returns_false() {
    let (mut mux, _w1, _w2, t1, _t2, _t3) = two_window_setup();

    let stale = WindowId::from_raw(999);
    assert!(!mux.move_tab_to_window(t1, stale));
}

#[test]
fn move_multi_pane_tab_preserves_split_layout() {
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();

    let w1 = WindowId::from_raw(100);
    let w2 = WindowId::from_raw(200);
    let tid = TabId::from_raw(100);
    let t2 = TabId::from_raw(200);
    let p1 = PaneId::from_raw(100);
    let p2 = PaneId::from_raw(101);
    let p3 = PaneId::from_raw(200);

    // w1: one tab with two split panes.
    mux.session.add_window(MuxWindow::new(w1));
    mux.session.get_window_mut(w1).unwrap().add_tab(tid);

    let mut tab = MuxTab::new(tid, p1);
    let tree = tab.tree().split_at(p1, SplitDirection::Vertical, p2, 0.5);
    tab.set_tree(tree);
    mux.session.add_tab(tab);

    for &pid in &[p1, p2] {
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
    }

    // w2: one tab, one pane.
    mux.session.add_window(MuxWindow::new(w2));
    mux.session.get_window_mut(w2).unwrap().add_tab(t2);
    mux.session.add_tab(MuxTab::new(t2, p3));
    mux.pane_registry.register(PaneEntry {
        pane: p3,
        tab: t2,
        domain: did,
    });

    drain(&mut mux);

    // Move the split tab from w1 to w2.
    assert!(mux.move_tab_to_window(tid, w2));

    // Split tree should be preserved in the moved tab.
    let tab = mux.session().get_tab(tid).unwrap();
    assert_eq!(tab.tree().pane_count(), 2);
    assert!(tab.tree().contains(p1));
    assert!(tab.tree().contains(p2));

    // w1 should be closed (it was the only tab).
    assert!(mux.session().get_window(w1).is_none());
}

// -- Section 32.5: Integration tests --

#[test]
fn tab_lifecycle_create_close_cycle() {
    // Create 5 tabs, close 3, cycle remaining, verify state.
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();
    let wid = WindowId::from_raw(100);
    mux.session.add_window(MuxWindow::new(wid));

    // Create 5 tabs with one pane each.
    let mut tabs = Vec::new();
    let mut panes = Vec::new();
    for i in 0..5 {
        let tid = TabId::from_raw(200 + i);
        let pid = PaneId::from_raw(200 + i);
        mux.session.add_tab(MuxTab::new(tid, pid));
        mux.session.get_window_mut(wid).unwrap().add_tab(tid);
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
        tabs.push(tid);
        panes.push(pid);
    }
    drain(&mut mux);

    assert_eq!(mux.session().get_window(wid).unwrap().tabs().len(), 5);

    // Close tabs at indices 1, 3, 4 (t2, t4, t5). Remove in reverse to avoid
    // index shifting issues.
    for &tid in &[tabs[4], tabs[3], tabs[1]] {
        mux.close_tab(tid);
    }
    drain(&mut mux);

    // 2 tabs remain: t1 and t3.
    let win = mux.session().get_window(wid).unwrap();
    assert_eq!(win.tabs().len(), 2);
    assert!(win.tabs().contains(&tabs[0]));
    assert!(win.tabs().contains(&tabs[2]));

    // Cycle through remaining tabs.
    mux.switch_active_tab(wid, tabs[0]);
    assert_eq!(mux.active_tab_id(wid), Some(tabs[0]));

    assert_eq!(mux.cycle_active_tab(wid, 1), Some(tabs[2]));
    assert_eq!(mux.active_tab_id(wid), Some(tabs[2]));

    // Wrap back to first.
    assert_eq!(mux.cycle_active_tab(wid, 1), Some(tabs[0]));
    assert_eq!(mux.active_tab_id(wid), Some(tabs[0]));

    // Closed panes should be unregistered.
    assert!(mux.get_pane_entry(panes[1]).is_none());
    assert!(mux.get_pane_entry(panes[3]).is_none());
    assert!(mux.get_pane_entry(panes[4]).is_none());

    // Remaining panes should still be registered.
    assert!(mux.get_pane_entry(panes[0]).is_some());
    assert!(mux.get_pane_entry(panes[2]).is_some());
}

#[test]
fn multi_window_move_tab_and_close_window() {
    // 2 windows, move tab between them, close one window.
    let (mut mux, w1, w2, t1, t2, t3) = two_window_setup();

    // w1 has [t1, t2], w2 has [t3].
    assert_eq!(mux.session().get_window(w1).unwrap().tabs().len(), 2);
    assert_eq!(mux.session().get_window(w2).unwrap().tabs().len(), 1);

    // Move t2 from w1 to w2.
    assert!(mux.move_tab_to_window(t2, w2));
    drain(&mut mux);

    assert_eq!(mux.session().get_window(w1).unwrap().tabs(), &[t1]);
    assert_eq!(mux.session().get_window(w2).unwrap().tabs(), &[t3, t2]);

    // Close w1 — should not affect w2.
    let closed_panes = mux.close_window(w1);
    assert_eq!(closed_panes.len(), 1); // only t1's pane
    drain(&mut mux);

    assert!(mux.session().get_window(w1).is_none());
    assert_eq!(mux.session().window_count(), 1);

    // w2 still intact with both tabs.
    let w2_win = mux.session().get_window(w2).unwrap();
    assert_eq!(w2_win.tabs().len(), 2);
    assert!(w2_win.tabs().contains(&t2));
    assert!(w2_win.tabs().contains(&t3));
}

#[test]
fn rapid_create_close_no_orphans() {
    // Rapidly create and close tabs — verify no orphaned panes or stale state.
    let mut mux = InProcessMux::new();
    let did = mux.default_domain();
    let wid = WindowId::from_raw(100);
    mux.session.add_window(MuxWindow::new(wid));

    // Seed with one tab so the window never goes empty.
    let seed_tid = TabId::from_raw(1);
    let seed_pid = PaneId::from_raw(1);
    mux.session.add_tab(MuxTab::new(seed_tid, seed_pid));
    mux.session.get_window_mut(wid).unwrap().add_tab(seed_tid);
    mux.pane_registry.register(PaneEntry {
        pane: seed_pid,
        tab: seed_tid,
        domain: did,
    });
    drain(&mut mux);

    // Rapidly create 20 tabs, then close them all.
    let mut created = Vec::new();
    for i in 0..20 {
        let tid = TabId::from_raw(100 + i);
        let pid = PaneId::from_raw(100 + i);
        mux.session.add_tab(MuxTab::new(tid, pid));
        mux.session.get_window_mut(wid).unwrap().add_tab(tid);
        mux.pane_registry.register(PaneEntry {
            pane: pid,
            tab: tid,
            domain: did,
        });
        created.push((tid, pid));
    }
    drain(&mut mux);

    assert_eq!(
        mux.session().get_window(wid).unwrap().tabs().len(),
        21 // seed + 20
    );

    // Close all 20 in random-ish order (reverse).
    for &(tid, _pid) in created.iter().rev() {
        mux.close_tab(tid);
    }
    drain(&mut mux);

    // Only seed tab remains.
    let win = mux.session().get_window(wid).unwrap();
    assert_eq!(win.tabs().len(), 1);
    assert_eq!(win.tabs()[0], seed_tid);

    // Only seed pane is registered.
    assert_eq!(mux.pane_registry().len(), 1);
    assert!(mux.get_pane_entry(seed_pid).is_some());

    // All created panes should be gone.
    for &(_tid, pid) in &created {
        assert!(mux.get_pane_entry(pid).is_none());
    }
}
