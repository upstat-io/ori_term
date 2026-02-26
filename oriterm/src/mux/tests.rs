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
