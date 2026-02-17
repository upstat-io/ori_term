use super::Row;
use crate::cell::{Cell, CellFlags};
use crate::index::Column;

#[test]
fn new_row_has_correct_length_and_defaults() {
    let row = Row::new(80);
    assert_eq!(row.cols(), 80);
    assert_eq!(row.occ(), 0);
    assert!(row[Column(0)].is_empty());
    assert!(row[Column(79)].is_empty());
}

#[test]
fn writing_cell_updates_occ() {
    let mut row = Row::new(80);
    let mut cell = Cell::default();
    cell.ch = 'A';
    row.append(Column(5), &cell);
    assert_eq!(row.occ(), 6);
    assert_eq!(row[Column(5)].ch, 'A');
}

#[test]
fn reset_clears_and_resets_occ() {
    let mut row = Row::new(80);
    let mut cell = Cell::default();
    cell.ch = 'X';
    row.append(Column(10), &cell);
    assert_eq!(row.occ(), 11);

    row.reset(80, &Cell::default());
    assert_eq!(row.occ(), 0);
    assert!(row[Column(10)].is_empty());
}

#[test]
fn index_returns_correct_cell() {
    let mut row = Row::new(80);
    let mut cell = Cell::default();
    cell.ch = 'B';
    cell.flags = CellFlags::BOLD;
    row.append(Column(3), &cell);

    assert_eq!(row[Column(3)].ch, 'B');
    assert!(row[Column(3)].flags.contains(CellFlags::BOLD));
}

#[test]
fn index_mut_updates_occ() {
    let mut row = Row::new(80);
    row[Column(20)].ch = 'Z';
    // IndexMut bumps occ as an upper bound — it does not check emptiness.
    assert_eq!(row.occ(), 21);
}

#[test]
fn clear_range_resets_columns() {
    let mut row = Row::new(80);
    let mut cell = Cell::default();
    cell.ch = 'X';
    for i in 0..10 {
        row.append(Column(i), &cell);
    }
    assert_eq!(row.occ(), 10);

    row.clear_range(Column(3)..Column(7), &Cell::default());
    assert!(row[Column(3)].is_empty());
    assert!(row[Column(6)].is_empty());
    assert_eq!(row[Column(2)].ch, 'X');
    assert_eq!(row[Column(7)].ch, 'X');
}

#[test]
fn truncate_clears_from_column_to_end() {
    let mut row = Row::new(80);
    let mut cell = Cell::default();
    cell.ch = 'A';
    for i in 0..20 {
        row.append(Column(i), &cell);
    }
    assert_eq!(row.occ(), 20);

    row.truncate(Column(10), &Cell::default());
    assert_eq!(row.occ(), 10);
    assert_eq!(row[Column(9)].ch, 'A');
    assert!(row[Column(10)].is_empty());
}

#[test]
fn reset_bce_across_consecutive_resets() {
    use vte::ansi::Color;

    let color1 = Color::Indexed(1);
    let color2 = Color::Indexed(2);
    let tmpl1 = Cell::from(color1);
    let tmpl2 = Cell::from(color2);

    let mut row = Row::new(10);

    // First reset: bg=color1 -> all cells get color1, occ drops to 0.
    row.reset(10, &tmpl1);
    assert_eq!(row.occ(), 0);
    assert_eq!(row[Column(0)].bg, color1);
    assert_eq!(row[Column(9)].bg, color1);

    // Second reset with different bg: even though occ is 0, the BCE
    // guard must detect the bg mismatch and repaint all cells.
    row.reset(10, &tmpl2);
    assert_eq!(row.occ(), 0);
    assert_eq!(row[Column(0)].bg, color2);
    assert_eq!(row[Column(9)].bg, color2);
}

// --- Additional tests from reference repo gap analysis ---

#[test]
fn reset_resizes_row_larger() {
    let mut row = Row::new(10);
    assert_eq!(row.cols(), 10);
    row.reset(20, &Cell::default());
    assert_eq!(row.cols(), 20);
    assert_eq!(row.occ(), 0);
}

#[test]
fn reset_shrinks_row() {
    let mut row = Row::new(20);
    let mut cell = Cell::default();
    cell.ch = 'A';
    row.append(Column(15), &cell);
    row.reset(10, &Cell::default());
    assert_eq!(row.cols(), 10);
    assert_eq!(row.occ(), 0);
}

#[test]
fn clear_range_full_row() {
    let mut row = Row::new(10);
    let mut cell = Cell::default();
    cell.ch = 'X';
    for i in 0..10 {
        row.append(Column(i), &cell);
    }
    row.clear_range(Column(0)..Column(10), &Cell::default());
    for i in 0..10 {
        assert!(row[Column(i)].is_empty(), "Column {i} not empty");
    }
}

#[test]
fn clear_range_with_bce() {
    use vte::ansi::Color;
    let mut row = Row::new(10);
    let mut cell = Cell::default();
    cell.ch = 'X';
    for i in 0..10 {
        row.append(Column(i), &cell);
    }
    let template = Cell::from(Color::Indexed(1));
    row.clear_range(Column(3)..Column(7), &template);
    assert_eq!(row[Column(3)].bg, Color::Indexed(1));
    assert_eq!(row[Column(6)].bg, Color::Indexed(1));
    assert_eq!(row[Column(3)].ch, ' ');
    // Cells outside range untouched.
    assert_eq!(row[Column(2)].ch, 'X');
    assert_eq!(row[Column(7)].ch, 'X');
}

#[test]
fn truncate_at_col_zero_clears_entire_row() {
    let mut row = Row::new(10);
    let mut cell = Cell::default();
    cell.ch = 'X';
    for i in 0..10 {
        row.append(Column(i), &cell);
    }
    row.truncate(Column(0), &Cell::default());
    assert_eq!(row.occ(), 0);
    for i in 0..10 {
        assert!(row[Column(i)].is_empty());
    }
}

#[test]
fn append_empty_cell_does_not_bump_occ() {
    let mut row = Row::new(10);
    row.append(Column(5), &Cell::default());
    assert_eq!(row.occ(), 0);
}

#[test]
fn row_equality() {
    let row1 = Row::new(10);
    let row2 = Row::new(10);
    assert_eq!(row1, row2);

    let mut row3 = Row::new(10);
    let mut cell = Cell::default();
    cell.ch = 'A';
    row3.append(Column(0), &cell);
    assert_ne!(row1, row3);
}

#[test]
fn clear_range_bce_updates_occ() {
    use vte::ansi::Color;
    let mut row = Row::new(10);
    assert_eq!(row.occ(), 0);
    let template = Cell::from(Color::Indexed(1));
    row.clear_range(Column(3)..Column(7), &template);
    // BCE clear must bump occ to cover the dirty cells.
    assert!(
        row.occ() >= 7,
        "occ should cover BCE cells, got {}",
        row.occ()
    );
}

#[test]
fn clear_range_bce_survives_reset() {
    use vte::ansi::Color;
    let mut row = Row::new(10);
    let bce = Cell::from(Color::Indexed(1));
    row.clear_range(Column(3)..Column(7), &bce);
    // Reset with default template must clear the BCE cells.
    row.reset(10, &Cell::default());
    for i in 0..10 {
        assert!(
            row[Column(i)].is_empty(),
            "Column {i} not empty after reset"
        );
    }
}

#[test]
fn truncate_bce_updates_occ() {
    use vte::ansi::Color;
    let mut row = Row::new(10);
    let bce = Cell::from(Color::Indexed(1));
    row.truncate(Column(5), &bce);
    // BCE truncate should set occ to cover all dirty cells.
    assert_eq!(row.occ(), 10);
}

#[test]
fn clear_range_inverted_is_noop() {
    let mut row = Row::new(10);
    let mut cell = Cell::default();
    cell.ch = 'A';
    row.append(Column(0), &cell);
    // Inverted range (start > end) should not panic or modify cells.
    row.clear_range(Column(7)..Column(3), &Cell::default());
    assert_eq!(row[Column(0)].ch, 'A');
}

#[test]
fn clear_range_start_beyond_row_is_noop() {
    let mut row = Row::new(10);
    // Start beyond row length should not panic.
    row.clear_range(Column(20)..Column(30), &Cell::default());
    assert_eq!(row.occ(), 0);
}

#[test]
fn truncate_beyond_row_is_noop() {
    let mut row = Row::new(10);
    let mut cell = Cell::default();
    cell.ch = 'A';
    row.append(Column(0), &cell);
    // Column beyond row length should not panic.
    row.truncate(Column(20), &Cell::default());
    assert_eq!(row[Column(0)].ch, 'A');
}
