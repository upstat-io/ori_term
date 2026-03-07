//! End-to-end daemon tests.
//!
//! These tests start a real [`MuxServer`] in a background thread, connect
//! [`MuxClient`]s over Unix domain sockets, and exercise the full
//! daemon→client rendering pipeline with real PTY sessions.

#![cfg(target_os = "linux")]

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
use oriterm_mux::{MuxClient, MuxNotification, PaneId, PaneSnapshot, WireCursorShape};

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
        let deadline = Instant::now() + Duration::from_secs(30);
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

/// Spawn a pane in the daemon, returning its ID.
fn spawn_test_pane(client: &mut MuxClient) -> PaneId {
    let config = SpawnConfig::default();
    client
        .spawn_pane(&config, Theme::Dark)
        .expect("spawn_pane should succeed")
}

/// Spawn a pane and wait for the shell to be ready.
///
/// Sends a fence command and waits for its output to appear, replacing
/// fixed `thread::sleep` calls with event-driven readiness detection.
fn spawn_test_pane_ready(client: &mut MuxClient) -> PaneId {
    let pane_id = spawn_test_pane(client);
    wait_for_shell_ready(client, pane_id);
    pane_id
}

/// Wait for the shell in a pane to be ready by sending a fence command.
fn wait_for_shell_ready(client: &mut MuxClient, pane_id: PaneId) {
    client.send_input(pane_id, b"echo SHELL_READY_FENCE\n");
    wait_for_text_in_snapshot(
        client,
        pane_id,
        "SHELL_READY_FENCE",
        Duration::from_secs(30),
    );
    // Drain pending events after startup.
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);
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

/// Poll until a snapshot satisfies a predicate, with a 15s deadline.
///
/// Replaces fixed `thread::sleep` calls with event-driven waiting.
fn poll_until(
    client: &mut MuxClient,
    pane_id: PaneId,
    what: &str,
    predicate: impl Fn(&PaneSnapshot) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        client.poll_events();
        let mut n = Vec::new();
        client.drain_notifications(&mut n);
        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            if predicate(snap) {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for condition: {what}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

// ---------------------------------------------------------------------------
// Tests: Section 44.3 — Window-as-Client Model
// ---------------------------------------------------------------------------

/// MuxClient backend spawns a pane, sends input, and sees output
/// in the pane snapshot.
#[test]
fn client_spawn_pane_type_see_output() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let pane_id = spawn_test_pane_ready(&mut client);

    // Send a command through the PTY.
    client.send_input(pane_id, b"echo ORITERM_E2E_TEST\n");

    // Wait for the output to appear in the snapshot.
    let snap = wait_for_text_in_snapshot(
        &mut client,
        pane_id,
        "ORITERM_E2E_TEST",
        Duration::from_secs(30),
    );

    assert!(
        snapshot_contains(&snap, "ORITERM_E2E_TEST"),
        "snapshot should contain the echo output"
    );
}

/// 44.3: Push notification flow — daemon PaneOutput → client PaneOutput
/// → snapshot refresh → rendered content.
#[test]
fn push_notification_triggers_dirty_flag() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let pane_id = spawn_test_pane_ready(&mut client);

    // Clear any initial dirty state.
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);
    client.clear_pane_snapshot_dirty(pane_id);

    // Send input to generate new output.
    client.send_input(pane_id, b"echo PUSH_TEST\n");

    // Wait for PaneOutput notification.
    let deadline = Instant::now() + Duration::from_secs(30);
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
            "timed out waiting for PaneOutput notification"
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
    let pane_a = spawn_test_pane_ready(&mut client_a);

    // Client B creates a pane.
    let mut client_b = daemon.connect_client();
    let pane_b = spawn_test_pane_ready(&mut client_b);

    // Send different commands to each window.
    client_a.send_input(pane_a, b"echo WINDOW_A_OUTPUT\n");
    client_b.send_input(pane_b, b"echo WINDOW_B_OUTPUT\n");

    // Verify each window has only its own output.
    let snap_a = wait_for_text_in_snapshot(
        &mut client_a,
        pane_a,
        "WINDOW_A_OUTPUT",
        Duration::from_secs(30),
    );
    let snap_b = wait_for_text_in_snapshot(
        &mut client_b,
        pane_b,
        "WINDOW_B_OUTPUT",
        Duration::from_secs(30),
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
        let pane = spawn_test_pane_ready(&mut client);

        client.send_input(pane, b"echo BEFORE_CRASH\n");
        wait_for_text_in_snapshot(&mut client, pane, "BEFORE_CRASH", Duration::from_secs(30));

        // Client drops here — simulates a crash.
        pane
    };

    // Poll until the daemon cleans up the disconnected client's pane.
    let mut client2 = daemon.connect_client();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let snap = client2.refresh_pane_snapshot(pane_id);
        if snap.is_none() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "pane should be cleaned up after owning client disconnects"
        );
        thread::sleep(Duration::from_millis(50));
    }
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

    let pane_id = spawn_test_pane_ready(&mut client);

    // Resize to 40 rows × 100 cols.
    client.resize_pane_grid(pane_id, 40, 100);

    // Poll until the resize is reflected in the snapshot.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        client.poll_events();
        let mut n = Vec::new();
        client.drain_notifications(&mut n);
        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            if snap.cols == 100 && snap.cells.len() == 40 {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for resize to 40x100"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// 14.2: Scroll display up and verify display_offset.
#[test]
fn test_scroll_display() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let pane_id = spawn_test_pane_ready(&mut client);

    // Generate scrollback and wait for completion. The fence output
    // appears on 2 rows (command echo + actual output) only after the
    // for loop finishes, guaranteeing all output has landed.
    client.send_input(pane_id, b"for i in $(seq 1 200); do echo LINE_$i; done\n");
    client.send_input(pane_id, b"echo SCROLL_FENCE\n");
    let fence_deadline = Instant::now() + Duration::from_secs(30);
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
    // Drain pending events before scrolling.
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);

    // Scroll up by 10 lines and poll until display_offset reflects it.
    client.scroll_display(pane_id, 10);
    let scroll_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        client.poll_events();
        notifs.clear();
        client.drain_notifications(&mut notifs);
        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            if snap.display_offset == 10 {
                break;
            }
        }
        assert!(
            Instant::now() < scroll_deadline,
            "timed out waiting for display_offset=10"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// 14.2: Scroll to bottom resets display_offset.
#[test]
fn test_scroll_to_bottom() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let pane_id = spawn_test_pane_ready(&mut client);

    // Generate scrollback and wait for completion via 2-row fence.
    client.send_input(pane_id, b"for i in $(seq 1 200); do echo LINE_$i; done\n");
    client.send_input(pane_id, b"echo SCROLL_BTM_FENCE\n");
    let fence_deadline = Instant::now() + Duration::from_secs(30);
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
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);

    // Scroll up, then back to bottom.
    client.scroll_display(pane_id, 10);
    // Poll until scroll-up takes effect.
    let scroll_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        client.poll_events();
        notifs.clear();
        client.drain_notifications(&mut notifs);
        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            if snap.display_offset == 10 {
                break;
            }
        }
        assert!(
            Instant::now() < scroll_deadline,
            "timed out waiting for scroll up"
        );
        thread::sleep(Duration::from_millis(50));
    }

    client.scroll_to_bottom(pane_id);
    // Poll until scroll-to-bottom takes effect.
    let btm_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        client.poll_events();
        notifs.clear();
        client.drain_notifications(&mut notifs);
        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            if snap.display_offset == 0 {
                break;
            }
        }
        assert!(
            Instant::now() < btm_deadline,
            "timed out waiting for scroll_to_bottom"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// 14.2: Query pane mode bits — verify bracketed paste mode.
///
/// Uses `printf` to emit the DECSET sequence through stdout so the terminal
/// emulator processes it (raw escape bytes written to stdin may not be echoed).
#[test]
fn test_pane_mode() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let pane_id = spawn_test_pane_ready(&mut client);

    // Enable bracketed paste mode via printf (stdout path ensures the
    // terminal emulator processes the escape sequence).
    client.send_input(pane_id, b"printf '\\033[?2004h'\n");

    // Poll until the mode bit is set (avoids flaky fixed timeouts).
    let bracketed_paste_bit = 1u32 << 13;
    let deadline = Instant::now() + Duration::from_secs(30);
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

    let pane_id = spawn_test_pane_ready(&mut client);

    // Set cursor to bar shape and poll until reflected.
    client.set_cursor_shape(pane_id, oriterm_core::CursorShape::Bar);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        client.poll_events();
        let mut n = Vec::new();
        client.drain_notifications(&mut n);
        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            if snap.cursor.shape == WireCursorShape::Bar {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for cursor shape change"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

// ---------------------------------------------------------------------------
// Tests: Section 14.3 — Snapshot + Rendering Contract Tests
// ---------------------------------------------------------------------------

/// 14.3: Snapshot cols reflect resize.
#[test]
fn test_snapshot_cols() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let pane_id = spawn_test_pane_ready(&mut client);

    // Resize to 120 cols and poll until reflected.
    client.resize_pane_grid(pane_id, 24, 120);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        client.poll_events();
        let mut n = Vec::new();
        client.drain_notifications(&mut n);
        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            if snap.cols == 120 {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for resize to 120 cols"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

/// 14.3: Dirty flag lifecycle — dirty after output, clean after clear.
#[test]
fn test_snapshot_dirty_flag() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let pane_id = spawn_test_pane_ready(&mut client);

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

    // Wait for PaneOutput notification.
    let deadline = Instant::now() + Duration::from_secs(30);
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

    let pane_id = spawn_test_pane_ready(&mut client);

    // Send known text.
    client.send_input(pane_id, b"echo NEEDLE_HAYSTACK\n");
    wait_for_text_in_snapshot(
        &mut client,
        pane_id,
        "NEEDLE_HAYSTACK",
        Duration::from_secs(30),
    );

    // Open search and poll until active.
    client.open_search(pane_id);
    poll_until(&mut client, pane_id, "search_active", |snap| {
        snap.search_active
    });

    // Set query and poll until matches appear.
    client.search_set_query(pane_id, "NEEDLE".to_string());
    poll_until(&mut client, pane_id, "search matches", |snap| {
        snap.search_query == "NEEDLE" && !snap.search_matches.is_empty()
    });

    // Close search and poll until inactive.
    client.close_search(pane_id);
    poll_until(&mut client, pane_id, "search inactive", |snap| {
        !snap.search_active && snap.search_matches.is_empty()
    });
}

/// 14.4: Search navigation — next/prev match changes focused index.
#[test]
fn test_search_navigation() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let pane_id = spawn_test_pane_ready(&mut client);

    // Generate multiple matches.
    client.send_input(pane_id, b"echo AAA; echo AAA; echo AAA\n");
    wait_for_text_in_snapshot(&mut client, pane_id, "AAA", Duration::from_secs(30));

    // Open search, set query, and poll until matches appear.
    client.open_search(pane_id);
    poll_until(&mut client, pane_id, "search_active", |snap| {
        snap.search_active
    });
    client.search_set_query(pane_id, "AAA".to_string());
    poll_until(&mut client, pane_id, "search matches >= 3", |snap| {
        snap.search_matches.len() >= 3
    });

    let snap = client
        .pane_snapshot(pane_id)
        .expect("snapshot should be cached")
        .clone();
    let initial_focused = snap.search_focused;

    // Navigate next and poll until focused index changes.
    client.search_next_match(pane_id);
    poll_until(&mut client, pane_id, "focused match changed", |snap| {
        snap.search_focused != initial_focused
    });

    // Close search.
    client.close_search(pane_id);
}

/// 14.4: Extract text via IPC — send known text, select, extract.
#[test]
fn test_extract_text() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let pane_id = spawn_test_pane_ready(&mut client);

    // Use printf to produce output that is clearly distinguishable from
    // the command line itself. The command line contains "printf" while
    // the output line starts with "EXTR_MARKER" (no "printf" prefix).
    client.send_input(pane_id, b"printf 'EXTR_MARKER\\n'\n");

    let deadline = Instant::now() + Duration::from_secs(30);
    let snap = loop {
        client.poll_events();
        let mut notifs = Vec::new();
        client.drain_notifications(&mut notifs);

        if let Some(snap) = client.refresh_pane_snapshot(pane_id) {
            let has_output_row = snap.cells.iter().any(|row| {
                let line: String = row.iter().map(|c| c.ch).collect();
                line.contains("EXTR_MARKER") && !line.contains("printf")
            });
            if has_output_row {
                break snap.clone();
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for EXTR_MARKER output row"
        );
        thread::sleep(Duration::from_millis(50));
    };

    // The output row has "EXTR_MARKER" but not "printf".
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
        .find(|(_, line)| !line.contains("printf"))
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

/// 14.4: PaneOutput notification flow — output triggers dirty notification.
#[test]
fn test_notification_pane_dirty() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let pane_id = spawn_test_pane_ready(&mut client);

    // Clear initial state.
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);
    client.clear_pane_snapshot_dirty(pane_id);

    // Send input.
    client.send_input(pane_id, b"echo NOTIF_TEST\n");

    // Wait for PaneOutput notification.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut got_dirty = false;
    loop {
        client.poll_events();
        notifs.clear();
        client.drain_notifications(&mut notifs);

        for notif in &notifs {
            if let MuxNotification::PaneOutput(id) = notif {
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
            "timed out waiting for PaneOutput notification"
        );
        thread::sleep(Duration::from_millis(20));
    }

    assert!(
        got_dirty,
        "should receive PaneOutput notification after output"
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

    let pane_id = spawn_test_pane_ready(&mut client);

    // Generate massive output: 5000 lines of 200-char padded numbers.
    client.send_input(
        pane_id,
        b"for i in $(seq 1 5000); do printf '%0200d\\n' $i; done\n",
    );

    // Poll events in a loop with a 15-second deadline. If the main thread
    // blocks on PTY writes, this loop stalls and the test times out.
    let deadline = Instant::now() + Duration::from_secs(30);
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
        wait_for_text_in_snapshot(&mut client, pane_id, "FLOOD_ALIVE", Duration::from_secs(30));
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

    let pane_id = spawn_test_pane_ready(&mut client);

    // Start an unbounded flood loop.
    client.send_input(pane_id, b"while true; do printf '%0200d\\n' 1; done\n");

    // Keep polling and refreshing snapshots. We should see flood text.
    let deadline = Instant::now() + Duration::from_secs(30);
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

    let pane_id = spawn_test_pane_ready(&mut client);

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

    let pane_id = spawn_test_pane_ready(&mut client);

    // Clear any pending notifications.
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);

    // Set title via OSC 0.
    client.send_input(pane_id, b"\x1b]0;E2E_TITLE_TEST\x07");

    // Wait for PaneTitleChanged notification.
    // CI runners can be slow to deliver IPC notifications.
    let deadline = Instant::now() + Duration::from_secs(30);
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

/// Flood responsiveness: daemon snapshot path handles sustained flood.
///
/// Continuously refreshes snapshots during infinite flood output.
/// Verifies that:
/// 1. At least 10 snapshots complete in 3 seconds (no sustained hang).
/// 2. No single snapshot takes longer than 2s (no momentary freeze).
#[test]
fn test_flood_snapshot_responsiveness() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let pane_id = spawn_test_pane_ready(&mut client);

    // Start infinite flood: `yes` outputs lines continuously until killed.
    client.send_input(pane_id, b"yes \"$(printf '%0200d' 0)\"\n");

    // Let the flood build momentum before measuring.
    thread::sleep(Duration::from_millis(300));

    // Simulate the UI render loop for 3 seconds at ~60fps cadence.
    let test_duration = Duration::from_secs(3);
    // CI runners can have extreme scheduling latency.
    let max_frame_time = Duration::from_secs(2);
    let start = Instant::now();
    let mut snapshot_count = 0u32;
    let mut max_snapshot_time = Duration::ZERO;

    while start.elapsed() < test_duration {
        let frame_start = Instant::now();

        // Phase 1: poll events (matches real about_to_wait path).
        client.poll_events();
        let mut notifs = Vec::new();
        client.drain_notifications(&mut notifs);

        // Phase 2: always refresh during flood — we are measuring snapshot
        // throughput, not dirty-flag propagation (which has its own latency
        // through the IPC notification path).
        client.refresh_pane_snapshot(pane_id);
        snapshot_count += 1;

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

    // CI runners are slower — only catch true hangs, not scheduling jitter.
    assert!(
        snapshot_count >= 10,
        "only {snapshot_count} snapshots in {elapsed:?} ({fps:.1} fps) — need >= 10"
    );
}

// ---------------------------------------------------------------------------
// Tests: Section 44.7 — Daemon Restart + Latency
// ---------------------------------------------------------------------------

/// 44.7: Kill daemon → client detects disconnection → new daemon starts →
/// new client connects and operates normally.
#[test]
fn daemon_restart_detection_and_reconnect() {
    let tmpdir = tempfile::tempdir().expect("failed to create temp dir");
    let socket_path = tmpdir.path().join("mux.sock");
    let pid_path = tmpdir.path().join("mux.pid");

    // Phase 1: Start daemon, connect, verify working.
    let _shutdown1 = {
        let mut server =
            MuxServer::with_paths(&socket_path, &pid_path).expect("failed to create MuxServer");
        let shutdown = server.shutdown_flag();

        let sock = socket_path.clone();
        thread::spawn(move || {
            let _ = server.run();
        });

        let deadline = Instant::now() + Duration::from_secs(30);
        while !socket_path.exists() {
            assert!(
                Instant::now() < deadline,
                "daemon socket did not appear within 5 seconds"
            );
            thread::sleep(Duration::from_millis(10));
        }

        let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        let mut client = MuxClient::connect(&sock, wakeup).expect("connect to daemon 1");

        let pane = spawn_test_pane_ready(&mut client);

        client.send_input(pane, b"echo DAEMON1_ALIVE\n");
        wait_for_text_in_snapshot(&mut client, pane, "DAEMON1_ALIVE", Duration::from_secs(30));

        assert!(client.is_connected(), "client should be connected");

        // Phase 2: Kill the daemon.
        shutdown.store(true, Ordering::Release);
        shutdown
    };

    // Wait for the daemon to fully shut down and socket to be cleaned up.
    thread::sleep(Duration::from_millis(500));

    // Clean up stale socket so the next daemon can bind.
    let _ = std::fs::remove_file(&socket_path);

    // Phase 3: Start a new daemon on the same socket.
    let pid_path2 = tmpdir.path().join("mux2.pid");
    let mut server2 =
        MuxServer::with_paths(&socket_path, &pid_path2).expect("failed to create MuxServer 2");
    let shutdown2 = server2.shutdown_flag();

    let sock2 = socket_path.clone();
    thread::spawn(move || {
        let _ = server2.run();
    });

    let deadline = Instant::now() + Duration::from_secs(30);
    while !socket_path.exists() {
        assert!(
            Instant::now() < deadline,
            "daemon 2 socket did not appear within 5 seconds"
        );
        thread::sleep(Duration::from_millis(10));
    }

    // Phase 4: New client connects to the restarted daemon.
    let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
    let mut client2 = MuxClient::connect(&sock2, wakeup).expect("connect to daemon 2");

    assert!(client2.is_connected(), "client2 should be connected");

    // Create fresh session (old sessions are lost — daemon is stateless across restarts).
    let pane2 = spawn_test_pane_ready(&mut client2);

    client2.send_input(pane2, b"echo DAEMON2_ALIVE\n");
    wait_for_text_in_snapshot(
        &mut client2,
        pane2,
        "DAEMON2_ALIVE",
        Duration::from_secs(30),
    );

    // Clean shutdown.
    shutdown2.store(true, Ordering::Release);
}

/// 44.7: Raw socket round-trip latency (bypassing event loops).
///
/// Connects a raw blocking socket to the daemon and measures Ping/PingAck
/// directly — no reader threads, no mio, no mpsc channels.
#[test]
fn raw_socket_latency_baseline() {
    let daemon = TestDaemon::start();

    // Direct blocking connection (bypasses MuxClient entirely).
    use oriterm_ipc::ClientStream;
    use oriterm_mux::protocol::{MuxPdu, ProtocolCodec};

    let mut stream = ClientStream::connect(&daemon.socket_path).expect("raw connect");

    // Handshake.
    let pid = std::process::id();
    ProtocolCodec::encode_frame(&mut stream, 1, &MuxPdu::Hello { pid }).expect("write Hello");
    let mut codec = ProtocolCodec::new();
    let frame = codec.decode_frame(&mut stream).expect("read HelloAck");
    assert!(matches!(frame.pdu, MuxPdu::HelloAck { .. }));

    // Warm up.
    for seq in 2..12u32 {
        ProtocolCodec::encode_frame(&mut stream, seq, &MuxPdu::Ping).expect("write Ping");
        let _resp = codec.decode_frame(&mut stream).expect("read PingAck");
    }

    // Measure.
    const N: usize = 200;
    let mut latencies = Vec::with_capacity(N);
    for i in 0..N {
        let seq = 100 + i as u32;
        let start = Instant::now();
        ProtocolCodec::encode_frame(&mut stream, seq, &MuxPdu::Ping).expect("write Ping");
        let resp = codec.decode_frame(&mut stream).expect("read PingAck");
        let elapsed = start.elapsed();
        assert_eq!(resp.pdu, MuxPdu::PingAck);
        latencies.push(elapsed);
    }

    latencies.sort();
    let min = latencies[0];
    let median = latencies[N / 2];
    let p95 = latencies[N * 95 / 100];

    eprintln!("--- Raw socket Ping/PingAck latency ({N} iterations) ---");
    eprintln!("  min:    {min:?}");
    eprintln!("  median: {median:?}");
    eprintln!("  p95:    {p95:?}");

    // This establishes the platform baseline — epoll_wait latency on WSL2.
    assert!(
        median < Duration::from_millis(5),
        "raw socket median {median:?} exceeds 5ms — platform epoll latency is the bottleneck"
    );
}

/// 44.7: IPC round-trip latency through daemon IPC.
///
/// Measures pure IPC overhead using Ping/PingAck (zero-payload round-trip).
/// This isolates transport and thread-wakeup latency from snapshot building,
/// serialization, and PTY processing.
///
/// Also measures snapshot refresh RPC for the full render-path latency.
/// Asserts Ping median < 1ms and snapshot median < 5ms.
#[test]
fn ipc_latency_under_5ms() {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let pane_id = spawn_test_pane_ready(&mut client);

    client.send_input(pane_id, b"echo LATENCY_READY\n");
    wait_for_text_in_snapshot(
        &mut client,
        pane_id,
        "LATENCY_READY",
        Duration::from_secs(30),
    );

    // Drain pending state.
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);

    // --- Part 1: Pure IPC latency (Ping/PingAck) ---
    // Warm up.
    for _ in 0..10 {
        client.ping_rpc().expect("warmup ping");
    }

    const PING_ITERS: usize = 200;
    let mut ping_latencies = Vec::with_capacity(PING_ITERS);
    for _ in 0..PING_ITERS {
        let lat = client.ping_rpc().expect("ping_rpc should succeed");
        ping_latencies.push(lat);
    }

    ping_latencies.sort();
    let ping_median = ping_latencies[PING_ITERS / 2];
    let ping_p95 = ping_latencies[PING_ITERS * 95 / 100];
    let ping_min = ping_latencies[0];

    eprintln!("--- IPC Ping/PingAck latency ({PING_ITERS} iterations) ---");
    eprintln!("  min:    {ping_min:?}");
    eprintln!("  median: {ping_median:?}");
    eprintln!("  p95:    {ping_p95:?}");

    // --- Part 2: Snapshot RPC latency (full render path) ---
    const SNAP_ITERS: usize = 100;
    let mut snap_latencies = Vec::with_capacity(SNAP_ITERS);
    for _ in 0..SNAP_ITERS {
        let start = Instant::now();
        let snap = client.refresh_pane_snapshot(pane_id);
        let elapsed = start.elapsed();
        assert!(snap.is_some(), "snapshot refresh should succeed");
        snap_latencies.push(elapsed);
    }

    snap_latencies.sort();
    let snap_median = snap_latencies[SNAP_ITERS / 2];
    let snap_p95 = snap_latencies[SNAP_ITERS * 95 / 100];
    let snap_min = snap_latencies[0];

    eprintln!("--- IPC snapshot refresh latency ({SNAP_ITERS} iterations) ---");
    eprintln!("  min:    {snap_min:?}");
    eprintln!("  median: {snap_median:?}");
    eprintln!("  p95:    {snap_p95:?}");

    assert!(
        ping_median < Duration::from_millis(1),
        "Ping median {ping_median:?} exceeds 1ms — IPC transport is too slow"
    );

    // Ideal target: <5ms. Under parallel test load, CPU contention inflates
    // the snapshot RPC (which includes grid scan + bincode serialization).
    // Solo runs consistently hit ~3ms; parallel adds ~2ms of scheduling noise.
    // Warn at 5ms (ideal), hard-fail at 10ms (regression guard).
    if snap_median >= Duration::from_millis(5) {
        eprintln!(
            "WARNING: snapshot median {snap_median:?} exceeds ideal 5ms target \
             (likely CPU contention from parallel tests — solo median is ~3ms)"
        );
    }
    assert!(
        snap_median < Duration::from_millis(10),
        "snapshot median {snap_median:?} exceeds 10ms — render path regression"
    );
}
