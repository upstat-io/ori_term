//! Tests for selection types, boundaries, and text extraction.

use crate::grid::{Grid, StableRowIndex};
use crate::index::{Column, Side};

use super::boundaries::delimiter_class;
use super::*;

/// Returns true if the character is a word delimiter (not a word character).
fn is_word_delimiter(c: char) -> bool {
    delimiter_class(c) != 0
}

/// Helper to create a `StableRowIndex` from a raw value.
fn sri(n: u64) -> StableRowIndex {
    StableRowIndex(n)
}

/// Write a string into the grid at (line, col=0).
fn write_str(grid: &mut Grid, line: usize, s: &str) {
    grid.move_to(line, Column(0));
    for c in s.chars() {
        grid.put_char(c);
    }
}

// -- SelectionPoint tests --

#[test]
fn new_char_sets_anchor_pivot_end_equal() {
    let sel = Selection::new_char(sri(5), 10, Side::Left);
    assert_eq!(sel.anchor, sel.pivot);
    assert_eq!(sel.anchor, sel.end);
    assert_eq!(sel.mode, SelectionMode::Char);
}

#[test]
fn new_word_sets_distinct_anchor_and_pivot() {
    let anchor = SelectionPoint {
        row: sri(5),
        col: 3,
        side: Side::Left,
    };
    let pivot = SelectionPoint {
        row: sri(5),
        col: 7,
        side: Side::Right,
    };
    let sel = Selection::new_word(anchor, pivot);
    assert_eq!(sel.anchor, anchor);
    assert_eq!(sel.pivot, pivot);
    assert_eq!(sel.end, anchor);
    assert_eq!(sel.mode, SelectionMode::Word);
}

#[test]
fn selection_point_ordering_row_then_col_then_side() {
    let a = SelectionPoint {
        row: sri(0),
        col: 5,
        side: Side::Left,
    };
    let b = SelectionPoint {
        row: sri(0),
        col: 5,
        side: Side::Right,
    };
    let c = SelectionPoint {
        row: sri(1),
        col: 0,
        side: Side::Left,
    };
    assert!(a < b, "Left < Right at same position");
    assert!(b < c, "earlier row < later row");
    assert!(a < c, "transitivity");
}

#[test]
fn ordered_returns_min_max_regardless_of_direction() {
    // Drag backwards: end < anchor.
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(5),
            col: 10,
            side: Side::Right,
        },
        pivot: SelectionPoint {
            row: sri(5),
            col: 10,
            side: Side::Right,
        },
        end: SelectionPoint {
            row: sri(3),
            col: 2,
            side: Side::Left,
        },
    };
    let (start, end) = sel.ordered();
    assert!(start <= end);
    assert_eq!(start.row, sri(3));
    assert_eq!(end.row, sri(5));
}

#[test]
fn contains_single_row_char_mode() {
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(5),
            col: 2,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(5),
            col: 2,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(5),
            col: 8,
            side: Side::Right,
        },
    };
    assert!(!sel.contains(sri(5), 1));
    assert!(sel.contains(sri(5), 2));
    assert!(sel.contains(sri(5), 5));
    assert!(sel.contains(sri(5), 8));
    assert!(!sel.contains(sri(5), 9));
    assert!(!sel.contains(sri(4), 5));
    assert!(!sel.contains(sri(6), 5));
}

#[test]
fn contains_multi_row_char_mode() {
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(2),
            col: 5,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(2),
            col: 5,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(4),
            col: 3,
            side: Side::Right,
        },
    };
    // Row 2: col >= 5.
    assert!(!sel.contains(sri(2), 4));
    assert!(sel.contains(sri(2), 5));
    assert!(sel.contains(sri(2), 100));
    // Row 3: fully selected.
    assert!(sel.contains(sri(3), 0));
    assert!(sel.contains(sri(3), 100));
    // Row 4: col <= 3.
    assert!(sel.contains(sri(4), 0));
    assert!(sel.contains(sri(4), 3));
    assert!(!sel.contains(sri(4), 4));
}

#[test]
fn contains_respects_side_at_boundary_cells() {
    // Anchor side=Right at col 3 → effective start col = 4.
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 3,
            side: Side::Right,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 3,
            side: Side::Right,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 7,
            side: Side::Left,
        },
    };
    assert!(!sel.contains(sri(0), 3), "Right side excludes start cell");
    assert!(sel.contains(sri(0), 4));
    assert!(sel.contains(sri(0), 6));
    assert!(!sel.contains(sri(0), 7), "Left side excludes end cell");
}

#[test]
fn block_selection_contains_rectangular_bounds() {
    let sel = Selection {
        mode: SelectionMode::Block,
        anchor: SelectionPoint {
            row: sri(2),
            col: 3,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(2),
            col: 3,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(5),
            col: 7,
            side: Side::Right,
        },
    };
    assert!(sel.contains(sri(3), 5), "inside rectangle");
    assert!(!sel.contains(sri(3), 2), "left of rectangle");
    assert!(!sel.contains(sri(3), 8), "right of rectangle");
    assert!(!sel.contains(sri(1), 5), "above rectangle");
    assert!(!sel.contains(sri(6), 5), "below rectangle");
}

#[test]
fn is_empty_zero_area_char_selection() {
    let sel = Selection::new_char(sri(5), 10, Side::Left);
    assert!(sel.is_empty());
}

#[test]
fn is_empty_false_when_dragged() {
    let mut sel = Selection::new_char(sri(5), 10, Side::Left);
    sel.end.col = 12;
    assert!(!sel.is_empty());
}

#[test]
fn is_empty_false_for_word_mode() {
    let anchor = SelectionPoint {
        row: sri(0),
        col: 0,
        side: Side::Left,
    };
    let sel = Selection::new_word(anchor, anchor);
    assert!(!sel.is_empty(), "Word mode is never empty");
}

// -- Boundary tests --

#[test]
fn delimiter_class_word_char() {
    assert_eq!(delimiter_class('a'), 0);
    assert_eq!(delimiter_class('Z'), 0);
    assert_eq!(delimiter_class('5'), 0);
    assert_eq!(delimiter_class('_'), 0);
}

#[test]
fn delimiter_class_whitespace() {
    assert_eq!(delimiter_class(' '), 1);
    assert_eq!(delimiter_class('\0'), 1);
    assert_eq!(delimiter_class('\t'), 1);
}

#[test]
fn delimiter_class_punctuation() {
    assert_eq!(delimiter_class(';'), 2);
    assert_eq!(delimiter_class('('), 2);
    assert_eq!(delimiter_class('"'), 2);
    assert_eq!(delimiter_class('-'), 2);
}

#[test]
fn is_word_delimiter_matches_class() {
    assert!(!is_word_delimiter('a'));
    assert!(!is_word_delimiter('_'));
    assert!(is_word_delimiter(' '));
    assert!(is_word_delimiter(';'));
}

#[test]
fn word_boundaries_simple_words() {
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "hello world");

    // Click on 'e' (col 1): selects "hello" (cols 0-4).
    let (s, e) = word_boundaries(&grid, 0, 1);
    assert_eq!((s, e), (0, 4));

    // Click on 'w' (col 6): selects "world" (cols 6-10).
    let (s, e) = word_boundaries(&grid, 0, 6);
    assert_eq!((s, e), (6, 10));

    // Click on space (col 5): selects just the space.
    let (s, e) = word_boundaries(&grid, 0, 5);
    assert_eq!((s, e), (5, 5));
}

#[test]
fn word_boundaries_wide_char_pair() {
    // "漢字 test" = [漢, spacer, 字, spacer, ' ', t, e, s, t].
    let mut grid = Grid::new(1, 20);
    grid.move_to(0, Column(0));
    grid.put_char('漢');
    grid.put_char('字');
    grid.put_char(' ');
    for c in "test".chars() {
        grid.put_char(c);
    }

    // Click on 漢 (col 0): selects "漢字" (cols 0-3 including spacers).
    let (s, e) = word_boundaries(&grid, 0, 0);
    assert_eq!((s, e), (0, 3));

    // Click on spacer of 漢 (col 1): redirects to base cell, same result.
    let (s, e) = word_boundaries(&grid, 0, 1);
    assert_eq!((s, e), (0, 3));

    // Click on 't' (col 5): selects "test" (cols 5-8).
    let (s, e) = word_boundaries(&grid, 0, 5);
    assert_eq!((s, e), (5, 8));
}

#[test]
fn word_boundaries_single_wide_char() {
    // "漢 A" = [漢, spacer, ' ', A].
    let mut grid = Grid::new(1, 20);
    grid.move_to(0, Column(0));
    grid.put_char('漢');
    grid.put_char(' ');
    grid.put_char('A');

    let (s, e) = word_boundaries(&grid, 0, 0);
    assert_eq!((s, e), (0, 1), "wide char + spacer");
}

#[test]
fn word_boundaries_spacer_redirect() {
    // Click on wide char spacer redirects to base cell.
    let mut grid = Grid::new(1, 20);
    grid.move_to(0, Column(0));
    grid.put_char('漢');

    let (s, e) = word_boundaries(&grid, 0, 1);
    assert_eq!((s, e), (0, 1));
}

#[test]
fn logical_line_start_walks_back_through_wrap() {
    // 3 visible lines, cols=5. Write 15 chars to create wraps.
    let mut grid = Grid::new(3, 5);
    write_str(&mut grid, 0, "hello");
    // Manually set WRAP on last cell of row 0 to simulate soft-wrap.
    grid[crate::index::Line(0)][Column(4)].flags |= crate::cell::CellFlags::WRAP;
    write_str(&mut grid, 1, "world");

    // Row 1 is part of the logical line starting at row 0.
    assert_eq!(logical_line_start(&grid, 1), 0);
    // Row 0 is already the start.
    assert_eq!(logical_line_start(&grid, 0), 0);
}

#[test]
fn logical_line_end_walks_forward_through_wrap() {
    let mut grid = Grid::new(3, 5);
    write_str(&mut grid, 0, "hello");
    grid[crate::index::Line(0)][Column(4)].flags |= crate::cell::CellFlags::WRAP;
    write_str(&mut grid, 1, "world");

    // Row 0 wraps to row 1.
    assert_eq!(logical_line_end(&grid, 0), 1);
    // Row 1 doesn't wrap further.
    assert_eq!(logical_line_end(&grid, 1), 1);
}

// -- Text extraction tests --

#[test]
fn extract_text_single_row() {
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "Hello");

    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 4,
            side: Side::Right,
        },
    };
    assert_eq!(extract_text(&grid, &sel), "Hello");
}

#[test]
fn extract_text_multi_row_separated_by_newline() {
    let mut grid = Grid::new(2, 20);
    write_str(&mut grid, 0, "Hello");
    write_str(&mut grid, 1, "World");

    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(1),
            col: 4,
            side: Side::Right,
        },
    };
    assert_eq!(extract_text(&grid, &sel), "Hello\nWorld");
}

#[test]
fn extract_text_skips_wide_char_spacer() {
    let mut grid = Grid::new(1, 20);
    grid.move_to(0, Column(0));
    grid.put_char('漢');
    grid.put_char('A');

    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 2,
            side: Side::Right,
        },
    };
    assert_eq!(extract_text(&grid, &sel), "漢A");
}

#[test]
fn extract_text_includes_combining_marks() {
    let mut grid = Grid::new(1, 20);
    grid.move_to(0, Column(0));
    grid.put_char('e');
    grid.push_zerowidth('\u{0301}'); // COMBINING ACUTE ACCENT
    grid.put_char('x');

    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 1,
            side: Side::Right,
        },
    };
    assert_eq!(extract_text(&grid, &sel), "e\u{0301}x");
}

#[test]
fn extract_text_wrapped_lines_joined_without_newline() {
    let mut grid = Grid::new(2, 5);
    write_str(&mut grid, 0, "hello");
    grid[crate::index::Line(0)][Column(4)].flags |= crate::cell::CellFlags::WRAP;
    write_str(&mut grid, 1, "world");

    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(1),
            col: 4,
            side: Side::Right,
        },
    };
    assert_eq!(extract_text(&grid, &sel), "helloworld");
}

#[test]
fn extract_text_trims_trailing_spaces() {
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "Hi");
    // Cols 2-19 are default spaces.

    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 19,
            side: Side::Right,
        },
    };
    assert_eq!(extract_text(&grid, &sel), "Hi");
}

#[test]
fn extract_text_null_chars_replaced_with_space() {
    use crate::index::Line;

    let mut grid = Grid::new(1, 10);
    grid.move_to(0, Column(0));
    grid.put_char('A');
    // Manually set col 1 to '\0' to test null replacement.
    grid[Line(0)][Column(1)].ch = '\0';
    grid.move_to(0, Column(2));
    grid.put_char('B');

    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 2,
            side: Side::Right,
        },
    };
    assert_eq!(extract_text(&grid, &sel), "A B");
}

#[test]
fn extract_text_block_mode() {
    let mut grid = Grid::new(3, 20);
    write_str(&mut grid, 0, "ABCDEFGHIJ");
    write_str(&mut grid, 1, "KLMNOPQRST");
    write_str(&mut grid, 2, "UVWXYZ1234");

    let sel = Selection {
        mode: SelectionMode::Block,
        anchor: SelectionPoint {
            row: sri(0),
            col: 2,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 2,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(2),
            col: 5,
            side: Side::Right,
        },
    };
    assert_eq!(extract_text(&grid, &sel), "CDEF\nMNOP\nWXYZ");
}

// -- Edge cases from reference repos (Alacritty, WezTerm) --

#[test]
fn single_cell_left_to_right() {
    // Alacritty: single_cell_left_to_right.
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 5,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 5,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 5,
            side: Side::Right,
        },
    };
    assert!(sel.contains(sri(0), 5));
    assert!(!sel.contains(sri(0), 4));
    assert!(!sel.contains(sri(0), 6));
}

#[test]
fn single_cell_right_to_left() {
    // Alacritty: single_cell_right_to_left — reversed direction.
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 5,
            side: Side::Right,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 5,
            side: Side::Right,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 5,
            side: Side::Left,
        },
    };
    assert!(sel.contains(sri(0), 5));
}

#[test]
fn between_adjacent_cells_is_empty() {
    // Alacritty: between_adjacent_cells_left_to_right.
    // Right side of col 3 + Left side of col 4 = gap between cells = nothing.
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 3,
            side: Side::Right,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 3,
            side: Side::Right,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 4,
            side: Side::Left,
        },
    };
    // effective_start_col = 4 (Right side), effective_end_col = 3 (Left side).
    // Start > end within the same row → nothing selected at col 3 or 4.
    assert!(!sel.contains(sri(0), 3));
    assert!(!sel.contains(sri(0), 4));
}

#[test]
fn block_selection_empty_same_column_same_side() {
    // Alacritty: block_is_empty edge cases.
    let sel = Selection {
        mode: SelectionMode::Block,
        anchor: SelectionPoint {
            row: sri(0),
            col: 5,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 5,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(2),
            col: 5,
            side: Side::Left,
        },
    };
    // Block with identical columns: col 5..5 is a valid 1-column block.
    assert!(sel.contains(sri(1), 5));
    assert!(!sel.contains(sri(1), 4));
    assert!(!sel.contains(sri(1), 6));
}

// -- StableRowIndex edge cases --

#[test]
fn stable_row_index_from_visible_no_scrollback() {
    use crate::grid::StableRowIndex;

    let grid = Grid::new(24, 80);
    let s = StableRowIndex::from_visible(&grid, 0);
    assert_eq!(s, StableRowIndex(0));
    let s = StableRowIndex::from_visible(&grid, 23);
    assert_eq!(s, StableRowIndex(23));
}

#[test]
fn stable_row_index_to_absolute_evicted_returns_none() {
    use crate::grid::StableRowIndex;

    // Grid with scrollback capacity 2.
    let mut grid = Grid::with_scrollback(3, 10, 2);
    // Write enough to push 5 lines of scrollback (evicting 3).
    for i in 0..8 {
        write_str(&mut grid, 0, &format!("line{i}"));
        if i < 7 {
            grid.move_to(2, Column(0)); // Move to bottom.
            grid.scroll_up(1); // Force scroll.
        }
    }

    // Row 0 was evicted — its stable index should resolve to None.
    let evicted_sri = StableRowIndex(0);
    assert_eq!(evicted_sri.to_absolute(&grid), None);
}

#[test]
fn stable_row_index_from_visible_with_display_offset() {
    use crate::grid::StableRowIndex;

    let mut grid = Grid::with_scrollback(3, 10, 10);
    // Push 3 rows into scrollback.
    for _ in 0..3 {
        write_str(&mut grid, 2, "text");
        grid.scroll_up(1);
    }
    assert_eq!(grid.scrollback().len(), 3);

    // Without display_offset: visible line 0 = absolute 3.
    let s = StableRowIndex::from_visible(&grid, 0);
    assert_eq!(s, StableRowIndex(3));

    // Scroll back into history by 2 lines.
    grid.scroll_display(2);
    let s = StableRowIndex::from_visible(&grid, 0);
    // Now visible line 0 = absolute 1 (scrollback index 1).
    assert_eq!(s, StableRowIndex(1));
}

#[test]
fn grid_reset_increments_total_evicted() {
    let mut grid = Grid::with_scrollback(3, 10, 10);
    // Push 5 rows into scrollback.
    for _ in 0..5 {
        write_str(&mut grid, 2, "text");
        grid.scroll_up(1);
    }
    assert_eq!(grid.scrollback().len(), 5);
    assert_eq!(grid.total_evicted(), 0);

    grid.reset();
    // After reset, those 5 scrollback rows are evicted.
    assert_eq!(grid.total_evicted(), 5);
    assert_eq!(grid.scrollback().len(), 0);
}

// -- Boundary edge cases --

#[test]
fn word_boundaries_empty_row() {
    // All spaces — space is class 1, so each space is its own "word".
    let grid = Grid::new(1, 10);
    let (s, e) = word_boundaries(&grid, 0, 5);
    // All default cells are ' '. Space is class 1. All same class → entire row.
    assert_eq!(s, 0);
    assert_eq!(e, 9);
}

#[test]
fn word_boundaries_all_punctuation() {
    let mut grid = Grid::new(1, 10);
    write_str(&mut grid, 0, ";;--;;");
    let (s, e) = word_boundaries(&grid, 0, 2);
    // '-' and ';' are both class 2. But char_class checks exact class match.
    // '--' at cols 2-3 vs ';;' at cols 0-1: all are class 2, so they group.
    assert_eq!(s, 0);
    assert_eq!(e, 5);
}

#[test]
fn word_boundaries_out_of_bounds_col() {
    let grid = Grid::new(1, 10);
    let (s, e) = word_boundaries(&grid, 0, 100);
    assert_eq!((s, e), (100, 100));
}

#[test]
fn word_boundaries_out_of_bounds_row() {
    let grid = Grid::new(1, 10);
    let (s, e) = word_boundaries(&grid, 99, 5);
    assert_eq!((s, e), (5, 5));
}

#[test]
fn delimiter_class_tab_is_whitespace() {
    assert_eq!(delimiter_class('\t'), 1);
}

// -- Text extraction edge cases --

#[test]
fn extract_text_empty_selection() {
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "Hello");

    let sel = Selection::new_char(sri(0), 5, Side::Left);
    assert!(sel.is_empty());
    // Even though is_empty() is true, extract_text still works (extracts 0 cols).
    let text = extract_text(&grid, &sel);
    // Anchor = end = (0, 5, Left). effective_start = 5, effective_end = 4.
    // Start > end within same row → empty result.
    assert!(text.is_empty() || text.trim().is_empty());
}

#[test]
fn extract_text_evicted_rows_returns_empty() {
    use crate::grid::StableRowIndex;

    let grid = Grid::new(3, 10);
    // Reference a row that was "evicted" (stable index points before current grid).
    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: StableRowIndex(999),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: StableRowIndex(999),
            col: 0,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: StableRowIndex(1000),
            col: 5,
            side: Side::Right,
        },
    };
    assert_eq!(extract_text(&grid, &sel), "");
}

#[test]
fn extract_text_reverse_selection_same_as_forward() {
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "Hello World");

    let forward = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 4,
            side: Side::Right,
        },
    };
    let reverse = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 4,
            side: Side::Right,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 4,
            side: Side::Right,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
    };
    assert_eq!(extract_text(&grid, &forward), extract_text(&grid, &reverse));
}

#[test]
fn extract_text_single_cell_selection() {
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "ABCDE");

    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(0),
            col: 2,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 2,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 2,
            side: Side::Right,
        },
    };
    assert_eq!(extract_text(&grid, &sel), "C");
}
