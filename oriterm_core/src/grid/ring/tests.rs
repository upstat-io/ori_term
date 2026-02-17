use crate::cell::Cell;
use crate::grid::Grid;
use crate::grid::row::Row;
use crate::index::{Column, Line};

use super::ScrollbackBuffer;

#[test]
fn new_buffer_is_empty() {
    let sb = ScrollbackBuffer::new(100);
    assert_eq!(sb.len(), 0);
    assert!(sb.is_empty());
    assert!(sb.get(0).is_none());
}

#[test]
fn push_and_retrieve_order() {
    let mut sb = ScrollbackBuffer::new(100);
    let r0 = make_row("AAA");
    let r1 = make_row("BBB");
    let r2 = make_row("CCC");

    sb.push(r0);
    sb.push(r1);
    sb.push(r2);

    assert_eq!(sb.len(), 3);
    assert!(!sb.is_empty());

    // Index 0 = most recent (CCC), index 2 = oldest (AAA).
    assert_eq!(row_text(sb.get(0).unwrap()), "CCC");
    assert_eq!(row_text(sb.get(1).unwrap()), "BBB");
    assert_eq!(row_text(sb.get(2).unwrap()), "AAA");
    assert!(sb.get(3).is_none());
}

#[test]
fn ring_wraps_evicts_oldest() {
    let mut sb = ScrollbackBuffer::new(3);

    // Push 5 rows into a buffer that holds 3.
    for i in 0..5 {
        sb.push(make_row(&format!("R{i}")));
    }

    assert_eq!(sb.len(), 3);
    // Only R2, R3, R4 should remain (R0, R1 evicted).
    assert_eq!(row_text(sb.get(0).unwrap()), "R4");
    assert_eq!(row_text(sb.get(1).unwrap()), "R3");
    assert_eq!(row_text(sb.get(2).unwrap()), "R2");
}

#[test]
fn ring_wraps_many_extra() {
    let max = 5;
    let mut sb = ScrollbackBuffer::new(max);

    // Push max + 10 rows.
    for i in 0..(max + 10) {
        sb.push(make_row(&format!("R{i}")));
    }

    assert_eq!(sb.len(), max);
    // Most recent is R14, oldest is R10.
    assert_eq!(row_text(sb.get(0).unwrap()), "R14");
    assert_eq!(row_text(sb.get(max - 1).unwrap()), "R10");
}

#[test]
fn clear_empties_buffer() {
    let mut sb = ScrollbackBuffer::new(100);
    sb.push(make_row("A"));
    sb.push(make_row("B"));
    assert_eq!(sb.len(), 2);

    sb.clear();
    assert_eq!(sb.len(), 0);
    assert!(sb.is_empty());
    assert!(sb.get(0).is_none());

    // Can push again after clear.
    sb.push(make_row("C"));
    assert_eq!(sb.len(), 1);
    assert_eq!(row_text(sb.get(0).unwrap()), "C");
}

#[test]
fn iter_newest_to_oldest() {
    let mut sb = ScrollbackBuffer::new(100);
    sb.push(make_row("X"));
    sb.push(make_row("Y"));
    sb.push(make_row("Z"));

    let texts: Vec<String> = sb.iter().map(row_text).collect();
    assert_eq!(texts, vec!["Z", "Y", "X"]);
}

#[test]
fn iter_after_wrap() {
    let mut sb = ScrollbackBuffer::new(3);
    for i in 0..7 {
        sb.push(make_row(&format!("R{i}")));
    }

    let texts: Vec<String> = sb.iter().map(row_text).collect();
    assert_eq!(texts, vec!["R6", "R5", "R4"]);
}

#[test]
fn zero_max_scrollback_returns_pushed_row() {
    let mut sb = ScrollbackBuffer::new(0);
    let returned = sb.push(make_row("A"));
    assert_eq!(sb.len(), 0);
    assert!(sb.is_empty());
    // Row returned immediately — no storage.
    assert_eq!(row_text(&returned.unwrap()), "A");
}

#[test]
fn max_scrollback_returns_configured_limit() {
    let sb = ScrollbackBuffer::new(500);
    assert_eq!(sb.max_scrollback(), 500);

    let sb_zero = ScrollbackBuffer::new(0);
    assert_eq!(sb_zero.max_scrollback(), 0);
}

// Ring buffer boundary tests

#[test]
fn exact_capacity_boundary_first_eviction() {
    let mut sb = ScrollbackBuffer::new(3);

    // Push exactly max_scrollback rows (buffer just becomes full).
    sb.push(make_row("R0"));
    sb.push(make_row("R1"));
    sb.push(make_row("R2"));
    assert_eq!(sb.len(), 3);
    assert_eq!(row_text(sb.get(0).unwrap()), "R2");
    assert_eq!(row_text(sb.get(2).unwrap()), "R0");

    // One more push triggers the first eviction (start becomes non-zero).
    sb.push(make_row("R3"));
    assert_eq!(sb.len(), 3);
    assert_eq!(row_text(sb.get(0).unwrap()), "R3");
    assert_eq!(row_text(sb.get(1).unwrap()), "R2");
    assert_eq!(row_text(sb.get(2).unwrap()), "R1");
}

// Push return value tests

#[test]
fn push_returns_none_during_growth() {
    let mut sb = ScrollbackBuffer::new(3);
    assert!(sb.push(make_row("R0")).is_none());
    assert!(sb.push(make_row("R1")).is_none());
    assert!(sb.push(make_row("R2")).is_none());
    assert_eq!(sb.len(), 3);
}

#[test]
fn push_returns_evicted_row_when_full() {
    let mut sb = ScrollbackBuffer::new(3);
    sb.push(make_row("R0"));
    sb.push(make_row("R1"));
    sb.push(make_row("R2"));

    // Buffer is full — next push evicts the oldest (R0).
    let evicted = sb.push(make_row("R3"));
    assert_eq!(row_text(&evicted.unwrap()), "R0");

    // Next push evicts R1.
    let evicted = sb.push(make_row("R4"));
    assert_eq!(row_text(&evicted.unwrap()), "R1");

    // Buffer still holds the 3 most recent.
    assert_eq!(row_text(sb.get(0).unwrap()), "R4");
    assert_eq!(row_text(sb.get(1).unwrap()), "R3");
    assert_eq!(row_text(sb.get(2).unwrap()), "R2");
}

// Wide character scrollback tests

#[test]
fn wide_char_flags_preserved_in_scrollback() {
    use crate::cell::CellFlags;

    let cols = 4;
    let mut row = Row::new(cols);

    // Simulate a wide char at col 0: char + spacer.
    row[Column(0)].ch = '\u{4e16}'; // CJK character
    row[Column(0)].flags = CellFlags::WIDE_CHAR;
    row[Column(1)].ch = ' ';
    row[Column(1)].flags = CellFlags::WIDE_CHAR_SPACER;
    // Normal char at col 2.
    row[Column(2)].ch = 'A';

    let mut sb = ScrollbackBuffer::new(10);
    sb.push(row);

    let retrieved = sb.get(0).unwrap();
    assert_eq!(retrieved[Column(0)].ch, '\u{4e16}');
    assert!(retrieved[Column(0)].flags.contains(CellFlags::WIDE_CHAR));
    assert_eq!(retrieved[Column(1)].ch, ' ');
    assert!(
        retrieved[Column(1)]
            .flags
            .contains(CellFlags::WIDE_CHAR_SPACER)
    );
    assert_eq!(retrieved[Column(2)].ch, 'A');
}

#[test]
fn wide_char_survives_scrollback_via_grid_scroll_up() {
    use crate::cell::CellFlags;

    let mut grid = Grid::new(3, 6);
    // Write a wide char on line 0 using put_char (the production path).
    grid.cursor_mut().set_line(0);
    grid.cursor_mut().set_col(Column(0));
    grid.put_char('\u{4e16}'); // width 2: cell + spacer
    grid.put_char('X');

    grid.scroll_up(1);

    assert_eq!(grid.scrollback().len(), 1);
    let row = grid.scrollback().get(0).unwrap();
    assert_eq!(row[Column(0)].ch, '\u{4e16}');
    assert!(row[Column(0)].flags.contains(CellFlags::WIDE_CHAR));
    assert!(row[Column(1)].flags.contains(CellFlags::WIDE_CHAR_SPACER));
    assert_eq!(row[Column(2)].ch, 'X');
}

// Grid integration tests

#[test]
fn scroll_up_pushes_to_scrollback() {
    let mut grid = Grid::new(3, 5);
    write_row(&mut grid, 0, "AAAAA");
    write_row(&mut grid, 1, "BBBBB");
    write_row(&mut grid, 2, "CCCCC");

    grid.scroll_up(1);

    // Row A scrolled off the top into scrollback.
    assert_eq!(grid.scrollback().len(), 1);
    assert_eq!(row_text(grid.scrollback().get(0).unwrap()), "AAAAA");

    // Visible grid: B, C, blank.
    assert_eq!(visible_text(&grid, 0), "BBBBB");
    assert_eq!(visible_text(&grid, 1), "CCCCC");
    assert!(grid[Line(2)].occ() == 0); // blank row
}

#[test]
fn scroll_up_multiple_pushes_in_order() {
    let mut grid = Grid::new(4, 3);
    write_row(&mut grid, 0, "AAA");
    write_row(&mut grid, 1, "BBB");
    write_row(&mut grid, 2, "CCC");
    write_row(&mut grid, 3, "DDD");

    grid.scroll_up(2);

    assert_eq!(grid.scrollback().len(), 2);
    // Most recent in scrollback = B (was row 1, evicted second).
    assert_eq!(row_text(grid.scrollback().get(0).unwrap()), "BBB");
    // Oldest in scrollback = A (was row 0, evicted first).
    assert_eq!(row_text(grid.scrollback().get(1).unwrap()), "AAA");
}

#[test]
fn scroll_up_sub_region_does_not_push_to_scrollback() {
    let mut grid = Grid::new(5, 3);
    write_row(&mut grid, 0, "AAA");
    write_row(&mut grid, 1, "BBB");
    write_row(&mut grid, 2, "CCC");

    // Set scroll region to lines 2..5 (sub-region, not full screen).
    grid.set_scroll_region(2, Some(5));
    grid.scroll_up(1);

    // No scrollback — sub-region scrolls don't preserve rows.
    assert_eq!(grid.scrollback().len(), 0);
}

#[test]
fn display_offset_scrolls_through_history() {
    let mut grid = Grid::new(3, 3);
    // Push 5 rows through scrollback.
    for i in 0..5 {
        write_row(&mut grid, 0, &format!("R{i:02}")[..3]);
        grid.scroll_up(1);
    }

    assert_eq!(grid.scrollback().len(), 5);
    assert_eq!(grid.display_offset(), 0);

    // Scroll back 3 lines.
    grid.scroll_display(3);
    assert_eq!(grid.display_offset(), 3);

    // Scroll forward 1 line.
    grid.scroll_display(-1);
    assert_eq!(grid.display_offset(), 2);
}

#[test]
fn display_offset_clamped_to_scrollback_len() {
    let mut grid = Grid::new(3, 3);
    // Push 2 rows to scrollback.
    write_row(&mut grid, 0, "AAA");
    grid.scroll_up(1);
    write_row(&mut grid, 0, "BBB");
    grid.scroll_up(1);

    assert_eq!(grid.scrollback().len(), 2);

    // Try to scroll back 100 lines — clamped to 2.
    grid.scroll_display(100);
    assert_eq!(grid.display_offset(), 2);

    // Try to scroll forward past live view — clamped to 0.
    grid.scroll_display(-100);
    assert_eq!(grid.display_offset(), 0);
}

#[test]
fn total_lines_reflects_scrollback() {
    let mut grid = Grid::new(3, 5);
    assert_eq!(grid.total_lines(), 3);

    write_row(&mut grid, 0, "AAAAA");
    grid.scroll_up(1);
    assert_eq!(grid.total_lines(), 4);

    write_row(&mut grid, 0, "BBBBB");
    grid.scroll_up(1);
    assert_eq!(grid.total_lines(), 5);
}

// Helpers

/// Create a row with ASCII characters (one char per cell).
fn make_row(text: &str) -> Row {
    let cols = text.len();
    let mut row = Row::new(cols);
    for (i, ch) in text.chars().enumerate() {
        let mut cell = Cell::default();
        cell.ch = ch;
        row[Column(i)] = cell;
    }
    row
}

/// Extract text from a row (stops at default cells).
fn row_text(row: &Row) -> String {
    (0..row.cols())
        .map(|i| row[Column(i)].ch)
        .take_while(|&ch| ch != '\0')
        .collect()
}

/// Write ASCII text into a visible grid row.
fn write_row(grid: &mut Grid, line: usize, text: &str) {
    for (i, ch) in text.chars().enumerate() {
        let mut cell = Cell::default();
        cell.ch = ch;
        grid[Line(line as i32)][Column(i)] = cell;
    }
}

/// Read visible text from a grid row.
fn visible_text(grid: &Grid, line: usize) -> String {
    let row = &grid[Line(line as i32)];
    (0..row.cols())
        .map(|i| row[Column(i)].ch)
        .take_while(|&ch| ch != '\0')
        .collect()
}
