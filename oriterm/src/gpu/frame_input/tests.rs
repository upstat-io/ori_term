//! Unit tests for frame input types.

use oriterm_core::{Column, CursorShape, RenderableContent, RenderableCursor, Rgb, TermMode};

use super::{CellMetrics, FrameInput, FramePalette, ViewportSize};

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
    let m = CellMetrics::new(8.0, 16.0, 12.0);

    // 1920 / 8 = 240 columns.
    assert_eq!(m.columns(1920), 240);
    // 1080 / 16 = 67.5 → floor = 67 rows.
    assert_eq!(m.rows(1080), 67);
}

#[test]
fn cell_metrics_fractional_cell_size() {
    let m = CellMetrics::new(8.5, 17.0, 13.0);

    // 1920 / 8.5 = 225.88... → floor = 225.
    assert_eq!(m.columns(1920), 225);
    // 1080 / 17.0 = 63.52... → floor = 63.
    assert_eq!(m.rows(1080), 63);
}

#[test]
fn cell_metrics_small_viewport() {
    let m = CellMetrics::new(8.0, 16.0, 12.0);
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
}

// --- FrameInput ---

#[test]
fn frame_input_grid_dimensions() {
    let input = FrameInput {
        content: empty_content(),
        viewport: ViewportSize::new(800, 600),
        cell_size: CellMetrics::new(8.0, 16.0, 12.0),
        palette: test_palette(),
        selection: None,
        search_matches: Vec::new(),
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
        cell_size: CellMetrics::new(8.0, 16.0, 12.0),
        palette: test_palette(),
        selection: None,
        search_matches: Vec::new(),
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
        cell_size: CellMetrics::new(8.0, 16.0, 12.0),
        palette: test_palette(),
        selection: None,
        search_matches: Vec::new(),
    };

    assert!(!input.needs_full_repaint());
}
