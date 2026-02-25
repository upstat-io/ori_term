//! Unit tests for frame input types.

use oriterm_core::{Column, CursorShape, RenderableContent, RenderableCursor, Rgb, TermMode};

use super::{FrameInput, FramePalette, ViewportSize};
use crate::font::CellMetrics;

const BG: Rgb = Rgb { r: 0, g: 0, b: 0 };
const FG: Rgb = Rgb {
    r: 211,
    g: 215,
    b: 207,
};
const CURSOR: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};

/// Build a minimal `RenderableContent` for testing.
fn empty_content() -> RenderableContent {
    RenderableContent {
        cells: Vec::new(),
        cursor: RenderableCursor {
            line: 0,
            column: Column(0),
            shape: CursorShape::default(),
            visible: true,
        },
        display_offset: 0,
        stable_row_base: 0,
        mode: TermMode::empty(),
        all_dirty: true,
        damage: Vec::new(),
    }
}

fn test_palette() -> FramePalette {
    FramePalette {
        background: BG,
        foreground: FG,
        cursor_color: CURSOR,
        opacity: 1.0,
    }
}

// --- ViewportSize ---

#[test]
fn viewport_clamps_zero_to_one() {
    let v = ViewportSize::new(0, 0);
    assert_eq!(v.width, 1);
    assert_eq!(v.height, 1);
}

#[test]
fn viewport_preserves_nonzero() {
    let v = ViewportSize::new(1920, 1080);
    assert_eq!(v.width, 1920);
    assert_eq!(v.height, 1080);
}

// --- CellMetrics ---

#[test]
fn cell_metrics_columns_and_rows() {
    let m = CellMetrics::new(8.0, 16.0, 12.0, 2.0, 1.0, 4.0);

    // 1920 / 8 = 240 columns.
    assert_eq!(m.columns(1920), 240);
    // 1080 / 16 = 67.5 → floor = 67 rows.
    assert_eq!(m.rows(1080), 67);
}

#[test]
fn cell_metrics_fractional_cell_size() {
    let m = CellMetrics::new(8.5, 17.0, 13.0, 2.0, 1.0, 4.5);

    // 1920 / 8.5 = 225.88... → floor = 225.
    assert_eq!(m.columns(1920), 225);
    // 1080 / 17.0 = 63.52... → floor = 63.
    assert_eq!(m.rows(1080), 63);
}

#[test]
fn cell_metrics_small_viewport() {
    let m = CellMetrics::new(8.0, 16.0, 12.0, 2.0, 1.0, 4.0);
    // Viewport smaller than one cell.
    assert_eq!(m.columns(4), 0);
    assert_eq!(m.rows(10), 0);
}

// --- FramePalette ---

#[test]
fn frame_palette_stores_colors() {
    let p = test_palette();
    assert_eq!(p.background, BG);
    assert_eq!(p.foreground, FG);
    assert_eq!(p.cursor_color, CURSOR);
    assert_eq!(p.opacity, 1.0);
}

// --- FrameInput ---

#[test]
fn frame_input_grid_dimensions() {
    let input = FrameInput {
        content: empty_content(),
        viewport: ViewportSize::new(800, 600),
        cell_size: CellMetrics::new(8.0, 16.0, 12.0, 2.0, 1.0, 4.0),
        palette: test_palette(),
        selection: None,
        search: None,
        hovered_cell: None,
        hovered_url_segments: Vec::new(),
        mark_cursor: None,
    };

    assert_eq!(input.columns(), 100);
    assert_eq!(input.rows(), 37);
}

#[test]
fn frame_input_needs_full_repaint() {
    let mut content = empty_content();
    content.all_dirty = true;

    let input = FrameInput {
        content,
        viewport: ViewportSize::new(800, 600),
        cell_size: CellMetrics::new(8.0, 16.0, 12.0, 2.0, 1.0, 4.0),
        palette: test_palette(),
        selection: None,
        search: None,
        hovered_cell: None,
        hovered_url_segments: Vec::new(),
        mark_cursor: None,
    };

    assert!(input.needs_full_repaint());
}

#[test]
fn frame_input_incremental_repaint() {
    let mut content = empty_content();
    content.all_dirty = false;

    let input = FrameInput {
        content,
        viewport: ViewportSize::new(800, 600),
        cell_size: CellMetrics::new(8.0, 16.0, 12.0, 2.0, 1.0, 4.0),
        palette: test_palette(),
        selection: None,
        search: None,
        hovered_cell: None,
        hovered_url_segments: Vec::new(),
        mark_cursor: None,
    };

    assert!(!input.needs_full_repaint());
}

// --- test_grid helper ---

#[test]
fn test_grid_creates_correct_dimensions() {
    let input = FrameInput::test_grid(80, 24, "");

    assert_eq!(input.content.cells.len(), 80 * 24);
    assert_eq!(input.columns(), 80);
    assert_eq!(input.rows(), 24);
}

#[test]
fn test_grid_fills_text_then_spaces() {
    let input = FrameInput::test_grid(4, 1, "AB");

    assert_eq!(input.content.cells[0].ch, 'A');
    assert_eq!(input.content.cells[1].ch, 'B');
    assert_eq!(input.content.cells[2].ch, ' ');
    assert_eq!(input.content.cells[3].ch, ' ');
}

#[test]
fn test_grid_wraps_text_across_rows() {
    let input = FrameInput::test_grid(3, 2, "ABCDE");

    // Row 0: A B C.
    assert_eq!(input.content.cells[0].ch, 'A');
    assert_eq!(input.content.cells[2].ch, 'C');
    // Row 1: D E <space>.
    assert_eq!(input.content.cells[3].ch, 'D');
    assert_eq!(input.content.cells[4].ch, 'E');
    assert_eq!(input.content.cells[5].ch, ' ');
}

#[test]
fn test_grid_cell_coordinates() {
    let input = FrameInput::test_grid(3, 2, "");

    // Row 0.
    assert_eq!(input.content.cells[0].line, 0);
    assert_eq!(input.content.cells[0].column, Column(0));
    assert_eq!(input.content.cells[2].line, 0);
    assert_eq!(input.content.cells[2].column, Column(2));
    // Row 1.
    assert_eq!(input.content.cells[3].line, 1);
    assert_eq!(input.content.cells[3].column, Column(0));
}

#[test]
fn test_grid_has_debug() {
    let input = FrameInput::test_grid(2, 2, "AB");
    // FrameInput derives Debug — verify it doesn't panic.
    let debug = format!("{input:?}");
    assert!(debug.contains("FrameInput"));
}

// --- hovered_cell ---

#[test]
fn hovered_cell_defaults_to_none() {
    let input = FrameInput::test_grid(4, 2, "");
    assert_eq!(input.hovered_cell, None);
}

// --- FrameSelection ---

#[test]
fn frame_selection_contains_viewport_line_zero() {
    use oriterm_core::{Selection, Side, StableRowIndex};

    use super::FrameSelection;

    // Selection covering cols 2..5 on stable row 10.
    let sel = Selection::new_char(StableRowIndex(10), 2, Side::Left);
    let mut sel = sel;
    sel.end = oriterm_core::SelectionPoint {
        row: StableRowIndex(10),
        col: 5,
        side: Side::Right,
    };

    // Viewport line 0 maps to stable row 10 (base = 10).
    let fs = FrameSelection::new(&sel, 10);
    assert!(
        fs.contains(0, 3),
        "col 3 on viewport line 0 should be selected"
    );
    assert!(!fs.contains(0, 1), "col 1 should not be selected");
    assert!(
        !fs.contains(1, 3),
        "viewport line 1 (stable 11) should not be selected"
    );
}

#[test]
fn frame_selection_with_scrollback_offset() {
    use oriterm_core::{Selection, Side, StableRowIndex};

    use super::FrameSelection;

    // Selection on stable row 50.
    let sel = Selection::new_char(StableRowIndex(50), 0, Side::Left);
    let mut sel = sel;
    sel.end = oriterm_core::SelectionPoint {
        row: StableRowIndex(50),
        col: 10,
        side: Side::Right,
    };

    // base=45 means viewport line 5 = stable row 50.
    let fs = FrameSelection::new(&sel, 45);
    assert!(
        !fs.contains(4, 5),
        "viewport line 4 (stable 49) should not be selected"
    );
    assert!(
        fs.contains(5, 5),
        "viewport line 5 (stable 50) should be selected"
    );
    assert!(
        !fs.contains(6, 5),
        "viewport line 6 (stable 51) should not be selected"
    );
}
