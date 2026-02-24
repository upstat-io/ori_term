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

// drain_oldest_first tests

#[test]
fn drain_oldest_first_empty_buffer() {
    let mut sb = ScrollbackBuffer::new(10);
    let result = sb.drain_oldest_first();
    assert!(result.is_empty());
    assert_eq!(sb.len(), 0);
}

#[test]
fn drain_oldest_first_growth_phase() {
    let mut sb = ScrollbackBuffer::new(10);
    sb.push(make_row("AAA"));
    sb.push(make_row("BBB"));
    sb.push(make_row("CCC"));

    let result = sb.drain_oldest_first();
    let texts: Vec<String> = result.iter().map(|r| row_text(r)).collect();
    assert_eq!(texts, vec!["AAA", "BBB", "CCC"]);
    assert_eq!(sb.len(), 0);
}

#[test]
fn drain_oldest_first_wrapped_ring() {
    // Capacity 3, push 5 rows → ring wraps, start > 0.
    let mut sb = ScrollbackBuffer::new(3);
    for i in 0..5 {
        sb.push(make_row(&format!("R{i}")));
    }
    assert_eq!(sb.len(), 3);

    // Should return R2, R3, R4 (oldest to newest).
    let result = sb.drain_oldest_first();
    let texts: Vec<String> = result.iter().map(|r| row_text(r)).collect();
    assert_eq!(texts, vec!["R2", "R3", "R4"]);
    assert_eq!(sb.len(), 0);
}

#[test]
fn drain_oldest_first_exactly_full() {
    // Capacity 3, push exactly 3 → full but start == 0.
    let mut sb = ScrollbackBuffer::new(3);
    sb.push(make_row("AAA"));
    sb.push(make_row("BBB"));
    sb.push(make_row("CCC"));

    let result = sb.drain_oldest_first();
    let texts: Vec<String> = result.iter().map(|r| row_text(r)).collect();
    assert_eq!(texts, vec!["AAA", "BBB", "CCC"]);
    assert_eq!(sb.len(), 0);
}

#[test]
fn drain_oldest_first_wrapped_many_extra() {
    // Capacity 5, push 15 → ring wraps many times.
    let mut sb = ScrollbackBuffer::new(5);
    for i in 0..15 {
        sb.push(make_row(&format!("R{i:02}")));
    }
    assert_eq!(sb.len(), 5);

    let result = sb.drain_oldest_first();
    let texts: Vec<String> = result.iter().map(|r| row_text(r)).collect();
    assert_eq!(texts, vec!["R10", "R11", "R12", "R13", "R14"]);
    assert_eq!(sb.len(), 0);
}

#[test]
fn drain_oldest_first_usable_after_drain() {
    let mut sb = ScrollbackBuffer::new(5);
    sb.push(make_row("AAA"));
    sb.push(make_row("BBB"));
    sb.drain_oldest_first();

    // Buffer should be usable after drain.
    sb.push(make_row("CCC"));
    assert_eq!(sb.len(), 1);
    assert_eq!(row_text(sb.get(0).unwrap()), "CCC");
}

// pop_newest + push interaction tests

#[test]
#[ignore = "reproduces ring buffer bug — fix in ScrollbackBuffer::push"]
fn pop_newest_then_push_full_buffer_preserves_new_entry() {
    // Fill to capacity, pop newest, then push a new row.
    // The new row must be retrievable as the newest entry.
    let mut sb = ScrollbackBuffer::new(3);
    sb.push(make_row("AAA"));
    sb.push(make_row("BBB"));
    sb.push(make_row("CCC"));
    assert_eq!(sb.len(), 3);

    // Pop newest (CCC).
    let popped = sb.pop_newest().unwrap();
    assert_eq!(row_text(&popped), "CCC");
    assert_eq!(sb.len(), 2);

    // Push a new row. Should NOT evict — there's room for one more.
    let evicted = sb.push(make_row("DDD"));
    assert!(
        evicted.is_none(),
        "buffer had room after pop, should not evict"
    );
    assert_eq!(sb.len(), 3);

    // Verify order: DDD (newest), BBB, AAA (oldest).
    assert_eq!(row_text(sb.get(0).unwrap()), "DDD");
    assert_eq!(row_text(sb.get(1).unwrap()), "BBB");
    assert_eq!(row_text(sb.get(2).unwrap()), "AAA");
}

#[test]
#[ignore = "reproduces ring buffer bug — fix in ScrollbackBuffer::push"]
fn pop_newest_twice_then_push_twice_preserves_order() {
    let mut sb = ScrollbackBuffer::new(3);
    sb.push(make_row("AAA"));
    sb.push(make_row("BBB"));
    sb.push(make_row("CCC"));

    sb.pop_newest(); // remove CCC
    sb.pop_newest(); // remove BBB
    assert_eq!(sb.len(), 1);

    sb.push(make_row("DDD"));
    sb.push(make_row("EEE"));
    assert_eq!(sb.len(), 3);

    assert_eq!(row_text(sb.get(0).unwrap()), "EEE");
    assert_eq!(row_text(sb.get(1).unwrap()), "DDD");
    assert_eq!(row_text(sb.get(2).unwrap()), "AAA");
}

#[test]
#[ignore = "reproduces ring buffer bug — fix in ScrollbackBuffer::push"]
fn pop_newest_then_push_after_wrap_preserves_data() {
    // Buffer wraps (start > 0), then pop + push.
    let mut sb = ScrollbackBuffer::new(3);
    sb.push(make_row("AAA"));
    sb.push(make_row("BBB"));
    sb.push(make_row("CCC"));
    sb.push(make_row("DDD")); // evicts AAA, start wraps
    sb.push(make_row("EEE")); // evicts BBB, start wraps again

    assert_eq!(sb.len(), 3);
    assert_eq!(row_text(sb.get(0).unwrap()), "EEE");
    assert_eq!(row_text(sb.get(1).unwrap()), "DDD");
    assert_eq!(row_text(sb.get(2).unwrap()), "CCC");

    // Pop newest (EEE).
    sb.pop_newest();
    assert_eq!(sb.len(), 2);
    assert_eq!(row_text(sb.get(0).unwrap()), "DDD");
    assert_eq!(row_text(sb.get(1).unwrap()), "CCC");

    // Push new row.
    let evicted = sb.push(make_row("FFF"));
    assert!(
        evicted.is_none(),
        "buffer had room after pop, should not evict"
    );
    assert_eq!(sb.len(), 3);

    assert_eq!(row_text(sb.get(0).unwrap()), "FFF");
    assert_eq!(row_text(sb.get(1).unwrap()), "DDD");
    assert_eq!(row_text(sb.get(2).unwrap()), "CCC");
}

#[test]
#[ignore = "reproduces ring buffer bug — fix in ScrollbackBuffer::push"]
fn pop_newest_then_push_growth_phase_no_placeholder_leak() {
    // Buffer not yet full (growth phase). Pop then push should not
    // leave a placeholder visible at any logical index.
    let mut sb = ScrollbackBuffer::new(5);
    sb.push(make_row("AAA"));
    sb.push(make_row("BBB"));
    sb.push(make_row("CCC"));
    assert_eq!(sb.len(), 3);

    sb.pop_newest(); // remove CCC
    assert_eq!(sb.len(), 2);

    sb.push(make_row("DDD"));
    assert_eq!(sb.len(), 3);

    // No entry should be a null/placeholder row.
    assert_eq!(row_text(sb.get(0).unwrap()), "DDD");
    assert_eq!(row_text(sb.get(1).unwrap()), "BBB");
    assert_eq!(row_text(sb.get(2).unwrap()), "AAA");
}

#[test]
#[ignore = "reproduces ring buffer bug — fix in ScrollbackBuffer::push"]
fn repeated_pop_push_cycles_preserve_integrity() {
    // Simulate what happens during repeated resize + scroll_up cycles.
    let mut sb = ScrollbackBuffer::new(5);
    for i in 0..5 {
        sb.push(make_row(&format!("R{i}")));
    }
    assert_eq!(sb.len(), 5);

    // Each cycle: pop 1 (resize overflow), then push 1 (new content).
    for i in 5..10 {
        sb.pop_newest();
        sb.push(make_row(&format!("R{i}")));
    }

    assert_eq!(sb.len(), 5);
    // Newest should be R9, oldest should be R5.
    // Each cycle removed the newest and added a new one, so the buffer
    // should contain R5, R6, R7, R8, R9.
    assert_eq!(row_text(sb.get(0).unwrap()), "R9");
    assert_eq!(row_text(sb.get(1).unwrap()), "R8");
    assert_eq!(row_text(sb.get(2).unwrap()), "R7");
    assert_eq!(row_text(sb.get(3).unwrap()), "R6");
    assert_eq!(row_text(sb.get(4).unwrap()), "R5");
}

#[test]
#[ignore = "reproduces ring buffer bug — fix in ScrollbackBuffer::push"]
fn iter_after_pop_push_matches_get() {
    let mut sb = ScrollbackBuffer::new(3);
    sb.push(make_row("AAA"));
    sb.push(make_row("BBB"));
    sb.push(make_row("CCC"));

    sb.pop_newest();
    sb.push(make_row("DDD"));

    let via_iter: Vec<String> = sb.iter().map(row_text).collect();
    let via_get: Vec<String> = (0..sb.len())
        .map(|i| row_text(sb.get(i).unwrap()))
        .collect();
    assert_eq!(via_iter, via_get);
    assert_eq!(via_iter, vec!["DDD", "BBB", "AAA"]);
}

// Grid-level reproduction: resize then scroll_up

#[test]
#[ignore = "reproduces ring buffer bug — fix in ScrollbackBuffer::push"]
fn scroll_up_after_grow_rows_preserves_scrollback() {
    // Simulate: terminal has full scrollback, then window grows (pop_newest),
    // then new content arrives (scroll_up → push). Scrollback must not
    // contain placeholder/empty rows.
    let mut grid = Grid::with_scrollback(3, 5, 5);

    // Fill scrollback to capacity.
    for i in 0..5 {
        write_row(&mut grid, 0, &format!("SB{i:02}")[..4]);
        grid.scroll_up(1);
    }
    assert_eq!(grid.scrollback().len(), 5);

    // Grow terminal height by 1 (3 → 4 lines). Cursor at bottom.
    grid.cursor_mut().set_line(2);
    grid.resize(4, 5, true);

    // Write new content and scroll.
    write_row(&mut grid, 0, "NEW0");
    grid.scroll_up(1);

    // The newly pushed row must appear as newest in scrollback.
    let newest = grid.scrollback().get(0).unwrap();
    let text: String = (0..4).map(|i| newest[Column(i)].ch).collect();
    assert_eq!(
        text, "NEW0",
        "newest scrollback row should be the just-pushed content"
    );

    // No scrollback entry should be a placeholder (all-null) row.
    for i in 0..grid.scrollback().len() {
        let row = grid.scrollback().get(i).unwrap();
        let any_content = (0..row.cols()).any(|c| row[Column(c)].ch != '\0');
        assert!(
            any_content,
            "scrollback row {i} is a placeholder/empty — ring buffer corruption"
        );
    }
}

#[test]
fn scroll_up_while_scrolled_back_no_duplication() {
    // Simulate: user scrolls back, new content arrives, then scrolls to
    // live view. Visible content must not be duplicated.
    let mut grid = Grid::with_scrollback(3, 5, 10);

    // Fill some scrollback.
    for i in 0..5 {
        write_row(&mut grid, 0, &format!("L{i:03}")[..4]);
        grid.scroll_up(1);
    }

    // Scroll back.
    grid.scroll_display(3);
    assert_eq!(grid.display_offset(), 3);

    // New content arrives while scrolled back.
    for i in 5..8 {
        write_row(&mut grid, 0, &format!("L{i:03}")[..4]);
        grid.scroll_up(1);
    }

    // Scroll back to live view.
    grid.scroll_display(-(grid.display_offset() as isize));
    assert_eq!(grid.display_offset(), 0);

    // Walk the full scrollback and check no two adjacent rows are identical.
    let sb = grid.scrollback();
    for i in 0..sb.len().saturating_sub(1) {
        let a: String = (0..sb.get(i).unwrap().cols())
            .map(|c| sb.get(i).unwrap()[Column(c)].ch)
            .collect();
        let b: String = (0..sb.get(i + 1).unwrap().cols())
            .map(|c| sb.get(i + 1).unwrap()[Column(c)].ch)
            .collect();
        assert_ne!(
            a,
            b,
            "scrollback rows {i} and {} are identical: {a:?}",
            i + 1
        );
    }
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
