//! Tests for mark mode motion functions and key dispatch.

use oriterm_core::grid::StableRowIndex;
use oriterm_core::{Selection, SelectionMode, SelectionPoint, Side};

use super::motion::{self, AbsCursor, GridBounds};
use super::{extend_or_create_selection, select_all};
use crate::tab::MarkCursor;

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
// Roadmap checkbox tests (9.3 spec)
// ---------------------------------------------------------------------------
// These verify the mark mode state transitions. Tests that require a full
// Tab with PTY are marked #[ignore] and follow the existing tab test pattern.

#[test]
#[ignore = "requires display server (winit event loop)"]
fn enter_mark_mode_sets_flag_exit_clears_it() {
    let mut tab = make_tab(24, 80);
    assert!(!tab.is_mark_mode());

    tab.enter_mark_mode();
    assert!(tab.is_mark_mode());
    assert!(tab.mark_cursor().is_some());

    tab.exit_mark_mode();
    assert!(!tab.is_mark_mode());
    assert!(tab.mark_cursor().is_none());
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn shift_right_extends_selection_by_one_column() {
    let mut tab = make_tab(24, 80);
    tab.enter_mark_mode();
    let old_mc = tab.mark_cursor().expect("mark cursor");
    let old_col = old_mc.col;

    // Simulate Shift+Right: compute new cursor, then extend selection.
    let new_mc = {
        let term = tab.terminal().lock();
        let g = term.grid();
        let Some(abs_row) = old_mc.row.to_absolute(g) else {
            panic!("row should be valid");
        };
        let bounds = GridBounds {
            total_rows: g.scrollback().len() + g.lines(),
            cols: g.cols(),
            visible_lines: g.lines(),
        };
        let cur = AbsCursor {
            abs_row,
            col: old_col,
        };
        let new_abs = motion::move_right(cur, bounds);
        let stable = StableRowIndex::from_absolute(g, new_abs.abs_row);
        MarkCursor {
            row: stable,
            col: new_abs.col,
        }
    };

    extend_or_create_selection(&mut tab, &old_mc, &new_mc);
    tab.set_mark_cursor(new_mc);

    assert!(tab.selection().is_some(), "selection should exist");
    assert_eq!(
        tab.mark_cursor().expect("mark cursor").col,
        old_col + 1,
        "cursor should have moved right by one",
    );
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn ctrl_a_selects_entire_buffer() {
    let mut tab = make_tab(24, 80);
    tab.enter_mark_mode();

    select_all(&mut tab);

    let sel = tab.selection().expect("selection should exist");
    assert_eq!(sel.mode, SelectionMode::Char);
    assert_eq!(sel.anchor.col, 0);
    assert_eq!(sel.anchor.side, Side::Left);
    assert_eq!(sel.end.side, Side::Right);
}

#[test]
#[ignore = "requires display server (winit event loop)"]
fn escape_clears_selection_and_exits_mark_mode() {
    let mut tab = make_tab(24, 80);
    tab.enter_mark_mode();

    // Create a selection via extend helper.
    let old_mc = tab.mark_cursor().expect("mark cursor");
    let new_mc = MarkCursor {
        row: old_mc.row,
        col: old_mc.col + 1,
    };
    extend_or_create_selection(&mut tab, &old_mc, &new_mc);
    tab.set_mark_cursor(new_mc);
    assert!(tab.selection().is_some());

    // Simulate Escape: clear selection and exit mark mode.
    tab.clear_selection();
    tab.exit_mark_mode();

    assert!(tab.selection().is_none(), "selection should be cleared");
    assert!(!tab.is_mark_mode(), "mark mode should be exited");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a Tab with default settings and a live PTY.
fn make_tab(rows: u16, cols: u16) -> crate::tab::Tab {
    use crate::tab::{Tab, TabId};

    let proxy = test_proxy();
    Tab::new(TabId::next(), rows, cols, 1000, proxy).expect("tab creation should succeed")
}

/// Get a cloned winit `EventLoopProxy` for tests.
fn test_proxy() -> winit::event_loop::EventLoopProxy<crate::tab::TermEvent> {
    use crate::tab::TermEvent;
    use std::sync::OnceLock;

    static PROXY: OnceLock<winit::event_loop::EventLoopProxy<TermEvent>> = OnceLock::new();
    PROXY
        .get_or_init(|| {
            let event_loop = build_event_loop();
            let proxy = event_loop.create_proxy();
            std::mem::forget(event_loop);
            proxy
        })
        .clone()
}

/// Build a winit event loop usable from test threads.
fn build_event_loop() -> winit::event_loop::EventLoop<crate::tab::TermEvent> {
    use crate::tab::TermEvent;

    #[cfg(windows)]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        winit::event_loop::EventLoop::<TermEvent>::with_user_event()
            .with_any_thread(true)
            .build()
            .expect("event loop")
    }
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        winit::event_loop::EventLoop::<TermEvent>::with_user_event()
            .with_any_thread(true)
            .build()
            .expect("event loop")
    }
    #[cfg(target_os = "macos")]
    {
        winit::event_loop::EventLoop::<TermEvent>::with_user_event()
            .build()
            .expect("event loop")
    }
}
