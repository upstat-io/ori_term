use std::time::Duration;

use super::{CursorBlink, DEFAULT_BLINK_INTERVAL};

#[test]
fn initial_state_is_visible() {
    let blink = CursorBlink::new(DEFAULT_BLINK_INTERVAL);
    assert!(blink.is_visible());
}

#[test]
fn update_before_interval_is_noop() {
    let mut blink = CursorBlink::new(DEFAULT_BLINK_INTERVAL);
    assert!(!blink.update());
    assert!(blink.is_visible());
}

#[test]
fn update_after_interval_toggles() {
    let mut blink = CursorBlink::new(DEFAULT_BLINK_INTERVAL);
    // Backdate the phase start so the interval has elapsed.
    blink.phase_start -= Duration::from_millis(600);
    assert!(blink.update());
    assert!(!blink.is_visible());
}

#[test]
fn double_toggle_restores_visibility() {
    let mut blink = CursorBlink::new(DEFAULT_BLINK_INTERVAL);

    // First toggle: visible → hidden.
    blink.phase_start -= Duration::from_millis(600);
    blink.update();
    assert!(!blink.is_visible());

    // Second toggle: hidden → visible.
    blink.phase_start -= Duration::from_millis(600);
    blink.update();
    assert!(blink.is_visible());
}

#[test]
fn reset_makes_visible() {
    let mut blink = CursorBlink::new(DEFAULT_BLINK_INTERVAL);

    // Toggle to hidden.
    blink.phase_start -= Duration::from_millis(600);
    blink.update();
    assert!(!blink.is_visible());

    // Reset restores visibility.
    blink.reset();
    assert!(blink.is_visible());

    // And the timer is fresh — update is a no-op.
    assert!(!blink.update());
}

#[test]
fn next_toggle_is_in_the_future() {
    let blink = CursorBlink::new(DEFAULT_BLINK_INTERVAL);
    let next = blink.next_toggle();
    assert!(next > std::time::Instant::now() - Duration::from_millis(10));
}

#[test]
fn custom_interval_respected() {
    let interval = Duration::from_millis(200);
    let mut blink = CursorBlink::new(interval);

    // 150ms is less than the 200ms interval — should not toggle.
    blink.phase_start -= Duration::from_millis(150);
    assert!(!blink.update());
    assert!(blink.is_visible());

    // 250ms exceeds the 200ms interval — should toggle.
    blink.phase_start -= Duration::from_millis(100);
    assert!(blink.update());
    assert!(!blink.is_visible());
}

#[test]
fn set_interval_changes_timing() {
    let mut blink = CursorBlink::new(Duration::from_millis(1000));

    // 600ms < 1000ms — no toggle.
    blink.phase_start -= Duration::from_millis(600);
    assert!(!blink.update());

    // Shorten interval to 500ms — now 600ms exceeds it.
    blink.set_interval(Duration::from_millis(500));
    assert!(blink.update());
    assert!(!blink.is_visible());
}
