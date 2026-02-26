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

// -- Gap analysis tests --

#[test]
fn to_viewport_large_stable_row_near_u64_max() {
    let base = u64::MAX - 100;
    let mc = MarkCursor {
        row: StableRowIndex(base + 10),
        col: 5,
    };
    // Viewport at near-max stable row — should work via checked_sub.
    assert_eq!(mc.to_viewport(base, 24), Some((10, 5)));
}

#[test]
fn to_viewport_overflow_stable_row_at_u64_max() {
    let mc = MarkCursor {
        row: StableRowIndex(u64::MAX),
        col: 0,
    };
    // Base is 0, offset would overflow usize on 32-bit — but checked_sub
    // succeeds and the cast to usize may wrap. On 64-bit it's just a huge
    // number > max_lines, so returns None.
    let result = mc.to_viewport(0, 24);
    assert_eq!(result, None);
}

#[test]
fn to_viewport_cursor_below_base_returns_none() {
    // Base is higher than cursor — checked_sub returns None.
    let mc = MarkCursor {
        row: StableRowIndex(0),
        col: 0,
    };
    assert_eq!(mc.to_viewport(1, 24), None);
}

#[test]
fn to_viewport_large_column_value() {
    let mc = MarkCursor {
        row: StableRowIndex(100),
        col: usize::MAX,
    };
    // Column is passed through as-is — no bounds checking in to_viewport.
    assert_eq!(mc.to_viewport(100, 24), Some((0, usize::MAX)));
}

#[test]
fn to_viewport_same_base_and_row_large_viewport() {
    let mc = MarkCursor {
        row: StableRowIndex(500),
        col: 42,
    };
    // Cursor is at base, viewport is very large.
    assert_eq!(mc.to_viewport(500, usize::MAX), Some((0, 42)));
}
