//! Tests for mark mode motion functions and key dispatch.

use oriterm_core::grid::StableRowIndex;
use oriterm_core::{Selection, SelectionMode, SelectionPoint, Side};
use oriterm_mux::{MarkCursor, PaneSnapshot, WireCell, WireCursor, WireCursorShape, WireRgb};

use super::motion::{self, AbsCursor, GridBounds};
use super::{ensure_visible, extend_or_create_selection, extract_word_context, select_all};
use crate::app::snapshot_grid::SnapshotGrid;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Build a simple WireCell with a character and no flags.
fn cell(ch: char) -> WireCell {
    WireCell {
        ch,
        fg: WireRgb {
            r: 255,
            g: 255,
            b: 255,
        },
        bg: WireRgb { r: 0, g: 0, b: 0 },
        flags: 0,
        underline_color: None,
        hyperlink_uri: None,
        zerowidth: Vec::new(),
    }
}

/// Build a test snapshot with configurable scrollback and display offset.
fn test_snapshot_full(
    cells: Vec<Vec<WireCell>>,
    cols: u16,
    stable_row_base: u64,
    scrollback_len: u32,
    display_offset: u32,
) -> PaneSnapshot {
    PaneSnapshot {
        cells,
        cursor: WireCursor {
            col: 0,
            row: 0,
            shape: WireCursorShape::Block,
            visible: true,
        },
        palette: vec![[0; 3]; 270],
        title: String::new(),
        icon_name: None,
        cwd: None,
        modes: 0,
        scrollback_len,
        display_offset,
        stable_row_base,
        cols,
        search_active: false,
        search_query: String::new(),
        search_matches: Vec::new(),
        search_focused: None,
        search_total_matches: 0,
    }
}

/// Build a test snapshot with 100 rows of scrollback and no display offset.
fn test_snapshot(cells: Vec<Vec<WireCell>>, cols: u16, stable_row_base: u64) -> PaneSnapshot {
    test_snapshot_full(cells, cols, stable_row_base, 100, 0)
}

// ---------------------------------------------------------------------------
// GridBounds helpers
// ---------------------------------------------------------------------------

/// Standard 80x24 grid with no scrollback.
fn bounds_80x24() -> GridBounds {
    GridBounds {
        total_rows: 24,
        cols: 80,
        visible_lines: 24,
    }
}

/// 80-column grid with 100 rows of scrollback + 24 visible.
fn bounds_with_scrollback() -> GridBounds {
    GridBounds {
        total_rows: 124,
        cols: 80,
        visible_lines: 24,
    }
}

// ---------------------------------------------------------------------------
// move_left
// ---------------------------------------------------------------------------

#[test]
fn move_left_decrements_col() {
    let c = AbsCursor { abs_row: 0, col: 5 };
    let r = motion::move_left(c, bounds_80x24());
    assert_eq!(r, AbsCursor { abs_row: 0, col: 4 });
}

#[test]
fn move_left_wraps_to_prev_row() {
    let c = AbsCursor { abs_row: 1, col: 0 };
    let r = motion::move_left(c, bounds_80x24());
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 0,
            col: 79
        }
    );
}

#[test]
fn move_left_clamps_at_buffer_start() {
    let c = AbsCursor { abs_row: 0, col: 0 };
    let r = motion::move_left(c, bounds_80x24());
    assert_eq!(r, AbsCursor { abs_row: 0, col: 0 });
}

// ---------------------------------------------------------------------------
// move_right
// ---------------------------------------------------------------------------

#[test]
fn move_right_increments_col() {
    let c = AbsCursor { abs_row: 0, col: 5 };
    let r = motion::move_right(c, bounds_80x24());
    assert_eq!(r, AbsCursor { abs_row: 0, col: 6 });
}

#[test]
fn move_right_wraps_to_next_row() {
    let c = AbsCursor {
        abs_row: 0,
        col: 79,
    };
    let r = motion::move_right(c, bounds_80x24());
    assert_eq!(r, AbsCursor { abs_row: 1, col: 0 });
}

#[test]
fn move_right_clamps_at_buffer_end() {
    let c = AbsCursor {
        abs_row: 23,
        col: 79,
    };
    let r = motion::move_right(c, bounds_80x24());
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 23,
            col: 79
        }
    );
}

// ---------------------------------------------------------------------------
// move_up / move_down
// ---------------------------------------------------------------------------

#[test]
fn move_up_decrements_row() {
    let c = AbsCursor {
        abs_row: 5,
        col: 10,
    };
    let r = motion::move_up(c);
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 4,
            col: 10
        }
    );
}

#[test]
fn move_up_clamps_at_top() {
    let c = AbsCursor {
        abs_row: 0,
        col: 10,
    };
    let r = motion::move_up(c);
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 0,
            col: 10
        }
    );
}

#[test]
fn move_down_increments_row() {
    let c = AbsCursor {
        abs_row: 5,
        col: 10,
    };
    let r = motion::move_down(c, bounds_80x24());
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 6,
            col: 10
        }
    );
}

#[test]
fn move_down_clamps_at_bottom() {
    let c = AbsCursor {
        abs_row: 23,
        col: 10,
    };
    let r = motion::move_down(c, bounds_80x24());
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 23,
            col: 10
        }
    );
}

#[test]
fn move_down_preserves_col() {
    let c = AbsCursor {
        abs_row: 5,
        col: 40,
    };
    let r = motion::move_down(c, bounds_80x24());
    assert_eq!(r.col, 40);
}

// ---------------------------------------------------------------------------
// page_up / page_down
// ---------------------------------------------------------------------------

#[test]
fn page_up_moves_by_visible_lines() {
    let b = bounds_with_scrollback();
    let c = AbsCursor {
        abs_row: 50,
        col: 10,
    };
    let r = motion::page_up(c, b);
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 26,
            col: 10
        }
    );
}

#[test]
fn page_up_clamps_at_top() {
    let b = bounds_with_scrollback();
    let c = AbsCursor {
        abs_row: 5,
        col: 10,
    };
    let r = motion::page_up(c, b);
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 0,
            col: 10
        }
    );
}

#[test]
fn page_down_moves_by_visible_lines() {
    let b = bounds_with_scrollback();
    let c = AbsCursor {
        abs_row: 50,
        col: 10,
    };
    let r = motion::page_down(c, b);
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 74,
            col: 10
        }
    );
}

#[test]
fn page_down_clamps_at_bottom() {
    let b = bounds_with_scrollback();
    let c = AbsCursor {
        abs_row: 120,
        col: 10,
    };
    let r = motion::page_down(c, b);
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 123,
            col: 10
        }
    );
}

// ---------------------------------------------------------------------------
// line_start / line_end
// ---------------------------------------------------------------------------

#[test]
fn line_start_moves_to_col_zero() {
    let c = AbsCursor {
        abs_row: 5,
        col: 40,
    };
    let r = motion::line_start(c);
    assert_eq!(r, AbsCursor { abs_row: 5, col: 0 });
}

#[test]
fn line_end_moves_to_last_col() {
    let c = AbsCursor { abs_row: 5, col: 0 };
    let r = motion::line_end(c, bounds_80x24());
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 5,
            col: 79
        }
    );
}

// ---------------------------------------------------------------------------
// buffer_start / buffer_end
// ---------------------------------------------------------------------------

#[test]
fn buffer_start_goes_to_origin() {
    let r = motion::buffer_start();
    assert_eq!(r, AbsCursor { abs_row: 0, col: 0 });
}

#[test]
fn buffer_end_goes_to_last_cell() {
    let r = motion::buffer_end(bounds_80x24());
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 23,
            col: 79
        }
    );
}

#[test]
fn buffer_end_with_scrollback() {
    let r = motion::buffer_end(bounds_with_scrollback());
    assert_eq!(
        r,
        AbsCursor {
            abs_row: 123,
            col: 79
        }
    );
}

// ---------------------------------------------------------------------------
// Selection containment (verifies extend_or_create_selection's Side logic)
// ---------------------------------------------------------------------------

#[test]
fn selection_forward_includes_both_endpoints() {
    // Forward selection from col 5 to col 8: both should be included.
    let anchor = SelectionPoint {
        row: StableRowIndex(0),
        col: 5,
        side: Side::Left,
    };
    let end = SelectionPoint {
        row: StableRowIndex(0),
        col: 8,
        side: Side::Right,
    };
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor,
        pivot: anchor,
        end,
    };

    assert!(sel.contains(StableRowIndex(0), 5));
    assert!(sel.contains(StableRowIndex(0), 6));
    assert!(sel.contains(StableRowIndex(0), 7));
    assert!(sel.contains(StableRowIndex(0), 8));
    assert!(!sel.contains(StableRowIndex(0), 4));
    assert!(!sel.contains(StableRowIndex(0), 9));
}

#[test]
fn selection_backward_includes_both_endpoints() {
    // Backward selection from col 8 to col 5.
    // anchor=(8, Right), end=(5, Left) → ordered start=(5,L), end=(8,R).
    let anchor = SelectionPoint {
        row: StableRowIndex(0),
        col: 8,
        side: Side::Right,
    };
    let end = SelectionPoint {
        row: StableRowIndex(0),
        col: 5,
        side: Side::Left,
    };
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor,
        pivot: anchor,
        end,
    };

    assert!(sel.contains(StableRowIndex(0), 5));
    assert!(sel.contains(StableRowIndex(0), 6));
    assert!(sel.contains(StableRowIndex(0), 7));
    assert!(sel.contains(StableRowIndex(0), 8));
    assert!(!sel.contains(StableRowIndex(0), 4));
    assert!(!sel.contains(StableRowIndex(0), 9));
}

#[test]
fn selection_across_rows() {
    // Selection from row 2 col 70 to row 3 col 5.
    let anchor = SelectionPoint {
        row: StableRowIndex(2),
        col: 70,
        side: Side::Left,
    };
    let end = SelectionPoint {
        row: StableRowIndex(3),
        col: 5,
        side: Side::Right,
    };
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor,
        pivot: anchor,
        end,
    };

    // Row 2: cols 70..=MAX should be selected.
    assert!(sel.contains(StableRowIndex(2), 70));
    assert!(sel.contains(StableRowIndex(2), 79));
    assert!(!sel.contains(StableRowIndex(2), 69));

    // Row 3: cols 0..=5 should be selected.
    assert!(sel.contains(StableRowIndex(3), 0));
    assert!(sel.contains(StableRowIndex(3), 5));
    assert!(!sel.contains(StableRowIndex(3), 6));
}

// ---------------------------------------------------------------------------
// Degenerate grid bounds
// ---------------------------------------------------------------------------

#[test]
fn single_row_single_col_grid_all_motions_clamp() {
    let b = GridBounds {
        total_rows: 1,
        cols: 1,
        visible_lines: 1,
    };
    let origin = AbsCursor { abs_row: 0, col: 0 };

    assert_eq!(motion::move_left(origin, b), origin);
    assert_eq!(motion::move_right(origin, b), origin);
    assert_eq!(motion::move_up(origin), origin);
    assert_eq!(motion::move_down(origin, b), origin);
    assert_eq!(motion::page_up(origin, b), origin);
    assert_eq!(motion::page_down(origin, b), origin);
    assert_eq!(motion::line_start(origin), origin);
    assert_eq!(motion::line_end(origin, b), origin);
    assert_eq!(motion::buffer_start(), origin);
    assert_eq!(motion::buffer_end(b), origin);
}

#[test]
fn zero_column_grid_does_not_panic() {
    let b = GridBounds {
        total_rows: 10,
        cols: 0,
        visible_lines: 10,
    };
    let c = AbsCursor { abs_row: 0, col: 0 };

    // These should not panic — saturating_sub handles cols=0.
    let _ = motion::move_left(c, b);
    let _ = motion::move_right(c, b);
    let _ = motion::line_end(c, b);
    let _ = motion::buffer_end(b);
}

#[test]
fn zero_row_grid_does_not_panic() {
    let b = GridBounds {
        total_rows: 0,
        cols: 80,
        visible_lines: 0,
    };
    let c = AbsCursor { abs_row: 0, col: 0 };

    let _ = motion::move_down(c, b);
    let _ = motion::page_down(c, b);
    let _ = motion::buffer_end(b);
}

// ---------------------------------------------------------------------------
// Sequential motions accumulate
// ---------------------------------------------------------------------------

#[test]
fn sequential_right_motions_accumulate() {
    let b = bounds_80x24();
    let mut c = AbsCursor { abs_row: 0, col: 0 };
    for _ in 0..5 {
        c = motion::move_right(c, b);
    }
    assert_eq!(c, AbsCursor { abs_row: 0, col: 5 });
}

#[test]
fn sequential_motions_wrap_across_rows() {
    let b = GridBounds {
        total_rows: 10,
        cols: 3,
        visible_lines: 10,
    };
    let mut c = AbsCursor { abs_row: 0, col: 0 };
    // 3 cols per row: move right 7 times → row 2 col 1.
    for _ in 0..7 {
        c = motion::move_right(c, b);
    }
    assert_eq!(c, AbsCursor { abs_row: 2, col: 1 });

    // Move left 7 times → back to origin.
    for _ in 0..7 {
        c = motion::move_left(c, b);
    }
    assert_eq!(c, AbsCursor { abs_row: 0, col: 0 });
}

#[test]
fn sequential_down_then_up_returns_to_start() {
    let b = bounds_80x24();
    let start = AbsCursor {
        abs_row: 10,
        col: 40,
    };
    let mut c = start;
    for _ in 0..5 {
        c = motion::move_down(c, b);
    }
    assert_eq!(c.abs_row, 15);
    assert_eq!(c.col, 40);
    for _ in 0..5 {
        c = motion::move_up(c);
    }
    assert_eq!(c, start);
}

// ---------------------------------------------------------------------------
// Page up preserves column
// ---------------------------------------------------------------------------

#[test]
fn page_up_preserves_col() {
    let b = bounds_with_scrollback();
    let c = AbsCursor {
        abs_row: 50,
        col: 42,
    };
    let r = motion::page_up(c, b);
    assert_eq!(r.col, 42);
}

// ---------------------------------------------------------------------------
// Selection direction reversal
// ---------------------------------------------------------------------------

#[test]
fn selection_reversal_forward_then_backward() {
    // Simulate: anchor at col 5, extend forward to col 8, then reverse to col 3.
    // After reversal, cols 3..=5 should be selected (anchor inclusive).
    let anchor = SelectionPoint {
        row: StableRowIndex(0),
        col: 5,
        side: Side::Right, // backward: anchor gets Right
    };
    let end = SelectionPoint {
        row: StableRowIndex(0),
        col: 3,
        side: Side::Left, // backward: end gets Left
    };
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor,
        pivot: anchor,
        end,
    };

    // After ordering: start=(3,Left), end=(5,Right).
    assert!(sel.contains(StableRowIndex(0), 3));
    assert!(sel.contains(StableRowIndex(0), 4));
    assert!(sel.contains(StableRowIndex(0), 5));
    assert!(!sel.contains(StableRowIndex(0), 2));
    assert!(!sel.contains(StableRowIndex(0), 6));
}

#[test]
fn selection_reversal_across_rows() {
    // Anchor at row 5 col 10, extend backward to row 3 col 70.
    let anchor = SelectionPoint {
        row: StableRowIndex(5),
        col: 10,
        side: Side::Right, // backward
    };
    let end = SelectionPoint {
        row: StableRowIndex(3),
        col: 70,
        side: Side::Left, // backward
    };
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor,
        pivot: anchor,
        end,
    };

    // Ordered: start=(3,70,Left), end=(5,10,Right).
    assert!(sel.contains(StableRowIndex(3), 70));
    assert!(sel.contains(StableRowIndex(3), 79));
    assert!(sel.contains(StableRowIndex(4), 0));
    assert!(sel.contains(StableRowIndex(4), 79));
    assert!(sel.contains(StableRowIndex(5), 0));
    assert!(sel.contains(StableRowIndex(5), 10));
    assert!(!sel.contains(StableRowIndex(5), 11));
    assert!(!sel.contains(StableRowIndex(3), 69));
}

// ---------------------------------------------------------------------------
// Single-cell selection (Equal case)
// ---------------------------------------------------------------------------

#[test]
fn selection_equal_position_is_empty() {
    // When anchor == end with (Left, Left), the selection is empty.
    // This is correct: shifting back to the anchor deselects everything.
    let point = SelectionPoint {
        row: StableRowIndex(0),
        col: 5,
        side: Side::Left,
    };
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: point,
        pivot: point,
        end: point,
    };

    assert!(sel.is_empty());
    // effective_start_col=5, effective_end_col=4 → nothing contained.
    assert!(!sel.contains(StableRowIndex(0), 5));
}

#[test]
fn selection_equal_at_col_zero_is_empty() {
    // Edge case: Equal at col 0 — effective_end_col returns 0 (not wrapping).
    let point = SelectionPoint {
        row: StableRowIndex(0),
        col: 0,
        side: Side::Left,
    };
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: point,
        pivot: point,
        end: point,
    };

    assert!(sel.is_empty());
    // effective_end_col for (col=0, Left) returns 0 (col > 0 check fails).
    // effective_start_col=0, effective_end_col=0 → contains col 0.
    // This is a special case: is_empty() is true but contains(0) may be true.
    // The is_empty check takes priority in rendering.
}

// ---------------------------------------------------------------------------
// extend_or_create_selection (pure function tests)
// ---------------------------------------------------------------------------

#[test]
fn extend_creates_new_selection_forward() {
    let anchor = MarkCursor {
        row: StableRowIndex(5),
        col: 10,
    };
    let end = MarkCursor {
        row: StableRowIndex(5),
        col: 15,
    };
    let sel = extend_or_create_selection(None, &anchor, &end);
    assert_eq!(sel.mode, SelectionMode::Char);
    assert_eq!(sel.anchor.row, StableRowIndex(5));
    assert_eq!(sel.anchor.col, 10);
    assert_eq!(sel.anchor.side, Side::Left);
    assert_eq!(sel.end.row, StableRowIndex(5));
    assert_eq!(sel.end.col, 15);
    assert_eq!(sel.end.side, Side::Right);
    // Both endpoints included.
    assert!(sel.contains(StableRowIndex(5), 10));
    assert!(sel.contains(StableRowIndex(5), 15));
    assert!(!sel.contains(StableRowIndex(5), 9));
    assert!(!sel.contains(StableRowIndex(5), 16));
}

#[test]
fn extend_creates_new_selection_backward() {
    let anchor = MarkCursor {
        row: StableRowIndex(5),
        col: 15,
    };
    let end = MarkCursor {
        row: StableRowIndex(5),
        col: 10,
    };
    let sel = extend_or_create_selection(None, &anchor, &end);
    assert_eq!(sel.anchor.side, Side::Right);
    assert_eq!(sel.end.side, Side::Left);
    assert!(sel.contains(StableRowIndex(5), 10));
    assert!(sel.contains(StableRowIndex(5), 15));
}

#[test]
fn extend_preserves_anchor_from_existing_selection() {
    let existing = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: StableRowIndex(5),
            col: 10,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: StableRowIndex(5),
            col: 10,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: StableRowIndex(5),
            col: 12,
            side: Side::Right,
        },
    };
    // Extend further right: anchor stays at col 10.
    let old_cursor = MarkCursor {
        row: StableRowIndex(5),
        col: 12,
    };
    let new_cursor = MarkCursor {
        row: StableRowIndex(5),
        col: 20,
    };
    let sel = extend_or_create_selection(Some(&existing), &old_cursor, &new_cursor);
    assert_eq!(sel.anchor.col, 10);
    assert_eq!(sel.end.col, 20);
    assert!(sel.contains(StableRowIndex(5), 10));
    assert!(sel.contains(StableRowIndex(5), 20));
}

#[test]
fn extend_equal_position_produces_empty_selection() {
    let mc = MarkCursor {
        row: StableRowIndex(0),
        col: 5,
    };
    let sel = extend_or_create_selection(None, &mc, &mc);
    assert!(sel.is_empty());
}

// ---------------------------------------------------------------------------
// select_all (SnapshotGrid-based)
// ---------------------------------------------------------------------------

#[test]
fn select_all_covers_entire_buffer() {
    let snap = test_snapshot(
        vec![
            vec![cell('a'), cell('b'), cell('c')],
            vec![cell('d'), cell('e'), cell('f')],
        ],
        3,
        100,
    );
    let grid = SnapshotGrid::new(&snap);
    let sel = select_all(&grid);
    assert_eq!(sel.mode, SelectionMode::Char);
    assert_eq!(sel.anchor.col, 0);
    assert_eq!(sel.anchor.side, Side::Left);
    assert_eq!(sel.end.col, 2); // cols - 1
    assert_eq!(sel.end.side, Side::Right);
}

// ---------------------------------------------------------------------------
// Auto-scroll (ensure_visible with SnapshotGrid)
// ---------------------------------------------------------------------------

#[test]
fn ensure_visible_in_viewport_returns_none() {
    let snap = test_snapshot(vec![vec![cell('a')]; 3], 1, 100);
    let grid = SnapshotGrid::new(&snap);
    // Cursor at viewport row 0 (stable 100, abs 100, first_visible=100).
    let cursor = MarkCursor {
        row: StableRowIndex(100),
        col: 0,
    };
    assert!(ensure_visible(&grid, &cursor).is_none());
}

#[test]
fn ensure_visible_above_viewport_returns_positive_delta() {
    let snap = test_snapshot(vec![vec![cell('a')]; 3], 1, 100);
    let grid = SnapshotGrid::new(&snap);
    // Cursor at absolute row 0 (stable 0), viewport starts at abs 100.
    let cursor = MarkCursor {
        row: StableRowIndex(0),
        col: 0,
    };
    let delta = ensure_visible(&grid, &cursor);
    assert_eq!(delta, Some(100)); // scroll up 100 rows
}

#[test]
fn ensure_visible_below_viewport_returns_negative_delta() {
    // Viewport scrolled into history: display_offset=10, first_visible=90.
    let snap = test_snapshot_full(vec![vec![cell('a')]; 3], 1, 90, 100, 10);
    let grid = SnapshotGrid::new(&snap);
    // Cursor at absolute row 100 (stable 100), last_visible = 90 + 2 = 92.
    let cursor = MarkCursor {
        row: StableRowIndex(100),
        col: 0,
    };
    let delta = ensure_visible(&grid, &cursor);
    assert_eq!(delta, Some(-8)); // scroll down 8 rows
}

// ---------------------------------------------------------------------------
// Word navigation (pure motion functions)
// ---------------------------------------------------------------------------

#[test]
fn word_left_jumps_to_word_start() {
    // Cursor inside a word (col 7, word starts at 5).
    let c = AbsCursor { abs_row: 2, col: 7 };
    let ctx = motion::WordContext {
        ws: 5,
        we: 9,
        prev_same_row_ws: None,
        prev_row_ws: None,
        next_same_row_we: None,
        next_row_we: None,
    };
    assert_eq!(motion::word_left(c, &ctx), AbsCursor { abs_row: 2, col: 5 });
}

#[test]
fn word_left_jumps_to_prev_word_on_same_row() {
    // Cursor at word start (col 5, ws=5), prev word starts at 0.
    let c = AbsCursor { abs_row: 2, col: 5 };
    let ctx = motion::WordContext {
        ws: 5,
        we: 9,
        prev_same_row_ws: Some(0),
        prev_row_ws: None,
        next_same_row_we: None,
        next_row_we: None,
    };
    assert_eq!(motion::word_left(c, &ctx), AbsCursor { abs_row: 2, col: 0 });
}

#[test]
fn word_left_wraps_to_prev_row() {
    // Cursor at col 0, ws=0, no prev word on same row, prev row available.
    let c = AbsCursor { abs_row: 3, col: 0 };
    let ctx = motion::WordContext {
        ws: 0,
        we: 4,
        prev_same_row_ws: None,
        prev_row_ws: Some(70),
        next_same_row_we: None,
        next_row_we: None,
    };
    assert_eq!(
        motion::word_left(c, &ctx),
        AbsCursor {
            abs_row: 2,
            col: 70
        }
    );
}

#[test]
fn word_left_at_origin_clamps() {
    let c = AbsCursor { abs_row: 0, col: 0 };
    let ctx = motion::WordContext {
        ws: 0,
        we: 0,
        prev_same_row_ws: None,
        prev_row_ws: None,
        next_same_row_we: None,
        next_row_we: None,
    };
    assert_eq!(motion::word_left(c, &ctx), AbsCursor { abs_row: 0, col: 0 });
}

#[test]
fn word_right_jumps_to_word_end() {
    // Cursor inside a word (col 2, word ends at 4).
    let c = AbsCursor { abs_row: 1, col: 2 };
    let ctx = motion::WordContext {
        ws: 0,
        we: 4,
        prev_same_row_ws: None,
        prev_row_ws: None,
        next_same_row_we: None,
        next_row_we: None,
    };
    assert_eq!(
        motion::word_right(c, &ctx, bounds_80x24()),
        AbsCursor { abs_row: 1, col: 4 }
    );
}

#[test]
fn word_right_jumps_to_next_word_on_same_row() {
    // Cursor at word end (col 4, we=4), next word ends at 9.
    let c = AbsCursor { abs_row: 1, col: 4 };
    let ctx = motion::WordContext {
        ws: 0,
        we: 4,
        prev_same_row_ws: None,
        prev_row_ws: None,
        next_same_row_we: Some(9),
        next_row_we: None,
    };
    assert_eq!(
        motion::word_right(c, &ctx, bounds_80x24()),
        AbsCursor { abs_row: 1, col: 9 }
    );
}

#[test]
fn word_right_wraps_to_next_row() {
    // Cursor at word end, no next word on same row, next row available.
    let c = AbsCursor {
        abs_row: 1,
        col: 75,
    };
    let ctx = motion::WordContext {
        ws: 70,
        we: 75,
        prev_same_row_ws: None,
        prev_row_ws: None,
        next_same_row_we: None,
        next_row_we: Some(5),
    };
    assert_eq!(
        motion::word_right(c, &ctx, bounds_80x24()),
        AbsCursor { abs_row: 2, col: 5 }
    );
}

#[test]
fn word_right_clamps_at_end_of_buffer() {
    // Last row, at word end, no next word, no next row.
    let c = AbsCursor {
        abs_row: 23,
        col: 75,
    };
    let ctx = motion::WordContext {
        ws: 70,
        we: 75,
        prev_same_row_ws: None,
        prev_row_ws: None,
        next_same_row_we: None,
        next_row_we: None,
    };
    assert_eq!(
        motion::word_right(c, &ctx, bounds_80x24()),
        AbsCursor {
            abs_row: 23,
            col: 79
        }
    );
}

// ---------------------------------------------------------------------------
// Word navigation with SnapshotGrid (integration)
// ---------------------------------------------------------------------------

#[test]
fn word_left_at_origin_clamps_with_snapshot_grid() {
    let snap = test_snapshot_full(
        vec![vec![cell('h'), cell('e'), cell('l'), cell('l'), cell('o')]],
        5,
        0,
        0,
        0,
    );
    let grid = SnapshotGrid::new(&snap);
    let ctx = extract_word_context(&grid, 0, 0, ",│`|:\"' ()[]{}<>\t");
    let c = AbsCursor { abs_row: 0, col: 0 };
    let r = motion::word_left(c, &ctx);
    assert_eq!(r, AbsCursor { abs_row: 0, col: 0 });
}

#[test]
fn word_right_at_end_clamps_with_snapshot_grid() {
    let snap = test_snapshot_full(
        vec![vec![cell('h'), cell('e'), cell('l'), cell('l'), cell('o')]],
        5,
        0,
        0,
        0,
    );
    let grid = SnapshotGrid::new(&snap);
    let bounds = GridBounds {
        total_rows: 1,
        cols: 5,
        visible_lines: 1,
    };
    let ctx = extract_word_context(&grid, 0, 4, ",│`|:\"' ()[]{}<>\t");
    let c = AbsCursor { abs_row: 0, col: 4 };
    let r = motion::word_right(c, &ctx, bounds);
    assert_eq!(r.abs_row, 0);
    assert_eq!(r.col, 4);
}
