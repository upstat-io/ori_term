//! Tests for the config file watcher.
//!
//! Most tests require a display server (winit `EventLoop`) and are
//! marked `#[ignore]`. Run with `--ignored` on systems with a display.

use std::sync::mpsc;
use std::time::Duration;

/// Dropping the `notify_tx` sender (simulating watcher drop) unblocks
/// `notify_rx.recv()`, allowing the watch loop thread to exit cleanly.
///
/// This tests the core invariant behind the deadlock fix (a8c39a9):
/// the watcher (which owns `notify_tx`) must drop before `thread.join()`.
#[test]
fn dropping_notify_sender_unblocks_receiver() {
    let (notify_tx, notify_rx) = mpsc::channel::<Result<notify::Event, notify::Error>>();

    let handle = std::thread::spawn(move || {
        // Simulate watch_loop's blocking recv.
        let result = notify_rx.recv();
        assert!(result.is_err(), "recv should return Err after sender drops");
    });

    // Dropping the sender should unblock the receiver.
    drop(notify_tx);

    // The thread should join within a reasonable time (no deadlock).
    let joined = handle.join();
    assert!(
        joined.is_ok(),
        "thread should join cleanly after sender drops"
    );
}

/// Shutdown channel disconnection is detected by `try_recv`.
///
/// The watch loop checks `shutdown_rx.try_recv()` — when the sender is
/// dropped, `try_recv` returns `Err(Disconnected)`.
#[test]
fn shutdown_channel_disconnection_detected() {
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
    drop(shutdown_tx);

    match shutdown_rx.try_recv() {
        Err(mpsc::TryRecvError::Disconnected) => {} // Expected.
        other => panic!("expected Disconnected, got {other:?}"),
    }
}

/// The debounce recv_timeout unblocks after the timeout expires.
///
/// This validates the 200ms debounce window doesn't hang forever.
#[test]
fn debounce_timeout_returns() {
    let (_tx, rx) = mpsc::channel::<Result<notify::Event, notify::Error>>();
    let start = std::time::Instant::now();
    let debounce = Duration::from_millis(50); // Shorter for test speed.
    let result = rx.recv_timeout(debounce);
    let elapsed = start.elapsed();

    assert!(result.is_err(), "should timeout with no events");
    assert!(
        elapsed >= debounce,
        "should wait at least the debounce period"
    );
}

// --- is_theme_file ---

#[test]
fn is_theme_file_matches_toml_in_themes_dir() {
    use std::path::Path;

    let themes = Path::new("/config/themes");
    assert!(super::is_theme_file(
        Path::new("/config/themes/nord.toml"),
        themes,
    ));
}

#[test]
fn is_theme_file_rejects_non_toml() {
    use std::path::Path;

    let themes = Path::new("/config/themes");
    assert!(!super::is_theme_file(
        Path::new("/config/themes/readme.txt"),
        themes,
    ));
}

#[test]
fn is_theme_file_rejects_wrong_dir() {
    use std::path::Path;

    let themes = Path::new("/config/themes");
    assert!(!super::is_theme_file(Path::new("/other/nord.toml"), themes,));
}
