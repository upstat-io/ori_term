use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use oriterm_core::{Event, EventListener};
use oriterm_mux::PaneId;

use super::{MuxEvent, MuxEventProxy};
use crate::event::TermEvent;

/// Shared winit event loop proxy for tests.
///
/// Winit only allows one event loop per process. We create it once via
/// `OnceLock` and share the proxy across all tests.
fn shared_proxy() -> winit::event_loop::EventLoopProxy<TermEvent> {
    use std::sync::OnceLock;

    static PROXY: OnceLock<winit::event_loop::EventLoopProxy<TermEvent>> = OnceLock::new();
    PROXY
        .get_or_init(|| {
            #[cfg(target_os = "linux")]
            {
                use winit::platform::x11::EventLoopBuilderExtX11;
                let event_loop = winit::event_loop::EventLoop::<TermEvent>::with_user_event()
                    .with_any_thread(true)
                    .build()
                    .expect("event loop");
                event_loop.create_proxy()
            }
            #[cfg(not(target_os = "linux"))]
            {
                let event_loop = winit::event_loop::EventLoop::<TermEvent>::with_user_event()
                    .build()
                    .expect("event loop");
                event_loop.create_proxy()
            }
        })
        .clone()
}

/// Create a test proxy with mpsc channel and shared winit proxy.
fn test_proxy() -> (
    MuxEventProxy,
    mpsc::Receiver<MuxEvent>,
    Arc<AtomicBool>,
    Arc<AtomicBool>,
) {
    let (tx, rx) = mpsc::channel();
    let wakeup = Arc::new(AtomicBool::new(false));
    let dirty = Arc::new(AtomicBool::new(false));
    let winit_proxy = shared_proxy();

    let proxy = MuxEventProxy::new(
        PaneId::from_raw(1),
        tx,
        Arc::clone(&wakeup),
        Arc::clone(&dirty),
        winit_proxy,
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
    let winit_proxy = shared_proxy();

    let proxy = MuxEventProxy::new(
        PaneId::from_raw(1),
        tx,
        Arc::clone(&wakeup),
        Arc::clone(&dirty),
        winit_proxy,
    );

    // Drop the receiver to simulate a disconnected channel.
    drop(rx);

    // All event types should be silently dropped, not panic.
    proxy.send_event(Event::Wakeup);
    proxy.send_event(Event::Bell);
    proxy.send_event(Event::Title("test".to_string()));
    proxy.send_event(Event::ResetTitle);
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
            MuxEvent::PaneCwdChanged {
                pane_id: id,
                cwd: "/tmp".to_string(),
            },
            "PaneCwdChanged(Pane(1), \"/tmp\")",
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
    use oriterm_mux::{TabId, WindowId};

    let pid = PaneId::from_raw(1);
    let tid = TabId::from_raw(2);
    let wid = WindowId::from_raw(3);

    let cases: Vec<(MuxNotification, &str)> = vec![
        (MuxNotification::PaneDirty(pid), "PaneDirty(Pane(1))"),
        (MuxNotification::PaneClosed(pid), "PaneClosed(Pane(1))"),
        (
            MuxNotification::TabLayoutChanged(tid),
            "TabLayoutChanged(Tab(2))",
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
