//! Tests for `MarkCursor::to_viewport`.

use oriterm_core::grid::StableRowIndex;

use super::MarkCursor;

#[test]
fn to_viewport_within_bounds() {
    let mc = MarkCursor {
        row: StableRowIndex(105),
        col: 7,
    };
    // Viewport starts at stable row 100, 24 lines tall.
    assert_eq!(mc.to_viewport(100, 24), Some((5, 7)));
}

#[test]
fn to_viewport_first_line() {
    let mc = MarkCursor {
        row: StableRowIndex(100),
        col: 0,
    };
    assert_eq!(mc.to_viewport(100, 24), Some((0, 0)));
}

#[test]
fn to_viewport_last_line() {
    let mc = MarkCursor {
        row: StableRowIndex(123),
        col: 3,
    };
    // Viewport rows 100..124 (24 lines), row 123 is the last visible.
    assert_eq!(mc.to_viewport(100, 24), Some((23, 3)));
}

#[test]
fn to_viewport_above_viewport_returns_none() {
    let mc = MarkCursor {
        row: StableRowIndex(50),
        col: 0,
    };
    // Viewport starts at 100 — cursor is above (scrolled off-screen).
    assert_eq!(mc.to_viewport(100, 24), None);
}

#[test]
fn to_viewport_below_viewport_returns_none() {
    let mc = MarkCursor {
        row: StableRowIndex(200),
        col: 0,
    };
    // Viewport covers rows 100..124, cursor at 200 is below.
    assert_eq!(mc.to_viewport(100, 24), None);
}

#[test]
fn to_viewport_exactly_at_max_lines_returns_none() {
    let mc = MarkCursor {
        row: StableRowIndex(124),
        col: 0,
    };
    // Viewport covers rows 100..124 (max_lines=24), row 124 is one past.
    assert_eq!(mc.to_viewport(100, 24), None);
}

#[test]
fn to_viewport_zero_max_lines_always_none() {
    let mc = MarkCursor {
        row: StableRowIndex(100),
        col: 0,
    };
    // Degenerate viewport with 0 lines — nothing is visible.
    assert_eq!(mc.to_viewport(100, 0), None);
}
