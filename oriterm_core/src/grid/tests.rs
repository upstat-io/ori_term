use super::Grid;
use crate::index::{Column, Line};

#[test]
fn new_grid_has_correct_dimensions() {
    let grid = Grid::new(24, 80);
    assert_eq!(grid.lines(), 24);
    assert_eq!(grid.cols(), 80);
}

#[test]
fn tab_stops_every_8_columns() {
    let grid = Grid::new(24, 80);
    let stops = grid.tab_stops();
    assert!(stops[0]); // Column 0.
    assert!(!stops[1]);
    assert!(stops[8]);
    assert!(stops[16]);
    assert!(!stops[79]);
    assert!(stops[72]);
}

#[test]
fn index_by_line_returns_correct_row() {
    let grid = Grid::new(24, 80);
    let row = &grid[Line(0)];
    assert_eq!(row.cols(), 80);
    let row_last = &grid[Line(23)];
    assert_eq!(row_last.cols(), 80);
}

#[test]
fn cursor_starts_at_origin() {
    let grid = Grid::new(24, 80);
    assert_eq!(grid.cursor().line(), 0);
    assert_eq!(grid.cursor().col(), Column(0));
}

// --- Additional tests from reference repo gap analysis ---

#[test]
fn grid_1x1_minimum_dimensions() {
    let grid = Grid::new(1, 1);
    assert_eq!(grid.lines(), 1);
    assert_eq!(grid.cols(), 1);
    assert!(grid[Line(0)][Column(0)].is_empty());
}

#[test]
fn scroll_region_defaults_to_full_grid() {
    let grid = Grid::new(24, 80);
    assert_eq!(grid.scroll_region, 0..24);
}

#[test]
fn saved_cursor_starts_as_none() {
    let grid = Grid::new(24, 80);
    assert!(grid.saved_cursor.is_none());
}

#[test]
fn tab_stops_for_narrow_grid() {
    // Grid narrower than 8 columns: only col 0 is a stop.
    let grid = Grid::new(1, 5);
    let stops = grid.tab_stops();
    assert!(stops[0]);
    assert!(!stops[1]);
    assert!(!stops[4]);
}

#[test]
fn all_rows_initialized_empty() {
    let grid = Grid::new(5, 10);
    for line in 0..5 {
        let row = &grid[Line(line as i32)];
        assert_eq!(row.cols(), 10);
        for col in 0..10 {
            assert!(row[Column(col)].is_empty());
        }
    }
}
