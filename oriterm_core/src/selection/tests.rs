//! Tests for selection types, boundaries, and text extraction.

use crate::grid::{Grid, StableRowIndex};
use crate::index::{Column, Side};

use super::boundaries::{DEFAULT_WORD_DELIMITERS, delimiter_class};
use super::*;

/// Returns true if the character is a word delimiter (not a word character).
fn is_word_delimiter(c: char) -> bool {
    delimiter_class(c, DEFAULT_WORD_DELIMITERS) != 0
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
    assert_eq!(delimiter_class('a', DEFAULT_WORD_DELIMITERS), 0);
    assert_eq!(delimiter_class('Z', DEFAULT_WORD_DELIMITERS), 0);
    assert_eq!(delimiter_class('5', DEFAULT_WORD_DELIMITERS), 0);
    assert_eq!(delimiter_class('_', DEFAULT_WORD_DELIMITERS), 0);
    // `-` is not in DEFAULT_WORD_DELIMITERS → word char (matches Alacritty).
    assert_eq!(delimiter_class('-', DEFAULT_WORD_DELIMITERS), 0);
}

#[test]
fn delimiter_class_whitespace() {
    assert_eq!(delimiter_class(' ', DEFAULT_WORD_DELIMITERS), 1);
    assert_eq!(delimiter_class('\0', DEFAULT_WORD_DELIMITERS), 1);
    assert_eq!(delimiter_class('\t', DEFAULT_WORD_DELIMITERS), 1);
}

#[test]
fn delimiter_class_punctuation() {
    // Characters in DEFAULT_WORD_DELIMITERS are class 2.
    assert_eq!(delimiter_class('(', DEFAULT_WORD_DELIMITERS), 2);
    assert_eq!(delimiter_class('"', DEFAULT_WORD_DELIMITERS), 2);
    assert_eq!(delimiter_class(',', DEFAULT_WORD_DELIMITERS), 2);
    assert_eq!(delimiter_class('|', DEFAULT_WORD_DELIMITERS), 2);
}

#[test]
fn is_word_delimiter_matches_class() {
    assert!(!is_word_delimiter('a'));
    assert!(!is_word_delimiter('_'));
    assert!(is_word_delimiter(' '));
    // `;` is NOT in DEFAULT_WORD_DELIMITERS → word char.
    assert!(!is_word_delimiter(';'));
    // `(` IS in DEFAULT_WORD_DELIMITERS → delimiter.
    assert!(is_word_delimiter('('));
}

#[test]
fn word_boundaries_simple_words() {
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "hello world");

    // Click on 'e' (col 1): selects "hello" (cols 0-4).
    let (s, e) = word_boundaries(&grid, 0, 1, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (0, 4));

    // Click on 'w' (col 6): selects "world" (cols 6-10).
    let (s, e) = word_boundaries(&grid, 0, 6, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (6, 10));

    // Click on space (col 5): selects just the space.
    let (s, e) = word_boundaries(&grid, 0, 5, DEFAULT_WORD_DELIMITERS);
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
    let (s, e) = word_boundaries(&grid, 0, 0, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (0, 3));

    // Click on spacer of 漢 (col 1): redirects to base cell, same result.
    let (s, e) = word_boundaries(&grid, 0, 1, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (0, 3));

    // Click on 't' (col 5): selects "test" (cols 5-8).
    let (s, e) = word_boundaries(&grid, 0, 5, DEFAULT_WORD_DELIMITERS);
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

    let (s, e) = word_boundaries(&grid, 0, 0, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (0, 1), "wide char + spacer");
}

#[test]
fn word_boundaries_spacer_redirect() {
    // Click on wide char spacer redirects to base cell.
    let mut grid = Grid::new(1, 20);
    grid.move_to(0, Column(0));
    grid.put_char('漢');

    let (s, e) = word_boundaries(&grid, 0, 1, DEFAULT_WORD_DELIMITERS);
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
    let (s, e) = word_boundaries(&grid, 0, 5, DEFAULT_WORD_DELIMITERS);
    // All default cells are ' '. Space is class 1. All same class → entire row.
    assert_eq!(s, 0);
    assert_eq!(e, 9);
}

#[test]
fn word_boundaries_all_punctuation() {
    let mut grid = Grid::new(1, 10);
    // `;` and `-` are NOT in DEFAULT_WORD_DELIMITERS, so they're all class 0.
    write_str(&mut grid, 0, ";;--;;");
    let (s, e) = word_boundaries(&grid, 0, 2, DEFAULT_WORD_DELIMITERS);
    assert_eq!(s, 0);
    assert_eq!(e, 5);
}

#[test]
fn word_boundaries_delimiter_chars() {
    // Characters IN the delimiter set stop word expansion.
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "hello(world)");
    let (s, e) = word_boundaries(&grid, 0, 1, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (0, 4), "'(' in delimiters stops at 'hello'");
}

#[test]
fn word_boundaries_custom_delimiters() {
    // Custom delimiter set: `-` breaks words.
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "hello-world");
    let (s, e) = word_boundaries(&grid, 0, 1, "-");
    assert_eq!(
        (s, e),
        (0, 4),
        "hyphen in custom delimiters stops at 'hello'"
    );
    // With default delimiters, `-` does NOT break words.
    let (s, e) = word_boundaries(&grid, 0, 1, DEFAULT_WORD_DELIMITERS);
    assert_eq!(
        (s, e),
        (0, 10),
        "hyphen not in defaults: 'hello-world' is one word"
    );
}

#[test]
fn word_boundaries_out_of_bounds_col() {
    let grid = Grid::new(1, 10);
    let (s, e) = word_boundaries(&grid, 0, 100, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (100, 100));
}

#[test]
fn word_boundaries_out_of_bounds_row() {
    let grid = Grid::new(1, 10);
    let (s, e) = word_boundaries(&grid, 99, 5, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (5, 5));
}

#[test]
fn delimiter_class_tab() {
    assert_eq!(delimiter_class('\t', DEFAULT_WORD_DELIMITERS), 1);
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

// -- Line mode tests (missing from original coverage) --

#[test]
fn new_line_sets_mode_and_points() {
    let anchor = SelectionPoint {
        row: sri(0),
        col: 0,
        side: Side::Left,
    };
    let pivot = SelectionPoint {
        row: sri(0),
        col: 79,
        side: Side::Right,
    };
    let sel = Selection::new_line(anchor, pivot);
    assert_eq!(sel.mode, SelectionMode::Line);
    assert_eq!(sel.anchor, anchor);
    assert_eq!(sel.pivot, pivot);
    assert_eq!(sel.end, anchor);
}

#[test]
fn line_mode_contains_all_columns_on_selected_rows() {
    // Line mode selects full rows. Anchor at col 0, pivot at col max.
    let sel = Selection {
        mode: SelectionMode::Line,
        anchor: SelectionPoint {
            row: sri(1),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(1),
            col: 79,
            side: Side::Right,
        },
        end: SelectionPoint {
            row: sri(3),
            col: 0,
            side: Side::Left,
        },
    };
    // Row 1: all columns selected (anchor row, col 0..79).
    assert!(sel.contains(sri(1), 0));
    assert!(sel.contains(sri(1), 40));
    assert!(sel.contains(sri(1), 79));
    // Row 2: fully interior, all columns selected.
    assert!(sel.contains(sri(2), 0));
    assert!(sel.contains(sri(2), 100));
    // Row 3: end row, but since anchor has col=0 Side::Left, effective_start is 0.
    // The end point is (3, 0, Left). In ordered(), min is (1,0,Left), max is (1,79,Right).
    // Wait — ordered() takes min/max of anchor, pivot, end. pivot=(1,79,Right) > end=(3,0,Left)?
    // (3,0,Left) > (1,79,Right) because row 3 > row 1. So max = (3,0,Left).
    // effective_end_col for (3, 0, Left) with col > 0 check: col is 0 so returns 0.
    // So row 3: col 0 is included but col 1 is not. Hmm, that's because end is at col 0.
    // In a real line selection, the caller would set end to (3, 79, Right) for full row.
    // Let's test with properly constructed line boundaries.
    assert!(sel.contains(sri(3), 0));
    assert!(!sel.contains(sri(0), 0), "row before selection");
    assert!(!sel.contains(sri(4), 0), "row after selection");
}

#[test]
fn line_mode_contains_full_rows_with_proper_boundaries() {
    // Simulates triple-click on row 1, then drag down to row 3.
    // Caller sets line boundaries: anchor at line start, pivot at line end,
    // end at the end of the target line.
    let sel = Selection {
        mode: SelectionMode::Line,
        anchor: SelectionPoint {
            row: sri(1),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(1),
            col: 79,
            side: Side::Right,
        },
        end: SelectionPoint {
            row: sri(3),
            col: 79,
            side: Side::Right,
        },
    };
    // All three rows fully selected.
    for row in 1..=3 {
        assert!(sel.contains(sri(row), 0), "row {row} col 0");
        assert!(sel.contains(sri(row), 40), "row {row} col 40");
        assert!(sel.contains(sri(row), 79), "row {row} col 79");
    }
    assert!(!sel.contains(sri(0), 0));
    assert!(!sel.contains(sri(4), 0));
}

#[test]
fn line_mode_extract_text_full_rows() {
    let mut grid = Grid::new(3, 10);
    write_str(&mut grid, 0, "AAAAAAAAAA");
    write_str(&mut grid, 1, "BBBBBBBBBB");
    write_str(&mut grid, 2, "CCCCCCCCCC");

    // Select rows 0-1 as full lines.
    let sel = Selection {
        mode: SelectionMode::Line,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 9,
            side: Side::Right,
        },
        end: SelectionPoint {
            row: sri(1),
            col: 9,
            side: Side::Right,
        },
    };
    assert_eq!(extract_text(&grid, &sel), "AAAAAAAAAA\nBBBBBBBBBB");
}

#[test]
fn line_mode_extract_text_with_wrapped_lines() {
    let mut grid = Grid::new(3, 5);
    write_str(&mut grid, 0, "hello");
    grid[crate::index::Line(0)][Column(4)].flags |= crate::cell::CellFlags::WRAP;
    write_str(&mut grid, 1, "world");
    write_str(&mut grid, 2, "!!!!!");

    // Line selection spanning rows 0-1 (which are one logical line).
    let sel = Selection {
        mode: SelectionMode::Line,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(1),
            col: 4,
            side: Side::Right,
        },
        end: SelectionPoint {
            row: sri(1),
            col: 4,
            side: Side::Right,
        },
    };
    // Wrapped lines should be joined without newline.
    assert_eq!(extract_text(&grid, &sel), "helloworld");
}

// -- Drag workflow tests --

#[test]
fn char_drag_extends_selection() {
    // Create selection, move end, verify containment changes.
    let mut sel = Selection::new_char(sri(5), 10, Side::Left);
    assert!(sel.is_empty(), "no drag yet");
    assert!(!sel.contains(sri(5), 11));

    // Simulate drag to col 15.
    sel.end = SelectionPoint {
        row: sri(5),
        col: 15,
        side: Side::Right,
    };
    assert!(!sel.is_empty());
    assert!(sel.contains(sri(5), 10));
    assert!(sel.contains(sri(5), 12));
    assert!(sel.contains(sri(5), 15));
    assert!(!sel.contains(sri(5), 16));

    // Drag backwards to col 3.
    sel.end = SelectionPoint {
        row: sri(5),
        col: 3,
        side: Side::Left,
    };
    // ordered(): min = end (3, Left), max = anchor (10, Left).
    // effective_start = 3, effective_end = 9 (Left side on max → col - 1).
    assert!(sel.contains(sri(5), 3));
    assert!(sel.contains(sri(5), 9));
    assert!(
        !sel.contains(sri(5), 10),
        "anchor side was Left — as end, effective_end_col = 9"
    );
}

#[test]
fn char_drag_across_rows() {
    let mut sel = Selection::new_char(sri(2), 5, Side::Left);

    // Drag down to row 4, col 10.
    sel.end = SelectionPoint {
        row: sri(4),
        col: 10,
        side: Side::Right,
    };
    assert!(sel.contains(sri(2), 5));
    assert!(sel.contains(sri(3), 0), "middle row fully included");
    assert!(sel.contains(sri(4), 10));
    assert!(!sel.contains(sri(4), 11));
    assert!(!sel.contains(sri(1), 0), "above selection");
}

#[test]
fn word_mode_drag_extends_by_pivot() {
    // Word selection: double-click on "hello" → anchor=(0,0,L), pivot=(0,4,R).
    // The pivot ensures that even when dragging backwards, "hello" stays selected.
    let sel = Selection {
        mode: SelectionMode::Word,
        anchor: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 4,
            side: Side::Right,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 10,
            side: Side::Right,
        },
    };
    // ordered() → min of (anchor, pivot, end) = anchor, max = end.
    // So selection spans col 0..10.
    assert!(sel.contains(sri(0), 0));
    assert!(sel.contains(sri(0), 4), "pivot word still included");
    assert!(sel.contains(sri(0), 10));
    assert!(!sel.contains(sri(0), 11));
}

#[test]
fn word_mode_drag_backwards_preserves_initial_word() {
    // Double-click on word at cols 5-9 ("world"), then drag backwards to col 0.
    // The pivot at (0,9,R) ensures "world" stays selected even when end < anchor.
    let sel = Selection {
        mode: SelectionMode::Word,
        anchor: SelectionPoint {
            row: sri(0),
            col: 5,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 9,
            side: Side::Right,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 0,
            side: Side::Left,
        },
    };
    // ordered() → min = end (0,0,L), max = pivot (0,9,R).
    assert!(sel.contains(sri(0), 0), "dragged-to area");
    assert!(sel.contains(sri(0), 5), "original word start");
    assert!(sel.contains(sri(0), 9), "original word end");
    assert!(!sel.contains(sri(0), 10));
}

// -- Emoji / multi-codepoint text extraction --

#[test]
fn extract_text_emoji_wide_char() {
    // Skull emoji (💀 U+1F480) has display width 2.
    let mut grid = Grid::new(1, 20);
    grid.move_to(0, Column(0));
    grid.put_char('💀');
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
    assert_eq!(extract_text(&grid, &sel), "💀A");
}

#[test]
fn word_boundaries_emoji() {
    // Emoji are not in DEFAULT_WORD_DELIMITERS → treated as word characters.
    // So 💀A is one word (emoji + A both class 0).
    let mut grid = Grid::new(1, 20);
    grid.move_to(0, Column(0));
    grid.put_char('💀');
    grid.put_char('A');

    // Click on emoji at col 0: both emoji and 'A' are class 0 → one word.
    let (s, e) = word_boundaries(&grid, 0, 0, DEFAULT_WORD_DELIMITERS);
    assert_eq!(s, 0);
    assert_eq!(e, 2, "emoji + A is one word");
}

// -- Selection across scrollback + visible boundary --

#[test]
fn extract_text_spanning_scrollback_and_visible() {
    let mut grid = Grid::with_scrollback(3, 10, 10);
    // Write content that gets pushed to scrollback.
    write_str(&mut grid, 0, "scrolled");
    write_str(&mut grid, 1, "also_scr");
    write_str(&mut grid, 2, "bottom");
    grid.scroll_up(2);
    // Now rows 0-1 are in scrollback, visible area has been shifted.
    // Write new content in visible area.
    write_str(&mut grid, 0, "visible0");
    write_str(&mut grid, 1, "visible1");

    // Select from scrollback row 0 through visible row 0.
    // Scrollback has 2 rows, so absolute:
    //   scrollback[0] = "scrolled" (StableRowIndex 0)
    //   scrollback[1] = "also_scr" (StableRowIndex 1)
    //   visible[0] = "visible0" (StableRowIndex 2)
    //   visible[1] = "visible1" (StableRowIndex 3)
    //   visible[2] = "bottom" (StableRowIndex 4) — wait, scroll_up shifts content.
    // Let's verify by selecting first two stable rows.
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
            col: 7,
            side: Side::Right,
        },
    };
    let text = extract_text(&grid, &sel);
    assert_eq!(text, "scrolled\nalso_scr");
}

// -- Block mode with uneven row content --

#[test]
fn block_mode_extract_text_short_rows() {
    // Block selection where some rows have content shorter than the block range.
    let mut grid = Grid::new(3, 20);
    write_str(&mut grid, 0, "ABCDEFGHIJ"); // 10 chars
    write_str(&mut grid, 1, "KL"); // 2 chars, rest are spaces
    write_str(&mut grid, 2, "UVWXYZ1234"); // 10 chars

    // Block select cols 2-7 across all 3 rows.
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
            col: 7,
            side: Side::Right,
        },
    };
    // Row 0: CDEFGH, Row 1: cols 2-7 are spaces (trimmed), Row 2: WXYZ12.
    let text = extract_text(&grid, &sel);
    assert_eq!(text, "CDEFGH\n\nWXYZ12");
}

// -- SelectionBounds direct tests --

#[test]
fn bounds_precomputed_matches_contains() {
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
            col: 10,
            side: Side::Right,
        },
    };
    let bounds = sel.bounds();
    // Verify bounds match direct contains for several points.
    for row in 0..6 {
        for col in [0, 5, 10, 15] {
            assert_eq!(
                sel.contains(sri(row), col),
                bounds.contains(sri(row), col),
                "mismatch at row={row}, col={col}"
            );
        }
    }
}

#[test]
fn bounds_block_mode_precomputed() {
    let sel = Selection {
        mode: SelectionMode::Block,
        anchor: SelectionPoint {
            row: sri(1),
            col: 3,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(1),
            col: 3,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(4),
            col: 8,
            side: Side::Right,
        },
    };
    let bounds = sel.bounds();
    assert_eq!(bounds.mode, SelectionMode::Block);
    assert_eq!(bounds.start.row, sri(1));
    assert_eq!(bounds.end.row, sri(4));
    // Interior point.
    assert!(bounds.contains(sri(2), 5));
    // Outside.
    assert!(!bounds.contains(sri(2), 2));
    assert!(!bounds.contains(sri(2), 9));
}

// -- Word boundaries at line edges --

#[test]
fn word_boundaries_word_at_col_zero() {
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "hello world");

    let (s, e) = word_boundaries(&grid, 0, 0, DEFAULT_WORD_DELIMITERS);
    assert_eq!(s, 0, "word starts at col 0");
    assert_eq!(e, 4);
}

#[test]
fn word_boundaries_word_at_last_col() {
    let mut grid = Grid::new(1, 10);
    // "ABC  hello" — "hello" ends at col 9 (last col).
    write_str(&mut grid, 0, "ABC  hello");

    let (s, e) = word_boundaries(&grid, 0, 9, DEFAULT_WORD_DELIMITERS);
    assert_eq!(s, 5);
    assert_eq!(e, 9, "word ends at last column");
}

// -- delimiter_class for Unicode --

#[test]
fn delimiter_class_cjk_is_word_char() {
    // CJK ideographs are alphanumeric per char::is_alphanumeric().
    assert_eq!(
        delimiter_class('漢', DEFAULT_WORD_DELIMITERS),
        0,
        "CJK should be word class"
    );
    assert_eq!(delimiter_class('字', DEFAULT_WORD_DELIMITERS), 0);
    assert_eq!(delimiter_class('好', DEFAULT_WORD_DELIMITERS), 0);
}

#[test]
fn delimiter_class_emoji_is_word_char() {
    // Emoji are not in DEFAULT_WORD_DELIMITERS → treated as word characters.
    assert_eq!(delimiter_class('💀', DEFAULT_WORD_DELIMITERS), 0);
    assert_eq!(delimiter_class('🎉', DEFAULT_WORD_DELIMITERS), 0);
}

#[test]
fn delimiter_class_unicode_letters() {
    // Non-ASCII alphabetic characters should be word class.
    assert_eq!(
        delimiter_class('é', DEFAULT_WORD_DELIMITERS),
        0,
        "accented latin"
    );
    assert_eq!(
        delimiter_class('ñ', DEFAULT_WORD_DELIMITERS),
        0,
        "Spanish ñ"
    );
    assert_eq!(
        delimiter_class('Ω', DEFAULT_WORD_DELIMITERS),
        0,
        "Greek omega"
    );
    assert_eq!(delimiter_class('д', DEFAULT_WORD_DELIMITERS), 0, "Cyrillic");
}

// -- Wrapped line word boundary (ref: WezTerm double_click_wrapped_selection) --

#[test]
fn word_boundaries_wrapped_line_stays_within_row() {
    // word_boundaries operates on a single row, so a word that spans
    // a soft-wrapped boundary is not detected as a single word.
    let mut grid = Grid::new(2, 5);
    write_str(&mut grid, 0, "hello");
    grid[crate::index::Line(0)][Column(4)].flags |= crate::cell::CellFlags::WRAP;
    write_str(&mut grid, 1, "world");

    // Click on 'e' in row 0: selects "hello" (cols 0-4), not "helloworld".
    let (s, e) = word_boundaries(&grid, 0, 1, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (0, 4));

    // Click on 'o' in row 1: selects "world" (cols 0-4).
    let (s, e) = word_boundaries(&grid, 1, 1, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (0, 4));
}

// -- Mixed punctuation clusters at word boundaries --

#[test]
fn word_boundaries_possessive_apostrophe() {
    // Apostrophe in DEFAULT_WORD_DELIMITERS: "test's" → "test" then "'" then "s".
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "test's value");
    let (s, e) = word_boundaries(&grid, 0, 1, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (0, 3), "apostrophe in delimiters stops at 'test'");
}

#[test]
fn word_boundaries_cpp_plus_plus() {
    // '+' is not in DEFAULT_WORD_DELIMITERS, so "C++" is one word.
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "C++ code");
    let (s, e) = word_boundaries(&grid, 0, 0, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (0, 2), "'C++' is one word ('+' not a delimiter)");
}

#[test]
fn word_boundaries_ellipsis_prefix() {
    // '.' is not in DEFAULT_WORD_DELIMITERS, so "...word" is one word.
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "...word...");
    let (s, e) = word_boundaries(&grid, 0, 4, DEFAULT_WORD_DELIMITERS);
    assert_eq!(
        (s, e),
        (0, 9),
        "'...word...' is one word ('.' not a delimiter)"
    );
}

#[test]
fn word_boundaries_parenthesized_word() {
    // '(' and ')' ARE in DEFAULT_WORD_DELIMITERS.
    let mut grid = Grid::new(1, 20);
    write_str(&mut grid, 0, "(hello) world");
    // Click on 'h' at col 1: stops at '(' and ')'.
    let (s, e) = word_boundaries(&grid, 0, 1, DEFAULT_WORD_DELIMITERS);
    assert_eq!((s, e), (1, 5), "parens in delimiters isolate 'hello'");
}

// -- Out-of-bounds selection (ref: WezTerm drag_selection beyond grid) --

#[test]
fn extract_text_selection_col_beyond_grid_width() {
    let mut grid = Grid::new(1, 10);
    write_str(&mut grid, 0, "Hello");

    // Selection end col (100) far exceeds grid width (10).
    // extract_text should clamp to grid width and not panic.
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
            col: 100,
            side: Side::Right,
        },
    };
    assert_eq!(extract_text(&grid, &sel), "Hello");
}

// -- Scrollback with display offset (ref: WezTerm selection_in_scrollback) --

#[test]
fn extract_text_scrollback_with_display_offset() {
    let mut grid = Grid::with_scrollback(3, 10, 10);
    write_str(&mut grid, 0, "row_A");
    write_str(&mut grid, 1, "row_B");
    write_str(&mut grid, 2, "row_C");
    grid.scroll_up(2);
    // scrollback: [row_A, row_B], visible: [row_C→overwritten, blank, blank]
    write_str(&mut grid, 0, "row_D");
    write_str(&mut grid, 1, "row_E");
    // scrollback: [row_A (SRI 0), row_B (SRI 1)]
    // visible:    [row_D (SRI 2), row_E (SRI 3), blank (SRI 4)]

    // Scroll viewport back 1 line into history.
    grid.scroll_display(1);
    // Viewport: line 0 = row_B, line 1 = row_D, line 2 = row_E.

    let start = StableRowIndex::from_visible(&grid, 0);
    let end = StableRowIndex::from_visible(&grid, 2);

    let sel = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: start,
            col: 0,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: start,
            col: 0,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: end,
            col: 9,
            side: Side::Right,
        },
    };
    let text = extract_text(&grid, &sel);
    assert_eq!(text, "row_B\nrow_D\nrow_E");
}

// -- Reverse multi-row drag (ref: WezTerm drag_select reverse) --

#[test]
fn extract_text_reverse_multi_row_same_as_forward() {
    let mut grid = Grid::new(3, 10);
    write_str(&mut grid, 0, "line_one");
    write_str(&mut grid, 1, "line_two");
    write_str(&mut grid, 2, "line_tre");

    let forward = Selection {
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
            row: sri(2),
            col: 5,
            side: Side::Right,
        },
    };
    let reverse = Selection {
        mode: SelectionMode::Char,
        anchor: SelectionPoint {
            row: sri(2),
            col: 5,
            side: Side::Right,
        },
        pivot: SelectionPoint {
            row: sri(2),
            col: 5,
            side: Side::Right,
        },
        end: SelectionPoint {
            row: sri(0),
            col: 2,
            side: Side::Left,
        },
    };
    let fwd_text = extract_text(&grid, &forward);
    let rev_text = extract_text(&grid, &reverse);
    assert_eq!(fwd_text, rev_text, "reverse drag should produce same text");
    assert_eq!(fwd_text, "ne_one\nline_two\nline_t");
}

// -- Selection expand then contract (ref: Alacritty selection_bigger_then_smaller) --

#[test]
fn selection_expand_then_contract_updates_containment() {
    let mut sel = Selection::new_char(sri(5), 10, Side::Left);

    // Expand to col 20.
    sel.end = SelectionPoint {
        row: sri(5),
        col: 20,
        side: Side::Right,
    };
    assert!(sel.contains(sri(5), 15), "expanded covers col 15");
    assert!(sel.contains(sri(5), 20), "expanded covers col 20");
    assert!(!sel.contains(sri(5), 21));

    // Contract back to col 8 (before anchor at 10).
    sel.end = SelectionPoint {
        row: sri(5),
        col: 8,
        side: Side::Left,
    };
    assert!(
        !sel.contains(sri(5), 15),
        "contracted no longer covers col 15"
    );
    assert!(
        !sel.contains(sri(5), 20),
        "contracted no longer covers col 20"
    );
    // ordered: min=(5,8,Left), max=(5,10,Left). effective_end_col = 9.
    assert!(sel.contains(sri(5), 8));
    assert!(sel.contains(sri(5), 9));
    assert!(!sel.contains(sri(5), 10), "anchor Side::Left → end_col=9");

    // Contract to same position as anchor → empty.
    sel.end = sel.anchor;
    assert!(sel.is_empty());
}

// -- Block mode with wide char at boundary (ref: WezTerm drag_selection emoji) --

#[test]
fn block_mode_extract_text_wide_char_at_block_edge() {
    let mut grid = Grid::new(2, 10);
    grid.move_to(0, Column(0));
    grid.put_char('A');
    grid.put_char('漢'); // cols 1-2 (wide)
    grid.put_char('B'); // col 3
    grid.move_to(1, Column(0));
    grid.put_char('X');
    grid.put_char('Y');
    grid.put_char('Z');
    grid.put_char('W');

    // Block select cols 1-3 across both rows.
    let sel = Selection {
        mode: SelectionMode::Block,
        anchor: SelectionPoint {
            row: sri(0),
            col: 1,
            side: Side::Left,
        },
        pivot: SelectionPoint {
            row: sri(0),
            col: 1,
            side: Side::Left,
        },
        end: SelectionPoint {
            row: sri(1),
            col: 3,
            side: Side::Right,
        },
    };
    // Row 0: col 1 = 漢, col 2 = spacer (skipped), col 3 = B → "漢B".
    // Row 1: Y(1), Z(2), W(3) → "YZW".
    assert_eq!(extract_text(&grid, &sel), "漢B\nYZW");
}
