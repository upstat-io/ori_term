//! Shared contract tests for the [`MuxBackend`] trait.
//!
//! A macro generates the same test suite for both [`EmbeddedMux`] (in-process)
//! and [`MuxClient`] (daemon IPC), verifying both backends produce identical
//! observable behavior for every `MuxBackend` method.

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
use oriterm_mux::{EmbeddedMux, MuxClient, PaneId, PaneSnapshot, WireCursorShape};

// ---------------------------------------------------------------------------
// Test context: holds the backend + IDs + optional daemon handle
// ---------------------------------------------------------------------------

/// Wrapper providing a `MuxBackend` and the pane ID needed for testing.
///
/// Owns either an `EmbeddedMux` directly or a `MuxClient` + `TestDaemon`.
/// The daemon (if any) is kept alive by the `_daemon` field.
struct TestContext {
    backend: Box<dyn MuxBackend>,
    pane_id: PaneId,
    _daemon: Option<TestDaemon>,
}

impl TestContext {
    /// Borrow the backend mutably.
    fn b(&mut self) -> &mut dyn MuxBackend {
        &mut *self.backend
    }

    /// Wait until the snapshot contains `text`, returning an owned copy.
    fn wait_for_text(&mut self, text: &str, timeout: Duration) -> PaneSnapshot {
        let deadline = Instant::now() + timeout;
        let pid = self.pane_id;
        loop {
            self.b().poll_events();
            let mut notifs = Vec::new();
            self.b().drain_notifications(&mut notifs);

            if let Some(snap) = self.b().refresh_pane_snapshot(pid) {
                if snapshot_contains(snap, text) {
                    return snap.clone();
                }
            }

            assert!(
                Instant::now() < deadline,
                "timed out waiting for text {text:?} in pane {pid}"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// Refresh and return an owned snapshot.
    fn snapshot(&mut self) -> PaneSnapshot {
        let pid = self.pane_id;
        self.b()
            .refresh_pane_snapshot(pid)
            .expect("snapshot should be available")
            .clone()
    }
}

// ---------------------------------------------------------------------------
// TestDaemon (duplicated from e2e.rs — integration tests can't share code)
// ---------------------------------------------------------------------------

struct TestDaemon {
    socket_path: std::path::PathBuf,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    _tmpdir: tempfile::TempDir,
}

impl TestDaemon {
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
// Factory functions
// ---------------------------------------------------------------------------

/// Create a `TestContext` backed by `EmbeddedMux`.
fn embedded_context() -> TestContext {
    let wakeup: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
    let mut mux = EmbeddedMux::new(wakeup);

    let config = SpawnConfig::default();
    let pane_id = mux.spawn_pane(&config, Theme::Dark).expect("spawn_pane");

    // Let the shell start up.
    thread::sleep(Duration::from_millis(500));
    mux.poll_events();
    let mut notifs = Vec::new();
    mux.drain_notifications(&mut notifs);

    TestContext {
        backend: Box::new(mux),
        pane_id,
        _daemon: None,
    }
}

/// Create a `TestContext` backed by `MuxClient` connected to a `TestDaemon`.
fn daemon_context() -> TestContext {
    let daemon = TestDaemon::start();
    let mut client = daemon.connect_client();

    let config = SpawnConfig::default();
    let pane_id = client.spawn_pane(&config, Theme::Dark).expect("spawn_pane");

    // Let the shell start up.
    thread::sleep(Duration::from_millis(500));
    client.poll_events();
    let mut notifs = Vec::new();
    client.drain_notifications(&mut notifs);

    TestContext {
        backend: Box::new(client),
        pane_id,
        _daemon: Some(daemon),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn snapshot_contains(snapshot: &PaneSnapshot, text: &str) -> bool {
    snapshot.cells.iter().any(|row| {
        let line: String = row.iter().map(|c| c.ch).collect();
        line.contains(text)
    })
}

// ---------------------------------------------------------------------------
// Contract test macro
// ---------------------------------------------------------------------------

/// Generate identical test functions for both backends.
///
/// Each test receives a `TestContext` with a window, tab, and pane already
/// created and the shell initialized. Wrapped in a `mod` for namespacing.
macro_rules! muxbackend_contract_tests {
    ($factory:path) => {
        use super::*;

        #[test]
        fn contract_spawn_pane_and_see_output() {
            let mut ctx = $factory();
            let pid = ctx.pane_id;
            ctx.b().send_input(pid, b"echo CONTRACT_OUTPUT\n");
            let snap = ctx.wait_for_text("CONTRACT_OUTPUT", Duration::from_secs(5));
            assert!(snapshot_contains(&snap, "CONTRACT_OUTPUT"));
        }

        #[test]
        fn contract_resize() {
            let mut ctx = $factory();
            let pid = ctx.pane_id;
            ctx.b().resize_pane_grid(pid, 30, 90);

            // Poll until the resize is reflected in the snapshot.
            // CI runners can be slow so a fixed sleep is unreliable.
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                ctx.b().poll_events();
                let mut n = Vec::new();
                ctx.b().drain_notifications(&mut n);
                if let Some(snap) = ctx.b().refresh_pane_snapshot(pid) {
                    if snap.cols == 90 && snap.cells.len() == 30 {
                        return;
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for resize to 30x90"
                );
                thread::sleep(Duration::from_millis(50));
            }
        }

        #[test]
        fn contract_scroll() {
            let mut ctx = $factory();
            let pid = ctx.pane_id;

            // Generate scrollback and wait for completion. Send a fence
            // command after the loop so we know all output has landed.
            ctx.b()
                .send_input(pid, b"for i in $(seq 1 200); do echo L$i; done\n");
            ctx.b().send_input(pid, b"echo SCROLL_FENCE\n");
            // Wait for the fence output (not the command echo). When the
            // fence appears on 2 rows, the loop output is fully rendered.
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                ctx.b().poll_events();
                let mut n = Vec::new();
                ctx.b().drain_notifications(&mut n);
                if let Some(snap) = ctx.b().refresh_pane_snapshot(pid) {
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
                    Instant::now() < deadline,
                    "timed out waiting for scroll fence"
                );
                thread::sleep(Duration::from_millis(50));
            }
            // Wait for the shell prompt after the fence to settle, then
            // drain all pending events so no late-arriving output resets
            // the scroll position after we scroll up.
            thread::sleep(Duration::from_millis(500));
            ctx.b().poll_events();
            let mut n = Vec::new();
            ctx.b().drain_notifications(&mut n);

            // Scroll up.
            ctx.b().scroll_display(pid, 10);
            thread::sleep(Duration::from_millis(300));
            ctx.b().poll_events();
            n.clear();
            ctx.b().drain_notifications(&mut n);
            let snap = ctx.snapshot();
            assert_eq!(
                snap.display_offset, 10,
                "display_offset after scroll_display(10)"
            );

            // Scroll to bottom.
            ctx.b().scroll_to_bottom(pid);
            thread::sleep(Duration::from_millis(300));
            ctx.b().poll_events();
            n.clear();
            ctx.b().drain_notifications(&mut n);
            let snap = ctx.snapshot();
            assert_eq!(
                snap.display_offset, 0,
                "display_offset after scroll_to_bottom"
            );
        }

        #[test]
        fn contract_mode_query() {
            let mut ctx = $factory();
            let pid = ctx.pane_id;
            let bracketed_paste_bit = 1u32 << 13;

            // Use printf to emit the DECSET sequence through the shell's
            // stdout, ensuring the terminal emulator processes it.
            ctx.b().send_input(pid, b"printf '\\033[?2004h'\n");

            // Poll until the mode bit is set (avoids flaky fixed timeouts).
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                ctx.b().poll_events();
                let mut n = Vec::new();
                ctx.b().drain_notifications(&mut n);
                let snap = ctx.snapshot();
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

        #[test]
        fn contract_cursor_shape() {
            let mut ctx = $factory();
            let pid = ctx.pane_id;
            ctx.b()
                .set_cursor_shape(pid, oriterm_core::CursorShape::Bar);
            thread::sleep(Duration::from_millis(200));
            let snap = ctx.snapshot();
            assert_eq!(
                snap.cursor.shape,
                WireCursorShape::Bar,
                "cursor shape should be Bar"
            );
        }

        #[test]
        fn contract_snapshot_lifecycle() {
            let mut ctx = $factory();
            let pid = ctx.pane_id;

            // Refresh + clear dirty.
            ctx.b().refresh_pane_snapshot(pid);
            ctx.b().clear_pane_snapshot_dirty(pid);
            assert!(!ctx.b().is_pane_snapshot_dirty(pid), "should be clean");

            // Generate output → dirty.
            ctx.b().send_input(pid, b"echo DIRTY\n");
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                ctx.b().poll_events();
                let mut n = Vec::new();
                ctx.b().drain_notifications(&mut n);
                if ctx.b().is_pane_snapshot_dirty(pid) {
                    break;
                }
                assert!(Instant::now() < deadline, "timed out waiting for dirty");
                thread::sleep(Duration::from_millis(20));
            }
            assert!(ctx.b().is_pane_snapshot_dirty(pid), "should be dirty");

            ctx.b().clear_pane_snapshot_dirty(pid);
            assert!(
                !ctx.b().is_pane_snapshot_dirty(pid),
                "should be clean again"
            );
        }

        #[test]
        fn contract_search() {
            let mut ctx = $factory();
            let pid = ctx.pane_id;
            ctx.b().send_input(pid, b"echo NEEDLE\n");
            ctx.wait_for_text("NEEDLE", Duration::from_secs(5));

            // Open search.
            ctx.b().open_search(pid);
            thread::sleep(Duration::from_millis(200));
            let snap = ctx.snapshot();
            assert!(snap.search_active, "search should be active");

            // Set query.
            ctx.b().search_set_query(pid, "NEEDLE".to_string());
            thread::sleep(Duration::from_millis(200));
            let snap = ctx.snapshot();
            assert_eq!(snap.search_query, "NEEDLE");
            assert!(!snap.search_matches.is_empty(), "should find NEEDLE");

            // Close search.
            ctx.b().close_search(pid);
            thread::sleep(Duration::from_millis(200));
            let snap = ctx.snapshot();
            assert!(!snap.search_active, "search should be inactive");
            assert!(snap.search_matches.is_empty(), "matches should be cleared");
        }

        #[test]
        fn contract_flood_output() {
            let mut ctx = $factory();
            let pid = ctx.pane_id;

            // Generate massive output: 5000 lines of 200-char padded numbers.
            ctx.b().send_input(
                pid,
                b"for i in $(seq 1 5000); do printf '%0200d\\n' $i; done\n",
            );

            // Poll until the flood finishes (last line appears).
            // CI runners (especially macOS) are slow — use a generous deadline.
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                ctx.b().poll_events();
                let mut n = Vec::new();
                ctx.b().drain_notifications(&mut n);
                if let Some(snap) = ctx.b().refresh_pane_snapshot(pid) {
                    if snapshot_contains(snap, "5000") {
                        break;
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "timed out during flood output — main thread likely blocked"
                );
                thread::sleep(Duration::from_millis(100));
            }

            // Verify responsiveness after the flood.
            ctx.b().send_input(pid, b"echo FLOOD_ALIVE\n");
            let snap = ctx.wait_for_text("FLOOD_ALIVE", Duration::from_secs(10));
            assert!(snapshot_contains(&snap, "FLOOD_ALIVE"));
        }

        /// Simulates the real UI rendering loop during flood output.
        ///
        /// Unlike `contract_flood_output` (which sleeps 100ms between polls),
        /// this test calls `refresh_pane_snapshot` in a tight loop at ~60fps,
        /// matching the actual rendering cadence. The test fails if the main
        /// thread blocks for more than 500ms on any single snapshot refresh,
        /// which would manifest as a UI hang/freeze in the real application.
        #[test]
        fn contract_flood_render_loop() {
            let mut ctx = $factory();
            let pid = ctx.pane_id;

            // Start infinite flood output.
            ctx.b()
                .send_input(pid, b"while true; do printf '%0200d\\n' 1; done\n");

            // Simulate the UI rendering loop for 3 seconds.
            // The real App does: poll_events → refresh_snapshot → GPU render.
            // We skip GPU but measure how long each snapshot takes.
            let test_duration = Duration::from_secs(3);
            let max_frame_time = Duration::from_millis(500);
            let start = Instant::now();
            let mut frame_count = 0u32;
            let mut max_snapshot_time = Duration::ZERO;
            let mut saw_output = false;

            while start.elapsed() < test_duration {
                let frame_start = Instant::now();

                // Phase 1: poll events (what about_to_wait does).
                ctx.b().poll_events();
                let mut notifs = Vec::new();
                ctx.b().drain_notifications(&mut notifs);

                // Phase 2: refresh snapshot (what handle_redraw does).
                // This is where the hang occurs — build_snapshot blocks on
                // pane.terminal().lock(), a fair lock that waits for the
                // PTY reader's lease to release.
                if ctx.b().is_pane_snapshot_dirty(pid) || ctx.b().pane_snapshot(pid).is_none() {
                    ctx.b().refresh_pane_snapshot(pid);
                }
                ctx.b().clear_pane_snapshot_dirty(pid);

                // Check if we got any output (sanity check).
                if let Some(snap) = ctx.b().pane_snapshot(pid) {
                    if snapshot_contains(snap, "0000000") {
                        saw_output = true;
                    }
                }

                let frame_time = frame_start.elapsed();
                if frame_time > max_snapshot_time {
                    max_snapshot_time = frame_time;
                }

                // This is the critical assertion: no single frame should
                // block for more than 500ms. A hang would block indefinitely.
                assert!(
                    frame_time < max_frame_time,
                    "frame {frame_count} took {frame_time:?} (max {max_frame_time:?}) — \
                     main thread blocked on terminal lock during flood output"
                );

                frame_count += 1;

                // Simulate GPU render time (~16ms for 60fps VSync).
                thread::sleep(Duration::from_millis(16));
            }

            // Stop the flood.
            ctx.b().send_input(pid, b"\x03");
            thread::sleep(Duration::from_millis(200));

            let elapsed = start.elapsed();
            let fps = frame_count as f64 / elapsed.as_secs_f64();

            eprintln!("--- flood render loop ---");
            eprintln!("  frames:          {frame_count}");
            eprintln!("  fps:             {fps:.1}");
            eprintln!("  max frame time:  {max_snapshot_time:?}");
            eprintln!("  saw output:      {saw_output}");

            // Must achieve at least 10 fps — CI runners (especially macOS)
            // run significantly slower than local machines. Real target is 60.
            assert!(
                fps >= 10.0,
                "rendering too slow during flood: {fps:.1} fps (need >= 10)"
            );
            assert!(saw_output, "flood output never appeared in snapshots");
        }

        #[test]
        fn contract_extract_text() {
            let mut ctx = $factory();
            let pid = ctx.pane_id;

            ctx.b().send_input(pid, b"echo CXTR_MARKER\n");

            // Wait until "CXTR_MARKER" appears on at least 2 rows: one is
            // the command echo ("$ echo CXTR_MARKER"), the other is the
            // actual output. This guarantees the output has arrived.
            let deadline = Instant::now() + Duration::from_secs(5);
            let snap = loop {
                ctx.b().poll_events();
                let mut n = Vec::new();
                ctx.b().drain_notifications(&mut n);

                if let Some(snap) = ctx.b().refresh_pane_snapshot(pid) {
                    let count = snap
                        .cells
                        .iter()
                        .filter(|row| {
                            let line: String = row.iter().map(|c| c.ch).collect();
                            line.contains("CXTR_MARKER")
                        })
                        .count();
                    if count >= 2 {
                        break snap.clone();
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for CXTR_MARKER on 2 rows"
                );
                thread::sleep(Duration::from_millis(50));
            };

            // The output row has "CXTR_MARKER" but not "echo".
            let (target_row, row_text) = snap
                .cells
                .iter()
                .enumerate()
                .filter_map(|(i, row)| {
                    let line: String = row.iter().map(|c| c.ch).collect();
                    if line.contains("CXTR_MARKER") {
                        Some((i, line))
                    } else {
                        None
                    }
                })
                .find(|(_, line)| !line.contains("echo"))
                .expect("should find output row with CXTR_MARKER");

            let col_start = row_text
                .find("CXTR_MARKER")
                .expect("should find text in row");
            let col_end = col_start + "CXTR_MARKER".len() - 1;
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

            let text = ctx
                .b()
                .extract_text(pid, &selection)
                .expect("extract_text should return text");
            assert_eq!(text.trim(), "CXTR_MARKER");
        }
    };
}

// ---------------------------------------------------------------------------
// Instantiate for both backends
// ---------------------------------------------------------------------------

mod embedded {
    muxbackend_contract_tests!(embedded_context);
}

mod daemon {
    muxbackend_contract_tests!(daemon_context);
}
