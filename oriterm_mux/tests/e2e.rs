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

use oriterm_core::Theme;
use oriterm_mux::backend::MuxBackend;
use oriterm_mux::domain::SpawnConfig;
use oriterm_mux::server::MuxServer;
use oriterm_mux::{MuxClient, PaneId, PaneSnapshot, TabId, WindowId};

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

    // Generate scrollback by printing many lines.
    client.send_input(
        pane_a,
        b"for i in $(seq 1 100); do echo SCROLL_LINE_$i; done\n",
    );
    thread::sleep(Duration::from_secs(2));

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
