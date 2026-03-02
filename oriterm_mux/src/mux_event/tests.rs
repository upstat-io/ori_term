use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use oriterm_core::{Event, EventListener};

use crate::PaneId;

use super::{MuxEvent, MuxEventProxy};

/// Create a test proxy with mpsc channel and no-op wakeup.
fn test_proxy() -> (
    MuxEventProxy,
    mpsc::Receiver<MuxEvent>,
    Arc<AtomicBool>,
    Arc<AtomicBool>,
) {
    let (tx, rx) = mpsc::channel();
    let wakeup = Arc::new(AtomicBool::new(false));
    let dirty = Arc::new(AtomicBool::new(false));
    let noop_wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});

    let proxy = MuxEventProxy::new(
        PaneId::from_raw(1),
        tx,
        Arc::clone(&wakeup),
        Arc::clone(&dirty),
        noop_wakeup,
    );
    (proxy, rx, wakeup, dirty)
}

#[test]
fn wakeup_sets_grid_dirty_and_sends_pane_output() {
    let (proxy, rx, wakeup, dirty) = test_proxy();

    proxy.send_event(Event::Wakeup);

    assert!(dirty.load(Ordering::Acquire));
    assert!(wakeup.load(Ordering::Acquire));

    let event = rx.try_recv().unwrap();
    assert!(matches!(event, MuxEvent::PaneOutput(id) if id == PaneId::from_raw(1)));
}

#[test]
fn wakeup_coalescing_skips_duplicate_send() {
    let (proxy, rx, wakeup, dirty) = test_proxy();

    // First wakeup — should send.
    proxy.send_event(Event::Wakeup);
    assert!(rx.try_recv().is_ok());

    // Second wakeup — coalesced, no channel send.
    assert!(wakeup.load(Ordering::Acquire));
    proxy.send_event(Event::Wakeup);
    assert!(rx.try_recv().is_err()); // No second message.

    // Grid dirty should still be set.
    assert!(dirty.load(Ordering::Acquire));
}

#[test]
fn wakeup_after_clear_sends_again() {
    let (proxy, rx, wakeup, dirty) = test_proxy();

    proxy.send_event(Event::Wakeup);
    let _ = rx.try_recv();

    // Simulate main thread clearing the flags.
    wakeup.store(false, Ordering::Release);
    dirty.store(false, Ordering::Release);

    // Next wakeup should send again.
    proxy.send_event(Event::Wakeup);
    assert!(rx.try_recv().is_ok());
    assert!(dirty.load(Ordering::Acquire));
}

#[test]
fn bell_maps_to_pane_bell() {
    let (proxy, rx, _, _) = test_proxy();
    proxy.send_event(Event::Bell);
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, MuxEvent::PaneBell(id) if id == PaneId::from_raw(1)));
}

#[test]
fn title_maps_to_pane_title_changed() {
    let (proxy, rx, _, _) = test_proxy();
    proxy.send_event(Event::Title("hello".to_string()));
    let event = rx.try_recv().unwrap();
    assert!(
        matches!(&event, MuxEvent::PaneTitleChanged { pane_id, title } if *pane_id == PaneId::from_raw(1) && title == "hello")
    );
}

#[test]
fn reset_title_maps_to_empty_title() {
    let (proxy, rx, _, _) = test_proxy();
    proxy.send_event(Event::ResetTitle);
    let event = rx.try_recv().unwrap();
    assert!(matches!(&event, MuxEvent::PaneTitleChanged { title, .. } if title.is_empty()));
}

#[test]
fn cwd_maps_to_pane_cwd_changed() {
    let (proxy, rx, _, _) = test_proxy();
    proxy.send_event(Event::Cwd("/home/user".to_string()));
    let event = rx.try_recv().unwrap();
    assert!(
        matches!(&event, MuxEvent::PaneCwdChanged { pane_id, cwd } if *pane_id == PaneId::from_raw(1) && cwd == "/home/user")
    );
}

#[test]
fn command_complete_maps_to_command_complete() {
    let (proxy, rx, _, _) = test_proxy();
    let dur = std::time::Duration::from_secs(15);
    proxy.send_event(Event::CommandComplete(dur));
    let event = rx.try_recv().unwrap();
    assert!(
        matches!(event, MuxEvent::CommandComplete { pane_id, duration } if pane_id == PaneId::from_raw(1) && duration == dur)
    );
}

#[test]
fn icon_name_maps_to_pane_icon_changed() {
    let (proxy, rx, _, _) = test_proxy();
    proxy.send_event(Event::IconName("\u{1f40d}python".to_string()));
    let event = rx.try_recv().unwrap();
    assert!(
        matches!(&event, MuxEvent::PaneIconChanged { pane_id, icon_name } if *pane_id == PaneId::from_raw(1) && icon_name == "\u{1f40d}python")
    );
}

#[test]
fn reset_icon_name_maps_to_empty_icon() {
    let (proxy, rx, _, _) = test_proxy();
    proxy.send_event(Event::ResetIconName);
    let event = rx.try_recv().unwrap();
    assert!(matches!(&event, MuxEvent::PaneIconChanged { icon_name, .. } if icon_name.is_empty()));
}

#[test]
fn pty_write_maps_to_pty_write() {
    let (proxy, rx, _, _) = test_proxy();
    proxy.send_event(Event::PtyWrite("data".to_string()));
    let event = rx.try_recv().unwrap();
    assert!(
        matches!(&event, MuxEvent::PtyWrite { pane_id, data } if *pane_id == PaneId::from_raw(1) && data == "data")
    );
}

#[test]
fn child_exit_maps_to_pane_exited() {
    let (proxy, rx, _, _) = test_proxy();
    proxy.send_event(Event::ChildExit(42));
    let event = rx.try_recv().unwrap();
    assert!(
        matches!(event, MuxEvent::PaneExited { pane_id, exit_code } if pane_id == PaneId::from_raw(1) && exit_code == 42)
    );
}

#[test]
fn clipboard_store_maps_correctly() {
    let (proxy, rx, _, _) = test_proxy();
    proxy.send_event(Event::ClipboardStore(
        oriterm_core::ClipboardType::Clipboard,
        "text".to_string(),
    ));
    let event = rx.try_recv().unwrap();
    assert!(matches!(&event, MuxEvent::ClipboardStore { text, .. } if text == "text"));
}

#[test]
fn clipboard_load_maps_correctly() {
    let (proxy, rx, _, _) = test_proxy();
    let fmt = Arc::new(|s: &str| format!("formatted:{s}"));
    proxy.send_event(Event::ClipboardLoad(
        oriterm_core::ClipboardType::Selection,
        fmt,
    ));
    let event = rx.try_recv().unwrap();
    assert!(
        matches!(&event, MuxEvent::ClipboardLoad { clipboard_type, .. } if *clipboard_type == oriterm_core::ClipboardType::Selection)
    );
}

#[test]
fn mux_event_debug_format() {
    let event = MuxEvent::PaneOutput(PaneId::from_raw(5));
    assert_eq!(format!("{event:?}"), "PaneOutput(Pane(5))");

    let event = MuxEvent::PaneExited {
        pane_id: PaneId::from_raw(3),
        exit_code: 1,
    };
    assert_eq!(format!("{event:?}"), "PaneExited(Pane(3), code=1)");
}

// --- Gap analysis tests ---

/// When the mpsc receiver is dropped, sending events doesn't panic.
#[test]
fn disconnected_receiver_does_not_panic() {
    let (tx, rx) = mpsc::channel();
    let wakeup = Arc::new(AtomicBool::new(false));
    let dirty = Arc::new(AtomicBool::new(false));
    let noop_wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});

    let proxy = MuxEventProxy::new(
        PaneId::from_raw(1),
        tx,
        Arc::clone(&wakeup),
        Arc::clone(&dirty),
        noop_wakeup,
    );

    // Drop the receiver to simulate a disconnected channel.
    drop(rx);

    // All event types should be silently dropped, not panic.
    proxy.send_event(Event::Wakeup);
    proxy.send_event(Event::Bell);
    proxy.send_event(Event::Title("test".to_string()));
    proxy.send_event(Event::ResetTitle);
    proxy.send_event(Event::IconName("\u{1f40d}".to_string()));
    proxy.send_event(Event::ResetIconName);
    proxy.send_event(Event::PtyWrite("data".to_string()));
    proxy.send_event(Event::ChildExit(0));

    // Grid dirty and wakeup should still be set (atomics don't depend on channel).
    assert!(dirty.load(Ordering::Acquire));
    assert!(wakeup.load(Ordering::Acquire));
}

/// Debug format for all MuxEvent variants.
#[test]
fn mux_event_debug_all_variants() {
    let id = PaneId::from_raw(1);

    let cases = [
        (MuxEvent::PaneOutput(id), "PaneOutput(Pane(1))"),
        (
            MuxEvent::PaneExited {
                pane_id: id,
                exit_code: 0,
            },
            "PaneExited(Pane(1), code=0)",
        ),
        (
            MuxEvent::PaneTitleChanged {
                pane_id: id,
                title: "hello".to_string(),
            },
            "PaneTitleChanged(Pane(1), \"hello\")",
        ),
        (
            MuxEvent::PaneIconChanged {
                pane_id: id,
                icon_name: "\u{1f40d}".to_string(),
            },
            "PaneIconChanged(Pane(1), \"\u{1f40d}\")",
        ),
        (
            MuxEvent::PaneCwdChanged {
                pane_id: id,
                cwd: "/tmp".to_string(),
            },
            "PaneCwdChanged(Pane(1), \"/tmp\")",
        ),
        (
            MuxEvent::CommandComplete {
                pane_id: id,
                duration: std::time::Duration::from_secs(15),
            },
            "CommandComplete(Pane(1), 15s)",
        ),
        (MuxEvent::PaneBell(id), "PaneBell(Pane(1))"),
        (
            MuxEvent::PtyWrite {
                pane_id: id,
                data: "abc".to_string(),
            },
            "PtyWrite(Pane(1), 3 bytes)",
        ),
    ];

    for (event, expected) in &cases {
        assert_eq!(format!("{event:?}"), *expected);
    }

    // ClipboardStore/Load contain closures — just verify they don't panic.
    let store = MuxEvent::ClipboardStore {
        pane_id: id,
        clipboard_type: oriterm_core::ClipboardType::Clipboard,
        text: "copied".to_string(),
    };
    let dbg = format!("{store:?}");
    assert!(dbg.contains("ClipboardStore"));
    assert!(dbg.contains("Clipboard"));

    let load = MuxEvent::ClipboardLoad {
        pane_id: id,
        clipboard_type: oriterm_core::ClipboardType::Selection,
        formatter: Arc::new(|s: &str| s.to_string()),
    };
    let dbg = format!("{load:?}");
    assert!(dbg.contains("ClipboardLoad"));
    assert!(dbg.contains("Selection"));
}

// --- MuxNotification Debug format ---

#[test]
fn mux_notification_debug_all_variants() {
    use super::MuxNotification;

    use crate::{TabId, WindowId};

    let pid = PaneId::from_raw(1);
    let tid = TabId::from_raw(2);
    let wid = WindowId::from_raw(3);

    let cases: Vec<(MuxNotification, &str)> = vec![
        (
            MuxNotification::PaneTitleChanged(pid),
            "PaneTitleChanged(Pane(1))",
        ),
        (MuxNotification::PaneDirty(pid), "PaneDirty(Pane(1))"),
        (MuxNotification::PaneClosed(pid), "PaneClosed(Pane(1))"),
        (
            MuxNotification::TabLayoutChanged(tid),
            "TabLayoutChanged(Tab(2))",
        ),
        (
            MuxNotification::FloatingPaneChanged(tid),
            "FloatingPaneChanged(Tab(2))",
        ),
        (
            MuxNotification::WindowTabsChanged(wid),
            "WindowTabsChanged(Window(3))",
        ),
        (
            MuxNotification::WindowClosed(wid),
            "WindowClosed(Window(3))",
        ),
        (MuxNotification::Alert(pid), "Alert(Pane(1))"),
        (
            MuxNotification::CommandComplete {
                pane_id: pid,
                duration: std::time::Duration::from_secs(30),
            },
            "CommandComplete(Pane(1), 30s)",
        ),
        (MuxNotification::LastWindowClosed, "LastWindowClosed"),
    ];

    for (notif, expected) in &cases {
        assert_eq!(format!("{notif:?}"), *expected);
    }

    // ClipboardStore/Load contain closures — verify they don't panic.
    let store = MuxNotification::ClipboardStore {
        pane_id: pid,
        clipboard_type: oriterm_core::ClipboardType::Clipboard,
        text: "copied".to_string(),
    };
    let dbg = format!("{store:?}");
    assert!(dbg.contains("ClipboardStore"));
    assert!(dbg.contains("Clipboard"));

    let load = MuxNotification::ClipboardLoad {
        pane_id: pid,
        clipboard_type: oriterm_core::ClipboardType::Selection,
        formatter: Arc::new(|s: &str| s.to_string()),
    };
    let dbg = format!("{load:?}");
    assert!(dbg.contains("ClipboardLoad"));
    assert!(dbg.contains("Selection"));
}

// --- Concurrent wakeup coalescing ---

#[test]
fn concurrent_wakeup_coalescing_does_not_lose_events() {
    let (tx, rx) = mpsc::channel();
    let wakeup = Arc::new(AtomicBool::new(false));
    let dirty = Arc::new(AtomicBool::new(false));
    let wakeup_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let wakeup_count2 = Arc::clone(&wakeup_count);
    let wakeup_fn: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        wakeup_count2.fetch_add(1, Ordering::Relaxed);
    });

    let proxy = MuxEventProxy::new(
        PaneId::from_raw(1),
        tx,
        Arc::clone(&wakeup),
        Arc::clone(&dirty),
        wakeup_fn,
    );
    let proxy = Arc::new(proxy);

    // Spawn multiple threads that all send Wakeup concurrently.
    let threads: Vec<_> = (0..10)
        .map(|_| {
            let p = Arc::clone(&proxy);
            std::thread::spawn(move || {
                p.send_event(Event::Wakeup);
            })
        })
        .collect();

    for t in threads {
        t.join().unwrap();
    }

    // Grid dirty must be set.
    assert!(dirty.load(Ordering::Acquire));
    // Wakeup pending must be set.
    assert!(wakeup.load(Ordering::Acquire));
    // At least 1 PaneOutput should have been sent (coalescing may reduce count).
    let mut count = 0;
    while rx.try_recv().is_ok() {
        count += 1;
    }
    assert!(
        count >= 1,
        "at least one PaneOutput should be sent, got {count}"
    );
    // Wakeup function should have been called at least once.
    assert!(wakeup_count.load(Ordering::Relaxed) >= 1);
}

// --- Non-routed events (wakeup-only) ---

#[test]
fn color_request_wakes_event_loop_without_mux_event() {
    let (tx, rx) = mpsc::channel();
    let wakeup = Arc::new(AtomicBool::new(false));
    let dirty = Arc::new(AtomicBool::new(false));
    let woke = Arc::new(AtomicBool::new(false));
    let woke2 = Arc::clone(&woke);
    let wakeup_fn: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        woke2.store(true, Ordering::Release);
    });

    let proxy = MuxEventProxy::new(
        PaneId::from_raw(1),
        tx,
        Arc::clone(&wakeup),
        Arc::clone(&dirty),
        wakeup_fn,
    );

    let formatter = Arc::new(|_: oriterm_core::color::Rgb| String::new());
    proxy.send_event(Event::ColorRequest(42, formatter));

    // Should wake the event loop.
    assert!(
        woke.load(Ordering::Acquire),
        "ColorRequest should wake event loop"
    );
    // Should NOT send a MuxEvent.
    assert!(
        rx.try_recv().is_err(),
        "ColorRequest should not produce a MuxEvent"
    );
    // Should NOT set grid dirty.
    assert!(
        !dirty.load(Ordering::Acquire),
        "ColorRequest should not set grid dirty"
    );
}

#[test]
fn cursor_blinking_change_wakes_event_loop_without_mux_event() {
    let (tx, rx) = mpsc::channel();
    let wakeup = Arc::new(AtomicBool::new(false));
    let dirty = Arc::new(AtomicBool::new(false));
    let woke = Arc::new(AtomicBool::new(false));
    let woke2 = Arc::clone(&woke);
    let wakeup_fn: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        woke2.store(true, Ordering::Release);
    });

    let proxy = MuxEventProxy::new(
        PaneId::from_raw(1),
        tx,
        Arc::clone(&wakeup),
        Arc::clone(&dirty),
        wakeup_fn,
    );

    proxy.send_event(Event::CursorBlinkingChange);

    assert!(
        woke.load(Ordering::Acquire),
        "CursorBlinkingChange should wake event loop"
    );
    assert!(
        rx.try_recv().is_err(),
        "CursorBlinkingChange should not produce a MuxEvent"
    );
}

#[test]
fn mouse_cursor_dirty_wakes_event_loop_without_mux_event() {
    let (tx, rx) = mpsc::channel();
    let wakeup = Arc::new(AtomicBool::new(false));
    let dirty = Arc::new(AtomicBool::new(false));
    let woke = Arc::new(AtomicBool::new(false));
    let woke2 = Arc::clone(&woke);
    let wakeup_fn: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        woke2.store(true, Ordering::Release);
    });

    let proxy = MuxEventProxy::new(
        PaneId::from_raw(1),
        tx,
        Arc::clone(&wakeup),
        Arc::clone(&dirty),
        wakeup_fn,
    );

    proxy.send_event(Event::MouseCursorDirty);

    assert!(
        woke.load(Ordering::Acquire),
        "MouseCursorDirty should wake event loop"
    );
    assert!(
        rx.try_recv().is_err(),
        "MouseCursorDirty should not produce a MuxEvent"
    );
}
