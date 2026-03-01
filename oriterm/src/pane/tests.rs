use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Lock-free dirty flag: set and clear round-trip.
#[test]
fn grid_dirty_set_and_clear() {
    let dirty = Arc::new(AtomicBool::new(false));

    // Simulate reader thread setting dirty.
    dirty.store(true, Ordering::Release);
    assert!(dirty.load(Ordering::Acquire));

    // Simulate main thread clearing dirty.
    dirty.store(false, Ordering::Release);
    assert!(!dirty.load(Ordering::Acquire));
}

/// Wakeup coalescing: swap returns previous value.
#[test]
fn wakeup_coalescing() {
    let wakeup = Arc::new(AtomicBool::new(false));

    // First wakeup: swap false → true, returns false (was not pending).
    let was_pending = wakeup.swap(true, Ordering::Release);
    assert!(!was_pending);

    // Second wakeup: swap true → true, returns true (was already pending).
    let was_pending = wakeup.swap(true, Ordering::Release);
    assert!(was_pending);

    // Clear: swap true → false.
    wakeup.store(false, Ordering::Release);
    assert!(!wakeup.load(Ordering::Acquire));
}

/// Mode cache: store and load round-trip.
#[test]
fn mode_cache_round_trip() {
    let cache = Arc::new(AtomicU32::new(0));

    // Simulate reader thread updating mode bits.
    cache.store(0x1234, Ordering::Release);
    assert_eq!(cache.load(Ordering::Acquire), 0x1234);

    // Update again.
    cache.store(0x5678, Ordering::Release);
    assert_eq!(cache.load(Ordering::Acquire), 0x5678);
}

// --- CWD short path ---

use super::cwd_short_path;

#[test]
fn short_path_last_component() {
    assert_eq!(cwd_short_path("/home/user/projects"), "projects");
}

#[test]
fn short_path_root() {
    assert_eq!(cwd_short_path("/"), "/");
}

#[test]
fn short_path_trailing_slash() {
    assert_eq!(cwd_short_path("/home/user/"), "user");
}

#[test]
fn short_path_single_dir() {
    assert_eq!(cwd_short_path("/tmp"), "tmp");
}

#[test]
fn short_path_triple_slash() {
    assert_eq!(cwd_short_path("///"), "/");
}

#[test]
fn short_path_double_slash() {
    assert_eq!(cwd_short_path("//"), "/");
}

/// Cross-thread atomic visibility (simulated with sequential ops).
#[test]
fn dirty_flag_cross_thread_pattern() {
    let dirty = Arc::new(AtomicBool::new(false));
    let dirty2 = Arc::clone(&dirty);

    // "Reader thread" sets dirty.
    std::thread::spawn(move || {
        dirty2.store(true, Ordering::Release);
    })
    .join()
    .unwrap();

    // "Main thread" reads dirty.
    assert!(dirty.load(Ordering::Acquire));
}
