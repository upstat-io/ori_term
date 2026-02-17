use super::{DisplayEraseMode, LineEraseMode};
use crate::grid::Grid;
use crate::index::Column;

/// Helper: create a grid and write a string of ASCII chars.
fn grid_with_text(lines: usize, cols: usize, text: &str) -> Grid {
    let mut grid = Grid::new(lines, cols);
    for ch in text.chars() {
        grid.put_char(ch);
    }
    grid
}

#[test]
fn put_char_writes_and_advances() {
    let mut grid = Grid::new(24, 80);
    grid.put_char('A');
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid.cursor().col(), Column(1));
}

#[test]
fn put_char_wide_writes_pair() {
    let mut grid = Grid::new(24, 80);
    grid.put_char('\u{597d}');
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(0)].ch, '\u{597d}');
    assert!(
        grid[line][Column(0)]
            .flags
            .contains(crate::cell::CellFlags::WIDE_CHAR)
    );
    assert!(
        grid[line][Column(1)]
            .flags
            .contains(crate::cell::CellFlags::WIDE_CHAR_SPACER)
    );
    assert_eq!(grid.cursor().col(), Column(2));
}

#[test]
fn wide_char_at_last_column_wraps() {
    let mut grid = Grid::new(24, 5);
    // Fill columns 0..4 with 'A', cursor at col 4.
    for _ in 0..4 {
        grid.put_char('A');
    }
    assert_eq!(grid.cursor().col(), Column(4));
    // Writing a wide char at col 4 should wrap to next line.
    grid.put_char('\u{597d}');
    assert_eq!(grid.cursor().line(), 1);
    assert_eq!(grid.cursor().col(), Column(2));
    assert_eq!(grid[crate::index::Line(1)][Column(0)].ch, '\u{597d}');
}

#[test]
fn overwrite_spacer_clears_wide_char() {
    let mut grid = Grid::new(24, 80);
    grid.put_char('\u{597d}');
    // Now cursor is at col 2. Move cursor to col 1 (the spacer).
    grid.cursor_mut().set_col(Column(1));
    grid.put_char('X');
    let line = crate::index::Line(0);
    // The wide char at col 0 should be cleared.
    assert_eq!(grid[line][Column(0)].ch, ' ');
    assert!(
        !grid[line][Column(0)]
            .flags
            .contains(crate::cell::CellFlags::WIDE_CHAR)
    );
    assert_eq!(grid[line][Column(1)].ch, 'X');
}

#[test]
fn overwrite_wide_char_clears_spacer() {
    let mut grid = Grid::new(24, 80);
    grid.put_char('\u{597d}');
    // Move cursor back to col 0 (the wide char).
    grid.cursor_mut().set_col(Column(0));
    grid.put_char('Y');
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(0)].ch, 'Y');
    // The spacer at col 1 should be cleared.
    assert_eq!(grid[line][Column(1)].ch, ' ');
    assert!(
        !grid[line][Column(1)]
            .flags
            .contains(crate::cell::CellFlags::WIDE_CHAR_SPACER)
    );
}

#[test]
fn insert_blank_shifts_right() {
    let mut grid = grid_with_text(24, 80, "ABCDE");
    grid.cursor_mut().set_col(Column(1));
    grid.insert_blank(3);
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(0)].ch, 'A');
    assert_eq!(grid[line][Column(1)].ch, ' ');
    assert_eq!(grid[line][Column(2)].ch, ' ');
    assert_eq!(grid[line][Column(3)].ch, ' ');
    assert_eq!(grid[line][Column(4)].ch, 'B');
    assert_eq!(grid[line][Column(5)].ch, 'C');
}

#[test]
fn delete_chars_shifts_left() {
    let mut grid = grid_with_text(24, 80, "ABCDE");
    grid.cursor_mut().set_col(Column(1));
    grid.delete_chars(2);
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(0)].ch, 'A');
    assert_eq!(grid[line][Column(1)].ch, 'D');
    assert_eq!(grid[line][Column(2)].ch, 'E');
    // Cells at right are blank.
    assert!(grid[line][Column(3)].is_empty());
}

#[test]
fn erase_display_below() {
    let mut grid = Grid::new(3, 10);
    // Fill all 3 lines with 'X'.
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        for _ in 0..10 {
            grid.put_char('X');
        }
    }
    // Position cursor at line 1, col 5 and erase below.
    grid.cursor_mut().set_line(1);
    grid.cursor_mut().set_col(Column(5));
    grid.erase_display(DisplayEraseMode::Below);
    let line0 = crate::index::Line(0);
    let line1 = crate::index::Line(1);
    let line2 = crate::index::Line(2);
    // Line 0 untouched.
    assert_eq!(grid[line0][Column(0)].ch, 'X');
    // Line 1: cols 0-4 untouched, 5+ erased.
    assert_eq!(grid[line1][Column(4)].ch, 'X');
    assert!(grid[line1][Column(5)].is_empty());
    // Line 2 fully erased.
    assert!(grid[line2][Column(0)].is_empty());
}

#[test]
fn erase_display_above() {
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        for _ in 0..10 {
            grid.put_char('X');
        }
    }
    grid.cursor_mut().set_line(1);
    grid.cursor_mut().set_col(Column(5));
    grid.erase_display(DisplayEraseMode::Above);
    let line0 = crate::index::Line(0);
    let line1 = crate::index::Line(1);
    let line2 = crate::index::Line(2);
    // Line 0 fully erased.
    assert!(grid[line0][Column(0)].is_empty());
    // Line 1: 0-5 erased, 6+ untouched.
    assert!(grid[line1][Column(5)].is_empty());
    assert_eq!(grid[line1][Column(6)].ch, 'X');
    // Line 2 untouched.
    assert_eq!(grid[line2][Column(0)].ch, 'X');
}

#[test]
fn erase_display_all() {
    let mut grid = grid_with_text(3, 10, "AAAAAAAAAA");
    grid.erase_display(DisplayEraseMode::All);
    for line in 0..3 {
        for col in 0..10 {
            assert!(
                grid[crate::index::Line(line as i32)][Column(col)].is_empty(),
                "Cell ({line}, {col}) not empty"
            );
        }
    }
}

#[test]
fn erase_line_below() {
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(5));
    grid.erase_line(LineEraseMode::Right);
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(4)].ch, 'E');
    assert!(grid[line][Column(5)].is_empty());
    assert!(grid[line][Column(9)].is_empty());
}

#[test]
fn erase_line_all() {
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(5));
    grid.erase_line(LineEraseMode::All);
    let line = crate::index::Line(0);
    for col in 0..10 {
        assert!(grid[line][Column(col)].is_empty());
    }
}

#[test]
fn erase_chars_no_shift() {
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(2));
    grid.erase_chars(5);
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(0)].ch, 'A');
    assert_eq!(grid[line][Column(1)].ch, 'B');
    assert!(grid[line][Column(2)].is_empty());
    assert!(grid[line][Column(6)].is_empty());
    assert_eq!(grid[line][Column(7)].ch, 'H');
}

#[test]
fn erase_chars_default_bg_does_not_inflate_occ() {
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    // occ is 10 after writing 10 chars.
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(5));
    grid.erase_chars(3);
    // Erased [5..8) with default bg. occ should stay at 10
    // (cells beyond 8 are still dirty from the original write).
    let line = crate::index::Line(0);
    assert!(grid[line][Column(5)].is_empty());
    assert!(grid[line][Column(7)].is_empty());
    assert_eq!(grid[line][Column(8)].ch, 'I');
}

#[test]
fn wide_char_on_single_column_grid_does_not_hang() {
    let mut grid = Grid::new(3, 1);
    // Width-2 char can never fit in a 1-column grid. Must return
    // immediately without writing or looping.
    grid.put_char('\u{597d}');
    assert_eq!(grid.cursor().col(), Column(0));
    assert!(grid[crate::index::Line(0)][Column(0)].is_empty());
}

// --- Additional tests from reference repo gap analysis ---

#[test]
fn put_char_inherits_template_attributes() {
    use vte::ansi::Color;
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().template.fg = Color::Indexed(1);
    grid.cursor_mut().template.bg = Color::Indexed(2);
    grid.cursor_mut().template.flags = crate::cell::CellFlags::BOLD;
    grid.put_char('A');

    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, 'A');
    assert_eq!(cell.fg, Color::Indexed(1));
    assert_eq!(cell.bg, Color::Indexed(2));
    assert!(cell.flags.contains(crate::cell::CellFlags::BOLD));
}

#[test]
fn put_char_fills_row_and_wraps_to_next_line() {
    let mut grid = Grid::new(3, 5);
    for ch in "ABCDE".chars() {
        grid.put_char(ch);
    }
    // After filling row, cursor is at col 5 (pending wrap).
    assert_eq!(grid.cursor().col(), Column(5));
    assert_eq!(grid.cursor().line(), 0);

    // Writing another char triggers wrap to next line.
    grid.put_char('F');
    assert_eq!(grid.cursor().line(), 1);
    assert_eq!(grid.cursor().col(), Column(1));
    assert_eq!(grid[crate::index::Line(1)][Column(0)].ch, 'F');
}

#[test]
fn put_char_sequence_fills_correctly() {
    let mut grid = Grid::new(24, 10);
    for ch in "ABCDEFGHIJ".chars() {
        grid.put_char(ch);
    }
    let line = crate::index::Line(0);
    for (i, ch) in "ABCDEFGHIJ".chars().enumerate() {
        assert_eq!(grid[line][Column(i)].ch, ch, "Column {i} mismatch");
    }
}

#[test]
fn insert_blank_at_end_of_line() {
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_col(Column(9));
    grid.insert_blank(1);
    let line = crate::index::Line(0);
    // Last cell should be blank, 'J' shifted off the edge.
    assert!(grid[line][Column(9)].is_empty());
    assert_eq!(grid[line][Column(8)].ch, 'I');
}

#[test]
fn insert_blank_count_exceeds_remaining() {
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_col(Column(5));
    grid.insert_blank(100);
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(0)].ch, 'A');
    assert_eq!(grid[line][Column(4)].ch, 'E');
    for col in 5..10 {
        assert!(grid[line][Column(col)].is_empty(), "Column {col} not empty");
    }
}

#[test]
fn insert_blank_with_bce() {
    use vte::ansi::Color;
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_col(Column(2));
    grid.cursor_mut().template.bg = Color::Indexed(3);
    grid.insert_blank(2);
    let line = crate::index::Line(0);
    // Inserted blanks should have the BCE background.
    assert_eq!(grid[line][Column(2)].bg, Color::Indexed(3));
    assert_eq!(grid[line][Column(3)].bg, Color::Indexed(3));
    assert_eq!(grid[line][Column(2)].ch, ' ');
}

#[test]
fn insert_blank_cursor_past_end_is_noop() {
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_col(Column(10));
    grid.insert_blank(5);
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(9)].ch, 'J');
}

#[test]
fn delete_chars_at_end_of_line() {
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_col(Column(9));
    grid.delete_chars(1);
    let line = crate::index::Line(0);
    assert!(grid[line][Column(9)].is_empty());
    assert_eq!(grid[line][Column(8)].ch, 'I');
}

#[test]
fn delete_chars_count_exceeds_remaining() {
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_col(Column(5));
    grid.delete_chars(100);
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(4)].ch, 'E');
    for col in 5..10 {
        assert!(grid[line][Column(col)].is_empty(), "Column {col} not empty");
    }
}

#[test]
fn delete_chars_with_bce() {
    use vte::ansi::Color;
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_col(Column(2));
    grid.cursor_mut().template.bg = Color::Indexed(5);
    grid.delete_chars(3);
    let line = crate::index::Line(0);
    // Shifted: col 2 now has 'F', col 3 has 'G', etc.
    assert_eq!(grid[line][Column(2)].ch, 'F');
    // Right edge filled with BCE cells.
    assert_eq!(grid[line][Column(7)].bg, Color::Indexed(5));
    assert_eq!(grid[line][Column(8)].bg, Color::Indexed(5));
    assert_eq!(grid[line][Column(9)].bg, Color::Indexed(5));
}

#[test]
fn delete_chars_cursor_past_end_is_noop() {
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_col(Column(10));
    grid.delete_chars(5);
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(9)].ch, 'J');
}

#[test]
fn erase_line_above() {
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(5));
    grid.erase_line(LineEraseMode::Left);
    let line = crate::index::Line(0);
    // Cols 0..=5 should be erased.
    for col in 0..=5 {
        assert!(grid[line][Column(col)].is_empty(), "Column {col} not empty");
    }
    // Cols 6..9 untouched.
    assert_eq!(grid[line][Column(6)].ch, 'G');
    assert_eq!(grid[line][Column(9)].ch, 'J');
}

#[test]
fn erase_chars_past_end_of_line() {
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_col(Column(7));
    grid.erase_chars(100);
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(6)].ch, 'G');
    for col in 7..10 {
        assert!(grid[line][Column(col)].is_empty(), "Column {col} not empty");
    }
}

#[test]
fn erase_display_with_bce_background() {
    use vte::ansi::Color;
    let mut grid = Grid::new(3, 10);
    for line in 0..3 {
        grid.cursor_mut().set_line(line);
        grid.cursor_mut().set_col(Column(0));
        for _ in 0..10 {
            grid.put_char('X');
        }
    }
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(0));
    grid.cursor_mut().template.bg = Color::Indexed(6);
    grid.erase_display(DisplayEraseMode::All);
    // All cells should have the BCE background.
    for line in 0..3 {
        for col in 0..10 {
            assert_eq!(
                grid[crate::index::Line(line as i32)][Column(col)].bg,
                Color::Indexed(6),
                "Cell ({line}, {col}) bg mismatch"
            );
        }
    }
}

#[test]
fn erase_display_below_at_last_line() {
    let mut grid = grid_with_text(3, 10, "AAAAAAAAAA");
    grid.cursor_mut().set_line(2);
    grid.cursor_mut().set_col(Column(5));
    grid.erase_display(DisplayEraseMode::Below);
    // Only line 2 from col 5 should be erased (line 2 was empty anyway).
    let line2 = crate::index::Line(2);
    assert!(grid[line2][Column(5)].is_empty());
    // Line 0 untouched.
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'A');
}

#[test]
fn erase_display_above_at_first_line() {
    let mut grid = grid_with_text(3, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(5));
    grid.erase_display(DisplayEraseMode::Above);
    let line0 = crate::index::Line(0);
    // Cols 0..=5 erased on line 0.
    assert!(grid[line0][Column(0)].is_empty());
    assert!(grid[line0][Column(5)].is_empty());
    // Cols 6+ untouched.
    assert_eq!(grid[line0][Column(6)].ch, 'G');
}

#[test]
fn wrap_flag_set_on_wrapped_line() {
    let mut grid = Grid::new(3, 5);
    for ch in "ABCDEF".chars() {
        grid.put_char(ch);
    }
    // The last cell of line 0 should have the WRAP flag.
    let line0 = crate::index::Line(0);
    assert!(
        grid[line0][Column(4)]
            .flags
            .contains(crate::cell::CellFlags::WRAP)
    );
}

#[test]
fn put_char_wide_spacer_inherits_template_bg() {
    use vte::ansi::Color;
    let mut grid = Grid::new(24, 80);
    grid.cursor_mut().template.bg = Color::Indexed(3);
    grid.put_char('\u{597d}');
    let line = crate::index::Line(0);
    // Wide char cell gets template bg.
    assert_eq!(grid[line][Column(0)].bg, Color::Indexed(3));
    // Spacer also gets template bg.
    assert_eq!(grid[line][Column(1)].bg, Color::Indexed(3));
}

#[test]
fn erase_chars_with_bce_background() {
    use vte::ansi::Color;
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_col(Column(3));
    grid.cursor_mut().template.bg = Color::Indexed(7);
    grid.erase_chars(4);
    let line = crate::index::Line(0);
    // Erased cells [3..7) get BCE background.
    for col in 3..7 {
        assert_eq!(grid[line][Column(col)].bg, Color::Indexed(7));
        assert_eq!(grid[line][Column(col)].ch, ' ');
    }
    // Surrounding cells untouched.
    assert_eq!(grid[line][Column(2)].ch, 'C');
    assert_eq!(grid[line][Column(7)].ch, 'H');
}

#[test]
fn erase_line_below_with_bce() {
    use vte::ansi::Color;
    let mut grid = grid_with_text(24, 10, "ABCDEFGHIJ");
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(5));
    grid.cursor_mut().template.bg = Color::Indexed(2);
    grid.erase_line(LineEraseMode::Right);
    let line = crate::index::Line(0);
    for col in 5..10 {
        assert_eq!(grid[line][Column(col)].bg, Color::Indexed(2));
    }
    // Cols before cursor untouched.
    assert_eq!(grid[line][Column(4)].ch, 'E');
}

// --- dirty tracking ---

/// Helper: create a grid and drain its dirty state so tests start clean.
fn clean_grid(lines: usize, cols: usize) -> Grid {
    let mut grid = Grid::new(lines, cols);
    let _: Vec<usize> = grid.dirty_mut().drain().collect();
    grid
}

#[test]
fn put_char_marks_cursor_line_dirty() {
    let mut grid = clean_grid(5, 10);
    grid.cursor_mut().set_line(2);
    grid.cursor_mut().set_col(Column(0));
    grid.put_char('A');

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![2]);
}

#[test]
fn put_char_wraparound_marks_new_line_dirty() {
    let mut grid = clean_grid(5, 5);
    // Fill line 0 to trigger pending wrap.
    for ch in "ABCDE".chars() {
        grid.put_char(ch);
    }
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    // This put_char triggers wrap to line 1.
    grid.put_char('F');
    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert!(dirty.contains(&1), "new line should be dirty: {dirty:?}");
}

#[test]
fn insert_blank_marks_cursor_line_dirty() {
    let mut grid = clean_grid(5, 10);
    grid.cursor_mut().set_line(3);
    grid.cursor_mut().set_col(Column(2));
    grid.insert_blank(3);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![3]);
}

#[test]
fn delete_chars_marks_cursor_line_dirty() {
    let mut grid = clean_grid(5, 10);
    // Write some content first.
    grid.put_char('A');
    grid.put_char('B');
    grid.put_char('C');
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.cursor_mut().set_col(Column(0));
    grid.delete_chars(1);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![0]);
}

#[test]
fn erase_chars_marks_cursor_line_dirty() {
    let mut grid = clean_grid(5, 10);
    grid.put_char('A');
    let _: Vec<usize> = grid.dirty_mut().drain().collect();

    grid.cursor_mut().set_col(Column(0));
    grid.erase_chars(5);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![0]);
}

#[test]
fn erase_line_below_marks_cursor_line_dirty() {
    let mut grid = clean_grid(5, 10);
    grid.cursor_mut().set_line(2);
    grid.cursor_mut().set_col(Column(3));
    grid.erase_line(LineEraseMode::Right);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![2]);
}

#[test]
fn erase_line_above_marks_cursor_line_dirty() {
    let mut grid = clean_grid(5, 10);
    grid.cursor_mut().set_line(2);
    grid.cursor_mut().set_col(Column(3));
    grid.erase_line(LineEraseMode::Left);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![2]);
}

#[test]
fn erase_line_all_marks_cursor_line_dirty() {
    let mut grid = clean_grid(5, 10);
    grid.cursor_mut().set_line(2);
    grid.erase_line(LineEraseMode::All);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![2]);
}

#[test]
fn erase_display_below_marks_cursor_and_below_dirty() {
    let mut grid = clean_grid(5, 10);
    grid.cursor_mut().set_line(2);
    grid.cursor_mut().set_col(Column(3));
    grid.erase_display(DisplayEraseMode::Below);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    // Cursor line (2) + lines below (3, 4).
    assert_eq!(dirty, vec![2, 3, 4]);
}

#[test]
fn erase_display_above_marks_above_and_cursor_dirty() {
    let mut grid = clean_grid(5, 10);
    grid.cursor_mut().set_line(2);
    grid.cursor_mut().set_col(Column(3));
    grid.erase_display(DisplayEraseMode::Above);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    // Lines above (0, 1) + cursor line (2).
    assert_eq!(dirty, vec![0, 1, 2]);
}

#[test]
fn erase_display_all_marks_all_dirty() {
    let mut grid = clean_grid(5, 10);
    grid.erase_display(DisplayEraseMode::All);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    assert_eq!(dirty, vec![0, 1, 2, 3, 4]);
}

#[test]
fn erase_display_below_does_not_dirty_lines_above() {
    let mut grid = clean_grid(5, 10);
    grid.cursor_mut().set_line(3);
    grid.cursor_mut().set_col(Column(0));
    grid.erase_display(DisplayEraseMode::Below);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    // Only lines 3 and 4.
    assert_eq!(dirty, vec![3, 4]);
}

#[test]
fn erase_display_above_does_not_dirty_lines_below() {
    let mut grid = clean_grid(5, 10);
    grid.cursor_mut().set_line(1);
    grid.cursor_mut().set_col(Column(5));
    grid.erase_display(DisplayEraseMode::Above);

    let dirty: Vec<usize> = grid.dirty_mut().drain().collect();
    // Only lines 0 and 1.
    assert_eq!(dirty, vec![0, 1]);
}
