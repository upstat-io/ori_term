use crate::cell::{Cell, CellFlags};
use crate::grid::Grid;
use crate::index::{Column, Line};

/// Helper: create a cell with the given character.
fn cell(ch: char) -> Cell {
    Cell {
        ch,
        ..Cell::default()
    }
}

/// Helper: write text into a grid row.
fn write_row(grid: &mut Grid, line: usize, text: &str) {
    for (col, ch) in text.chars().enumerate() {
        grid[Line(line as i32)][Column(col)] = cell(ch);
    }
}

/// Helper: read text from a grid row (trimming trailing spaces).
fn read_row(grid: &Grid, line: usize) -> String {
    let row = &grid[Line(line as i32)];
    let mut s: String = (0..row.cols()).map(|c| row[Column(c)].ch).collect();
    let trimmed = s.trim_end().len();
    s.truncate(trimmed);
    s
}

// ── Zero-size guards ────────────────────────────────────────────────

#[test]
fn resize_zero_cols_is_noop() {
    let mut grid = Grid::new(24, 80);
    grid.resize(24, 0, false);
    assert_eq!(grid.cols(), 80);
    assert_eq!(grid.lines(), 24);
}

#[test]
fn resize_zero_lines_is_noop() {
    let mut grid = Grid::new(24, 80);
    grid.resize(0, 80, false);
    assert_eq!(grid.cols(), 80);
    assert_eq!(grid.lines(), 24);
}

#[test]
fn resize_same_dimensions_is_noop() {
    let mut grid = Grid::new(24, 80);
    write_row(&mut grid, 0, "hello");
    grid.resize(24, 80, true);
    assert_eq!(read_row(&grid, 0), "hello");
}

// ── Row resize (vertical) ───────────────────────────────────────────

#[test]
fn shrink_rows_trims_trailing_blanks_first() {
    let mut grid = Grid::new(10, 80);
    // Write content in the first 3 rows, leave rest blank.
    write_row(&mut grid, 0, "line0");
    write_row(&mut grid, 1, "line1");
    write_row(&mut grid, 2, "line2");
    grid.cursor_mut().set_line(2);

    grid.resize(5, 80, false);

    assert_eq!(grid.lines(), 5);
    assert_eq!(read_row(&grid, 0), "line0");
    assert_eq!(read_row(&grid, 1), "line1");
    assert_eq!(read_row(&grid, 2), "line2");
    // No rows pushed to scrollback — blanks were trimmed.
    assert_eq!(grid.scrollback().len(), 0);
}

#[test]
fn shrink_rows_pushes_excess_to_scrollback() {
    let mut grid = Grid::new(5, 80);
    write_row(&mut grid, 0, "line0");
    write_row(&mut grid, 1, "line1");
    write_row(&mut grid, 2, "line2");
    write_row(&mut grid, 3, "line3");
    write_row(&mut grid, 4, "line4");
    grid.cursor_mut().set_line(4);

    grid.resize(3, 80, false);

    assert_eq!(grid.lines(), 3);
    // Top 2 rows pushed to scrollback.
    assert_eq!(grid.scrollback().len(), 2);
    // Visible rows are the last 3.
    assert_eq!(read_row(&grid, 0), "line2");
    assert_eq!(read_row(&grid, 1), "line3");
    assert_eq!(read_row(&grid, 2), "line4");
    // Cursor adjusted.
    assert_eq!(grid.cursor().line(), 2);
}

#[test]
fn shrink_rows_cursor_adjusted_for_scrollback_push() {
    let mut grid = Grid::new(5, 80);
    write_row(&mut grid, 0, "a");
    write_row(&mut grid, 1, "b");
    write_row(&mut grid, 2, "c");
    grid.cursor_mut().set_line(2);

    // Shrink by 1: trailing blanks trimmed (rows 3,4 blank), none pushed.
    grid.resize(4, 80, false);
    assert_eq!(grid.cursor().line(), 2);
    assert_eq!(grid.scrollback().len(), 0);
}

#[test]
fn grow_rows_appends_blanks_when_cursor_in_middle() {
    let mut grid = Grid::new(5, 80);
    write_row(&mut grid, 0, "line0");
    write_row(&mut grid, 1, "line1");
    grid.cursor_mut().set_line(1);

    grid.resize(8, 80, false);

    assert_eq!(grid.lines(), 8);
    assert_eq!(read_row(&grid, 0), "line0");
    assert_eq!(read_row(&grid, 1), "line1");
    assert_eq!(grid.cursor().line(), 1);
    assert_eq!(grid.scrollback().len(), 0);
}

#[test]
fn grow_rows_consumes_scrollback_inserts_blanks() {
    let mut grid = Grid::new(3, 80);
    write_row(&mut grid, 0, "line0");
    write_row(&mut grid, 1, "line1");
    write_row(&mut grid, 2, "line2");
    grid.cursor_mut().set_line(2);

    // Shrink to push rows to scrollback.
    grid.resize(2, 80, false);
    assert_eq!(grid.scrollback().len(), 1);
    assert_eq!(grid.cursor().line(), 1);

    // Grow back — consumes scrollback slot, inserts blank row at top.
    // Content is blanked to prevent stale scrollback ghosting.
    grid.resize(3, 80, false);
    assert_eq!(grid.lines(), 3);
    assert_eq!(grid.scrollback().len(), 0);
    assert_eq!(read_row(&grid, 0), ""); // blank (consumed, not restored)
    assert_eq!(read_row(&grid, 1), "line1");
    assert_eq!(read_row(&grid, 2), "line2");
    assert_eq!(grid.cursor().line(), 2);
}

// ── Column resize (no reflow) ───────────────────────────────────────

#[test]
fn grow_cols_no_reflow_pads_with_blanks() {
    let mut grid = Grid::new(3, 10);
    write_row(&mut grid, 0, "hello");

    grid.resize(3, 20, false);

    assert_eq!(grid.cols(), 20);
    assert_eq!(read_row(&grid, 0), "hello");
    assert_eq!(grid[Line(0)].cols(), 20);
}

#[test]
fn shrink_cols_no_reflow_truncates() {
    let mut grid = Grid::new(3, 20);
    write_row(&mut grid, 0, "hello world here");

    grid.resize(3, 5, false);

    assert_eq!(grid.cols(), 5);
    assert_eq!(read_row(&grid, 0), "hello");
}

// ── Scroll region and cursor clamping ───────────────────────────────

#[test]
fn resize_resets_scroll_region() {
    let mut grid = Grid::new(24, 80);
    grid.set_scroll_region(5, Some(20));
    assert_eq!(*grid.scroll_region(), 4..20);

    grid.resize(10, 80, false);

    assert_eq!(*grid.scroll_region(), 0..10);
}

#[test]
fn resize_clamps_cursor_to_new_bounds() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(23);
    grid.cursor_mut().set_col(Column(79));

    grid.resize(10, 40, false);

    assert_eq!(grid.cursor().line(), 9);
    assert_eq!(grid.cursor().col(), Column(39));
}

#[test]
fn resize_clamps_display_offset() {
    let mut grid = Grid::with_scrollback(5, 80, 100);
    // Push some content to scrollback.
    for i in 0..10 {
        write_row(&mut grid, 0, &format!("line{i}"));
        grid.scroll_up(1);
    }
    // Scroll back into history.
    grid.scroll_display(5);
    assert!(grid.display_offset() > 0);

    grid.resize(5, 80, false);

    assert!(grid.display_offset() <= grid.scrollback().len());
}

// ── Tab stops ───────────────────────────────────────────────────────

#[test]
fn resize_resets_tab_stops_for_new_width() {
    let mut grid = Grid::new(24, 80);
    grid.resize(24, 40, false);

    // Tab stops should be reset for new column count.
    let stops = grid.tab_stops();
    assert_eq!(stops.len(), 40);
    assert!(stops[0]);
    assert!(stops[8]);
    assert!(stops[16]);
    assert!(stops[24]);
    assert!(stops[32]);
    assert!(!stops[39]);
}

// ── Reflow: column grow (unwrap) ────────────────────────────────────

#[test]
fn reflow_grow_unwraps_soft_wrapped_line() {
    let mut grid = Grid::new(3, 10);

    // Simulate a soft-wrapped line: "helloabcde" + "world" split across two rows.
    // Fill first row fully, then set WRAP on the last cell.
    write_row(&mut grid, 0, "helloabcde");
    grid[Line(0)][Column(9)].flags.insert(CellFlags::WRAP);
    write_row(&mut grid, 1, "world");

    // Grow to 20 cols: the wrapped line should unwrap into one row.
    grid.resize(3, 20, true);

    assert_eq!(grid.cols(), 20);
    let row0 = read_row(&grid, 0);
    assert_eq!(row0, "helloabcdeworld");
}

#[test]
fn reflow_grow_non_wrapped_lines_stay_separate() {
    let mut grid = Grid::new(3, 10);
    write_row(&mut grid, 0, "hello");
    // No WRAP flag — hard newline.
    write_row(&mut grid, 1, "world");

    grid.resize(3, 20, true);

    assert_eq!(read_row(&grid, 0), "hello");
    assert_eq!(read_row(&grid, 1), "world");
}

// ── Reflow: column shrink (wrap) ────────────────────────────────────

#[test]
fn reflow_shrink_wraps_long_line() {
    let mut grid = Grid::new(20, 20);
    write_row(&mut grid, 0, "hello world here!!");

    grid.resize(20, 10, true);

    assert_eq!(grid.cols(), 10);

    // 18 chars wraps to 2 rows at 10 cols. Trailing blank rows are trimmed,
    // so both rows stay visible — no scrollback.
    assert_eq!(grid.scrollback().len(), 0);

    // Row 0 has the first 10 chars with WRAP flag.
    assert_eq!(read_row(&grid, 0), "hello worl");
    assert!(grid[Line(0)][Column(9)].flags.contains(CellFlags::WRAP));

    // Row 1 has the remaining 8 chars.
    assert_eq!(read_row(&grid, 1), "d here!!");
}

#[test]
fn reflow_shrink_preserves_cursor_within_bounds() {
    let mut grid = Grid::new(20, 20);
    write_row(&mut grid, 0, "hello world");
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(6)); // On 'w'.

    grid.resize(20, 5, true);

    // "hello world" (11 chars) at 5 cols wraps to 3 rows. Trailing blank
    // rows are trimmed, so all content stays visible — no scrollback.
    assert!(grid.cursor().line() < grid.lines());
    assert!(grid.cursor().col().0 < grid.cols());
    assert_eq!(grid.scrollback().len(), 0);

    // All 3 rows of wrapped content are visible.
    assert_eq!(read_row(&grid, 0), "hello");
    assert_eq!(read_row(&grid, 1), " worl");
    assert_eq!(read_row(&grid, 2), "d");
}

// ── Reflow: round-trip ──────────────────────────────────────────────

#[test]
fn reflow_shrink_then_grow_preserves_content() {
    // Use enough lines so wrapped content stays visible during shrink.
    let mut grid = Grid::new(10, 20);
    write_row(&mut grid, 0, "hello world here!!");
    write_row(&mut grid, 1, "second line");

    // Shrink to 10 cols (wrap).
    grid.resize(10, 10, true);
    // Grow back to 20 cols (unwrap).
    grid.resize(10, 20, true);

    assert_eq!(read_row(&grid, 0), "hello world here!!");
    assert_eq!(read_row(&grid, 1), "second line");
}

// ── Reflow: empty grid ──────────────────────────────────────────────

#[test]
fn reflow_empty_grid_produces_valid_state() {
    let mut grid = Grid::new(3, 10);
    grid.resize(3, 20, true);

    assert_eq!(grid.cols(), 20);
    assert_eq!(grid.lines(), 3);
    assert!(grid[Line(0)][Column(0)].is_empty());
}

// ── Reflow: wide characters ─────────────────────────────────────────

#[test]
fn reflow_wide_char_at_boundary_wraps_correctly() {
    let mut grid = Grid::new(10, 10);

    // Write "abcd" then a wide CJK char at cols 4-5.
    for (col, ch) in "abcd".chars().enumerate() {
        grid[Line(0)][Column(col)] = cell(ch);
    }
    let mut wide = cell('\u{4e16}'); // CJK char (width=2).
    wide.flags.insert(CellFlags::WIDE_CHAR);
    grid[Line(0)][Column(4)] = wide;
    let mut spacer = Cell::default();
    spacer.flags.insert(CellFlags::WIDE_CHAR_SPACER);
    grid[Line(0)][Column(5)] = spacer;

    // Shrink to 5 cols: wide char at cols 4-5 can't fit (only col 4 left).
    grid.resize(10, 5, true);

    assert_eq!(grid.cols(), 5);
    // Trailing blanks trimmed, so both rows stay visible — no scrollback.
    assert_eq!(grid.scrollback().len(), 0);

    // Row 0 has "abcd" + leading spacer with WRAP.
    let r0: String = (0..4).map(|c| grid[Line(0)][Column(c)].ch).collect();
    assert_eq!(r0, "abcd");

    // Row 1 has the wide char.
    assert!(
        grid[Line(1)][Column(0)]
            .flags
            .contains(CellFlags::WIDE_CHAR)
    );
    assert!(
        grid[Line(1)][Column(1)]
            .flags
            .contains(CellFlags::WIDE_CHAR_SPACER)
    );
}

#[test]
fn reflow_wide_char_boundary_sets_leading_spacer() {
    let mut grid = Grid::new(10, 10);

    // "abcd" (4 cols) + wide CJK (2 cols) = 6 cols.
    for (col, ch) in "abcd".chars().enumerate() {
        grid[Line(0)][Column(col)] = cell(ch);
    }
    let mut wide = cell('\u{4e16}');
    wide.flags.insert(CellFlags::WIDE_CHAR);
    grid[Line(0)][Column(4)] = wide;
    let mut spacer = Cell::default();
    spacer.flags.insert(CellFlags::WIDE_CHAR_SPACER);
    grid[Line(0)][Column(5)] = spacer;

    // Shrink to 5 cols: wide char can't fit at col 4 (needs 2 cells).
    // Cell at col 4 should become LEADING_WIDE_CHAR_SPACER.
    grid.resize(10, 5, true);

    // Trailing blanks trimmed — row stays visible.
    assert_eq!(grid.scrollback().len(), 0);
    assert!(
        grid[Line(0)][Column(4)]
            .flags
            .contains(CellFlags::LEADING_WIDE_CHAR_SPACER),
        "boundary cell should be LEADING_WIDE_CHAR_SPACER"
    );
    assert!(
        grid[Line(0)][Column(4)].flags.contains(CellFlags::WRAP),
        "boundary cell should also have WRAP"
    );
}

#[test]
fn reflow_wide_char_round_trip_preserves_content() {
    let mut grid = Grid::new(10, 10);

    // "abcd" + wide CJK char at cols 4-5.
    for (col, ch) in "abcd".chars().enumerate() {
        grid[Line(0)][Column(col)] = cell(ch);
    }
    let mut wide = cell('\u{4e16}');
    wide.flags.insert(CellFlags::WIDE_CHAR);
    grid[Line(0)][Column(4)] = wide;
    let mut spacer = Cell::default();
    spacer.flags.insert(CellFlags::WIDE_CHAR_SPACER);
    grid[Line(0)][Column(5)] = spacer;

    // Shrink to 5 cols, then grow back to 10 cols.
    grid.resize(10, 5, true);
    grid.resize(10, 10, true);

    // Content should be preserved without spurious spaces.
    let r: String = (0..6).map(|c| grid[Line(0)][Column(c)].ch).collect();
    assert_eq!(r, "abcd\u{4e16} ");
    assert!(
        grid[Line(0)][Column(4)]
            .flags
            .contains(CellFlags::WIDE_CHAR)
    );
    assert!(
        grid[Line(0)][Column(5)]
            .flags
            .contains(CellFlags::WIDE_CHAR_SPACER)
    );
}

#[test]
fn reflow_leading_spacer_skipped_during_reflow() {
    let mut grid = Grid::new(10, 6);

    // "abc" + wide CJK at cols 3-4 in a 6-col grid.
    for (col, ch) in "abc".chars().enumerate() {
        grid[Line(0)][Column(col)] = cell(ch);
    }
    let mut wide = cell('\u{4e16}');
    wide.flags.insert(CellFlags::WIDE_CHAR);
    grid[Line(0)][Column(3)] = wide;
    let mut spacer = Cell::default();
    spacer.flags.insert(CellFlags::WIDE_CHAR_SPACER);
    grid[Line(0)][Column(4)] = spacer;

    // Shrink to 4 cols: wide char at col 3 can't fit (needs col 3+4, only 4 available).
    grid.resize(10, 4, true);

    // Trailing blanks trimmed — row stays visible.
    assert_eq!(grid.scrollback().len(), 0);
    // Boundary cell at col 3 should be LEADING_WIDE_CHAR_SPACER.
    assert!(
        grid[Line(0)][Column(3)]
            .flags
            .contains(CellFlags::LEADING_WIDE_CHAR_SPACER)
    );

    // Grow back: leading spacer should be skipped, no extra space.
    grid.resize(10, 6, true);
    let r: String = (0..5).map(|c| grid[Line(0)][Column(c)].ch).collect();
    assert_eq!(r, "abc\u{4e16} ");
}

// ── Combined row + col resize ───────────────────────────────────────

#[test]
fn resize_both_dimensions_simultaneously() {
    let mut grid = Grid::new(10, 80);
    write_row(&mut grid, 0, "hello");
    write_row(&mut grid, 1, "world");
    grid.cursor_mut().set_line(1);

    grid.resize(5, 40, false);

    assert_eq!(grid.cols(), 40);
    assert_eq!(grid.lines(), 5);
    assert_eq!(read_row(&grid, 0), "hello");
    assert_eq!(read_row(&grid, 1), "world");
}

// ── Rapid resize sequences ──────────────────────────────────────────

#[test]
fn rapid_resize_sequence_does_not_panic() {
    let mut grid = Grid::new(24, 80);
    write_row(&mut grid, 0, "hello world");
    grid.cursor_mut().set_line(5);

    // Simulate rapid resize events.
    grid.resize(12, 40, true);
    grid.resize(30, 120, true);
    grid.resize(24, 80, true);
    grid.resize(5, 10, true);
    grid.resize(24, 80, true);

    assert_eq!(grid.cols(), 80);
    assert_eq!(grid.lines(), 24);
    // Grid survives without panicking. Content may move to scrollback
    // during shrink and not be restored during grow (blank rows inserted
    // instead to prevent stale-content ghosting).
}

#[test]
fn resize_to_minimum_1x1() {
    let mut grid = Grid::new(24, 80);
    write_row(&mut grid, 0, "hello");

    grid.resize(1, 1, true);

    assert_eq!(grid.cols(), 1);
    assert_eq!(grid.lines(), 1);
    assert_eq!(grid.cursor().line(), 0);
    assert_eq!(grid.cursor().col(), Column(0));
}

// ── Sparse content reflow ────────────────────────────────────────────

#[test]
fn reflow_sparse_cells_preserves_interior_blanks() {
    // "a  b  c" with interior spaces — reflow must not collapse them.
    let mut grid = Grid::new(10, 10);
    grid[Line(0)][Column(0)] = cell('a');
    grid[Line(0)][Column(3)] = cell('b');
    grid[Line(0)][Column(6)] = cell('c');

    // Shrink to 4 cols: wraps at col 4.
    grid.resize(10, 4, true);
    // Grow back: should recover exact positions.
    grid.resize(10, 10, true);

    assert_eq!(grid[Line(0)][Column(0)].ch, 'a');
    assert_eq!(grid[Line(0)][Column(3)].ch, 'b');
    assert_eq!(grid[Line(0)][Column(6)].ch, 'c');
}

// ── Multi-line wide char unwrap ─────────────────────────────────────

#[test]
fn reflow_multiline_wide_spacer_head_unwrap() {
    // 3-line scenario: "abcde" wrapped at 3 cols with a wide char that
    // splits across line 2→3. Growing should reconstruct all content.
    let mut grid = Grid::new(10, 6);

    // Row 0: "ab" + wide char at cols 2-3 = 4 display cols.
    grid[Line(0)][Column(0)] = cell('a');
    grid[Line(0)][Column(1)] = cell('b');
    let mut w1 = cell('\u{4e16}');
    w1.flags.insert(CellFlags::WIDE_CHAR);
    grid[Line(0)][Column(2)] = w1;
    let mut s1 = Cell::default();
    s1.flags.insert(CellFlags::WIDE_CHAR_SPACER);
    grid[Line(0)][Column(3)] = s1;
    // Row 1: another wide char at cols 0-1 + "cd".
    let mut w2 = cell('\u{4e16}');
    w2.flags.insert(CellFlags::WIDE_CHAR);
    grid[Line(1)][Column(0)] = w2;
    let mut s2 = Cell::default();
    s2.flags.insert(CellFlags::WIDE_CHAR_SPACER);
    grid[Line(1)][Column(1)] = s2;
    grid[Line(1)][Column(2)] = cell('c');
    grid[Line(1)][Column(3)] = cell('d');

    // Set WRAP to form a logical line.
    grid[Line(0)][Column(5)].flags.insert(CellFlags::WRAP);

    // Shrink to 3 cols, then grow back to 6.
    grid.resize(10, 3, true);
    grid.resize(10, 6, true);

    // Content should survive: "ab" + wide + wide + "cd".
    assert_eq!(grid[Line(0)][Column(0)].ch, 'a');
    assert_eq!(grid[Line(0)][Column(1)].ch, 'b');
    assert!(
        grid[Line(0)][Column(2)]
            .flags
            .contains(CellFlags::WIDE_CHAR)
    );
}

// ── Cursor tracking across multi-step reflows ───────────────────────

#[test]
fn cursor_tracks_through_narrow_grow_narrow_grow() {
    let mut grid = Grid::new(10, 20);
    write_row(&mut grid, 0, "hello world!!");
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(5)); // On ' '.

    // Narrow → grow → narrow → grow.
    grid.resize(10, 5, true);
    grid.resize(10, 20, true);
    grid.resize(10, 7, true);
    grid.resize(10, 20, true);

    // Cursor should remain within bounds.
    assert!(grid.cursor().line() < grid.lines());
    assert!(grid.cursor().col().0 < grid.cols());

    // Content should survive.
    assert_eq!(read_row(&grid, 0), "hello world!!");
}

#[test]
fn cursor_on_wide_char_tracks_through_reflow() {
    let mut grid = Grid::new(10, 10);
    // "ab" + wide char at cols 2-3.
    grid[Line(0)][Column(0)] = cell('a');
    grid[Line(0)][Column(1)] = cell('b');
    let mut wide = cell('\u{4e16}');
    wide.flags.insert(CellFlags::WIDE_CHAR);
    grid[Line(0)][Column(2)] = wide;
    let mut spacer = Cell::default();
    spacer.flags.insert(CellFlags::WIDE_CHAR_SPACER);
    grid[Line(0)][Column(3)] = spacer;

    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(2)); // On the wide char.

    // Shrink to 3 cols: wide char wraps.
    grid.resize(10, 3, true);
    // Grow back.
    grid.resize(10, 10, true);

    // Cursor should be on or near the wide char.
    assert!(grid.cursor().line() < grid.lines());
    assert!(grid.cursor().col().0 < grid.cols());
}

// ── Exact-fit boundary ──────────────────────────────────────────────

#[test]
fn reflow_content_fits_exactly_in_new_width() {
    let mut grid = Grid::new(10, 10);
    // 10 chars fills the row exactly.
    write_row(&mut grid, 0, "abcdefghij");
    grid[Line(0)][Column(9)].flags.insert(CellFlags::WRAP);
    write_row(&mut grid, 1, "klmno");

    // Grow to 15: "abcdefghij" + "klmno" = 15 chars, fits exactly.
    grid.resize(10, 15, true);

    assert_eq!(read_row(&grid, 0), "abcdefghijklmno");
    // No WRAP should remain since content fits the width exactly.
    assert!(!grid[Line(0)][Column(14)].flags.contains(CellFlags::WRAP));
}

#[test]
fn reflow_shrink_to_exact_content_length() {
    let mut grid = Grid::new(10, 20);
    write_row(&mut grid, 0, "hello");

    // Shrink cols to exactly match content length (5).
    grid.resize(10, 5, true);

    assert_eq!(grid.cols(), 5);
    assert_eq!(read_row(&grid, 0), "hello");
    // Content fits exactly — no wrapping should occur.
    assert!(!grid[Line(0)][Column(4)].flags.contains(CellFlags::WRAP));
    assert_eq!(grid.scrollback().len(), 0);
}

// ── Wide char multi-size round-trip ─────────────────────────────────

#[test]
fn wide_char_survives_multiple_intermediate_sizes() {
    let mut grid = Grid::new(10, 10);
    // "a" + wide at cols 1-2.
    grid[Line(0)][Column(0)] = cell('a');
    let mut wide = cell('\u{4e16}');
    wide.flags.insert(CellFlags::WIDE_CHAR);
    grid[Line(0)][Column(1)] = wide;
    let mut spacer = Cell::default();
    spacer.flags.insert(CellFlags::WIDE_CHAR_SPACER);
    grid[Line(0)][Column(2)] = spacer;

    // Cycle through multiple sizes.
    grid.resize(10, 2, true); // Wide char wraps.
    grid.resize(10, 5, true); // Unwrap.
    grid.resize(10, 3, true); // Wrap again.
    grid.resize(10, 10, true); // Back to original.

    assert_eq!(grid[Line(0)][Column(0)].ch, 'a');
    assert!(
        grid[Line(0)][Column(1)]
            .flags
            .contains(CellFlags::WIDE_CHAR)
    );
    assert!(
        grid[Line(0)][Column(2)]
            .flags
            .contains(CellFlags::WIDE_CHAR_SPACER)
    );
}

// ── Attribute preservation ──────────────────────────────────────────

#[test]
fn reflow_preserves_cell_attributes() {
    use vte::ansi::Color;

    let mut grid = Grid::new(10, 10);
    let mut c = cell('X');
    c.flags = CellFlags::BOLD | CellFlags::ITALIC;
    c.fg = Color::Indexed(1); // red
    grid[Line(0)][Column(0)] = c;
    write_row(&mut grid, 0, "Xbcdefghij");
    // Restore the styled cell after write_row.
    let mut styled = cell('X');
    styled.flags = CellFlags::BOLD | CellFlags::ITALIC;
    styled.fg = Color::Indexed(1);
    grid[Line(0)][Column(0)] = styled;
    grid[Line(0)][Column(9)].flags.insert(CellFlags::WRAP);
    write_row(&mut grid, 1, "klmno");

    // Shrink to 5, then grow back.
    grid.resize(10, 5, true);
    grid.resize(10, 10, true);

    // The styled cell should retain its attributes.
    let recovered = &grid[Line(0)][Column(0)];
    assert!(recovered.flags.contains(CellFlags::BOLD));
    assert!(recovered.flags.contains(CellFlags::ITALIC));
    assert_eq!(recovered.fg, Color::Indexed(1));
}

// ── Scrollback overflow during reflow ───────────────────────────────

#[test]
fn reflow_scrollback_overflow_evicts_oldest() {
    // Small scrollback capacity. Wrapping should evict oldest rows.
    let mut grid = Grid::with_scrollback(5, 10, 3);
    for i in 0..5 {
        write_row(&mut grid, i, &format!("line{i}____")); // Fill 10 cols.
    }
    grid.cursor_mut().set_line(4);

    // Shrink to 5 cols: each 10-char row wraps into 2 rows.
    // 5 rows × 2 = 10 rows. 5 visible, 5 to scrollback.
    // But scrollback capacity is only 3, so oldest 2 are evicted.
    grid.resize(5, 5, true);

    assert!(grid.scrollback().len() <= 3);
    // Grid should still be valid.
    assert_eq!(grid.lines(), 5);
    assert_eq!(grid.cols(), 5);
}

// ── Saved cursor tracking ───────────────────────────────────────────

#[test]
fn resize_clamps_saved_cursor() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(20);
    grid.cursor_mut().set_col(Column(70));
    grid.save_cursor();

    // Shrink well below saved cursor position.
    grid.resize(10, 40, false);

    // Restore and verify it was clamped.
    grid.restore_cursor();
    assert!(grid.cursor().line() < grid.lines());
    assert!(grid.cursor().col().0 < grid.cols());
}

// ── Display offset edge cases ───────────────────────────────────────

#[test]
fn resize_clamps_display_offset_when_scrollback_shrinks() {
    let mut grid = Grid::with_scrollback(5, 10, 100);
    // Fill grid and push content to scrollback.
    for i in 0..15 {
        write_row(&mut grid, 0, &format!("line{i:02}___"));
        grid.scroll_up(1);
    }
    grid.cursor_mut().set_line(4);

    // Scroll back into history.
    let sb_len = grid.scrollback().len();
    grid.scroll_display(sb_len as isize);
    assert_eq!(grid.display_offset(), sb_len);

    // Grow: pulls from scrollback, reducing its length.
    grid.resize(8, 10, false);

    // Display offset must be clamped to new scrollback length.
    assert!(grid.display_offset() <= grid.scrollback().len());
}

#[test]
fn resize_display_offset_zero_stays_zero() {
    let mut grid = Grid::with_scrollback(5, 10, 100);
    write_row(&mut grid, 0, "hello");
    // display_offset starts at 0 (live view).
    assert_eq!(grid.display_offset(), 0);

    grid.resize(10, 20, true);

    assert_eq!(grid.display_offset(), 0);
}

// ── Reflow with only wide chars ─────────────────────────────────────

#[test]
fn reflow_grid_of_only_wide_chars() {
    let mut grid = Grid::new(10, 6);
    // Fill row 0 with 3 wide chars (6 display cols).
    for i in 0..3 {
        let col = i * 2;
        let mut wide = cell('\u{4e16}');
        wide.flags.insert(CellFlags::WIDE_CHAR);
        grid[Line(0)][Column(col)] = wide;
        let mut spacer = Cell::default();
        spacer.flags.insert(CellFlags::WIDE_CHAR_SPACER);
        grid[Line(0)][Column(col + 1)] = spacer;
    }

    // Shrink to 4 cols: 3rd wide char wraps.
    grid.resize(10, 4, true);
    assert_eq!(grid.cols(), 4);

    // Grow back to 6.
    grid.resize(10, 6, true);

    // All 3 wide chars should survive.
    for i in 0..3 {
        let col = i * 2;
        assert!(
            grid[Line(0)][Column(col)]
                .flags
                .contains(CellFlags::WIDE_CHAR),
            "wide char {i} missing at col {col}"
        );
        assert!(
            grid[Line(0)][Column(col + 1)]
                .flags
                .contains(CellFlags::WIDE_CHAR_SPACER),
            "spacer for wide char {i} missing at col {}",
            col + 1
        );
    }
}

// ── Mixed wide + narrow across scrollback boundary ──────────────────

#[test]
fn reflow_mixed_wide_narrow_across_scrollback() {
    let mut grid = Grid::new(5, 10);
    // Row 0: "ab" + wide + "c" = 5 display cols.
    grid[Line(0)][Column(0)] = cell('a');
    grid[Line(0)][Column(1)] = cell('b');
    let mut wide = cell('\u{4e16}');
    wide.flags.insert(CellFlags::WIDE_CHAR);
    grid[Line(0)][Column(2)] = wide;
    let mut spacer = Cell::default();
    spacer.flags.insert(CellFlags::WIDE_CHAR_SPACER);
    grid[Line(0)][Column(3)] = spacer;
    grid[Line(0)][Column(4)] = cell('c');
    // Row 1-4: other content.
    for i in 1..5 {
        write_row(&mut grid, i, &format!("row{i}______"));
    }
    grid.cursor_mut().set_line(4);

    // Shrink to 3 cols: forces wrapping + scrollback interaction.
    grid.resize(5, 3, true);
    // Grow back.
    grid.resize(5, 10, true);

    // First row content should be recoverable.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'a');
    assert_eq!(grid[Line(0)][Column(1)].ch, 'b');
}

// ── Wrapped scrollback ring + resize ─────────────────────────────────

#[test]
fn reflow_with_wrapped_scrollback_ring() {
    // Small scrollback that wraps before resize.
    let mut grid = Grid::with_scrollback(5, 10, 3);
    // Fill visible area.
    for i in 0..5 {
        write_row(&mut grid, i, &format!("line{i}_____"));
    }
    grid.cursor_mut().set_line(4);

    // Scroll to push rows into scrollback until it wraps.
    grid.scroll_up(1);
    write_row(&mut grid, 4, "line5_____");
    grid.scroll_up(1);
    write_row(&mut grid, 4, "line6_____");
    grid.scroll_up(1);
    write_row(&mut grid, 4, "line7_____");
    grid.scroll_up(1);
    write_row(&mut grid, 4, "line8_____");

    // Scrollback should be full (3 rows), with oldest rows evicted.
    assert_eq!(grid.scrollback().len(), 3);

    // Now resize cols with reflow — exercises drain_oldest_first on wrapped ring.
    grid.resize(5, 20, true);

    // Grid should be valid.
    assert_eq!(grid.cols(), 20);
    assert_eq!(grid.lines(), 5);
    assert!(grid.cursor().line() < grid.lines());
}

#[test]
fn reflow_shrink_with_wrapped_scrollback_ring() {
    let mut grid = Grid::with_scrollback(5, 20, 3);
    for i in 0..5 {
        write_row(&mut grid, i, &format!("line{i}_______________"));
    }
    grid.cursor_mut().set_line(4);

    // Push rows to scrollback until it wraps.
    for _ in 0..5 {
        grid.scroll_up(1);
    }
    assert_eq!(grid.scrollback().len(), 3);

    // Shrink cols with reflow.
    grid.resize(5, 10, true);

    assert_eq!(grid.cols(), 10);
    assert_eq!(grid.lines(), 5);
    assert!(grid.cursor().line() < grid.lines());
}

#[test]
fn reflow_round_trip_with_wrapped_scrollback() {
    let mut grid = Grid::with_scrollback(3, 10, 3);
    write_row(&mut grid, 0, "AAAAAAAAAA");
    write_row(&mut grid, 1, "BBBBBBBBBB");
    write_row(&mut grid, 2, "CCCCCCCCCC");
    grid.cursor_mut().set_line(2);

    // Push some rows to scrollback so the ring wraps.
    grid.scroll_up(1);
    write_row(&mut grid, 2, "DDDDDDDDDD");
    grid.scroll_up(1);
    write_row(&mut grid, 2, "EEEEEEEEEE");
    grid.scroll_up(1);
    write_row(&mut grid, 2, "FFFFFFFFFF");
    grid.scroll_up(1);
    write_row(&mut grid, 2, "GGGGGGGGGG");

    assert_eq!(grid.scrollback().len(), 3);

    // Shrink then grow — content should survive.
    grid.resize(3, 5, true);
    grid.resize(3, 10, true);

    assert_eq!(grid.cols(), 10);
    assert_eq!(grid.lines(), 3);
}

// ── put_char-produced content (natural WRAP flags) ──────────────────

/// Helper: write a string via put_char (produces natural WRAP flags).
fn put_str(grid: &mut Grid, text: &str) {
    for ch in text.chars() {
        grid.put_char(ch);
    }
}

/// Helper: read all visible rows as text lines.
fn read_all_rows(grid: &Grid) -> Vec<String> {
    (0..grid.lines()).map(|i| read_row(grid, i)).collect()
}

#[test]
fn put_char_sets_wrap_flag_on_overflow() {
    let mut grid = Grid::new(5, 10);
    // Write 15 chars into a 10-col grid. First 10 fill row 0,
    // 11th char triggers WRAP on row 0 + linefeed, then writes continue.
    put_str(&mut grid, "abcdefghijklmno");

    // Row 0 last cell should have WRAP flag.
    assert!(
        grid[Line(0)][Column(9)].flags.contains(CellFlags::WRAP),
        "put_char should set WRAP on overflow"
    );
    assert_eq!(read_row(&grid, 0), "abcdefghij");
    assert_eq!(read_row(&grid, 1), "klmno");
}

#[test]
fn put_char_content_survives_grow_reflow() {
    let mut grid = Grid::new(5, 10);
    put_str(&mut grid, "abcdefghijklmno");

    // Grow to 20 cols: wrapped line should unwrap.
    grid.resize(5, 20, true);

    assert_eq!(read_row(&grid, 0), "abcdefghijklmno");
}

#[test]
fn put_char_content_survives_shrink_reflow() {
    let mut grid = Grid::new(5, 20);
    put_str(&mut grid, "abcdefghijklmno");

    // Shrink to 10 cols: should wrap at col 10.
    grid.resize(5, 10, true);

    assert_eq!(read_row(&grid, 0), "abcdefghij");
    assert_eq!(read_row(&grid, 1), "klmno");
    assert!(grid[Line(0)][Column(9)].flags.contains(CellFlags::WRAP));
}

#[test]
fn put_char_content_round_trip_shrink_grow() {
    let mut grid = Grid::new(5, 20);
    put_str(&mut grid, "hello world here!");

    // Shrink then grow back.
    grid.resize(5, 10, true);
    grid.resize(5, 20, true);

    assert_eq!(read_row(&grid, 0), "hello world here!");
}

#[test]
fn put_char_multiline_content_round_trip() {
    let mut grid = Grid::new(10, 10);
    // Write first line, then move to next line manually.
    put_str(&mut grid, "hello");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "world");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "foo bar!!!");

    // Shrink then grow.
    grid.resize(10, 5, true);
    grid.resize(10, 10, true);

    assert_eq!(read_row(&grid, 0), "hello");
    assert_eq!(read_row(&grid, 1), "world");
    assert_eq!(read_row(&grid, 2), "foo bar!!!");
}

#[test]
fn put_char_wrapped_content_with_scrollback_round_trip() {
    // Small grid: 3 lines × 10 cols.
    let mut grid = Grid::new(3, 10);
    // Write 25 chars: fills row 0 (10) + row 1 (10) + row 2 (5).
    // Wraps at row boundaries, cursor ends at row 2.
    put_str(&mut grid, "abcdefghijklmnopqrstuvwxy");

    assert_eq!(read_row(&grid, 0), "abcdefghij");
    assert_eq!(read_row(&grid, 1), "klmnopqrst");
    assert_eq!(read_row(&grid, 2), "uvwxy");
    assert!(grid[Line(0)][Column(9)].flags.contains(CellFlags::WRAP));
    assert!(grid[Line(1)][Column(9)].flags.contains(CellFlags::WRAP));

    // Shrink to 5 cols: content wraps more, excess goes to scrollback.
    grid.resize(3, 5, true);

    assert_eq!(grid.cols(), 5);
    // Content is split into 5-char chunks (5 rows total).
    // With 3 visible lines, 2 must be in scrollback.
    assert_eq!(grid.scrollback().len(), 2);

    // Grow back to 10 cols: should unwrap.
    grid.resize(3, 10, true);

    // All content should be recoverable across scrollback + visible.
    let sb_len = grid.scrollback().len();
    let mut all_text = String::new();
    for i in 0..sb_len {
        let row = grid.scrollback().get(sb_len - 1 - i).unwrap();
        let text: String = (0..row.cols()).map(|c| row[Column(c)].ch).collect();
        all_text.push_str(text.trim_end());
    }
    for i in 0..grid.lines() {
        all_text.push_str(&read_row(&grid, i));
    }
    assert_eq!(all_text, "abcdefghijklmnopqrstuvwxy");
}

#[test]
fn put_char_fills_grid_then_scrolls_then_resizes() {
    // Simulate a realistic scenario: terminal fills up, scrolls, then resizes.
    let mut grid = Grid::with_scrollback(5, 10, 100);

    // Write 8 lines of 10 chars each. Grid has 5 lines, so 3 scroll off.
    for i in 0..8 {
        let s: String = std::iter::repeat(char::from(b'A' + i as u8))
            .take(10)
            .collect();
        if i > 0 {
            grid.linefeed();
            grid.cursor_mut().set_col(Column(0));
        }
        put_str(&mut grid, &s);
    }

    // 3 lines should be in scrollback (lines 0-2 scrolled off).
    assert_eq!(grid.scrollback().len(), 3);

    // Visible: lines 3-7 (DDDD, EEEE, FFFF, GGGG, HHHH).
    assert_eq!(read_row(&grid, 0), "DDDDDDDDDD");
    assert_eq!(read_row(&grid, 4), "HHHHHHHHHH");

    // Shrink cols to 5: each 10-char row wraps into 2 rows.
    grid.resize(5, 5, true);
    assert_eq!(grid.cols(), 5);

    // Grow back to 10: should unwrap.
    grid.resize(5, 10, true);

    // Verify visible content is intact.
    let visible: Vec<String> = read_all_rows(&grid);
    for row_text in &visible {
        if !row_text.is_empty() {
            // Each non-empty row should be 10 identical chars.
            let first = row_text.chars().next().unwrap();
            assert!(
                row_text.chars().all(|c| c == first),
                "row content corrupted: {row_text:?}"
            );
        }
    }
}

#[test]
fn put_char_mixed_hard_soft_wraps_resize() {
    let mut grid = Grid::new(10, 10);
    // Line 1: 15 chars (soft wraps at col 10).
    put_str(&mut grid, "abcdefghijklmno");
    // Line 2: hard newline + short content.
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "xyz");

    // Row 0: "abcdefghij" with WRAP.
    // Row 1: "klmno" (continuation), no WRAP.
    // Row 2: "xyz" (hard newline), no WRAP.
    assert!(grid[Line(0)][Column(9)].flags.contains(CellFlags::WRAP));
    assert!(!grid[Line(1)][Column(9)].flags.contains(CellFlags::WRAP));

    // Grow to 20: soft-wrapped line unwraps, hard newline stays.
    grid.resize(10, 20, true);

    assert_eq!(read_row(&grid, 0), "abcdefghijklmno");
    assert_eq!(read_row(&grid, 1), "xyz");
}

#[test]
fn put_char_both_dimensions_change() {
    let mut grid = Grid::new(10, 10);
    put_str(&mut grid, "abcdefghijklmno");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "12345");

    // Change both rows and cols simultaneously.
    grid.resize(5, 20, true);

    assert_eq!(grid.lines(), 5);
    assert_eq!(grid.cols(), 20);
    // Soft-wrapped content should unwrap.
    assert_eq!(read_row(&grid, 0), "abcdefghijklmno");
    assert_eq!(read_row(&grid, 1), "12345");
}

#[test]
fn put_char_shrink_both_dimensions() {
    let mut grid = Grid::new(10, 20);
    put_str(&mut grid, "abcdefghijklmno");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "hello world");
    grid.cursor_mut().set_line(9); // Move cursor to bottom.

    // Shrink both dimensions.
    grid.resize(5, 10, true);

    assert_eq!(grid.lines(), 5);
    assert_eq!(grid.cols(), 10);
    // Content should be preserved (possibly with scrollback).
    let mut all_text = String::new();
    let sb_len = grid.scrollback().len();
    for i in 0..sb_len {
        let row = grid.scrollback().get(sb_len - 1 - i).unwrap();
        let text: String = (0..row.cols()).map(|c| row[Column(c)].ch).collect();
        all_text.push_str(text.trim_end());
    }
    for i in 0..grid.lines() {
        all_text.push_str(&read_row(&grid, i));
    }
    assert!(
        all_text.contains("abcdefghijklmno"),
        "content lost: {all_text:?}"
    );
    assert!(
        all_text.contains("hello world"),
        "content lost: {all_text:?}"
    );
}

// ── Trailing blank rows during reflow (root cause) ──────────────────

#[test]
fn reflow_shrink_trims_trailing_blanks_before_scrollback() {
    // Root cause test: wrapping content + trailing blank rows should not
    // push real content into scrollback. The reflow should trim trailing
    // blanks from the result before distributing.
    let mut grid = Grid::new(5, 20);
    write_row(&mut grid, 0, "hello world here!!");
    // Rows 1-4 are blank. Cursor at (0,0).

    // Shrink to 10 cols: "hello world here!!" (18 chars) wraps to 2 rows.
    // Reflow produces: 2 content rows + 4 blank rows = 6. With 5 visible
    // lines, the naive code pushes row 0 to scrollback. But the trailing
    // blanks should be trimmed, keeping all content visible.
    grid.resize(5, 10, true);

    assert_eq!(
        grid.scrollback().len(),
        0,
        "trailing blanks should be trimmed, not cause scrollback"
    );
    assert_eq!(read_row(&grid, 0), "hello worl");
    assert_eq!(read_row(&grid, 1), "d here!!");
}

#[test]
fn reflow_shrink_trailing_blanks_write_row_small_grid() {
    // Same bug pattern with write_row: 3-line grid, 1 line of content,
    // 2 trailing blanks. Wrapping to 5 cols → 4 content rows + 2 blanks = 6.
    // 6 > 3, so 3 go to scrollback. But only 1 row of trailing blanks
    // should remain; content should not be in scrollback.
    let mut grid = Grid::new(3, 20);
    write_row(&mut grid, 0, "abcdefghijklmnopqrst"); // exactly 20 chars

    grid.resize(3, 5, true);

    // 20 chars at 5 cols = 4 rows. Grid has 3 visible.
    // 1 row MUST go to scrollback (legitimate overflow).
    // But trailing blanks from old rows 1-2 should NOT add more.
    assert_eq!(
        grid.scrollback().len(),
        1,
        "only 1 row to scrollback, trailing blanks trimmed"
    );
}

#[test]
fn reflow_shrink_with_cursor_at_content_end() {
    // Realistic: cursor is just past written content. Trailing rows are blank.
    let mut grid = Grid::new(10, 20);
    put_str(&mut grid, "line one content!");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "line two");
    // Cursor now at line 1, col 8. Rows 2-9 are blank.

    grid.resize(10, 10, true);

    // "line one content!" = 17 chars → 2 rows at 10 cols.
    // "line two" = 8 chars → 1 row at 10 cols.
    // Total real content = 3 rows. 7 trailing blank rows should be trimmed.
    // No content should go to scrollback.
    assert_eq!(
        grid.scrollback().len(),
        0,
        "trailing blanks should not push content to scrollback"
    );
    assert_eq!(read_row(&grid, 0), "line one c");
    assert_eq!(read_row(&grid, 1), "ontent!");
    assert_eq!(read_row(&grid, 2), "line two");
}

#[test]
fn reflow_complex_terminal_output_then_resize() {
    // Simulate complex terminal output like launching a CLI tool.
    // Multiple lines of varying length, some wrapping.
    let mut grid = Grid::with_scrollback(24, 80, 1000);

    let lines = [
        "~ Welcome to ori_term v0.1.0",
        "",
        "  Type 'help' for commands, 'exit' to quit.",
        "",
        "user@host:~/projects/ori_term$ cargo build --target x86_64-pc-windows-gnu --release",
        "   Compiling oriterm_core v0.1.0 (/home/user/projects/ori_term/oriterm_core)",
        "   Compiling oriterm v0.1.0 (/home/user/projects/ori_term/oriterm)",
        "    Finished release [optimized] target(s) in 12.34s",
        "user@host:~/projects/ori_term$ ",
    ];

    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            grid.linefeed();
            grid.cursor_mut().set_col(Column(0));
        }
        put_str(&mut grid, line);
    }

    // Record content before resize.
    let before: Vec<String> = read_all_rows(&grid);

    // Shrink to 40 cols (common half-screen width).
    grid.resize(24, 40, true);

    // Grow back to 80 cols.
    grid.resize(24, 80, true);

    // Content should be identical after round-trip resize.
    let after: Vec<String> = read_all_rows(&grid);
    assert_eq!(
        before, after,
        "round-trip resize changed content\nbefore: {before:?}\nafter:  {after:?}"
    );
}

// ── Dirty tracking ──────────────────────────────────────────────────

#[test]
fn resize_marks_all_dirty() {
    let mut grid = Grid::new(10, 80);
    // Drain dirty state.
    grid.dirty_mut().drain().for_each(drop);

    grid.resize(5, 40, false);

    assert!(grid.dirty().is_all_dirty());
}

// ── Snapshot tests (insta) ──────────────────────────────────────────

// Basic reflow operations.

#[test]
fn snapshot_reflow_shrink_wraps() {
    let mut grid = Grid::new(5, 10);
    put_str(&mut grid, "HelloWorld");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "abcde");

    grid.resize(5, 6, true);

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 5x6 cursor=(2,5)]
    |HelloW+
    |orld  |
    |abcde |
    |      |
    |      |
    ");
}

#[test]
fn snapshot_reflow_grow_unwraps() {
    let mut grid = Grid::new(5, 6);
    put_str(&mut grid, "HelloWorld!!");

    grid.resize(5, 12, true);

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 5x12 cursor=(0,6)]
    |HelloWorld!!|
    |            |
    |            |
    |            |
    |            |
    ");
}

#[test]
fn snapshot_shrink_pushes_to_scrollback() {
    let mut grid = Grid::new(3, 10);
    put_str(&mut grid, "AAAAAAAAAA");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "BBBBBBBBBB");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "CCCCCCCCCC");

    grid.resize(2, 10, false);

    insta::assert_snapshot!(grid.snapshot_with_scrollback(), @r"
    [Grid 2x10 cursor=(1,9) scrollback=1]
    --- scrollback ---
    |AAAAAAAAAA|
    --- visible ---
    |BBBBBBBBBB|
    |CCCCCCCCCC|
    ");
}

#[test]
fn snapshot_round_trip_shrink_grow() {
    let mut grid = Grid::new(5, 20);
    put_str(&mut grid, "hello world here!!");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "second line");

    grid.resize(5, 10, true);
    grid.resize(5, 20, true);

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 5x20 cursor=(1,9)]
    |hello world here!!  |
    |second line         |
    |                    |
    |                    |
    |                    |
    ");
}

// Cursor tracking — precise position after reflow.

#[test]
fn snapshot_cursor_tracks_through_shrink() {
    let mut grid = Grid::new(5, 10);
    put_str(&mut grid, "abcdefghij");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "xyz");

    grid.resize(5, 5, true);

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 5x5 cursor=(2,3)]
    |abcde+
    |fghij|
    |xyz  |
    |     |
    |     |
    ");
}

#[test]
fn snapshot_cursor_exact_position_after_shrink() {
    // Cursor on 'h' of "here" at col 10 in a 20-col grid.
    let mut grid = Grid::new(10, 20);
    put_str(&mut grid, "hello world here!!");
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(12)); // On 'h' of "here".

    grid.resize(10, 10, true);

    // "hello worl" on row 0, "d here!!" on row 1.
    // Cursor was on 'h' (col 12 in old layout) → col 2 of row 1 in new layout.
    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 10x10 cursor=(1,2)]
    |hello worl+
    |d here!!  |
    |          |
    |          |
    |          |
    |          |
    |          |
    |          |
    |          |
    |          |
    ");
}

#[test]
fn snapshot_cursor_exact_position_after_grow() {
    // Soft-wrapped content: "abcdefghij" + "klmno". Cursor on 'm' (row 1, col 2).
    let mut grid = Grid::new(10, 10);
    put_str(&mut grid, "abcdefghijklmno");
    grid.cursor_mut().set_line(1);
    grid.cursor_mut().set_col(Column(2)); // On 'm'.

    grid.resize(10, 20, true);

    // Unwraps to "abcdefghijklmno". Cursor on 'm' = col 12.
    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 10x20 cursor=(0,12)]
    |abcdefghijklmno     |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    ");
}

#[test]
fn snapshot_cursor_at_wrap_boundary_after_grow() {
    // Cursor at col 0 of continuation row (on 'x').
    let mut grid = Grid::new(10, 10);
    put_str(&mut grid, "abcdefghij");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "xyz");
    grid.cursor_mut().set_line(1);
    grid.cursor_mut().set_col(Column(0)); // On 'x' in continuation row.

    // Row 0 has no WRAP (it was a natural linefeed after 10 chars).
    // So grow should NOT unwrap these into one row.
    grid.resize(10, 20, true);

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 10x20 cursor=(1,0)]
    |abcdefghij          |
    |xyz                 |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    ");
}

#[test]
fn snapshot_cursor_past_content_on_wrapped_row() {
    // Cursor at col 5, past "kl" content on continuation row.
    let mut grid = Grid::new(10, 10);
    put_str(&mut grid, "abcdefghijkl");
    grid.cursor_mut().set_line(1);
    grid.cursor_mut().set_col(Column(5)); // Past content.

    grid.resize(10, 20, true);

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 10x20 cursor=(0,5)]
    |abcdefghijkl        |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    |                    |
    ");
}

// Long logical lines spanning 3+ rows.

#[test]
fn snapshot_5_row_soft_wrap_unwraps_fully() {
    let mut grid = Grid::new(10, 10);
    let text: String = (0..50).map(|i| char::from(b'A' + (i % 26) as u8)).collect();
    put_str(&mut grid, &text);

    // 50 chars in 10 cols = 5 rows of soft-wrapped content.
    assert_eq!(grid.scrollback().len(), 0);
    assert!(grid[Line(0)][Column(9)].flags.contains(CellFlags::WRAP));

    grid.resize(10, 50, true);

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 10x50 cursor=(0,10)]
    |ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWX|
    |                                                  |
    |                                                  |
    |                                                  |
    |                                                  |
    |                                                  |
    |                                                  |
    |                                                  |
    |                                                  |
    |                                                  |
    ");
}

#[test]
fn snapshot_5_row_soft_wrap_shrink_grow_round_trip() {
    let mut grid = Grid::new(10, 10);
    let text: String = (0..50).map(|i| char::from(b'A' + (i % 26) as u8)).collect();
    put_str(&mut grid, &text);

    grid.resize(10, 7, true); // Rewrap to 7 cols: 50 / 7 = 8 rows.
    grid.resize(10, 10, true); // Grow back to 10.

    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 10x10 cursor=(4,6)]
    |ABCDEFGHIJ+
    |KLMNOPQRST+
    |UVWXYZABCD+
    |EFGHIJKLMN+
    |OPQRSTUVWX|
    |          |
    |          |
    |          |
    |          |
    |          |
    ");
}

#[test]
fn snapshot_long_line_spanning_scrollback_and_visible() {
    // 80 chars in a 5x10 grid: 8 rows of soft-wrapped content.
    // 5 visible + 3 in scrollback.
    let mut grid = Grid::with_scrollback(5, 10, 100);
    let text: String = (0..80).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
    put_str(&mut grid, &text);

    // Grow to 80 cols: all 80 chars should unwrap into a single row.
    grid.resize(5, 80, true);

    insta::assert_snapshot!(grid.snapshot_with_scrollback(), @r"
    [Grid 5x80 cursor=(0,10)]
    |abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzab|
    |                                                                                |
    |                                                                                |
    |                                                                                |
    |                                                                                |
    ");
    assert_eq!(grid.scrollback().len(), 0);
}

// Mixed hard and soft wraps — the common real-world pattern.

#[test]
fn snapshot_mixed_hard_soft_wraps() {
    // Line 1: 15 chars (soft wraps at col 10).
    // Line 2: 5 chars (hard newline).
    // Line 3: 25 chars (soft wraps at cols 10 and 20).
    let mut grid = Grid::new(10, 10);
    put_str(&mut grid, "abcdefghijklmno"); // 15 chars, soft wraps to 2 rows.
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "short"); // 5 chars, hard newline.
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "1234567890ABCDEFGHIJ12345"); // 25 chars, wraps to 3 rows.

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 10x10 cursor=(5,5)]
    |abcdefghij+
    |klmno     |
    |short     |
    |1234567890+
    |ABCDEFGHIJ+
    |12345     |
    |          |
    |          |
    |          |
    |          |
    ");

    // Grow to 30 cols: soft-wrapped lines rejoin, hard newlines stay.
    grid.resize(10, 30, true);

    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 10x30 cursor=(2,5)]
    |abcdefghijklmno               |
    |short                         |
    |1234567890ABCDEFGHIJ12345     |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    ");
}

#[test]
fn snapshot_mixed_hard_soft_wraps_in_scrollback() {
    // Push a mix of hard and soft wrapped lines into scrollback, then reflow.
    let mut grid = Grid::with_scrollback(5, 10, 100);

    // Line 1: 20 chars (soft wraps to 2 rows).
    put_str(&mut grid, "AAAAAAAAAABBBBBBBBBB");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    // Line 2: 3 chars (hard newline).
    put_str(&mut grid, "XYZ");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    // Line 3: 15 chars (soft wraps to 2 rows).
    put_str(&mut grid, "abcdefghijklmno");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    // Lines 4-6: fill visible area and push earlier content to scrollback.
    put_str(&mut grid, "line4_____");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "line5_____");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "line6_____");

    // Grow to 20 cols with reflow.
    grid.resize(5, 20, true);

    // Soft-wrapped lines should unwrap (AAAA+BBBB, abcde+klmno).
    // Hard newlines should stay separate.
    insta::assert_snapshot!(grid.snapshot_with_scrollback(), @"
    [Grid 5x20 cursor=(4,10) scrollback=1]
    --- scrollback ---
    |AAAAAAAAAABBBBBBBBBB|
    --- visible ---
    |XYZ                 |
    |abcdefghijklmno     |
    |line4_____          |
    |line5_____          |
    |line6_____          |
    ");
}

// Blank rows between content — must not be collapsed.

#[test]
fn snapshot_blank_row_between_content_preserved() {
    let mut grid = Grid::new(10, 10);
    put_str(&mut grid, "hello");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    // Row 1: intentionally blank (hard newline).
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "world");

    // Shrink to 3 cols, grow back to 10.
    grid.resize(10, 3, true);
    grid.resize(10, 10, true);

    // Blank row must be preserved between "hello" and "world".
    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 10x10 cursor=(2,2)]
    |hello     |
    |          |
    |world     |
    |          |
    |          |
    |          |
    |          |
    |          |
    |          |
    |          |
    ");
}

#[test]
fn snapshot_multiple_blank_rows_preserved() {
    let mut grid = Grid::new(10, 10);
    put_str(&mut grid, "AAA");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    grid.linefeed(); // Blank.
    grid.cursor_mut().set_col(Column(0));
    grid.linefeed(); // Blank.
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "BBB");

    grid.resize(10, 5, true);
    grid.resize(10, 10, true);

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 10x10 cursor=(3,3)]
    |AAA       |
    |          |
    |          |
    |BBB       |
    |          |
    |          |
    |          |
    |          |
    |          |
    |          |
    ");
}

// Wide chars — boundary, reflow, and round-trip.

#[test]
fn snapshot_wide_char_at_boundary() {
    let mut grid = Grid::new(3, 5);
    put_str(&mut grid, "abcd");
    grid.put_char('\u{4e16}'); // Wide CJK char — can't fit at col 4.

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 3x5 cursor=(1,2)]
    |abcd^+
    |世_   |
    |     |
    ");
}

#[test]
fn snapshot_wide_char_boundary_reflow_round_trip() {
    // Wide char at boundary, shrink further, then grow back.
    let mut grid = Grid::new(5, 5);
    put_str(&mut grid, "abcd");
    grid.put_char('\u{4e16}');

    grid.resize(5, 3, true);

    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 5x3 cursor=(1,2)]
    |abc+
    |d世_|
    |   |
    |   |
    |   |
    ");

    grid.resize(5, 10, true);

    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 5x10 cursor=(0,5)]
    |abcd世_    |
    |          |
    |          |
    |          |
    |          |
    ");
}

#[test]
fn snapshot_multiple_wide_chars_reflow() {
    let mut grid = Grid::new(5, 10);
    // "AB世界CD好EF" = 2 narrow + 2 wide + 2 narrow + 1 wide + 2 narrow = 12 display cols.
    put_str(&mut grid, "AB");
    grid.put_char('\u{4e16}'); // 世
    grid.put_char('\u{754c}'); // 界
    put_str(&mut grid, "CD");
    grid.put_char('\u{597d}'); // 好
    put_str(&mut grid, "EF");
    // 12 cols at width 10: wraps after col 9.

    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 5x10 cursor=(1,2)]
    |AB世_界_CD好_+
    |EF        |
    |          |
    |          |
    |          |
    ");

    // Shrink to 6 cols.
    grid.resize(5, 6, true);

    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 5x6 cursor=(1,2)]
    |AB世_界_+
    |CD好_EF|
    |      |
    |      |
    |      |
    ");

    // Grow back to 10.
    grid.resize(5, 10, true);

    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 5x10 cursor=(0,8)]
    |AB世_界_CD好_+
    |EF        |
    |          |
    |          |
    |          |
    ");
}

#[test]
fn snapshot_wide_char_at_1_col_grid() {
    // Wide char in a grid that shrinks to 1 column.
    // The WIDE_CHAR flag must be stripped — no valid 2-cell placement.
    let mut grid = Grid::new(10, 6);
    put_str(&mut grid, "a");
    grid.put_char('\u{4e16}'); // Wide at cols 1-2.
    put_str(&mut grid, "b");

    grid.resize(10, 1, true);

    // Each char occupies its own row. Wide char loses its flag but
    // the character itself survives.
    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 10x1 cursor=(2,0)]
    |a+
    |世+
    |b|
    | |
    | |
    | |
    | |
    | |
    | |
    | |
    ");

    // Verify wide flag is stripped.
    assert!(
        !grid[Line(1)][Column(0)]
            .flags
            .contains(CellFlags::WIDE_CHAR),
        "WIDE_CHAR flag should be stripped at 1-col width"
    );
}

#[test]
fn snapshot_wide_chars_leading_spacer_placement() {
    // "ab" + wide + "cd" shrunk to 3 cols: wide can't fit at col 2
    // (needs 2 cells but only 1 remaining), so col 2 gets LEADING_WIDE_CHAR_SPACER.
    let mut grid = Grid::new(5, 10);
    put_str(&mut grid, "ab");
    grid.put_char('\u{4e16}');
    put_str(&mut grid, "cd");

    grid.resize(5, 3, true);

    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 5x3 cursor=(2,2)]
    |ab^+
    |世_c+
    |d  |
    |   |
    |   |
    ");

    // Verify the leading spacer flag.
    assert!(
        grid[Line(0)][Column(2)]
            .flags
            .contains(CellFlags::LEADING_WIDE_CHAR_SPACER),
    );
}

// Scrollback reflow with wrapping.

#[test]
fn snapshot_reflow_with_scrollback() {
    let mut grid = Grid::new(3, 10);
    put_str(&mut grid, "AAAAAAAAAA");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "BBBBBBBBBB");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "CCCCCCCCCC");
    grid.cursor_mut().set_line(2);

    grid.resize(3, 5, true);

    insta::assert_snapshot!(grid.snapshot_with_scrollback(), @r"
    [Grid 3x5 cursor=(2,4) scrollback=3]
    --- scrollback ---
    |AAAAA+
    |AAAAA|
    |BBBBB+
    --- visible ---
    |BBBBB|
    |CCCCC+
    |CCCCC|
    ");
}

#[test]
fn snapshot_zero_scrollback_capacity_shrink() {
    // Scrollback capacity 0: excess rows are simply lost.
    let mut grid = Grid::with_scrollback(3, 10, 0);
    put_str(&mut grid, "AAAAAAAAAA");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "BBBBBBBBBB");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "CCCCCCCCCC");
    grid.cursor_mut().set_line(2);

    // Each row wraps to 2 at 5 cols → 6 total, only 3 visible, 3 to scrollback.
    // But capacity is 0, so all overflow is lost.
    grid.resize(3, 5, true);

    assert_eq!(grid.scrollback().len(), 0);
    assert_eq!(grid.lines(), 3);
    assert_eq!(grid.cols(), 5);
    // Grid still valid, cursor in bounds.
    assert!(grid.cursor().line() < grid.lines());
}

// Simultaneous row and column changes.

#[test]
fn snapshot_grow_cols_shrink_rows() {
    // Soft-wrapped content. Growing cols reduces logical rows, but we also
    // shrink the visible line count.
    let mut grid = Grid::new(10, 10);
    put_str(&mut grid, "abcdefghijklmnopqrst"); // 20 chars → 2 rows at 10 cols.
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "hello");
    grid.cursor_mut().set_line(2);

    // Grow cols to 20 (unwrap) and shrink rows to 5.
    grid.resize(5, 20, true);

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 5x20 cursor=(1,5)]
    |abcdefghijklmnopqrst|
    |hello               |
    |                    |
    |                    |
    |                    |
    ");
}

#[test]
fn snapshot_shrink_cols_shrink_rows() {
    // 3 lines of 15-char content in a 10x20 grid.
    let mut grid = Grid::new(10, 20);
    put_str(&mut grid, "aaaaabbbbbccccc");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "dddddeeeeefff");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "ggggghhhhhiiiii");
    grid.cursor_mut().set_line(2);

    grid.resize(5, 5, true);

    // 15 chars each → 3 rows at 5 cols. 3 lines × 3 = 9 rows total.
    // 5 visible, 4 in scrollback.
    insta::assert_snapshot!(grid.snapshot_with_scrollback(), @r"
    [Grid 5x5 cursor=(4,4) scrollback=4]
    --- scrollback ---
    |aaaaa+
    |bbbbb+
    |ccccc|
    |ddddd+
    --- visible ---
    |eeeee+
    |fff  |
    |ggggg+
    |hhhhh+
    |iiiii|
    ");
}

// Realistic terminal session patterns.

#[test]
fn snapshot_cli_prompt_and_long_command_output() {
    // Simulate: prompt, then a command that produces long output lines.
    let mut grid = Grid::with_scrollback(24, 80, 1000);

    // Prompt line (hard newline).
    put_str(&mut grid, "user@host:~/projects$ ");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));

    // Long cargo build output that will wrap at 80→40 transition.
    let cargo_line = "   Compiling oriterm_core v0.1.0 (/home/user/projects/ori_term/oriterm_core)";
    put_str(&mut grid, cargo_line); // 76 chars, fits in 80 cols.
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));

    let long_line = "error[E0308]: mismatched types --> oriterm_core/src/grid/resize/mod.rs:42:5";
    put_str(&mut grid, long_line);
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));

    // Another prompt.
    put_str(&mut grid, "user@host:~/projects$ ");

    // Shrink to 40 cols (common half-screen width).
    grid.resize(24, 40, true);

    // Grow back to 80.
    grid.resize(24, 80, true);

    // Content should be identical (no wrapping artifacts on these lines).
    assert_eq!(read_row(&grid, 0), "user@host:~/projects$");
    assert_eq!(read_row(&grid, 1), cargo_line);
    assert_eq!(read_row(&grid, 2), long_line);
    assert_eq!(read_row(&grid, 3), "user@host:~/projects$");
}

#[test]
fn snapshot_multiline_prompt_with_decorations() {
    // Starship/p10k style 2-line prompt with box-drawing chars.
    let mut grid = Grid::new(10, 40);
    let prompt_line1 = "\u{256D}\u{2500} user@host ~/projects/ori_term";
    let prompt_line2 = "\u{2570}\u{2500} \u{276F} ";
    put_str(&mut grid, prompt_line1);
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, prompt_line2);
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    // Command that wraps.
    put_str(
        &mut grid,
        "cargo build --target x86_64-pc-windows-gnu --release",
    );

    // Shrink to 20 cols.
    grid.resize(10, 20, true);
    // Grow back to 40.
    grid.resize(10, 40, true);

    // Prompt lines (hard newlines) must stay separate.
    // The command (52 chars) still wraps at 40 cols, but the soft wrap should
    // rejoin correctly. read_row trims trailing spaces.
    assert_eq!(read_row(&grid, 0), prompt_line1);
    assert_eq!(read_row(&grid, 1), prompt_line2.trim_end());
    // Command wraps to 2 physical rows at 40 cols.
    let cmd_row0 = read_row(&grid, 2);
    let cmd_row1 = read_row(&grid, 3);
    assert_eq!(
        format!("{cmd_row0}{cmd_row1}"),
        "cargo build --target x86_64-pc-windows-gnu --release"
    );
}

#[test]
fn snapshot_interactive_session_with_scrollback() {
    // Multiple commands, some output wrapping, building up scrollback.
    let mut grid = Grid::with_scrollback(10, 40, 1000);

    let commands = [
        ("$ ls -la", "total 48"),
        ("$ git status", "On branch dev"),
        (
            "$ cargo test --workspace",
            "running 1366 tests ... test result: ok",
        ),
        ("$ echo done", "done"),
    ];

    for (cmd, output) in &commands {
        put_str(&mut grid, cmd);
        grid.linefeed();
        grid.cursor_mut().set_col(Column(0));
        put_str(&mut grid, output);
        grid.linefeed();
        grid.cursor_mut().set_col(Column(0));
    }
    put_str(&mut grid, "$ ");

    // Shrink to 20 cols, grow back.
    grid.resize(10, 20, true);
    grid.resize(10, 40, true);

    // All lines are under 40 chars, so round-trip should be lossless.
    // Verify last visible lines.
    let rows = read_all_rows(&grid);
    assert!(rows.iter().any(|r| r.contains("$ echo done")));
    assert!(rows.iter().any(|r| r == "done"));
}

#[test]
fn snapshot_long_wrapped_output_like_base64() {
    // Simulates long single-line output (like `base64`, `xxd`, etc.)
    // that wraps across many rows.
    let mut grid = Grid::with_scrollback(10, 20, 100);
    let long_output: String = (0..100)
        .map(|i| char::from(b'A' + (i % 26) as u8))
        .collect();
    put_str(&mut grid, "$ cmd");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, &long_output); // 100 chars, wraps to 5 rows at 20 cols.
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "$ ");

    // Shrink to 10 cols: the 100-char line re-wraps to 10 rows.
    grid.resize(10, 10, true);

    // The prompt "$ " should still be on a separate row (hard newline).
    let rows = read_all_rows(&grid);
    assert!(rows.iter().any(|r| r == "$"));

    // Grow back to 20.
    grid.resize(10, 20, true);

    // Verify the long output is contiguous after unwrap.
    let rows = read_all_rows(&grid);
    let mut all_text = String::new();
    let sb_len = grid.scrollback().len();
    for i in 0..sb_len {
        let row = grid.scrollback().get(sb_len - 1 - i).unwrap();
        let text: String = (0..row.cols()).map(|c| row[Column(c)].ch).collect();
        all_text.push_str(text.trim_end());
    }
    for row_text in &rows {
        all_text.push_str(row_text);
    }
    assert!(
        all_text.contains(&long_output),
        "long output lost after reflow"
    );
}

// Cell attributes through reflow.

#[test]
fn snapshot_combining_marks_survive_reflow() {
    // 'e' + combining acute accent (U+0301) at col 0.
    let mut grid = Grid::new(5, 10);
    grid.put_char('e');
    // Move cursor back to col 0 to push combining mark onto 'e'.
    grid.cursor_mut().set_col(Column(1));
    grid.push_zerowidth('\u{0301}');
    grid.cursor_mut().set_col(Column(1));
    put_str(&mut grid, "bcdefghij"); // Fill to 10 cols.

    // Soft-wrap is set (row 0 full, col 10 pending).
    grid.resize(5, 5, true);
    grid.resize(5, 10, true);

    // The 'e' cell should still have the combining mark.
    let cell = &grid[Line(0)][Column(0)];
    assert_eq!(cell.ch, 'e');
    let extra = cell.extra.as_ref().expect("combining mark lost");
    assert_eq!(extra.zerowidth, vec!['\u{0301}']);
}

#[test]
fn snapshot_hyperlinks_survive_reflow() {
    use crate::cell::Hyperlink;

    let mut grid = Grid::new(5, 10);
    // Write "click" with hyperlink at cols 0-4.
    for (i, ch) in "click".chars().enumerate() {
        grid[Line(0)][Column(i)] = cell(ch);
        grid[Line(0)][Column(i)].set_hyperlink(Some(Hyperlink {
            id: None,
            uri: "https://example.com".to_string(),
        }));
    }
    // Fill rest of row to force soft wrap.
    for (i, ch) in "ABCDE".chars().enumerate() {
        grid[Line(0)][Column(5 + i)] = cell(ch);
    }
    grid[Line(0)][Column(9)].flags.insert(CellFlags::WRAP);
    write_row(&mut grid, 1, "next");

    grid.resize(5, 5, true);
    grid.resize(5, 10, true);

    // Hyperlinks should survive the reflow.
    let link_cell = &grid[Line(0)][Column(0)];
    assert_eq!(link_cell.ch, 'c');
    let extra = link_cell.extra.as_ref().expect("hyperlink lost");
    assert!(extra.hyperlink.is_some());
    assert_eq!(extra.hyperlink.as_ref().unwrap().uri, "https://example.com");
}

#[test]
fn snapshot_underline_color_survives_reflow() {
    use vte::ansi::Color;

    let mut grid = Grid::new(5, 10);
    grid[Line(0)][Column(0)] = cell('U');
    grid[Line(0)][Column(0)]
        .flags
        .insert(CellFlags::CURLY_UNDERLINE);
    grid[Line(0)][Column(0)].set_underline_color(Some(Color::Indexed(4)));
    // Fill to force wrap.
    for (i, ch) in "BCDEFGHIJ".chars().enumerate() {
        grid[Line(0)][Column(1 + i)] = cell(ch);
    }
    grid[Line(0)][Column(9)].flags.insert(CellFlags::WRAP);
    write_row(&mut grid, 1, "next");

    grid.resize(5, 5, true);
    grid.resize(5, 10, true);

    let cell = &grid[Line(0)][Column(0)];
    assert_eq!(cell.ch, 'U');
    assert!(cell.flags.contains(CellFlags::CURLY_UNDERLINE));
    let extra = cell.extra.as_ref().expect("underline color lost");
    assert_eq!(extra.underline_color, Some(Color::Indexed(4)));
}

#[test]
fn snapshot_bce_colored_blanks_reflow_correctly() {
    use vte::ansi::Color;

    // A row of spaces with colored background (BCE). These are NOT empty,
    // so content_len is 10, and they should wrap properly.
    let mut grid = Grid::new(5, 10);
    for c in 0..10 {
        grid[Line(0)][Column(c)].bg = Color::Indexed(1);
    }
    // No WRAP — hard line.

    grid.resize(5, 5, true);

    // BCE row has content_len 10 → wraps to 2 rows at 5 cols.
    // Both rows should preserve the colored background.
    for c in 0..5 {
        assert_eq!(grid[Line(0)][Column(c)].bg, Color::Indexed(1));
        assert_eq!(grid[Line(1)][Column(c)].bg, Color::Indexed(1));
    }
}

// Edge cases in the reflow algorithm.

#[test]
fn snapshot_wide_spacer_at_row_end_during_reflow() {
    // Wide char at cols 8-9 in a 10-col grid (spacer at col 9, the last cell).
    // Set WRAP, continue on next row. Reflow must handle the spacer at the
    // boundary correctly.
    let mut grid = Grid::new(5, 10);
    write_row(&mut grid, 0, "abcdefgh");
    let mut wide = cell('\u{4e16}');
    wide.flags.insert(CellFlags::WIDE_CHAR);
    grid[Line(0)][Column(8)] = wide;
    let mut spacer = Cell::default();
    spacer.flags.insert(CellFlags::WIDE_CHAR_SPACER);
    grid[Line(0)][Column(9)] = spacer;
    grid[Line(0)][Column(9)].flags.insert(CellFlags::WRAP);
    write_row(&mut grid, 1, "xyz");

    grid.resize(5, 6, true);
    grid.resize(5, 10, true);

    // Content should survive: "abcdefgh" + wide + "xyz" on separate logical line.
    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 5x10 cursor=(0,0)]
    |abcdefgh世_+
    |xyz       |
    |          |
    |          |
    |          |
    ");

    // Wait — the spacer at col 9 also has WRAP. After reflow to 6 cols:
    // "abcdefgh" = 8 narrow chars. At 6 cols, wraps: "abcdef" + "gh" + wide.
    // Then "xyz" on a new logical line. Growing back to 10 should unwrap
    // the soft wrap but keep "xyz" separate.
    // The snapshot above verifies this.
}

#[test]
fn snapshot_content_exactly_fills_new_width() {
    // 10 chars into 10 cols with WRAP, then 5 chars. Grow to 15: exactly fits.
    let mut grid = Grid::new(5, 10);
    put_str(&mut grid, "abcdefghijklmno"); // 15 chars.

    // Row 0: "abcdefghij" with WRAP, Row 1: "klmno".
    grid.resize(5, 15, true);

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 5x15 cursor=(0,5)]
    |abcdefghijklmno|
    |               |
    |               |
    |               |
    |               |
    ");

    // No WRAP should remain.
    assert!(!grid[Line(0)][Column(14)].flags.contains(CellFlags::WRAP));
}

#[test]
fn snapshot_reflow_display_offset_clamped() {
    // Build scrollback, scroll display back, then reflow.
    let mut grid = Grid::with_scrollback(5, 10, 100);
    for i in 0..15 {
        write_row(&mut grid, 0, &format!("line{i:02}____"));
        grid.scroll_up(1);
    }
    grid.cursor_mut().set_line(4);

    // Scroll back into history.
    grid.scroll_display(5);
    let old_offset = grid.display_offset();
    assert!(old_offset > 0);

    // Reflow: grow cols.
    grid.resize(5, 20, true);

    // display_offset must be clamped.
    assert!(grid.display_offset() <= grid.scrollback().len());
    assert!(grid.cursor().line() < grid.lines());
}

#[test]
fn snapshot_saved_cursor_clamped_after_reflow() {
    let mut grid = Grid::new(10, 20);
    put_str(&mut grid, "hello world here!!");
    grid.cursor_mut().set_line(5);
    grid.cursor_mut().set_col(Column(15));
    grid.save_cursor();

    // Shrink to 10 cols with reflow.
    grid.resize(10, 10, true);

    // Restore cursor — should be within bounds.
    grid.restore_cursor();
    assert!(grid.cursor().line() < grid.lines());
    assert!(grid.cursor().col().0 < grid.cols());
}

// Stress: aggressive multi-step resize sequences.

#[test]
fn snapshot_aggressive_resize_sequence_with_wide_chars() {
    let mut grid = Grid::new(10, 20);
    // Mix of narrow and wide chars.
    put_str(&mut grid, "Hello ");
    grid.put_char('\u{4e16}'); // 世
    grid.put_char('\u{754c}'); // 界
    put_str(&mut grid, " World ");
    grid.put_char('\u{597d}'); // 好
    put_str(&mut grid, "!");
    // Total: 6 + 2 + 2 + 7 + 2 + 1 = 20 display cols.

    // Aggressive resize sequence.
    grid.resize(10, 3, true);
    grid.resize(10, 7, true);
    grid.resize(10, 1, true);
    grid.resize(10, 15, true);
    grid.resize(10, 20, true);

    // Content should survive (wide flags may be lost from the 1-col step).
    let row_text = read_row(&grid, 0);
    assert!(row_text.contains("Hello"), "content lost: {row_text:?}");
    assert!(row_text.contains("World"), "content lost: {row_text:?}");
}

#[test]
fn snapshot_rapid_resize_with_scrollback_interaction() {
    let mut grid = Grid::with_scrollback(5, 40, 100);

    // Write several lines of varying length.
    let lines = [
        "Short",
        "A medium-length line that fits in 40 co",
        "",
        "Another line with some content here",
        "The last visible line of content!!!!!!!!",
    ];
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            grid.linefeed();
            grid.cursor_mut().set_col(Column(0));
        }
        put_str(&mut grid, line);
    }
    grid.cursor_mut().set_line(4);

    // Rapid resize: 40 → 10 → 80 → 20 → 40.
    grid.resize(5, 10, true);
    grid.resize(5, 80, true);
    grid.resize(5, 20, true);
    grid.resize(5, 40, true);

    // All content should survive.
    let mut all_text = String::new();
    let sb_len = grid.scrollback().len();
    for i in 0..sb_len {
        let row = grid.scrollback().get(sb_len - 1 - i).unwrap();
        let text: String = (0..row.cols()).map(|c| row[Column(c)].ch).collect();
        all_text.push_str(text.trim_end());
        all_text.push('\n');
    }
    for i in 0..grid.lines() {
        all_text.push_str(&read_row(&grid, i));
        all_text.push('\n');
    }
    for line in &lines {
        if !line.is_empty() {
            assert!(
                all_text.contains(line),
                "lost content: {line:?}\nfull: {all_text:?}"
            );
        }
    }
}

// Occ tracking correctness after reflow.

#[test]
fn snapshot_occ_correct_after_reflow() {
    let mut grid = Grid::new(10, 10);
    put_str(&mut grid, "hello");

    grid.resize(10, 3, true);

    // "hello" at 3 cols: "hel" (occ=3) + "lo" (occ=2).
    assert!(grid[Line(0)].occ() >= 3);
    assert!(grid[Line(1)].occ() >= 2);
}

// Complex: many hard-newlined short lines (like git log output).

#[test]
fn snapshot_many_short_lines_reflow() {
    // Simulates `git log --oneline` or `ls` output: many short lines.
    let mut grid = Grid::with_scrollback(10, 40, 100);

    for i in 0..20 {
        if i > 0 {
            grid.linefeed();
            grid.cursor_mut().set_col(Column(0));
        }
        put_str(&mut grid, &format!("{:07x} commit message {i}", i * 0x1234));
    }
    grid.cursor_mut().set_line(9);

    // None of these lines wrap at 40 cols. Shrinking to 20 should wrap
    // the longer ones, growing back should unwrap them.
    grid.resize(10, 20, true);
    grid.resize(10, 40, true);

    // Verify last visible rows are intact.
    let rows = read_all_rows(&grid);
    let last_nonempty: Vec<&String> = rows.iter().filter(|r| !r.is_empty()).collect();
    assert!(last_nonempty.last().unwrap().contains("commit message 19"));
}

#[test]
fn snapshot_wrapped_line_across_scrollback_boundary() {
    // A soft-wrapped line that straddles the scrollback/visible boundary.
    let mut grid = Grid::with_scrollback(3, 10, 100);

    // Write 2 short hard-newlined lines.
    put_str(&mut grid, "line1");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "line2");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    // Write 25-char line that wraps to 3 rows at 10 cols.
    put_str(&mut grid, "abcdefghijklmnopqrstuvwxy");
    // This pushes earlier content to scrollback.

    insta::assert_snapshot!(grid.snapshot_with_scrollback(), @r"
    [Grid 3x10 cursor=(2,5) scrollback=2]
    --- scrollback ---
    |line1     |
    |line2     |
    --- visible ---
    |abcdefghij+
    |klmnopqrst+
    |uvwxy     |
    ");

    // Grow to 25 cols: the 25-char line should unwrap to 1 row.
    grid.resize(3, 25, true);

    insta::assert_snapshot!(grid.snapshot_with_scrollback(), @"
    [Grid 3x25 cursor=(2,5)]
    |line1                    |
    |line2                    |
    |abcdefghijklmnopqrstuvwxy|
    ");
}

#[test]
fn snapshot_mixed_content_types_realistic() {
    // Simulates a realistic terminal session with different content types:
    // - Short prompt
    // - Long command (wraps)
    // - Short output lines
    // - Blank line
    // - Another prompt with cursor
    let mut grid = Grid::with_scrollback(24, 30, 1000);

    // Prompt.
    put_str(&mut grid, "$ ");
    // Long command that wraps.
    put_str(
        &mut grid,
        "find . -name '*.rs' -exec grep -l 'reflow' {} \\;",
    );
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    // Output lines.
    put_str(&mut grid, "./src/grid/resize/mod.rs");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "./src/grid/resize/tests.rs");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    // Blank line.
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    // Next prompt with cursor.
    put_str(&mut grid, "$ ");

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 24x30 cursor=(5,2)]
    |$ find . -name '*.rs' -exec gr+
    |ep -l 'reflow' {} \;          |
    |./src/grid/resize/mod.rs      |
    |./src/grid/resize/tests.rs    |
    |                              |
    |$                             |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    |                              |
    ");

    // Shrink to 15 cols.
    grid.resize(24, 15, true);
    // Grow back to 30.
    grid.resize(24, 30, true);

    // Verify content integrity. The command wraps, so check parts separately.
    let rows = read_all_rows(&grid);
    let text = rows.join("");
    assert!(
        text.contains("$ find . -name '*.rs' -exec grep -l 'reflow' {} \\;"),
        "command lost: {rows:?}"
    );
    assert!(text.contains("./src/grid/resize/mod.rs"));
    assert!(text.contains("./src/grid/resize/tests.rs"));
}

#[test]
fn snapshot_claude_code_like_output() {
    // Simulates Claude Code style output: prompt, response with code blocks,
    // mixed-length lines, some wrapping.
    let mut grid = Grid::with_scrollback(24, 60, 1000);

    let lines = [
        "> How do I resize a terminal grid?",
        "",
        "Here's how to resize a grid in oriterm:",
        "",
        "```rust",
        "grid.resize(new_lines, new_cols, reflow);",
        "```",
        "",
        "The `reflow` parameter controls whether soft-wrapped lines",
        "are re-wrapped to fit the new width. When `true`, the grid",
        "performs cell-by-cell rewriting to unwrap/wrap content.",
    ];

    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            grid.linefeed();
            grid.cursor_mut().set_col(Column(0));
        }
        put_str(&mut grid, line);
    }

    // Shrink to 30 cols (half width).
    grid.resize(24, 30, true);
    // Grow back to 60.
    grid.resize(24, 60, true);

    // All hard-newlined lines should round-trip.
    for line in &lines {
        if !line.is_empty() {
            assert_eq!(
                read_row(&grid, lines.iter().position(|l| l == line).unwrap()),
                *line,
                "line corrupted after reflow: {line:?}"
            );
        }
    }
}

#[test]
fn snapshot_vim_like_full_screen_reflow() {
    // Full-screen app: all rows have content (like vim, htop).
    // No WRAP flags (each row is independent).
    let mut grid = Grid::new(5, 20);
    let rows_content = [
        "  1| fn main() {   ",
        "  2|     println!()",
        "  3| }             ",
        "~                   ",
        "[No Name] 1,1   All",
    ];
    for (i, content) in rows_content.iter().enumerate() {
        grid.cursor_mut().set_line(i);
        grid.cursor_mut().set_col(Column(0));
        // Pad/trim to exactly 20 cols.
        let padded: String = content
            .chars()
            .chain(std::iter::repeat(' '))
            .take(20)
            .collect();
        put_str(&mut grid, &padded);
    }
    grid.cursor_mut().set_line(1);
    grid.cursor_mut().set_col(Column(18)); // Cursor at end of println.

    // No-reflow resize (alt screen behavior): rows are truncated/padded.
    grid.resize(5, 30, false);

    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 5x30 cursor=(1,18)]
    |  1| fn main() {              |
    |  2|     println!()           |
    |  3| }                        |
    |~                             |
    |[No Name] 1,1   All           |
    ");
}

// Extreme edge cases.

#[test]
fn snapshot_reflow_shrink_forces_massive_scrollback() {
    // 3x20 grid, all rows full. Shrink to 5 cols: each 20-char row becomes
    // 4 rows. 3 × 4 = 12 total, 3 visible, 9 to scrollback.
    let mut grid = Grid::with_scrollback(3, 20, 100);
    put_str(&mut grid, "AAAAAAAAAABBBBBBBBBB");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "CCCCCCCCCCDDDDDDDDDD");
    grid.linefeed();
    grid.cursor_mut().set_col(Column(0));
    put_str(&mut grid, "EEEEEEEEEEFFFFFFFFFF");
    grid.cursor_mut().set_line(2);

    grid.resize(3, 5, true);

    insta::assert_snapshot!(grid.snapshot_with_scrollback(), @r"
    [Grid 3x5 cursor=(2,4) scrollback=9]
    --- scrollback ---
    |AAAAA+
    |AAAAA+
    |BBBBB+
    |BBBBB|
    |CCCCC+
    |CCCCC+
    |DDDDD+
    |DDDDD|
    |EEEEE+
    --- visible ---
    |EEEEE+
    |FFFFF+
    |FFFFF|
    ");
}

#[test]
fn snapshot_reflow_empty_grid() {
    let mut grid = Grid::new(3, 10);

    grid.resize(3, 20, true);

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 3x20 cursor=(0,0)]
    |                    |
    |                    |
    |                    |
    ");
}

#[test]
fn snapshot_reflow_single_char_grid() {
    let mut grid = Grid::new(1, 1);
    grid.put_char('X');

    grid.resize(1, 10, true);

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 1x10 cursor=(0,1)]
    |X         |
    ");
}

#[test]
fn snapshot_wide_char_exactly_fills_row() {
    // 5 wide chars in a 10-col row: exactly fills the row.
    let mut grid = Grid::new(5, 10);
    for _ in 0..5 {
        grid.put_char('\u{4e16}');
    }

    insta::assert_snapshot!(grid.snapshot(), @r"
    [Grid 5x10 cursor=(0,10)]
    |世_世_世_世_世_|
    |          |
    |          |
    |          |
    |          |
    ");

    // Shrink to 6 cols: 5 wide chars = 10 display cols.
    // At 6 cols: 3 wide chars fit (6 cols), then boundary for 4th.
    grid.resize(5, 6, true);

    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 5x6 cursor=(1,5)]
    |世_世_世_+
    |世_世_  |
    |      |
    |      |
    |      |
    ");

    grid.resize(5, 10, true);

    insta::assert_snapshot!(grid.snapshot(), @"
    [Grid 5x10 cursor=(0,5)]
    |世_世_世_世_世_|
    |          |
    |          |
    |          |
    |          |
    ");
}

// ── Display offset reset on resize ─────────────────────────────────

/// Resize must reset display_offset to 0 (live view).
///
/// Regression: during drag resize, the scroll code incremented display_offset
/// to keep a scrollback view stable, but after reflow the scrollback content
/// is rewritten so the old offset is meaningless. Keeping a stale offset made
/// the renderer show duplicated/corrupted scrollback instead of the live view.
#[test]
fn resize_resets_display_offset_to_zero() {
    let mut grid = Grid::with_scrollback(5, 20, 100);
    // Fill all 5 rows with content so shrinking pushes rows to scrollback.
    write_row(&mut grid, 0, "Line A");
    write_row(&mut grid, 1, "Line B");
    write_row(&mut grid, 2, "Line C");
    write_row(&mut grid, 3, "Line D");
    write_row(&mut grid, 4, "Line E");
    // Place cursor at bottom so shrink_rows pushes top rows to scrollback.
    grid.cursor_mut().set_line(4);

    // Shrink from 5 to 3 visible lines — top 2 rows go to scrollback.
    grid.resize(3, 20, true);
    assert!(
        grid.scrollback().len() >= 2,
        "shrink should push rows to scrollback"
    );
    assert_eq!(grid.display_offset(), 0);

    // Simulate user scrolling back into scrollback.
    grid.scroll_display(2);
    assert_eq!(grid.display_offset(), 2);

    // Resize again — display_offset must reset to 0 (live view).
    grid.resize(3, 15, true);
    assert_eq!(
        grid.display_offset(),
        0,
        "resize must reset display_offset to 0"
    );
}

/// Reproduces the exact real-world resize ghosting bug from log data.
///
/// Real app sequence (from oriterm.log resize #5→#6→#7→#8):
/// 1. Shell writes TUI with long border lines that WRAP on resize
/// 2. Resize: reflow wraps long lines, overflow (including mascot) to scrollback
/// 3. Shell responds: scroll_up (resets resize_pushed!) → erase_display → redraw
/// 4. scroll_up resets resize_pushed=0, so erase_display can't clean overflow
/// 5. Next resize: stale overflow with mascot is still in scrollback
/// 6. Scrollback compounds with duplicate mascots each cycle
///
/// Key: the border lines must be LONGER than the visible width so reflow
/// wraps them, pushing content above (including the mascot) to scrollback.
#[test]
fn real_world_resize_ghosting_with_scroll_up() {
    use crate::grid::editing::DisplayEraseMode;

    // 10 visible rows at 80 cols — like the real Claude Code TUI.
    let mut grid = Grid::with_scrollback(10, 80, 500);

    // Build a TUI at a given column width. Border lines (rows 5, 7) are
    // FULL WIDTH so they WRAP when resized to smaller widths. This pushes
    // the mascot (rows 3-4) into scrollback as reflow overflow.
    let tui = |cols: usize| -> [String; 10] {
        let b = "─".repeat(cols);
        [
            "cmd.exe prompt line one padding to fill".to_string(),
            "shell prompt line two extra padding zzz".to_string(),
            String::new(),
            " MASCOT   Claude Code v2.1.50".to_string(),
            "MASCOT2   Opus 4.6 Claude Max".to_string(),
            b.clone(),
            "Try: how does <filepath> work?".to_string(),
            b,
            format!("{:>w$}", "0 tokens", w = cols),
            format!("{:>w$}", "current: 2.1.50", w = cols),
        ]
    };

    // Write helper that truncates by chars (not bytes) to fit grid width.
    let write_tui = |grid: &mut Grid, lines: &[String; 10], cols: usize| {
        for (i, text) in lines.iter().enumerate() {
            if !text.is_empty() {
                let s: String = text.chars().take(cols).collect();
                write_row(grid, i, &s);
            }
        }
    };

    // Initial TUI at 80 cols.
    let initial = tui(80);
    write_tui(&mut grid, &initial, 80);
    grid.cursor_mut().set_line(9);
    grid.cursor_mut().set_col(Column(79));

    let count_mascot = |g: &Grid| -> usize {
        g.snapshot_with_scrollback()
            .matches("MASCOT   Claude Code")
            .count()
    };
    assert_eq!(count_mascot(&grid), 1, "start: 1 mascot");
    assert_eq!(grid.scrollback().len(), 0);

    // 3 resize+redraw cycles. Each shrinks cols so borders wrap.
    for &new_cols in &[50, 40, 30] {
        // 1. Resize — reflow wraps the 80-char borders, pushing content to scrollback.
        grid.resize(10, new_cols, true);

        // 2. Shell responds: scroll_up to push old top rows, then erase + redraw.
        //    scroll_up resets resize_pushed — this is the bug trigger.
        grid.scroll_up(4);

        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(0));
        grid.erase_display(DisplayEraseMode::All);

        // Shell redraws TUI at new width.
        let fresh = tui(new_cols);
        write_tui(&mut grid, &fresh, new_cols);
        grid.cursor_mut().set_line(9);
        grid.cursor_mut()
            .set_col(Column(new_cols.saturating_sub(1)));
    }

    // === Final check with golden snapshot ===
    let final_snap = grid.snapshot_with_scrollback();
    let final_mascot_count = count_mascot(&grid);
    let final_sb = grid.scrollback().len();

    // BUG: mascot appears 3+ times from stale reflow overflow compounding.
    // Expected: at most 2 (1 in visible from fresh redraw, 1 in scrollback
    // from the scroll_up that pushed it as real history).
    assert!(
        final_mascot_count <= 2,
        "mascot 'MASCOT   Claude Code' appears {final_mascot_count} times \
         (expected <= 2). Stale reflow overflow is compounding in scrollback.\n\n\
         scrollback={final_sb}\n{final_snap}"
    );
}

/// Height-only resize: visible area has no ghosting.
///
/// When height shrinks, `shrink_rows` pushes top rows to scrollback as
/// real history (not reflow overflow). `scroll_up` always pushes evicted
/// rows normally. The visible area should have exactly 1 copy of the
/// TUI content after each resize+redraw cycle. Scrollback accumulates
/// history rows, which is correct terminal behavior.
#[test]
fn height_only_resize_ghosting() {
    use crate::grid::editing::DisplayEraseMode;

    let mut grid = Grid::with_scrollback(10, 40, 500);

    let tui = [
        "cmd.exe prompt line one padding",
        "shell prompt line two padding z",
        "",
        " MASCOT   Claude Code v2.1.50",
        "MASCOT2   Opus 4.6 Claude Max",
        "────────────────────────────────────────",
        "Try: how does <filepath> work?",
        "────────────────────────────────────────",
        "                        0 tokens",
        "                 current: 2.1.50",
    ];

    // Write initial TUI at 10 rows x 40 cols.
    for (i, text) in tui.iter().enumerate() {
        if !text.is_empty() {
            let s: String = text.chars().take(40).collect();
            write_row(&mut grid, i, &s);
        }
    }
    grid.cursor_mut().set_line(9);
    grid.cursor_mut().set_col(Column(39));

    let count_visible_mascot =
        |g: &Grid| -> usize { g.snapshot().matches("MASCOT   Claude Code").count() };
    assert_eq!(count_visible_mascot(&grid), 1, "start: 1 mascot");

    // 3 cycles of height shrink + shell response. Each shrink pushes top
    // rows to scrollback as real history. The visible area should always
    // have exactly 1 mascot after the shell redraws.
    for &new_rows in &[8, 6, 5] {
        // 1. Height shrinks — pushes top rows to scrollback.
        grid.resize(new_rows, 40, true);

        // 2. Shell responds: scroll_up + erase + redraw.
        grid.scroll_up(2);

        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(0));
        grid.erase_display(DisplayEraseMode::All);

        // Shell redraws TUI at new height (fewer rows, same cols).
        for (i, text) in tui.iter().take(new_rows).enumerate() {
            if !text.is_empty() {
                let s: String = text.chars().take(40).collect();
                write_row(&mut grid, i, &s);
            }
        }
        grid.cursor_mut().set_line(new_rows - 1);
        grid.cursor_mut().set_col(Column(39));

        assert_eq!(
            count_visible_mascot(&grid),
            1,
            "after shrink to {new_rows}: visible should have exactly 1 mascot\n{}",
            grid.snapshot()
        );
    }
}
