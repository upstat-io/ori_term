//! Unit tests for tab management helpers and wrap_index arithmetic.

use super::wrap_index;

// -- wrap_index tests --

#[test]
fn wrap_forward_within_range() {
    assert_eq!(wrap_index(0, 1, 3), 1);
    assert_eq!(wrap_index(1, 1, 3), 2);
}

#[test]
fn wrap_forward_wraps_around() {
    assert_eq!(wrap_index(2, 1, 3), 0);
}

#[test]
fn wrap_backward_within_range() {
    assert_eq!(wrap_index(1, -1, 3), 0);
    assert_eq!(wrap_index(2, -1, 3), 1);
}

#[test]
fn wrap_backward_wraps_around() {
    assert_eq!(wrap_index(0, -1, 3), 2);
}

#[test]
fn wrap_forward_by_two() {
    assert_eq!(wrap_index(1, 2, 3), 0);
}

#[test]
fn wrap_backward_by_two() {
    assert_eq!(wrap_index(0, -2, 3), 1);
}

#[test]
fn wrap_single_tab() {
    // With one tab, any delta returns 0.
    assert_eq!(wrap_index(0, 1, 1), 0);
    assert_eq!(wrap_index(0, -1, 1), 0);
}

#[test]
fn wrap_large_delta() {
    // Large forward delta wraps multiple times.
    assert_eq!(wrap_index(0, 7, 3), 1);
    // Large backward delta wraps multiple times.
    assert_eq!(wrap_index(0, -7, 3), 2);
}

// -- MuxWindow tab management tests --

use oriterm_mux::session::MuxWindow;
use oriterm_mux::{TabId, WindowId};

/// Create a window with N tabs for testing.
fn window_with_tabs(n: usize) -> MuxWindow {
    let mut win = MuxWindow::new(WindowId::from_raw(1));
    for i in 0..n {
        win.add_tab(TabId::from_raw(100 + i as u64));
    }
    win
}

#[test]
fn create_three_tabs_unique_ids() {
    let win = window_with_tabs(3);
    let tabs = win.tabs();
    assert_eq!(tabs.len(), 3);
    // All unique.
    assert_ne!(tabs[0], tabs[1]);
    assert_ne!(tabs[1], tabs[2]);
    assert_ne!(tabs[0], tabs[2]);
}

#[test]
fn close_middle_tab_preserves_order() {
    let mut win = window_with_tabs(3);
    let t0 = win.tabs()[0];
    let t1 = win.tabs()[1];
    let t2 = win.tabs()[2];

    win.set_active_tab_idx(0);
    win.remove_tab(t1);

    assert_eq!(win.tabs(), &[t0, t2]);
    assert_eq!(win.active_tab_idx(), 0);
}

#[test]
fn close_active_tab_adjusts_index() {
    let mut win = window_with_tabs(3);
    // Active tab is last.
    win.set_active_tab_idx(2);
    let t2 = win.tabs()[2];
    win.remove_tab(t2);
    // Active should clamp to new last.
    assert_eq!(win.active_tab_idx(), 1);
}

#[test]
fn cycle_wrap_forward() {
    let win = window_with_tabs(3);
    // tab 2 of 3 → next → tab 0.
    let next = wrap_index(2, 1, win.tabs().len());
    assert_eq!(next, 0);
}

#[test]
fn cycle_wrap_backward() {
    let win = window_with_tabs(3);
    // tab 0 of 3 → prev → tab 2.
    let next = wrap_index(0, -1, win.tabs().len());
    assert_eq!(next, 2);
}

#[test]
fn reorder_tabs() {
    let mut win = window_with_tabs(4);
    let tabs: Vec<TabId> = win.tabs().to_vec();
    // Move tab 0 to position 2.
    assert!(win.reorder_tab(0, 2));
    assert_eq!(win.tabs()[0], tabs[1]);
    assert_eq!(win.tabs()[1], tabs[2]);
    assert_eq!(win.tabs()[2], tabs[0]);
    assert_eq!(win.tabs()[3], tabs[3]);
}

#[test]
fn closing_last_tab_leaves_empty() {
    let mut win = window_with_tabs(1);
    let t0 = win.tabs()[0];
    win.remove_tab(t0);
    assert!(win.tabs().is_empty());
}
