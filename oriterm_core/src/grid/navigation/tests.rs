use super::TabClearMode;
use crate::grid::Grid;
use crate::index::{Column, Line};

#[test]
fn move_up_from_line_5_to_line_2() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(5);
    grid.move_up(3);
    assert_eq!(grid.cursor().line(), 2);
}

#[test]
fn move_up_clamps_to_top() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(5);
    grid.move_up(100);
    assert_eq!(grid.cursor().line(), 0);
}

#[test]
fn move_down_from_line_0_to_line_3() {
    let mut grid = Grid::new(24, 80);
    grid.move_down(3);
    assert_eq!(grid.cursor().line(), 3);
}

#[test]
fn move_down_clamps_to_bottom() {
    let mut grid = Grid::new(24, 80);
    grid.move_down(100);
    assert_eq!(grid.cursor().line(), 23);
}

#[test]
fn move_forward_from_col_0_to_col_5() {
    let mut grid = Grid::new(24, 80);
    grid.move_forward(5);
    assert_eq!(grid.cursor().col(), Column(5));
}

#[test]
fn move_forward_clamps_to_last_column() {
    let mut grid = Grid::new(24, 80);
    grid.move_forward(100);
    assert_eq!(grid.cursor().col(), Column(79));
}

#[test]
fn move_backward_from_col_5_to_col_2() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_col(Column(5));
    grid.move_backward(3);
    assert_eq!(grid.cursor().col(), Column(2));
}

#[test]
fn move_to_sets_position() {
    let mut grid = Grid::new(24, 80);
    grid.move_to(5, Column(10));
    assert_eq!(grid.cursor().line(), 5);
    assert_eq!(grid.cursor().col(), Column(10));
}

#[test]
fn carriage_return_sets_col_zero() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_col(Column(40));
    grid.carriage_return();
    assert_eq!(grid.cursor().col(), Column(0));
}

#[test]
fn linefeed_at_bottom_triggers_scroll() {
    let mut grid = Grid::new(3, 10);
    // Write 'A' on line 0.
    grid.put_char('A');
    // Move to bottom line.
    grid.cursor_mut().set_line(2);
    grid.cursor_mut().set_col(Column(0));
    grid.put_char('Z');

    // Linefeed at bottom should scroll up.
    grid.linefeed();
    assert_eq!(grid.cursor().line(), 2);
    // Line 0 content ('A') should have scrolled off; line 0 is now
    // what was line 1 (empty).
    assert!(grid[Line(0)][Column(0)].is_empty());
    // Line 1 now has what was line 2 ('Z').
    assert_eq!(grid[Line(1)][Column(0)].ch, 'Z');
    // Line 2 is the new blank row.
    assert!(grid[Line(2)][Column(0)].is_empty());
}

#[test]
fn linefeed_in_middle_moves_down() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(5);
    grid.linefeed();
    assert_eq!(grid.cursor().line(), 6);
}

#[test]
fn reverse_index_at_top_triggers_scroll_down() {
    let mut grid = Grid::new(3, 10);
    // Write 'B' on line 0.
    grid.put_char('B');
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(0));

    // Reverse index at top should scroll down.
    grid.reverse_index();
    assert_eq!(grid.cursor().line(), 0);
    // Line 0 is now a blank row (inserted at top).
    assert!(grid[Line(0)][Column(0)].is_empty());
    // Line 1 has what was line 0 ('B').
    assert_eq!(grid[Line(1)][Column(0)].ch, 'B');
}

#[test]
fn tab_advances_to_next_stop() {
    let mut grid = Grid::new(24, 80);
    // Default stops at 0, 8, 16, 24, ...
    grid.cursor_mut().set_col(Column(1));
    grid.tab();
    assert_eq!(grid.cursor().col(), Column(8));
}

#[test]
fn tab_at_last_stop_goes_to_end() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_col(Column(72));
    grid.tab();
    // No tab stop after 72 until end of line.
    assert_eq!(grid.cursor().col(), Column(79));
}

#[test]
fn tab_backward_moves_to_previous_stop() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_col(Column(10));
    grid.tab_backward();
    assert_eq!(grid.cursor().col(), Column(8));
}

#[test]
fn set_and_clear_tab_stop() {
    let mut grid = Grid::new(24, 80);
    // Column 5 is not a default stop.
    assert!(!grid.tab_stops()[5]);

    grid.cursor_mut().set_col(Column(5));
    grid.set_tab_stop();
    assert!(grid.tab_stops()[5]);

    grid.clear_tab_stop(TabClearMode::Current);
    assert!(!grid.tab_stops()[5]);

    // Clear all.
    grid.set_tab_stop();
    grid.clear_tab_stop(TabClearMode::All);
    assert!(!grid.tab_stops()[0]); // Even default stops are cleared.
    assert!(!grid.tab_stops()[8]);
}

#[test]
fn save_and_restore_cursor_round_trip() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(10);
    grid.cursor_mut().set_col(Column(42));
    grid.save_cursor();

    // Move cursor elsewhere.
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(0));
    assert_eq!(grid.cursor().line(), 0);

    grid.restore_cursor();
    assert_eq!(grid.cursor().line(), 10);
    assert_eq!(grid.cursor().col(), Column(42));
}

// Backspace

#[test]
fn backspace_from_mid_line_moves_left() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_col(Column(10));
    grid.backspace();
    assert_eq!(grid.cursor().col(), Column(9));
}

#[test]
fn backspace_at_col_zero_is_noop() {
    let mut grid = Grid::new(24, 80);
    grid.backspace();
    assert_eq!(grid.cursor().col(), Column(0));
}

#[test]
fn backspace_from_wrap_pending_snaps_to_last_column() {
    let mut grid = Grid::new(24, 80);
    // Simulate wrap-pending: col == cols.
    grid.cursor_mut().set_col(Column(80));
    grid.backspace();
    assert_eq!(grid.cursor().col(), Column(79));
}

// Additional edge cases

#[test]
fn move_backward_clamps_to_zero() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_col(Column(3));
    grid.move_backward(100);
    assert_eq!(grid.cursor().col(), Column(0));
}

#[test]
fn move_to_clamps_out_of_bounds() {
    let mut grid = Grid::new(24, 80);
    grid.move_to(999, Column(999));
    assert_eq!(grid.cursor().line(), 23);
    assert_eq!(grid.cursor().col(), Column(79));
}

#[test]
fn move_to_column_clamps_to_last() {
    let mut grid = Grid::new(24, 80);
    grid.move_to_column(Column(999));
    assert_eq!(grid.cursor().col(), Column(79));
}

#[test]
fn move_to_line_clamps_to_last() {
    let mut grid = Grid::new(24, 80);
    grid.move_to_line(999);
    assert_eq!(grid.cursor().line(), 23);
}

#[test]
fn next_line_combines_cr_and_lf() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(5);
    grid.cursor_mut().set_col(Column(40));
    grid.next_line();
    assert_eq!(grid.cursor().line(), 6);
    assert_eq!(grid.cursor().col(), Column(0));
}

#[test]
fn linefeed_at_last_line_outside_scroll_region_is_noop() {
    let mut grid = Grid::new(5, 10);
    grid.scroll_region = 0..3;
    grid.cursor_mut().set_line(4);
    grid.linefeed();
    // Cursor at last line, outside scroll region bottom: no movement.
    assert_eq!(grid.cursor().line(), 4);
}

#[test]
fn reverse_index_in_middle_moves_up() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(5);
    grid.reverse_index();
    assert_eq!(grid.cursor().line(), 4);
}

#[test]
fn reverse_index_at_line_zero_outside_scroll_region_is_noop() {
    let mut grid = Grid::new(5, 10);
    grid.scroll_region = 2..5;
    grid.cursor_mut().set_line(0);
    grid.reverse_index();
    // Line 0 is outside scroll region; already at 0, can't go further.
    assert_eq!(grid.cursor().line(), 0);
}

#[test]
fn tab_from_col_zero() {
    let mut grid = Grid::new(24, 80);
    // Col 0 is a tab stop; next stop is at col 8.
    grid.tab();
    assert_eq!(grid.cursor().col(), Column(8));
}

#[test]
fn tab_backward_at_col_zero_stays() {
    let mut grid = Grid::new(24, 80);
    grid.tab_backward();
    assert_eq!(grid.cursor().col(), Column(0));
}

#[test]
fn tab_after_clearing_all_stops_goes_to_end() {
    let mut grid = Grid::new(24, 80);
    grid.clear_tab_stop(TabClearMode::All);
    grid.cursor_mut().set_col(Column(5));
    grid.tab();
    // No tab stops anywhere: go to last column.
    assert_eq!(grid.cursor().col(), Column(79));
}

#[test]
fn restore_cursor_without_save_resets_to_origin() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(10);
    grid.cursor_mut().set_col(Column(40));
    // No save_cursor() call.
    grid.restore_cursor();
    assert_eq!(grid.cursor().line(), 0);
    assert_eq!(grid.cursor().col(), Column(0));
}

#[test]
fn scroll_region_up_preserves_content_outside() {
    let mut grid = Grid::new(5, 10);
    // Write identifiable chars on each line.
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    // Set scroll region to lines 1..4 (middle three lines).
    grid.scroll_region = 1..4;
    grid.cursor_mut().set_line(3);
    // Linefeed at bottom of scroll region triggers scroll up.
    grid.linefeed();

    // Line 0 ('A') should be untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    // Line 4 ('E') should be untouched.
    assert_eq!(grid[Line(4)][Column(0)].ch, 'E');
    // Inside region: line 1 now has what was line 2 ('C').
    assert_eq!(grid[Line(1)][Column(0)].ch, 'C');
    // Line 2 now has what was line 3 ('D').
    assert_eq!(grid[Line(2)][Column(0)].ch, 'D');
    // Line 3 is the new blank row.
    assert!(grid[Line(3)][Column(0)].is_empty());
}

#[test]
fn scroll_region_down_preserves_content_outside() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    // Set scroll region to lines 1..4.
    grid.scroll_region = 1..4;
    grid.cursor_mut().set_line(1);
    // Reverse index at top of scroll region triggers scroll down.
    grid.reverse_index();

    // Line 0 ('A') untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    // Line 4 ('E') untouched.
    assert_eq!(grid[Line(4)][Column(0)].ch, 'E');
    // Line 1 is the new blank row (inserted at top of region).
    assert!(grid[Line(1)][Column(0)].is_empty());
    // Line 2 now has what was line 1 ('B').
    assert_eq!(grid[Line(2)][Column(0)].ch, 'B');
    // Line 3 now has what was line 2 ('C').
    assert_eq!(grid[Line(3)][Column(0)].ch, 'C');
}

#[test]
fn scroll_region_fill_uses_bce_background() {
    use vte::ansi::Color;
    let mut grid = Grid::new(3, 10);
    grid.put_char('A');
    grid.cursor_mut().set_line(2);
    grid.cursor_mut().set_col(Column(0));
    grid.cursor_mut().template.bg = Color::Indexed(4);
    // Linefeed at bottom triggers scroll up with BCE.
    grid.linefeed();
    // The new bottom row should have the cursor's bg color.
    assert_eq!(grid[Line(2)][Column(0)].bg, Color::Indexed(4));
    assert_eq!(grid[Line(2)][Column(9)].bg, Color::Indexed(4));
}

#[test]
fn move_up_clamped_to_scroll_region_top() {
    let mut grid = Grid::new(10, 80);
    grid.scroll_region = 3..8;
    grid.cursor_mut().set_line(5);
    grid.move_up(100);
    assert_eq!(grid.cursor().line(), 3);
}

#[test]
fn move_down_clamped_to_scroll_region_bottom() {
    let mut grid = Grid::new(10, 80);
    grid.scroll_region = 3..8;
    grid.cursor_mut().set_line(5);
    grid.move_down(100);
    // Clamped to scroll_region.end - 1.
    assert_eq!(grid.cursor().line(), 7);
}

#[test]
fn move_up_outside_scroll_region_clamps_to_zero() {
    let mut grid = Grid::new(10, 80);
    grid.scroll_region = 3..8;
    // Cursor outside scroll region (line 1).
    grid.cursor_mut().set_line(1);
    grid.move_up(100);
    assert_eq!(grid.cursor().line(), 0);
}

#[test]
fn move_down_outside_scroll_region_clamps_to_last() {
    let mut grid = Grid::new(10, 80);
    grid.scroll_region = 3..8;
    // Cursor outside scroll region (line 9).
    grid.cursor_mut().set_line(9);
    grid.move_down(100);
    assert_eq!(grid.cursor().line(), 9);
}

#[test]
fn cursor_only_movement_does_not_dirty() {
    let mut grid = Grid::new(24, 80);
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.move_up(3);
    grid.move_down(5);
    grid.move_forward(10);
    grid.move_backward(3);
    grid.move_to(10, Column(40));
    grid.move_to_column(Column(20));
    grid.move_to_line(5);
    grid.carriage_return();
    grid.backspace();
    grid.tab();
    grid.tab_backward();
    grid.save_cursor();
    grid.restore_cursor();

    assert!(
        !grid.dirty().is_any_dirty(),
        "cursor-only movement should not mark dirty"
    );
}

#[test]
fn save_cursor_preserves_template() {
    use vte::ansi::Color;
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(3);
    grid.cursor_mut().set_col(Column(7));
    grid.cursor_mut().template.fg = Color::Indexed(1);
    grid.cursor_mut().template.flags = crate::cell::CellFlags::BOLD;
    grid.save_cursor();

    // Change cursor state.
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().template.fg = Color::Named(vte::ansi::NamedColor::Foreground);
    grid.cursor_mut().template.flags = crate::cell::CellFlags::empty();

    grid.restore_cursor();
    assert_eq!(grid.cursor().line(), 3);
    assert_eq!(grid.cursor().col(), Column(7));
    assert_eq!(grid.cursor().template.fg, Color::Indexed(1));
    assert!(grid.cursor().template.flags.contains(crate::cell::CellFlags::BOLD));
}

// Reference repo gap analysis edge cases

#[test]
fn backspace_consecutive_moves_left_each_time() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_col(Column(5));
    grid.backspace();
    grid.backspace();
    grid.backspace();
    assert_eq!(grid.cursor().col(), Column(2));
}

#[test]
fn carriage_return_from_wrap_pending() {
    let mut grid = Grid::new(24, 80);
    // Wrap-pending: col == cols.
    grid.cursor_mut().set_col(Column(80));
    grid.carriage_return();
    assert_eq!(grid.cursor().col(), Column(0));
}

#[test]
fn linefeed_preserves_column() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(5);
    grid.cursor_mut().set_col(Column(42));
    grid.linefeed();
    assert_eq!(grid.cursor().line(), 6);
    assert_eq!(grid.cursor().col(), Column(42));
}

#[test]
fn reverse_index_preserves_column() {
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().set_line(5);
    grid.cursor_mut().set_col(Column(42));
    grid.reverse_index();
    assert_eq!(grid.cursor().line(), 4);
    assert_eq!(grid.cursor().col(), Column(42));
}

#[test]
fn tab_from_wrap_pending_snaps_to_last_column() {
    let mut grid = Grid::new(24, 80);
    // Wrap-pending: col == cols.
    grid.cursor_mut().set_col(Column(80));
    grid.tab();
    // col+1 = 81 >= 80, so no stop found; snaps to last column.
    assert_eq!(grid.cursor().col(), Column(79));
}

#[test]
fn next_line_at_bottom_of_scroll_region_scrolls() {
    let mut grid = Grid::new(5, 10);
    for line in 0..5 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        grid.put_char((b'A' + line as u8) as char);
    }
    grid.scroll_region = 1..4;
    grid.cursor_mut().set_line(3);
    grid.cursor_mut().set_col(Column(5));
    // NEL at bottom of scroll region: CR + LF (scroll).
    grid.next_line();
    assert_eq!(grid.cursor().line(), 3);
    assert_eq!(grid.cursor().col(), Column(0));
    // Region scrolled: line 1 now has what was line 2 ('C').
    assert_eq!(grid[Line(1)][Column(0)].ch, 'C');
    // Line 3 is the new blank row.
    assert!(grid[Line(3)][Column(0)].is_empty());
    // Lines outside region untouched.
    assert_eq!(grid[Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[Line(4)][Column(0)].ch, 'E');
}

#[test]
fn multiple_saves_overwrite_not_stack() {
    let mut grid = Grid::new(24, 80);
    // First save at (3, 10).
    grid.cursor_mut().set_line(3);
    grid.cursor_mut().set_col(Column(10));
    grid.save_cursor();

    // Second save at (7, 50) overwrites the first.
    grid.cursor_mut().set_line(7);
    grid.cursor_mut().set_col(Column(50));
    grid.save_cursor();

    // Move elsewhere.
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(0));

    // Restore should return to the second save, not the first.
    grid.restore_cursor();
    assert_eq!(grid.cursor().line(), 7);
    assert_eq!(grid.cursor().col(), Column(50));
}

#[test]
fn tab_backward_from_wrap_pending_snaps_to_last_stop() {
    let mut grid = Grid::new(24, 80);
    // Wrap-pending: col == cols.
    grid.cursor_mut().set_col(Column(80));
    grid.tab_backward();
    // Search backward from col 80: first stop at 72.
    assert_eq!(grid.cursor().col(), Column(72));
}
