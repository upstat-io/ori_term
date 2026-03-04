//! End-to-end daemon tests.
//!
//! These tests start a real [`MuxServer`] in a background thread, connect
//! [`MuxClient`]s over Unix domain sockets, and exercise the full
//! daemon→client rendering pipeline with real PTY sessions.

#![cfg(unix)]

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use oriterm_core::grid::StableRowIndex;
use oriterm_core::selection::{Selection, SelectionMode, SelectionPoint};
use oriterm_core::{Side, Theme};
use oriterm_mux::backend::MuxBackend;
use oriterm_mux::domain::SpawnConfig;
use oriterm_mux::server::MuxServer;
use oriterm_mux::{
    MuxClient, MuxNotification, PaneId, PaneSnapshot, TabId, WindowId, WireCursorShape,
};

// ---------------------------------------------------------------------------
// Test daemon harness
// ---------------------------------------------------------------------------

/// A daemon server running in a background thread for testing.
struct TestDaemon {
    socket_path: std::path::PathBuf,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    _tmpdir: tempfile::TempDir,
}

impl TestDaemon {
    /// Start a daemon with a unique socket in a temp directory.
    fn start() -> Self {
        let tmpdir = tempfile::tempdir().expect("failed to create temp dir");
        let socket_path = tmpdir.path().join("mux.sock");
        let pid_path = tmpdir.path().join("mux.pid");

        let mut server =
            MuxServer::with_paths(&socket_path, &pid_path).expect("failed to create MuxServer");
        let shutdown = server.shutdown_flag();

        let thread = thread::spawn(move || {
            if let Err(e) = server.run() {
                eprintln!("MuxServer error: {e}");
            }
        });

        // Wait for the socket to appear (server needs time to bind).
        let deadline = Instant::now() + Duration::from_secs(5);
        while !socket_path.exists() {
            if Instant::now() > deadline {
                panic!("daemon socket did not appear within 5 seconds");
            }
            thread::sleep(Duration::from_millis(10));
        }

        Self {
            socket_path,
            shutdown,
            thread: Some(thread),
            _tmpdir: tmpdir,
        }
    }

    /// Connect a new client to this daemon.
    fn connect_client(&self) -> MuxClient {
        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        MuxClient::connect(&self.socket_path, wakeup).expect("failed to connect MuxClient")
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a window and tab in a single client, returning IDs.
fn create_window_and_tab(client: &mut MuxClient) -> (WindowId, TabId, PaneId) {
    let window_id = client
        .create_window()
        .expect("create_window should succeed");
    let config = SpawnConfig::default();
    let (tab_id, pane_id) = client
        .create_tab(window_id, &config, Theme::Dark)
        .expect("create_tab should succeed");
    (window_id, tab_id, pane_id)
}

/// Wait until a direct snapshot fetch contains the expected text.
///
/// Polls the daemon for a fresh snapshot every 50ms until the text
/// appears or the timeout expires.
fn wait_for_text_in_snapshot(
    client: &mut MuxClient,
    pane_id: PaneId,
    text: &str,
    timeout: Duration,
) -> PaneSnapshot {
    let deadline = Instant::now() + timeout;
    loop {
        // Drain pending notifications so dirty flags propagate.
        client.poll_events();
        let mut notifs = Vec::new();
        client.drain_notifications(&mut notifs);

        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            if snapshot_contains(snap, text) {
                return snap.clone();
            }
        }

        if Instant::now() > deadline {
            if let Some(snap) = client.pane_snapshot(pane_id) {
                eprintln!("=== Snapshot cells at timeout ===");
                for (i, row) in snap.cells.iter().enumerate() {
                    let line: String = row.iter().map(|c| c.ch).collect();
                    eprintln!("  row {i}: {line:?}");
                }
            }
            panic!("timed out waiting for text {text:?} in pane {pane_id}");
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Check whether a snapshot's visible cells contain a substring.
fn snapshot_contains(snapshot: &PaneSnapshot, text: &str) -> bool {
    for row in &snapshot.cells {
        let line: String = row.iter().map(|c| c.ch).collect();
        if line.contains(text) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests: Section 44.3 — Window-as-Client Model
// ---------------------------------------------------------------------------

/// 44.3: MuxClient backend creates a tab, sends input, and sees output
/// in the pane snapshot.
#[test]
fn client_create_tab_type_see_output() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");

    // Give the shell time to initialize.
    thread::sleep(Duration::from_millis(500));

    // Send a command through the PTY.
    client.send_input(pane_id, b"echo ORITERM_E2E_TEST\n");

    // Wait for the output to appear in the snapshot.
    let snap = wait_for_text_in_snapshot(
        &mut client,
        pane_id,
        "ORITERM_E2E_TEST",
        Duration::from_secs(5),
    );

    assert!(
        snapshot_contains(&snap, "ORITERM_E2E_TEST"),
        "snapshot should contain the echo output"
    );
}

/// 44.3: Push notification flow — daemon PaneOutput → client PaneDirty
/// → snapshot refresh → rendered content.
#[test]
fn push_notification_triggers_dirty_flag() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");

    // Give shell time to start, then clear any initial dirty state.
    thread::sleep(Duration::from_millis(500));
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);
    client.clear_pane_snapshot_dirty(pane_id);

    // Send input to generate new output.
    client.send_input(pane_id, b"echo PUSH_TEST\n");

    // Wait for PaneDirty notification.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        client.poll_events();
        notifs.clear();
        client.drain_notifications(&mut notifs);

        if client.is_pane_snapshot_dirty(pane_id) {
            client.refresh_pane_snapshot(pane_id);
            let snap = client
                .pane_snapshot(pane_id)
                .expect("refresh should cache snapshot")
                .clone();
            client.clear_pane_snapshot_dirty(pane_id);
            assert!(
                snapshot_contains(&snap, "PUSH_TEST"),
                "refreshed snapshot should contain the new output"
            );
            return;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for PaneDirty notification"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

/// 44.3: Multiple windows (clients) connected, each rendering its own tabs.
#[test]
fn multiple_clients_independent_windows() {
    let daemon = TestDaemon::start();

    // Client A creates window A with one tab.
    let mut client_a = daemon.connect_client();
    let (win_a, _tab_a, pane_a) = create_window_and_tab(&mut client_a);
    client_a.claim_window(win_a).expect("claim A");

    // Client B creates window B with one tab.
    let mut client_b = daemon.connect_client();
    let (win_b, _tab_b, pane_b) = create_window_and_tab(&mut client_b);
    client_b.claim_window(win_b).expect("claim B");

    // Wait for shells to initialize.
    thread::sleep(Duration::from_millis(500));

    // Send different commands to each window.
    client_a.send_input(pane_a, b"echo WINDOW_A_OUTPUT\n");
    client_b.send_input(pane_b, b"echo WINDOW_B_OUTPUT\n");

    // Verify each window has only its own output.
    let snap_a = wait_for_text_in_snapshot(
        &mut client_a,
        pane_a,
        "WINDOW_A_OUTPUT",
        Duration::from_secs(5),
    );
    let snap_b = wait_for_text_in_snapshot(
        &mut client_b,
        pane_b,
        "WINDOW_B_OUTPUT",
        Duration::from_secs(5),
    );

    assert!(snapshot_contains(&snap_a, "WINDOW_A_OUTPUT"));
    assert!(!snapshot_contains(&snap_a, "WINDOW_B_OUTPUT"));
    assert!(snapshot_contains(&snap_b, "WINDOW_B_OUTPUT"));
    assert!(!snapshot_contains(&snap_b, "WINDOW_A_OUTPUT"));
}

/// 44.3: Window process crash → daemon keeps sessions → new client reconnects.
#[test]
fn client_crash_cleans_up_owned_window() {
    let daemon = TestDaemon::start();

    // Client creates window and tab, sends some output.
    let pane_id = {
        let mut client = daemon.connect_client();
        let (win, _tab, pane) = create_window_and_tab(&mut client);
        client.claim_window(win).expect("claim");

        thread::sleep(Duration::from_millis(500));
        client.send_input(pane, b"echo BEFORE_CRASH\n");
        wait_for_text_in_snapshot(&mut client, pane, "BEFORE_CRASH", Duration::from_secs(5));

        // Client drops here — simulates a crash.
        pane
    };

    // Brief pause for daemon to notice disconnection.
    thread::sleep(Duration::from_millis(200));

    // New client connects — the crashed client's window should be gone.
    let mut client2 = daemon.connect_client();

    // The pane should no longer exist (window was closed on disconnect).
    let snap = client2.refresh_pane_snapshot(pane_id);
    assert!(
        snap.is_none(),
        "pane should be cleaned up after owning client disconnects"
    );
}

// ---------------------------------------------------------------------------
// Tests: Section 44.4 — Cross-Process Tab Migration
// ---------------------------------------------------------------------------

/// 44.4: PTY session survives tab move — running command continues.
#[test]
fn pty_session_survives_tab_move() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    // Create two windows.
    let (win_a, tab_a, pane_a) = create_window_and_tab(&mut client);
    let win_b = client.create_window().expect("create second window");
    client.claim_window(win_a).expect("claim A");

    // Let shell start and send a marker.
    thread::sleep(Duration::from_millis(500));
    client.send_input(pane_a, b"echo BEFORE_MOVE\n");
    wait_for_text_in_snapshot(&mut client, pane_a, "BEFORE_MOVE", Duration::from_secs(5));

    // Move tab from window A to window B.
    assert!(
        client.move_tab_to_window(tab_a, win_b),
        "move_tab_to_window should succeed"
    );

    // Send more input — PTY should still be alive.
    client.send_input(pane_a, b"echo AFTER_MOVE\n");
    let snap = wait_for_text_in_snapshot(&mut client, pane_a, "AFTER_MOVE", Duration::from_secs(5));

    assert!(
        snapshot_contains(&snap, "AFTER_MOVE"),
        "PTY should continue working after tab move"
    );
}

/// 44.4: Scrollback preserved after tab move.
#[test]
fn scrollback_preserved_after_move() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (win_a, tab_a, pane_a) = create_window_and_tab(&mut client);
    let win_b = client.create_window().expect("create second window");
    client.claim_window(win_a).expect("claim A");

    // Let shell start.
    thread::sleep(Duration::from_millis(500));

    // Generate scrollback and wait for completion.
    client.send_input(
        pane_a,
        b"for i in $(seq 1 100); do echo SCROLL_LINE_$i; done\n",
    );
    wait_for_text_in_snapshot(
        &mut client,
        pane_a,
        "SCROLL_LINE_100",
        Duration::from_secs(10),
    );
    thread::sleep(Duration::from_millis(200));

    // Get scrollback length before move.
    let snap_before = client
        .refresh_pane_snapshot(pane_a)
        .expect("snapshot before move");
    let scrollback_before = snap_before.scrollback_len;

    // Move tab.
    assert!(client.move_tab_to_window(tab_a, win_b));

    // Get scrollback length after move.
    let snap_after = client
        .refresh_pane_snapshot(pane_a)
        .expect("snapshot after move");

    assert_eq!(
        snap_after.scrollback_len, scrollback_before,
        "scrollback length should be preserved after tab move"
    );
}

/// 44.4: Terminal modes preserved after tab move.
#[test]
fn terminal_modes_preserved_after_move() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (win_a, tab_a, pane_a) = create_window_and_tab(&mut client);
    let win_b = client.create_window().expect("create second window");
    client.claim_window(win_a).expect("claim A");

    // Let shell start.
    thread::sleep(Duration::from_millis(500));

    // Enable bracketed paste mode (DECSET 2004).
    client.send_input(pane_a, b"\x1b[?2004h");
    thread::sleep(Duration::from_millis(200));

    // Get modes before move.
    let snap_before = client
        .refresh_pane_snapshot(pane_a)
        .expect("snapshot before move");
    let modes_before = snap_before.modes;

    // Move tab.
    assert!(client.move_tab_to_window(tab_a, win_b));

    // Get modes after move.
    let snap_after = client
        .refresh_pane_snapshot(pane_a)
        .expect("snapshot after move");

    assert_eq!(
        snap_after.modes, modes_before,
        "terminal modes should be preserved after tab move"
    );
}

/// 44.4: Concurrent tab moves don't corrupt state.
#[test]
fn concurrent_tab_moves_no_corruption() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    // Create window A with two tabs (so neither is the "last tab").
    let (win_a, tab_a, pane_a) = create_window_and_tab(&mut client);
    let config = SpawnConfig::default();
    let (tab_a2, pane_a2) = client
        .create_tab(win_a, &config, Theme::Dark)
        .expect("second tab in A");
    // Create window C as move target.
    let win_c = client.create_window().expect("create window C");
    client.claim_window(win_a).expect("claim A");

    // Let shells start and mark them.
    thread::sleep(Duration::from_millis(500));
    client.send_input(pane_a, b"echo TAB_A_MARKER\n");
    client.send_input(pane_a2, b"echo TAB_A2_MARKER\n");
    wait_for_text_in_snapshot(&mut client, pane_a, "TAB_A_MARKER", Duration::from_secs(5));
    wait_for_text_in_snapshot(
        &mut client,
        pane_a2,
        "TAB_A2_MARKER",
        Duration::from_secs(5),
    );

    // Move both tabs from window A to window C simultaneously.
    assert!(client.move_tab_to_window(tab_a, win_c), "move tab A to C");
    assert!(client.move_tab_to_window(tab_a2, win_c), "move tab A2 to C");

    // Verify both panes still have their markers — no state corruption.
    client.refresh_pane_snapshot(pane_a);
    let snap_a = client
        .pane_snapshot(pane_a)
        .expect("snapshot pane A after moves")
        .clone();
    client.refresh_pane_snapshot(pane_a2);
    let snap_a2 = client
        .pane_snapshot(pane_a2)
        .expect("snapshot pane A2 after moves")
        .clone();

    assert!(
        snapshot_contains(&snap_a, "TAB_A_MARKER"),
        "pane A should retain its content after concurrent moves"
    );
    assert!(
        snapshot_contains(&snap_a2, "TAB_A2_MARKER"),
        "pane A2 should retain its content after concurrent moves"
    );
}

/// 44.4: Keystrokes route to the correct pane after tab migration.
#[test]
fn keystrokes_route_correctly_after_move() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (win_a, tab_a, pane_a) = create_window_and_tab(&mut client);
    let win_b = client.create_window().expect("create window B");
    client.claim_window(win_a).expect("claim A");

    // Let shell start.
    thread::sleep(Duration::from_millis(500));

    // Move tab to window B.
    assert!(client.move_tab_to_window(tab_a, win_b));

    // Send input after the move — should still reach the pane.
    client.send_input(pane_a, b"echo AFTER_MOVE_TYPING\n");

    let snap = wait_for_text_in_snapshot(
        &mut client,
        pane_a,
        "AFTER_MOVE_TYPING",
        Duration::from_secs(5),
    );

    assert!(
        snapshot_contains(&snap, "AFTER_MOVE_TYPING"),
        "keystrokes should route to the pane after tab migration"
    );
}

/// 44.4: Tab tear-off mechanics — moving to a new window works at the
/// daemon level (window positioning is a GUI concern, not tested here).
#[test]
fn tab_tearoff_mechanics() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    // Create window with two tabs so moving one doesn't leave empty window.
    let (win_a, tab_a1, pane_a1) = create_window_and_tab(&mut client);
    let config = SpawnConfig::default();
    let (_tab_a2, _pane_a2) = client
        .create_tab(win_a, &config, Theme::Dark)
        .expect("create second tab");
    client.claim_window(win_a).expect("claim A");

    // Let shell start.
    thread::sleep(Duration::from_millis(500));
    client.send_input(pane_a1, b"echo TEAROFF_TEST\n");
    wait_for_text_in_snapshot(&mut client, pane_a1, "TEAROFF_TEST", Duration::from_secs(5));

    // "Tear off" — move tab to a new window (daemon creates it).
    let new_win = client.create_window().expect("create tear-off window");
    assert!(client.move_tab_to_window(tab_a1, new_win));

    // Verify: pane still alive, content preserved.
    let snap = client
        .refresh_pane_snapshot(pane_a1)
        .expect("snapshot after tear-off");
    assert!(
        snapshot_contains(snap, "TEAROFF_TEST"),
        "content should survive tear-off"
    );

    // Verify: original window still has the other tab.
    let tabs = client
        .session()
        .get_window(win_a)
        .map(|w| w.tabs().to_vec())
        .unwrap_or_default();
    assert_eq!(tabs.len(), 1, "original window should have 1 remaining tab");
}

// ---------------------------------------------------------------------------
// Helpers: Section 14.1 — Test Harness Extensions
// ---------------------------------------------------------------------------

/// Wait until a snapshot is available for the given pane.
///
/// Returns an owned snapshot to avoid borrow/lifetime issues in test loops.
fn wait_for_snapshot(client: &mut MuxClient, pane_id: PaneId, timeout: Duration) -> PaneSnapshot {
    let deadline = Instant::now() + timeout;
    loop {
        client.poll_events();
        let mut notifs = Vec::new();
        client.drain_notifications(&mut notifs);

        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            return snap.clone();
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for snapshot on pane {pane_id}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

// ---------------------------------------------------------------------------
// Tests: Section 14.2 — Core Operation Tests
// ---------------------------------------------------------------------------

/// 14.2: Resize pane grid via IPC.
#[test]
fn test_resize_pane() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");

    // Let shell initialize.
    thread::sleep(Duration::from_millis(500));

    // Resize to 40 rows × 100 cols.
    client.resize_pane_grid(pane_id, 40, 100);

    // Give the daemon time to process the fire-and-forget resize.
    thread::sleep(Duration::from_millis(200));

    let snap = wait_for_snapshot(&mut client, pane_id, Duration::from_secs(5));
    assert_eq!(snap.cols, 100, "snapshot cols should match resize");
    assert_eq!(snap.cells.len(), 40, "snapshot rows should match resize");
}

/// 14.2: Scroll display up and verify display_offset.
#[test]
fn test_scroll_display() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    // Generate scrollback and wait for completion. The fence output
    // appears on 2 rows (command echo + actual output) only after the
    // for loop finishes, guaranteeing all output has landed.
    client.send_input(pane_id, b"for i in $(seq 1 200); do echo LINE_$i; done\n");
    client.send_input(pane_id, b"echo SCROLL_FENCE\n");
    let fence_deadline = Instant::now() + Duration::from_secs(10);
    loop {
        client.poll_events();
        let mut notifs = Vec::new();
        client.drain_notifications(&mut notifs);
        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            let count = snap
                .cells
                .iter()
                .filter(|row| {
                    let line: String = row.iter().map(|c| c.ch).collect();
                    line.contains("SCROLL_FENCE")
                })
                .count();
            if count >= 2 {
                break;
            }
        }
        assert!(
            Instant::now() < fence_deadline,
            "timed out waiting for scroll fence"
        );
        thread::sleep(Duration::from_millis(50));
    }
    // Wait for the shell prompt after the fence to settle.
    thread::sleep(Duration::from_millis(500));
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);

    // Scroll up by 10 lines.
    client.scroll_display(pane_id, 10);
    thread::sleep(Duration::from_millis(300));
    client.poll_events();
    notifs.clear();
    client.drain_notifications(&mut notifs);

    let snap = wait_for_snapshot(&mut client, pane_id, Duration::from_secs(5));
    assert_eq!(
        snap.display_offset, 10,
        "display_offset should be 10 after scrolling up 10 lines"
    );
}

/// 14.2: Scroll to bottom resets display_offset.
#[test]
fn test_scroll_to_bottom() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    // Generate scrollback and wait for completion via 2-row fence.
    client.send_input(pane_id, b"for i in $(seq 1 200); do echo LINE_$i; done\n");
    client.send_input(pane_id, b"echo SCROLL_BTM_FENCE\n");
    let fence_deadline = Instant::now() + Duration::from_secs(10);
    loop {
        client.poll_events();
        let mut notifs = Vec::new();
        client.drain_notifications(&mut notifs);
        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            let count = snap
                .cells
                .iter()
                .filter(|row| {
                    let line: String = row.iter().map(|c| c.ch).collect();
                    line.contains("SCROLL_BTM_FENCE")
                })
                .count();
            if count >= 2 {
                break;
            }
        }
        assert!(
            Instant::now() < fence_deadline,
            "timed out waiting for scroll bottom fence"
        );
        thread::sleep(Duration::from_millis(50));
    }
    thread::sleep(Duration::from_millis(500));
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);

    // Scroll up, then back to bottom.
    client.scroll_display(pane_id, 10);
    thread::sleep(Duration::from_millis(100));
    client.scroll_to_bottom(pane_id);
    thread::sleep(Duration::from_millis(200));

    let snap = wait_for_snapshot(&mut client, pane_id, Duration::from_secs(5));
    assert_eq!(
        snap.display_offset, 0,
        "display_offset should be 0 after scroll_to_bottom"
    );
}

/// 14.2: Query pane mode bits — verify bracketed paste mode.
///
/// Uses `printf` to emit the DECSET sequence through stdout so the terminal
/// emulator processes it (raw escape bytes written to stdin may not be echoed).
#[test]
fn test_pane_mode() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    // Enable bracketed paste mode via printf (stdout path ensures the
    // terminal emulator processes the escape sequence).
    client.send_input(pane_id, b"printf '\\033[?2004h'\n");

    // Poll until the mode bit is set (avoids flaky fixed timeouts).
    let bracketed_paste_bit = 1u32 << 13;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        client.poll_events();
        let mut notifs = Vec::new();
        client.drain_notifications(&mut notifs);
        let snap = wait_for_snapshot(&mut client, pane_id, Duration::from_secs(2));
        if snap.modes & bracketed_paste_bit != 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for bracketed paste mode bit"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// 14.2: Set cursor shape via IPC.
#[test]
fn test_set_cursor_shape() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    // Set cursor to bar shape.
    client.set_cursor_shape(pane_id, oriterm_core::CursorShape::Bar);
    thread::sleep(Duration::from_millis(200));

    let snap = wait_for_snapshot(&mut client, pane_id, Duration::from_secs(5));
    assert_eq!(
        snap.cursor.shape,
        WireCursorShape::Bar,
        "cursor shape should be Bar after set_cursor_shape"
    );
}

// ---------------------------------------------------------------------------
// Tests: Section 14.3 — Snapshot + Rendering Contract Tests
// ---------------------------------------------------------------------------

/// 14.3: Snapshot cols reflect resize.
#[test]
fn test_snapshot_cols() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    // Resize to 120 cols.
    client.resize_pane_grid(pane_id, 24, 120);
    thread::sleep(Duration::from_millis(200));

    let snap = wait_for_snapshot(&mut client, pane_id, Duration::from_secs(5));
    assert_eq!(snap.cols, 120, "snapshot cols should match resized width");
}

/// 14.3: Dirty flag lifecycle — dirty after output, clean after clear.
#[test]
fn test_snapshot_dirty_flag() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    // Clear initial dirty state.
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);
    client.refresh_pane_snapshot(pane_id);
    client.clear_pane_snapshot_dirty(pane_id);

    assert!(
        !client.is_pane_snapshot_dirty(pane_id),
        "dirty flag should be false after clear"
    );

    // Generate output to make it dirty.
    client.send_input(pane_id, b"echo DIRTY_TEST\n");

    // Wait for PaneDirty notification.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        client.poll_events();
        notifs.clear();
        client.drain_notifications(&mut notifs);

        if client.is_pane_snapshot_dirty(pane_id) {
            break;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for dirty flag to be set"
        );
        thread::sleep(Duration::from_millis(20));
    }

    assert!(
        client.is_pane_snapshot_dirty(pane_id),
        "dirty flag should be true after pane output"
    );

    // Clear and verify.
    client.clear_pane_snapshot_dirty(pane_id);
    assert!(
        !client.is_pane_snapshot_dirty(pane_id),
        "dirty flag should be false after clear again"
    );
}

// ---------------------------------------------------------------------------
// Tests: Section 14.4 — Search + Clipboard + Notification Integration
// ---------------------------------------------------------------------------

/// 14.4: Search lifecycle — open, query, verify matches, close.
#[test]
fn test_search_lifecycle() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");

    // Send known text.
    thread::sleep(Duration::from_millis(500));
    client.send_input(pane_id, b"echo NEEDLE_HAYSTACK\n");
    wait_for_text_in_snapshot(
        &mut client,
        pane_id,
        "NEEDLE_HAYSTACK",
        Duration::from_secs(5),
    );

    // Open search.
    client.open_search(pane_id);
    thread::sleep(Duration::from_millis(200));

    let snap = wait_for_snapshot(&mut client, pane_id, Duration::from_secs(5));
    assert!(
        snap.search_active,
        "search should be active after open_search"
    );

    // Set query.
    client.search_set_query(pane_id, "NEEDLE".to_string());
    thread::sleep(Duration::from_millis(200));

    let snap = wait_for_snapshot(&mut client, pane_id, Duration::from_secs(5));
    assert_eq!(snap.search_query, "NEEDLE", "search query should match");
    assert!(
        !snap.search_matches.is_empty(),
        "search should find at least one match for NEEDLE"
    );

    // Close search.
    client.close_search(pane_id);
    thread::sleep(Duration::from_millis(200));

    let snap = wait_for_snapshot(&mut client, pane_id, Duration::from_secs(5));
    assert!(
        !snap.search_active,
        "search should be inactive after close_search"
    );
    assert!(
        snap.search_matches.is_empty(),
        "search matches should be cleared after close"
    );
}

/// 14.4: Search navigation — next/prev match changes focused index.
#[test]
fn test_search_navigation() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    // Generate multiple matches.
    client.send_input(pane_id, b"echo AAA; echo AAA; echo AAA\n");
    wait_for_text_in_snapshot(&mut client, pane_id, "AAA", Duration::from_secs(5));

    // Open search and set query.
    client.open_search(pane_id);
    thread::sleep(Duration::from_millis(100));
    client.search_set_query(pane_id, "AAA".to_string());
    thread::sleep(Duration::from_millis(200));

    let snap = wait_for_snapshot(&mut client, pane_id, Duration::from_secs(5));
    let match_count = snap.search_matches.len();
    assert!(
        match_count >= 3,
        "should find at least 3 matches for AAA, found {match_count}"
    );
    let initial_focused = snap.search_focused;

    // Navigate next.
    client.search_next_match(pane_id);
    thread::sleep(Duration::from_millis(200));

    let snap2 = wait_for_snapshot(&mut client, pane_id, Duration::from_secs(5));

    // The focused match should have changed (or wrapped).
    assert_ne!(
        snap2.search_focused, initial_focused,
        "focused match should change after search_next_match"
    );

    // Close search.
    client.close_search(pane_id);
}

/// 14.4: Extract text via IPC — send known text, select, extract.
#[test]
fn test_extract_text() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    // Send echo command and wait until the marker appears on at least
    // 2 rows: the command echo and the actual output. This guarantees
    // the output has arrived before we build the selection.
    client.send_input(pane_id, b"echo EXTR_MARKER\n");

    let deadline = Instant::now() + Duration::from_secs(5);
    let snap = loop {
        client.poll_events();
        let mut notifs = Vec::new();
        client.drain_notifications(&mut notifs);

        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            let count = snap
                .cells
                .iter()
                .filter(|row| {
                    let line: String = row.iter().map(|c| c.ch).collect();
                    line.contains("EXTR_MARKER")
                })
                .count();
            if count >= 2 {
                break snap.clone();
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for EXTR_MARKER on 2 rows"
        );
        thread::sleep(Duration::from_millis(50));
    };

    // The output row has "EXTR_MARKER" but not "echo".
    let (target_row, row_text) = snap
        .cells
        .iter()
        .enumerate()
        .filter_map(|(i, row)| {
            let line: String = row.iter().map(|c| c.ch).collect();
            if line.contains("EXTR_MARKER") {
                Some((i, line))
            } else {
                None
            }
        })
        .find(|(_, line)| !line.contains("echo"))
        .expect("should find output row with EXTR_MARKER");

    let col_start = row_text
        .find("EXTR_MARKER")
        .expect("should find EXTR_MARKER in row text");
    let col_end = col_start + "EXTR_MARKER".len() - 1;

    // Build a selection covering that text.
    // `stable_row_base` is the absolute row index of viewport row 0.
    let abs_row = snap.stable_row_base + target_row as u64;

    let selection = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: StableRowIndex(abs_row),
            col: col_start,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: StableRowIndex(abs_row),
            col: col_start,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: StableRowIndex(abs_row),
            col: col_end,
            side: Side::Right,
        },
    };

    let text = client
        .extract_text(pane_id, &selection)
        .expect("extract_text should return text");

    assert_eq!(
        text.trim(),
        "EXTR_MARKER",
        "extracted text should match the marker"
    );
}

/// 14.4: PaneDirty notification flow — output triggers dirty notification.
#[test]
fn test_notification_pane_dirty() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    // Clear initial state.
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);
    client.clear_pane_snapshot_dirty(pane_id);

    // Send input.
    client.send_input(pane_id, b"echo NOTIF_TEST\n");

    // Wait for PaneDirty notification.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut got_dirty = false;
    loop {
        client.poll_events();
        notifs.clear();
        client.drain_notifications(&mut notifs);

        for notif in &notifs {
            if let MuxNotification::PaneDirty(id) = notif {
                if *id == pane_id {
                    got_dirty = true;
                }
            }
        }

        if got_dirty {
            break;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for PaneDirty notification"
        );
        thread::sleep(Duration::from_millis(20));
    }

    assert!(
        got_dirty,
        "should receive PaneDirty notification after output"
    );
}

/// Flood output does not hang the event loop.
///
/// Sends a command that generates massive output and verifies the pane
/// remains responsive afterwards. Before the fix, `PaneNotifier::notify()`
/// blocked on the PTY writer when the kernel buffer was full, freezing the
/// main thread. Now writes go through the reader thread's channel.
#[test]
fn test_flood_output_no_hang() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");

    // Let shell initialize.
    thread::sleep(Duration::from_millis(500));

    // Generate massive output: 5000 lines of 200-char padded numbers.
    client.send_input(
        pane_id,
        b"for i in $(seq 1 5000); do printf '%0200d\\n' $i; done\n",
    );

    // Poll events in a loop with a 15-second deadline. If the main thread
    // blocks on PTY writes, this loop stalls and the test times out.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        client.poll_events();
        let mut notifs = Vec::new();
        client.drain_notifications(&mut notifs);

        // Check if the shell has returned to a prompt (output complete).
        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            // Look for the last flood line — when this appears, the bulk
            // output is finishing or done.
            if snapshot_contains(snap, "5000") {
                break;
            }
        }

        assert!(
            Instant::now() < deadline,
            "timed out during flood output — main thread likely blocked on PTY write"
        );
        thread::sleep(Duration::from_millis(100));
    }

    // Verify the pane is still responsive by sending a marker command.
    client.send_input(pane_id, b"echo FLOOD_ALIVE\n");
    let snap =
        wait_for_text_in_snapshot(&mut client, pane_id, "FLOOD_ALIVE", Duration::from_secs(10));
    assert!(
        snapshot_contains(&snap, "FLOOD_ALIVE"),
        "pane should be responsive after flood output"
    );
}

/// Infinite flood output should still stream visible updates.
///
/// Reproduces the user's manual stress case (`while true` flood loop) and
/// verifies the daemon can continue serving snapshots while output is
/// unbounded.
#[test]
fn test_infinite_flood_streams_updates() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    // Start an unbounded flood loop.
    client.send_input(pane_id, b"while true; do printf '%0200d\\n' 1; done\n");

    // Keep polling and refreshing snapshots. We should see flood text.
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut saw_output = false;
    let mut refresh_ok = 0usize;

    while Instant::now() < deadline {
        client.poll_events();
        let mut notifs = Vec::new();
        client.drain_notifications(&mut notifs);

        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            refresh_ok += 1;
            if snapshot_contains(snap, "0000000000") {
                saw_output = true;
                break;
            }
        }

        thread::sleep(Duration::from_millis(50));
    }

    // Stop the flood loop.
    client.send_input(pane_id, b"\x03");

    assert!(
        saw_output,
        "no visible flood output in snapshots (successful refreshes: {refresh_ok})"
    );
}

/// Exact user flood payload with concatenated `$RANDOM` values should stream.
#[test]
fn test_infinite_flood_random_streams_updates() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    client.send_input(
        pane_id,
        b"while true; do printf '%0200d\\n' $RANDOM$RANDOM$RANDOM$RANDOM; done\n",
    );

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut saw_output = false;
    let mut refresh_ok = 0usize;

    while Instant::now() < deadline {
        client.poll_events();
        let mut notifs = Vec::new();
        client.drain_notifications(&mut notifs);

        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            refresh_ok += 1;
            if snapshot_contains(snap, "0000000000") || snapshot_contains(snap, "printf: warning:")
            {
                saw_output = true;
                break;
            }
        }

        thread::sleep(Duration::from_millis(50));
    }

    client.send_input(pane_id, b"\x03");

    assert!(
        saw_output,
        "no visible random flood output in snapshots (successful refreshes: {refresh_ok})"
    );
}

/// 14.4: PaneTitleChanged notification fires on title change.
///
/// The shell's prompt may subsequently overwrite the title via OSC 0/7,
/// so we only verify the notification arrives, not the final snapshot
/// title (which races with shell integration).
#[test]
fn test_notification_title_changed() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    // Clear any pending notifications.
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);

    // Set title via OSC 0.
    client.send_input(pane_id, b"\x1b]0;E2E_TITLE_TEST\x07");

    // Wait for PaneTitleChanged notification.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut got_title = false;
    loop {
        client.poll_events();
        notifs.clear();
        client.drain_notifications(&mut notifs);

        for notif in &notifs {
            if let MuxNotification::PaneTitleChanged(id) = notif {
                if *id == pane_id {
                    got_title = true;
                }
            }
        }

        if got_title {
            break;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for PaneTitleChanged notification"
        );
        thread::sleep(Duration::from_millis(20));
    }

    assert!(
        got_title,
        "should receive PaneTitleChanged notification after OSC 0"
    );
}

/// Flood responsiveness: daemon snapshot path handles sustained flood at >= 20fps.
///
/// Simulates the real UI render loop (poll → dirty check → refresh → clear)
/// during infinite flood output. Verifies that:
/// 1. At least 20 snapshots complete in 3 seconds (no sustained hang).
/// 2. No single snapshot takes longer than 500ms (no momentary freeze).
#[test]
fn test_flood_snapshot_responsiveness() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let (window_id, _tab_id, pane_id) = create_window_and_tab(&mut client);
    client.claim_window(window_id).expect("claim_window");
    thread::sleep(Duration::from_millis(500));

    // Start infinite flood: `yes` outputs lines continuously until killed.
    client.send_input(pane_id, b"yes \"$(printf '%0200d' 0)\"\n");

    // Let the flood build momentum before measuring.
    thread::sleep(Duration::from_millis(300));

    // Simulate the UI render loop for 3 seconds at ~60fps cadence.
    let test_duration = Duration::from_secs(3);
    let max_frame_time = Duration::from_millis(500);
    let start = Instant::now();
    let mut snapshot_count = 0u32;
    let mut max_snapshot_time = Duration::ZERO;

    while start.elapsed() < test_duration {
        let frame_start = Instant::now();

        // Phase 1: poll events (matches real about_to_wait path).
        client.poll_events();
        let mut notifs = Vec::new();
        client.drain_notifications(&mut notifs);

        // Phase 2: refresh snapshot if dirty (matches redraw/mod.rs:110).
        if client.is_pane_snapshot_dirty(pane_id) || client.pane_snapshot(pane_id).is_none() {
            client.refresh_pane_snapshot(pane_id);
            snapshot_count += 1;
        }
        client.clear_pane_snapshot_dirty(pane_id);

        let frame_time = frame_start.elapsed();
        if frame_time > max_snapshot_time {
            max_snapshot_time = frame_time;
        }

        assert!(
            frame_time < max_frame_time,
            "snapshot {snapshot_count} took {frame_time:?} (max {max_frame_time:?}) — \
             daemon snapshot path blocked during flood output"
        );

        // Simulate GPU render time (~16ms for 60fps VSync).
        thread::sleep(Duration::from_millis(16));
    }

    // Stop the flood.
    client.send_input(pane_id, b"\x03");
    thread::sleep(Duration::from_millis(200));

    let elapsed = start.elapsed();
    let fps = snapshot_count as f64 / elapsed.as_secs_f64();

    eprintln!("--- flood snapshot responsiveness ---");
    eprintln!("  snapshots:       {snapshot_count}");
    eprintln!("  fps:             {fps:.1}");
    eprintln!("  max frame time:  {max_snapshot_time:?}");

    assert!(
        snapshot_count >= 20,
        "only {snapshot_count} snapshots in {elapsed:?} ({fps:.1} fps) — need >= 20"
    );
}
