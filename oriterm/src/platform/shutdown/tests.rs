use super::{init, should_shutdown};

#[test]
fn init_succeeds() {
    // Should not panic or error on any platform.
    assert!(init().is_ok());
}

#[test]
fn init_is_idempotent() {
    // Multiple calls should succeed without error.
    assert!(init().is_ok());
    assert!(init().is_ok());
}

#[test]
fn shutdown_flag_initially_false() {
    // Before any signal is sent, should_shutdown returns false.
    assert!(!should_shutdown());
}
