use oriterm_core::grid::StableRowIndex;
use oriterm_mux::{PaneSnapshot, WireCell, WireCursor, WireCursorShape, WireRgb};

use super::SnapshotGrid;

/// White RGB for test cells.
const WHITE: WireRgb = WireRgb {
    r: 255,
    g: 255,
    b: 255,
};

/// Black RGB for test cells.
const BLACK: WireRgb = WireRgb { r: 0, g: 0, b: 0 };

/// Build a simple WireCell with a character and no flags.
fn cell(ch: char) -> WireCell {
    WireCell {
        ch,
        fg: WHITE,
        bg: BLACK,
        flags: 0,
        underline_color: None,
        hyperlink_uri: None,
        zerowidth: Vec::new(),
    }
}

/// Build a WireCell with a character and custom flags.
fn cell_with_flags(ch: char, flags: u16) -> WireCell {
    WireCell {
        ch,
        fg: WHITE,
        bg: BLACK,
        flags,
        underline_color: None,
        hyperlink_uri: None,
        zerowidth: Vec::new(),
    }
}

/// Build a minimal test snapshot from rows of cells.
fn test_snapshot(cells: Vec<Vec<WireCell>>, cols: u16, stable_row_base: u64) -> PaneSnapshot {
    PaneSnapshot {
        cells,
        cursor: WireCursor {
            col: 0,
            row: 0,
            shape: WireCursorShape::Block,
            visible: true,
        },
        palette: vec![[0; 3]; 270],
        title: String::new(),
        icon_name: None,
        cwd: None,
        modes: 0,
        scrollback_len: 100,
        display_offset: 0,
        stable_row_base,
        cols,
        search_active: false,
        search_query: String::new(),
        search_matches: Vec::new(),
        search_focused: None,
        search_total_matches: 0,
    }
}

#[test]
fn cols_and_lines() {
    let snap = test_snapshot(
        vec![
            vec![cell('a'), cell('b'), cell('c')],
            vec![cell('d'), cell('e'), cell('f')],
        ],
        3,
        50,
    );
    let g = SnapshotGrid::new(&snap);
    assert_eq!(g.cols(), 3);
    assert_eq!(g.lines(), 2);
    assert_eq!(g.stable_row_base(), 50);
    assert_eq!(g.scrollback_len(), 100);
    assert_eq!(g.display_offset(), 0);
}

#[test]
fn cell_char_in_bounds() {
    let snap = test_snapshot(vec![vec![cell('x'), cell('y')]], 2, 0);
    let g = SnapshotGrid::new(&snap);
    assert_eq!(g.cell_char(0, 0), 'x');
    assert_eq!(g.cell_char(0, 1), 'y');
}

#[test]
fn cell_char_out_of_bounds() {
    let snap = test_snapshot(vec![vec![cell('a')]], 1, 0);
    let g = SnapshotGrid::new(&snap);
    assert_eq!(g.cell_char(0, 5), ' ');
    assert_eq!(g.cell_char(5, 0), ' ');
}

#[test]
fn viewport_to_stable_row() {
    let snap = test_snapshot(vec![vec![cell('a')]; 3], 1, 100);
    let g = SnapshotGrid::new(&snap);
    assert_eq!(g.viewport_to_stable_row(0), StableRowIndex(100));
    assert_eq!(g.viewport_to_stable_row(2), StableRowIndex(102));
}

#[test]
fn stable_row_to_viewport_visible() {
    let snap = test_snapshot(vec![vec![cell('a')]; 3], 1, 100);
    let g = SnapshotGrid::new(&snap);
    assert_eq!(g.stable_row_to_viewport(StableRowIndex(100)), Some(0));
    assert_eq!(g.stable_row_to_viewport(StableRowIndex(102)), Some(2));
}

#[test]
fn stable_row_to_viewport_out_of_range() {
    let snap = test_snapshot(vec![vec![cell('a')]; 3], 1, 100);
    let g = SnapshotGrid::new(&snap);
    assert_eq!(g.stable_row_to_viewport(StableRowIndex(99)), None);
    assert_eq!(g.stable_row_to_viewport(StableRowIndex(103)), None);
}

#[test]
fn redirect_spacer_base_cell() {
    // col 0: wide char, col 1: spacer
    let wide_char_spacer_bit: u16 = 1 << 9;
    let snap = test_snapshot(
        vec![vec![
            cell_with_flags('漢', 1 << 8), // WIDE_CHAR
            cell_with_flags(' ', wide_char_spacer_bit),
            cell('z'),
        ]],
        3,
        0,
    );
    let g = SnapshotGrid::new(&snap);
    assert_eq!(g.redirect_spacer(0, 0), 0); // base cell stays
    assert_eq!(g.redirect_spacer(0, 1), 0); // spacer redirects to 0
    assert_eq!(g.redirect_spacer(0, 2), 2); // normal cell stays
}

#[test]
fn word_boundaries_simple() {
    // "hello world"
    let snap = test_snapshot(
        vec![vec![
            cell('h'),
            cell('e'),
            cell('l'),
            cell('l'),
            cell('o'),
            cell(' '),
            cell('w'),
            cell('o'),
            cell('r'),
            cell('l'),
            cell('d'),
        ]],
        11,
        0,
    );
    let g = SnapshotGrid::new(&snap);
    let delimiters = ",│`|:\"' ()[]{}<>\t";

    // Click on 'l' (col 2) → selects "hello" (0..4)
    assert_eq!(g.word_boundaries(0, 2, delimiters), (0, 4));
    // Click on space (col 5) → selects just the space (5..5)
    assert_eq!(g.word_boundaries(0, 5, delimiters), (5, 5));
    // Click on 'r' (col 8) → selects "world" (6..10)
    assert_eq!(g.word_boundaries(0, 8, delimiters), (6, 10));
}

#[test]
fn word_boundaries_with_wide_char() {
    let wide = 1u16 << 8;
    let spacer = 1u16 << 9;
    // "a漢b" → 4 cells: [a] [漢(wide)] [spacer] [b]
    let snap = test_snapshot(
        vec![vec![
            cell('a'),
            cell_with_flags('漢', wide),
            cell_with_flags(' ', spacer),
            cell('b'),
        ]],
        4,
        0,
    );
    let g = SnapshotGrid::new(&snap);
    let delimiters = ",│`|:\"' ()[]{}<>\t";

    // "a漢b" are all word chars — should select all 4 cells.
    assert_eq!(g.word_boundaries(0, 0, delimiters), (0, 3));
    // Click on the spacer (col 2) → redirects to col 1, selects all.
    assert_eq!(g.word_boundaries(0, 2, delimiters), (0, 3));
}

#[test]
fn logical_line_start_no_wrap() {
    let snap = test_snapshot(
        vec![vec![cell('a'), cell('b')], vec![cell('c'), cell('d')]],
        2,
        0,
    );
    let g = SnapshotGrid::new(&snap);
    assert_eq!(g.logical_line_start(0), 0);
    assert_eq!(g.logical_line_start(1), 1);
}

#[test]
fn logical_line_start_with_wrap() {
    let wrap = 1u16 << 10;
    let snap = test_snapshot(
        vec![
            vec![cell('a'), cell_with_flags('b', wrap)], // row 0 wraps
            vec![cell('c'), cell('d')],                  // row 1 is continuation
            vec![cell('e'), cell('f')],                  // row 2 is separate
        ],
        2,
        0,
    );
    let g = SnapshotGrid::new(&snap);
    assert_eq!(g.logical_line_start(0), 0);
    assert_eq!(g.logical_line_start(1), 0); // row 1 starts at row 0
    assert_eq!(g.logical_line_start(2), 2);
}

#[test]
fn logical_line_end_with_wrap() {
    let wrap = 1u16 << 10;
    let snap = test_snapshot(
        vec![
            vec![cell('a'), cell_with_flags('b', wrap)], // row 0 wraps
            vec![cell('c'), cell_with_flags('d', wrap)], // row 1 wraps
            vec![cell('e'), cell('f')],                  // row 2 is end
        ],
        2,
        0,
    );
    let g = SnapshotGrid::new(&snap);
    assert_eq!(g.logical_line_end(0), 2);
    assert_eq!(g.logical_line_end(1), 2);
    assert_eq!(g.logical_line_end(2), 2);
}
