//! Tests for tab identity, event types, EventProxy, Notifier, and Tab.

use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use winit::event_loop::EventLoopProxy;

use oriterm_core::{Column, Event, EventListener, Line, Theme};

use super::{EventProxy, Notifier, Tab, TabId};
use crate::event::TermEvent;
use crate::pty::Msg;

// ---------------------------------------------------------------------------
// TabId
// ---------------------------------------------------------------------------

#[test]
fn tab_id_next_generates_unique_ids() {
    let a = TabId::next();
    let b = TabId::next();
    let c = TabId::next();
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
}

#[test]
fn tab_id_is_copy() {
    let id = TabId::next();
    let copy = id;
    assert_eq!(id, copy);
}

#[test]
fn tab_id_hash_equality() {
    use std::collections::HashSet;

    let a = TabId::next();
    let b = TabId::next();
    let mut set = HashSet::new();
    set.insert(a);
    set.insert(b);
    set.insert(a); // duplicate
    assert_eq!(set.len(), 2);
}

// ---------------------------------------------------------------------------
// TermEvent
// ---------------------------------------------------------------------------

#[test]
fn term_event_terminal_variant() {
    let id = TabId::next();
    let event = TermEvent::Terminal {
        tab_id: id,
        event: Event::Wakeup,
    };

    match event {
        TermEvent::Terminal { tab_id, event } => {
            assert_eq!(tab_id, id);
            assert!(matches!(event, Event::Wakeup));
        }
        TermEvent::ConfigReload => panic!("expected Terminal variant"),
    }
}

#[test]
fn term_event_debug_format() {
    let id = TabId::next();
    let event = TermEvent::Terminal {
        tab_id: id,
        event: Event::Bell,
    };
    let debug = format!("{event:?}");
    assert!(debug.contains("Terminal"));
    assert!(debug.contains("Bell"));
}

// ---------------------------------------------------------------------------
// EventProxy
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires display server (winit event loop)"]
fn event_proxy_sends_terminal_event() {
    let proxy = test_proxy();
    let tab_id = TabId::next();
    let event_proxy = EventProxy::new(proxy, tab_id);

    // Should not panic. The event is queued but there's no receiver
    // processing it — that's fine, send_event silently drops on error.
    event_proxy.send_event(Event::Wakeup);
    event_proxy.send_event(Event::Bell);
    event_proxy.send_event(Event::Title("test".into()));
}

#[test]
fn event_proxy_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<EventProxy>();
}

// ---------------------------------------------------------------------------
// Notifier
// ---------------------------------------------------------------------------

#[test]
fn notifier_writes_input_to_pipe() {
    let (pipe_reader, pipe_writer) = std::io::pipe().expect("pipe");
    let (tx, _rx) = mpsc::channel();
    let notifier = Notifier::new(Box::new(pipe_writer), tx);

    notifier.notify(b"hello");

    let mut buf = [0u8; 5];
    let mut r = pipe_reader;
    std::io::Read::read_exact(&mut r, &mut buf).expect("read");
    assert_eq!(&buf, b"hello");
}

#[test]
fn notifier_skips_empty_input() {
    let (_pipe_reader, pipe_writer) = std::io::pipe().expect("pipe");
    let (tx, rx) = mpsc::channel();
    let notifier = Notifier::new(Box::new(pipe_writer), tx);

    notifier.notify(b"");

    // Channel should be empty — empty bytes are not written.
    assert!(
        rx.try_recv().is_err(),
        "empty input should not produce a message",
    );
}

#[test]
fn notifier_sends_resize() {
    let (_pipe_reader, pipe_writer) = std::io::pipe().expect("pipe");
    let (tx, rx) = mpsc::channel();
    let notifier = Notifier::new(Box::new(pipe_writer), tx);

    notifier.resize(40, 120);

    match rx.recv().expect("should receive") {
        Msg::Resize { rows, cols } => {
            assert_eq!(rows, 40);
            assert_eq!(cols, 120);
        }
        other => panic!("expected Resize, got {other:?}"),
    }
}

#[test]
fn notifier_sends_shutdown() {
    let (_pipe_reader, pipe_writer) = std::io::pipe().expect("pipe");
    let (tx, rx) = mpsc::channel();
    let notifier = Notifier::new(Box::new(pipe_writer), tx);

    notifier.shutdown();

    assert!(
        matches!(rx.recv().expect("should receive"), Msg::Shutdown),
        "expected Shutdown message",
    );
}

#[test]
fn notifier_survives_dropped_receiver() {
    let (_pipe_reader, pipe_writer) = std::io::pipe().expect("pipe");
    let (tx, rx) = mpsc::channel::<Msg>();
    let notifier = Notifier::new(Box::new(pipe_writer), tx);
    drop(rx);

    // Should not panic when receiver is gone.
    notifier.notify(b"orphaned");
    notifier.resize(24, 80);
    notifier.shutdown();
}

// ---------------------------------------------------------------------------
// Tab (live PTY)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires display server (winit event loop)"]
fn tab_spawns_with_live_pty() {
    let proxy = test_proxy();
    let id = TabId::next();

    let tab = Tab::new(id, &tab_cfg(24, 80), proxy).expect("tab creation should succeed");

    assert_eq!(tab.id(), id);
    assert_eq!(tab.title(), "");
    assert!(!tab.has_bell());
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn tab_terminal_is_accessible() {
    let proxy = test_proxy();
    let id = TabId::next();

    let tab = Tab::new(id, &tab_cfg(24, 80), proxy).expect("tab creation should succeed");

    // Lock the terminal and verify grid dimensions.
    let term = tab.terminal().lock();
    let grid = term.grid();
    assert_eq!(grid.cols(), 80);
    assert_eq!(grid.lines(), 24);
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn tab_write_input_reaches_pty() {
    let proxy = test_proxy();
    let id = TabId::next();

    let tab = Tab::new(id, &tab_cfg(24, 80), proxy).expect("tab creation should succeed");

    // Should not panic — bytes go through Notifier → channel → PTY writer.
    tab.write_input(b"echo hello\r\n");
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn tab_resize_sends_to_pty() {
    let proxy = test_proxy();
    let id = TabId::next();

    let tab = Tab::new(id, &tab_cfg(24, 80), proxy).expect("tab creation should succeed");

    // Should not panic — resize goes through Notifier → channel → PTY control.
    tab.resize(40, 120);
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn tab_bell_state() {
    let proxy = test_proxy();
    let id = TabId::next();

    let mut tab = Tab::new(id, &tab_cfg(24, 80), proxy).expect("tab creation should succeed");

    assert!(!tab.has_bell());
    tab.set_bell();
    assert!(tab.has_bell());
    tab.clear_bell();
    assert!(!tab.has_bell());
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn tab_title_update() {
    let proxy = test_proxy();
    let id = TabId::next();

    let mut tab = Tab::new(id, &tab_cfg(24, 80), proxy).expect("tab creation should succeed");

    assert_eq!(tab.title(), "");
    tab.set_title("my terminal".into());
    assert_eq!(tab.title(), "my terminal");
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn tab_drop_is_clean() {
    let proxy = test_proxy();
    let id = TabId::next();

    let tab = Tab::new(id, &tab_cfg(24, 80), proxy).expect("tab creation should succeed");

    // Drop should send Shutdown, kill child, and join reader thread
    // without panicking.
    drop(tab);
}

// ---------------------------------------------------------------------------
// End-to-end verification (Section 4.9)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "requires display server (winit event loop)"]
fn echo_appears_in_terminal_grid() {
    let tab = make_tab(24, 80);

    // Send "echo hello" to the shell. The shell will:
    // 1. Echo the typed command (terminal echo mode).
    // 2. Execute it, printing "hello".
    tab.write_input(b"echo hello\r\n");

    // Poll until "hello" appears in the grid.
    let found = poll_grid_contains(&tab, "hello", Duration::from_secs(5));
    assert!(found, "'hello' should appear in the terminal grid");
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn thread_lifecycle_spawn_and_drop() {
    // Create multiple tabs in sequence to verify each one spawns and
    // shuts down cleanly without leaking threads or panicking.
    for _ in 0..3 {
        let tab = make_tab(24, 80);

        // Verify the reader thread is alive (terminal is being updated).
        tab.write_input(b"echo alive\r\n");
        thread::sleep(Duration::from_millis(100));

        // Drop triggers: Shutdown → kill child → join reader thread.
        drop(tab);
    }
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn thread_lifecycle_drop_joins_reader() {
    let tab = make_tab(24, 80);

    // Feed some data so the reader thread is actively processing.
    tab.write_input(b"echo working\r\n");
    thread::sleep(Duration::from_millis(100));

    // Measure drop time — should complete within the 2-second timeout.
    let start = Instant::now();
    drop(tab);
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "tab drop should complete promptly, took {elapsed:?}",
    );
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn fair_mutex_concurrent_access() {
    let tab = make_tab(24, 80);
    let terminal = Arc::clone(tab.terminal());

    // Spawn a thread that simulates a renderer: repeatedly locks the
    // terminal and reads grid state.
    let render_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let render_done_clone = Arc::clone(&render_done);
    let mut render_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let render_count_clone = Arc::clone(&render_count);

    let render_thread = thread::spawn(move || {
        while !render_done_clone.load(std::sync::atomic::Ordering::Relaxed) {
            let term = terminal.lock();
            let _lines = term.grid().lines();
            drop(term);
            render_count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            thread::sleep(Duration::from_millis(5));
        }
    });

    // Main thread sends rapid input while the render thread is locking.
    for i in 0..20 {
        tab.write_input(format!("echo iter{i}\r\n").as_bytes());
        thread::sleep(Duration::from_millis(10));
    }

    // Poll until PTY output appears in the grid (up to 2 seconds).
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut has_content = false;
    while Instant::now() < deadline {
        let term = tab.terminal().lock();
        let grid = term.grid();
        for line_idx in 0..grid.lines() {
            let row = &grid[Line(line_idx as i32)];
            let text: String = (0..grid.cols()).map(|c| row[Column(c)].ch).collect();
            if text.contains("iter") {
                has_content = true;
                break;
            }
        }
        drop(term);
        if has_content {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    // Stop the render thread.
    render_done.store(true, std::sync::atomic::Ordering::Relaxed);
    render_thread
        .join()
        .expect("render thread should not panic");

    // Both threads should have made progress.
    let renders = Arc::get_mut(&mut render_count)
        .expect("sole owner")
        .get_mut();
    assert!(
        *renders > 0,
        "render thread should have acquired the lock at least once",
    );

    assert!(
        has_content,
        "PTY reader should have parsed output into grid"
    );
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn resize_updates_pty_dimensions() {
    let tab = make_tab(24, 80);

    // Initial dimensions.
    {
        let term = tab.terminal().lock();
        assert_eq!(term.grid().lines(), 24);
        assert_eq!(term.grid().cols(), 80);
    }

    // Resize the PTY to 120x40.
    tab.resize(40, 120);

    // Wait for the resize to propagate through the PTY event loop.
    // The PTY reports the new size, but the terminal grid is NOT
    // resized here — grid reflow is in Section 12. We verify the
    // PTY accepted the resize without error.
    thread::sleep(Duration::from_millis(200));

    // Send a command after resize to verify the pipeline still works.
    tab.write_input(b"echo after_resize\r\n");
    let found = poll_grid_contains(&tab, "after_resize", Duration::from_secs(5));
    assert!(found, "pipeline should still work after resize");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a Tab with default settings and a live PTY.
fn make_tab(rows: u16, cols: u16) -> Tab {
    Tab::new(TabId::next(), &tab_cfg(rows, cols), test_proxy())
        .expect("tab creation should succeed")
}

/// Build a [`TabConfig`] with defaults suitable for tests.
fn tab_cfg(rows: u16, cols: u16) -> super::TabConfig {
    super::TabConfig {
        rows,
        cols,
        scrollback: 1000,
        theme: Theme::default(),
    }
}

/// Poll the terminal grid until `needle` appears in any visible row.
fn poll_grid_contains(tab: &Tab, needle: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));

        let term = tab.terminal().lock();
        let grid = term.grid();

        for line_idx in 0..grid.lines() {
            let row = &grid[Line(line_idx as i32)];
            let text: String = (0..grid.cols()).map(|c| row[Column(c)].ch).collect();
            if text.contains(needle) {
                return true;
            }
        }
    }

    false
}

/// Get a cloned winit `EventLoopProxy` for tests.
///
/// Winit only allows one `EventLoop` per process. This creates one
/// on first call (leaked to keep the proxy valid) and clones the
/// proxy for each test.
fn test_proxy() -> EventLoopProxy<TermEvent> {
    use std::sync::OnceLock;

    static PROXY: OnceLock<EventLoopProxy<TermEvent>> = OnceLock::new();
    PROXY
        .get_or_init(|| {
            let event_loop = build_event_loop();
            let proxy = event_loop.create_proxy();
            // Leak so the event loop stays alive for the process lifetime.
            std::mem::forget(event_loop);
            proxy
        })
        .clone()
}

/// Build a winit event loop usable from test threads.
///
/// Tests run outside the main thread. winit requires `any_thread(true)`
/// on both Windows and Linux (X11/Wayland) to allow this.
fn build_event_loop() -> winit::event_loop::EventLoop<TermEvent> {
    #[cfg(windows)]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        winit::event_loop::EventLoop::<TermEvent>::with_user_event()
            .with_any_thread(true)
            .build()
            .expect("event loop")
    }
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        winit::event_loop::EventLoop::<TermEvent>::with_user_event()
            .with_any_thread(true)
            .build()
            .expect("event loop")
    }
    #[cfg(target_os = "macos")]
    {
        winit::event_loop::EventLoop::<TermEvent>::with_user_event()
            .build()
            .expect("event loop")
    }
}
