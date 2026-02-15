use vte::ansi::Color;

use crate::grid::Grid;
use crate::index::{Column, Line};

// --- set_scroll_region ---

#[test]
fn set_scroll_region_full_screen() {
    let mut grid = Grid::new(24, 80);
    grid.set_scroll_region(1, None);
    assert_eq!(grid.scroll_region, 0..24);
}

#[test]
fn set_scroll_region_sub_region() {
    let mut grid = Grid::new(24, 80);
    grid.set_scroll_region(2, Some(10));
    assert_eq!(grid.scroll_region, 1..10);
}

#[test]
fn set_scroll_region_default_bottom() {
    let mut grid = Grid::new(24, 80);
    grid.set_scroll_region(5, None);
    assert_eq!(grid.scroll_region, 4..24);
}

#[test]
fn set_scroll_region_invalid_top_ge_bottom() {
    let mut grid = Grid::new(24, 80);
    let original = grid.scroll_region.clone();
    // top >= bottom: no change.
    grid.set_scroll_region(10, Some(5));
    assert_eq!(grid.scroll_region, original);
}

#[test]
fn set_scroll_region_top_zero_treated_as_one() {
    let mut grid = Grid::new(24, 80);
    grid.set_scroll_region(0, Some(10));
    // top=0 treated as top=1 -> 0-based top=0.
    assert_eq!(grid.scroll_region, 0..10);
}

#[test]
fn set_scroll_region_clamps_oversized_bottom() {
    let mut grid = Grid::new(10, 80);
    grid.set_scroll_region(1, Some(100));
    assert_eq!(grid.scroll_region, 0..10);
}

#[test]
fn set_scroll_region_does_not_move_cursor() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(10);
    grid.cursor_mut().set_col(Column(40));
    grid.set_scroll_region(5, Some(20));
    // Cursor positioning is the handler's job (respects ORIGIN mode).
    assert_eq!(grid.cursor().line(), 10);
    assert_eq!(grid.cursor().col(), Column(40));
}

// --- scroll_up ---

#[test]
fn scroll_up_one_line_full_screen() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_up(1);
    // Line 0 now has what was line 1 ('B').
    assert_eq!(grid[Line(0)][Column(0)].ch, 'B');
    // Line 1 now has what was line 2 ('C').
    assert_eq!(grid[Line(1)][Column(0)].ch, 'C');
    // Line 2 is blank.
    assert!(grid[Line(2)][Column(0)].is_empty());
}

#[test]
fn scroll_up_three_lines_full_screen() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_up(3);
    // Lines 0-1 have what was lines 3-4 ('D', 'E').
    assert_eq!(grid[Line(0)][Column(0)].ch, 'D');
    assert_eq!(grid[Line(1)][Column(0)].ch, 'E');
    // Lines 2-4 are blank.
    for line in 2..5 {
        assert!(grid[Line(line)][Column(0)].is_empty());
    }
}

#[test]
fn scroll_up_sub_region_preserves_outside() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_region = 1..4;
    grid.scroll_up(1);
    // Line 0 ('A') untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    // Line 4 ('E') untouched.
    assert_eq!(grid[Line(4)][Column(0)].ch, 'E');
    // Inside region: line 1 now has 'C', line 2 has 'D', line 3 blank.
    assert_eq!(grid[Line(1)][Column(0)].ch, 'C');
    assert_eq!(grid[Line(2)][Column(0)].ch, 'D');
    assert!(grid[Line(3)][Column(0)].is_empty());
}

#[test]
fn scroll_up_count_exceeds_region() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    // Count larger than region: clamped, all lines blank.
    grid.scroll_up(100);
    for line in 0..3 {
        assert!(grid[Line(line)][Column(0)].is_empty());
    }
}

#[test]
fn scroll_up_bce_fill() {
    let mut grid = Grid::new(3, 10);
    grid.put_char('A');
    grid.cursor_mut().template.bg = Color::Indexed(4);
    grid.scroll_up(1);
    // New bottom row has BCE background.
    assert_eq!(grid[Line(2)][Column(0)].bg, Color::Indexed(4));
    assert_eq!(grid[Line(2)][Column(9)].bg, Color::Indexed(4));
}

// --- scroll_down ---

#[test]
fn scroll_down_one_line_full_screen() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_down(1);
    // Line 0 is blank.
    assert!(grid[Line(0)][Column(0)].is_empty());
    // Line 1 has what was line 0 ('A').
    assert_eq!(grid[Line(1)][Column(0)].ch, 'A');
    // Line 2 has what was line 1 ('B').
    assert_eq!(grid[Line(2)][Column(0)].ch, 'B');
}

#[test]
fn scroll_down_sub_region_preserves_outside() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_region = 1..4;
    grid.scroll_down(1);
    // Line 0 ('A') untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    // Line 4 ('E') untouched.
    assert_eq!(grid[Line(4)][Column(0)].ch, 'E');
    // Inside region: line 1 blank, line 2 has 'B', line 3 has 'C'.
    assert!(grid[Line(1)][Column(0)].is_empty());
    assert_eq!(grid[Line(2)][Column(0)].ch, 'B');
    assert_eq!(grid[Line(3)][Column(0)].ch, 'C');
}

#[test]
fn scroll_down_count_exceeds_region() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_down(100);
    for line in 0..3 {
        assert!(grid[Line(line)][Column(0)].is_empty());
    }
}

#[test]
fn scroll_down_bce_fill() {
    let mut grid = Grid::new(3, 10);
    grid.put_char('A');
    grid.cursor_mut().template.bg = Color::Indexed(2);
    grid.scroll_down(1);
    // New top row has BCE background.
    assert_eq!(grid[Line(0)][Column(0)].bg, Color::Indexed(2));
    assert_eq!(grid[Line(0)][Column(9)].bg, Color::Indexed(2));
}

// --- insert_lines ---

#[test]
fn insert_lines_mid_region() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    // Cursor at line 2, insert 2 blank lines.
    grid.cursor_mut().set_line(2);
    grid.insert_lines(2);
    // Lines 0-1 untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[Line(1)][Column(0)].ch, 'B');
    // Lines 2-3 are blank (inserted).
    assert!(grid[Line(2)][Column(0)].is_empty());
    assert!(grid[Line(3)][Column(0)].is_empty());
    // Line 4 has what was line 2 ('C'). Lines D and E pushed off.
    assert_eq!(grid[Line(4)][Column(0)].ch, 'C');
}

#[test]
fn insert_lines_outside_region_is_noop() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_region = 1..4;
    // Cursor outside scroll region.
    grid.cursor_mut().set_line(0);
    grid.insert_lines(1);
    // Nothing changed.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[Line(1)][Column(0)].ch, 'B');
}

#[test]
fn insert_lines_count_capped() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.cursor_mut().set_line(2);
    // Insert more lines than remaining in region.
    grid.insert_lines(100);
    // Lines 0-1 untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[Line(1)][Column(0)].ch, 'B');
    // Lines 2-4 all blank.
    for line in 2..5 {
        assert!(grid[Line(line)][Column(0)].is_empty());
    }
}

#[test]
fn insert_lines_bce_fill() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.cursor_mut().set_line(1);
    grid.cursor_mut().template.bg = Color::Indexed(5);
    grid.insert_lines(1);
    // Inserted line at 1 has BCE background.
    assert_eq!(grid[Line(1)][Column(0)].bg, Color::Indexed(5));
    assert_eq!(grid[Line(1)][Column(9)].bg, Color::Indexed(5));
}

// --- delete_lines ---

#[test]
fn delete_lines_mid_region() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    // Cursor at line 1, delete 2 lines.
    grid.cursor_mut().set_line(1);
    grid.delete_lines(2);
    // Line 0 untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    // Line 1 now has what was line 3 ('D').
    assert_eq!(grid[Line(1)][Column(0)].ch, 'D');
    // Line 2 now has what was line 4 ('E').
    assert_eq!(grid[Line(2)][Column(0)].ch, 'E');
    // Lines 3-4 are blank.
    assert!(grid[Line(3)][Column(0)].is_empty());
    assert!(grid[Line(4)][Column(0)].is_empty());
}

#[test]
fn delete_lines_outside_region_is_noop() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_region = 1..4;
    // Cursor outside scroll region.
    grid.cursor_mut().set_line(4);
    grid.delete_lines(1);
    // Nothing changed.
    assert_eq!(grid[Line(4)][Column(0)].ch, 'E');
    assert_eq!(grid[Line(3)][Column(0)].ch, 'D');
}

#[test]
fn delete_lines_count_capped() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.cursor_mut().set_line(2);
    grid.delete_lines(100);
    // Lines 0-1 untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[Line(1)][Column(0)].ch, 'B');
    // Lines 2-4 all blank.
    for line in 2..5 {
        assert!(grid[Line(line)][Column(0)].is_empty());
    }
}

#[test]
fn delete_lines_bce_fill() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().template.bg = Color::Indexed(3);
    grid.delete_lines(1);
    // New bottom row has BCE background.
    assert_eq!(grid[Line(2)][Column(0)].bg, Color::Indexed(3));
    assert_eq!(grid[Line(2)][Column(9)].bg, Color::Indexed(3));
}

// --- display offset stabilization ---

#[test]
fn scroll_up_stabilizes_display_offset() {
    let mut grid = Grid::new(3, 5);
    // Push some rows into scrollback.
    for i in 0..5u8 {
        grid.cursor_mut().set_line(0);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + i) as char);
        grid.scroll_up(1);
    }
    assert_eq!(grid.scrollback().len(), 5);

    // Scroll back 3 lines into history.
    grid.scroll_display(3);
    assert_eq!(grid.display_offset(), 3);

    // New content arrives — scroll_up should bump display_offset.
    grid.scroll_up(1);
    assert_eq!(grid.display_offset(), 4);

    // Another scroll_up.
    grid.scroll_up(1);
    assert_eq!(grid.display_offset(), 5);
}

#[test]
fn scroll_up_display_offset_clamped_to_max_scrollback() {
    let mut grid = Grid::new(3, 5);
    // Fill scrollback near capacity.
    for _ in 0..9998 {
        grid.scroll_up(1);
    }

    // Scroll back to near the limit.
    grid.scroll_display(9998);
    assert_eq!(grid.display_offset(), 9998);

    // Two more scroll_ups — offset should clamp at max_scrollback (10_000).
    grid.scroll_up(1);
    assert_eq!(grid.display_offset(), 9999);
    grid.scroll_up(1);
    assert_eq!(grid.display_offset(), 10_000);
    grid.scroll_up(1);
    // Clamped — can't exceed max.
    assert_eq!(grid.display_offset(), 10_000);
}

#[test]
fn scroll_up_no_offset_change_when_at_live_view() {
    let mut grid = Grid::new(3, 5);
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(0));
    grid.put_char('A');
    grid.scroll_up(1);

    // display_offset is 0 (live view) — should stay 0.
    assert_eq!(grid.display_offset(), 0);
    grid.scroll_up(1);
    assert_eq!(grid.display_offset(), 0);
}

// --- scrollback interaction invariants ---

#[test]
fn linefeed_at_bottom_pushes_to_scrollback() {
    let mut grid = Grid::new(3, 5);
    // Write identifiable content on line 0.
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(0));
    grid.put_char('A');
    grid.put_char('B');
    grid.put_char('C');

    // Move to bottom and trigger linefeed (the real production path).
    grid.cursor_mut().set_line(2);
    grid.linefeed();

    // The evicted row should appear in scrollback.
    assert_eq!(grid.scrollback().len(), 1);
    let row = grid.scrollback().get(0).unwrap();
    assert_eq!(row[Column(0)].ch, 'A');
    assert_eq!(row[Column(1)].ch, 'B');
    assert_eq!(row[Column(2)].ch, 'C');
}

#[test]
fn delete_lines_does_not_push_to_scrollback() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }

    // Delete 2 lines at the cursor in a full-screen region.
    grid.cursor_mut().set_line(0);
    grid.delete_lines(2);

    // DL uses scroll_range_up, NOT scroll_up — no scrollback.
    assert_eq!(grid.scrollback().len(), 0);
}

#[test]
fn insert_lines_does_not_push_to_scrollback() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }

    grid.cursor_mut().set_line(0);
    grid.insert_lines(2);

    // IL uses scroll_range_down — no scrollback.
    assert_eq!(grid.scrollback().len(), 0);
}

#[test]
fn reverse_index_at_top_does_not_push_to_scrollback() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }

    // RI at top of scroll region calls scroll_down, not scroll_up.
    grid.cursor_mut().set_line(0);
    grid.reverse_index();

    assert_eq!(grid.scrollback().len(), 0);
}

// --- occ tracking after scroll operations ---

#[test]
fn scroll_up_blank_rows_have_zero_occ() {
    let mut grid = Grid::new(4, 10);
    for line in 0..4 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }

    grid.scroll_up(2);

    // Bottom 2 rows should be blank with occ=0.
    assert_eq!(grid[Line(2)].occ(), 0);
    assert_eq!(grid[Line(3)].occ(), 0);
    // Top 2 rows still have content.
    assert!(grid[Line(0)].occ() > 0);
    assert!(grid[Line(1)].occ() > 0);
}

#[test]
fn scroll_down_blank_rows_have_zero_occ() {
    let mut grid = Grid::new(4, 10);
    for line in 0..4 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }

    grid.scroll_down(2);

    // Top 2 rows should be blank with occ=0.
    assert_eq!(grid[Line(0)].occ(), 0);
    assert_eq!(grid[Line(1)].occ(), 0);
    // Bottom 2 rows still have content.
    assert!(grid[Line(2)].occ() > 0);
    assert!(grid[Line(3)].occ() > 0);
}

#[test]
fn insert_lines_blank_rows_have_zero_occ() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }

    grid.cursor_mut().set_line(1);
    grid.insert_lines(2);

    // Inserted rows at lines 1-2 should have occ=0.
    assert_eq!(grid[Line(1)].occ(), 0);
    assert_eq!(grid[Line(2)].occ(), 0);
}

#[test]
fn delete_lines_blank_rows_have_zero_occ() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }

    grid.cursor_mut().set_line(1);
    grid.delete_lines(2);

    // Bottom 2 rows (new blanks) should have occ=0.
    assert_eq!(grid[Line(3)].occ(), 0);
    assert_eq!(grid[Line(4)].occ(), 0);
}

// --- BCE background preserved in scrollback ---

#[test]
fn scroll_up_preserves_bce_background_in_scrollback() {
    let mut grid = Grid::new(3, 5);
    // Write a row with non-default background.
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().template.bg = Color::Indexed(4);
    for col in 0..5 {
        grid.cursor_mut().set_col(Column(col));
        grid.put_char('X');
    }

    grid.scroll_up(1);

    // Scrollback row should preserve the background color.
    let row = grid.scrollback().get(0).unwrap();
    for col in 0..5 {
        assert_eq!(row[Column(col)].bg, Color::Indexed(4));
        assert_eq!(row[Column(col)].ch, 'X');
    }
}

// --- cursor position unchanged after scroll ---

#[test]
fn scroll_up_does_not_move_cursor() {
    let mut grid = Grid::new(5, 10);
    grid.cursor_mut().set_line(2);
    grid.cursor_mut().set_col(Column(7));
    grid.scroll_up(1);
    assert_eq!(grid.cursor().line(), 2);
    assert_eq!(grid.cursor().col(), Column(7));
}

#[test]
fn scroll_down_does_not_move_cursor() {
    let mut grid = Grid::new(5, 10);
    grid.cursor_mut().set_line(2);
    grid.cursor_mut().set_col(Column(7));
    grid.scroll_down(1);
    assert_eq!(grid.cursor().line(), 2);
    assert_eq!(grid.cursor().col(), Column(7));
}

// --- count=0 edge case ---

#[test]
fn scroll_up_zero_is_noop() {
    let mut grid = Grid::new(3, 10);
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(0));
    grid.put_char('A');

    grid.scroll_up(0);

    // Nothing changed.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid.scrollback().len(), 0);
}

#[test]
fn scroll_down_zero_is_noop() {
    let mut grid = Grid::new(3, 10);
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(0));
    grid.put_char('A');

    grid.scroll_down(0);

    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
}

// --- accumulated scrollback via linefeed cycles ---

#[test]
fn accumulated_scrollback_via_linefeed_cycles() {
    let mut grid = Grid::new(3, 4);

    // Fill initial screen — every row has content.
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char); // A, B, C
    }

    // Simulate new lines arriving: write at bottom, linefeed scrolls.
    // This is the real production path: put_char → linefeed → scroll_up → push.
    for i in 0..4u8 {
        grid.cursor_mut().set_line(2);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'D' + i) as char); // D, E, F, G
        grid.linefeed();
    }

    // Each linefeed at bottom evicts the TOP row to scrollback:
    //   Initial:  [A, B, C]
    //   Write D@2 → [A, B, D] → evict A → [B, D, blank]
    //   Write E@2 → [B, D, E] → evict B → [D, E, blank]
    //   Write F@2 → [D, E, F] → evict D → [E, F, blank]
    //   Write G@2 → [E, F, G] → evict E → [F, G, blank]
    // Note: C was overwritten by D before reaching the top — correct
    // terminal behavior (overwritten content is lost).
    assert_eq!(grid.scrollback().len(), 4);
    assert_eq!(grid.scrollback().get(0).unwrap()[Column(0)].ch, 'E');
    assert_eq!(grid.scrollback().get(1).unwrap()[Column(0)].ch, 'D');
    assert_eq!(grid.scrollback().get(2).unwrap()[Column(0)].ch, 'B');
    assert_eq!(grid.scrollback().get(3).unwrap()[Column(0)].ch, 'A');

    // Visible grid has the remaining content.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'F');
    assert_eq!(grid[Line(1)][Column(0)].ch, 'G');
}

// --- scroll_display(0) is noop ---

#[test]
fn scroll_display_zero_is_noop() {
    let mut grid = Grid::new(3, 5);
    // Push some scrollback.
    grid.scroll_up(1);
    grid.scroll_up(1);
    grid.scroll_display(1);
    assert_eq!(grid.display_offset(), 1);

    grid.scroll_display(0);
    assert_eq!(grid.display_offset(), 1);
}

// --- dirty tracking ---

#[test]
fn scroll_display_marks_dirty_when_offset_changes() {
    let mut grid = Grid::new(3, 5);
    // Build up some scrollback.
    for _ in 0..5 {
        grid.scroll_up(1);
    }
    // Drain dirty state from scroll_up calls.
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.scroll_display(2);
    assert_eq!(grid.display_offset(), 2);
    assert!(grid.dirty().is_any_dirty());
}

#[test]
fn scroll_display_no_dirty_when_offset_unchanged() {
    let mut grid = Grid::new(3, 5);
    // No scrollback — delta=0 or clamped to 0.
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.scroll_display(0);
    assert!(!grid.dirty().is_any_dirty());

    // Already at 0, negative delta clamps to 0.
    grid.scroll_display(-5);
    assert!(!grid.dirty().is_any_dirty());
}

#[test]
fn sub_region_scroll_up_marks_only_region() {
    let mut grid = Grid::new(10, 5);
    // Drain initial dirty state.
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.scroll_region = 3..7;
    grid.scroll_up(1);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![3, 4, 5, 6]);
}

#[test]
fn sub_region_scroll_down_marks_only_region() {
    let mut grid = Grid::new(10, 5);
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.scroll_region = 2..6;
    grid.scroll_down(1);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![2, 3, 4, 5]);
}

#[test]
fn full_screen_scroll_up_marks_all_visible_lines() {
    let mut grid = Grid::new(5, 5);
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.scroll_up(1);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![0, 1, 2, 3, 4]);
}

#[test]
fn insert_lines_marks_only_affected_region() {
    let mut grid = Grid::new(10, 5);
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    // Cursor at line 4, scroll region is full screen (0..10).
    // insert_lines uses range cursor..scroll_region.end.
    grid.cursor_mut().set_line(4);
    grid.insert_lines(2);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![4, 5, 6, 7, 8, 9]);
}

#[test]
fn delete_lines_marks_only_affected_region() {
    let mut grid = Grid::new(10, 5);
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.cursor_mut().set_line(3);
    grid.delete_lines(2);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![3, 4, 5, 6, 7, 8, 9]);
}

#[test]
fn insert_lines_outside_scroll_region_no_dirty() {
    let mut grid = Grid::new(10, 5);
    grid.scroll_region = 3..7;
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    // Cursor outside scroll region — insert_lines is a noop.
    grid.cursor_mut().set_line(1);
    grid.insert_lines(2);

    assert!(!grid.dirty().is_any_dirty());
}

#[test]
fn delete_lines_outside_scroll_region_no_dirty() {
    let mut grid = Grid::new(10, 5);
    grid.scroll_region = 3..7;
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    // Cursor outside scroll region — delete_lines is a noop.
    grid.cursor_mut().set_line(8);
    grid.delete_lines(1);

    assert!(!grid.dirty().is_any_dirty());
}

#[test]
fn linefeed_at_bottom_marks_scroll_region_dirty() {
    let mut grid = Grid::new(5, 5);
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.cursor_mut().set_line(4);
    grid.linefeed();

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    // Full-screen scroll region: all lines dirty.
    assert_eq!(dirty, vec![0, 1, 2, 3, 4]);
}

#[test]
fn linefeed_at_bottom_of_sub_region_marks_only_region() {
    let mut grid = Grid::new(10, 5);
    grid.scroll_region = 3..7;
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    // Cursor at bottom of scroll region (line 6, since region is 3..7).
    grid.cursor_mut().set_line(6);
    grid.linefeed();

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![3, 4, 5, 6]);
}

#[test]
fn linefeed_in_middle_does_not_dirty() {
    let mut grid = Grid::new(10, 5);
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    // Cursor in the middle — linefeed just moves cursor down.
    grid.cursor_mut().set_line(3);
    grid.linefeed();

    assert!(!grid.dirty().is_any_dirty());
}

#[test]
fn reverse_index_at_top_marks_scroll_region_dirty() {
    let mut grid = Grid::new(5, 5);
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.cursor_mut().set_line(0);
    grid.reverse_index();

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![0, 1, 2, 3, 4]);
}

#[test]
fn reverse_index_at_top_of_sub_region_marks_only_region() {
    let mut grid = Grid::new(10, 5);
    grid.scroll_region = 2..6;
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.cursor_mut().set_line(2);
    grid.reverse_index();

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![2, 3, 4, 5]);
}

#[test]
fn reverse_index_in_middle_does_not_dirty() {
    let mut grid = Grid::new(10, 5);
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.cursor_mut().set_line(5);
    grid.reverse_index();

    assert!(!grid.dirty().is_any_dirty());
}

// --- scrollback content after mem::replace ---

#[test]
fn scroll_up_preserves_row_content_in_scrollback() {
    let mut grid = Grid::new(3, 5);
    // Write distinct content into each row.
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        for col in 0..5 {
            grid.cursor_mut().set_col(Column(col));
            grid.put_char((b'A' + line as u8) as char);
        }
    }

    // Scroll all 3 rows off.
    grid.scroll_up(3);

    assert_eq!(grid.scrollback().len(), 3);
    // Most recent = row 2 (CCC), oldest = row 0 (AAA).
    let newest = grid.scrollback().get(0).unwrap();
    let oldest = grid.scrollback().get(2).unwrap();
    for col in 0..5 {
        assert_eq!(newest[Column(col)].ch, 'C');
        assert_eq!(oldest[Column(col)].ch, 'A');
    }
}
