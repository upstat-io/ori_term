//! Tests for mouse selection coordinate conversion and state tracking.

use std::time::Instant;

use winit::dpi::PhysicalPosition;

use oriterm_core::Side;
use oriterm_ui::draw::DrawList;
use oriterm_ui::geometry::Rect;
use oriterm_ui::text::{ShapedText, TextMetrics, TextStyle};
use oriterm_ui::theme::UiTheme;
use oriterm_ui::widgets::text_measurer::TextMeasurer;
use oriterm_ui::widgets::{DrawCtx, Widget};

use super::{
    DRAG_THRESHOLD_PX, GridCtx, MouseState, pixel_to_cell, pixel_to_side, redirect_spacer,
};
use crate::font::CellMetrics;
use crate::widgets::terminal_grid::TerminalGridWidget;

/// Minimal text measurer for tests.
struct TestMeasurer;

impl TextMeasurer for TestMeasurer {
    fn measure(&self, text: &str, _style: &TextStyle, _max_width: f32) -> TextMetrics {
        TextMetrics {
            width: text.len() as f32 * 8.0,
            height: 16.0,
            line_count: 1,
        }
    }

    fn shape(&self, _text: &str, _style: &TextStyle, _max_width: f32) -> ShapedText {
        ShapedText {
            glyphs: Vec::new(),
            width: 0.0,
            height: 16.0,
            baseline: 12.0,
        }
    }
}

fn test_cell_metrics(w: f32, h: f32) -> CellMetrics {
    CellMetrics {
        width: w,
        height: h,
        baseline: h * 0.8,
        underline_offset: 2.0,
        stroke_size: 1.0,
        strikeout_offset: h * 0.3,
    }
}

/// Build a grid widget with bounds set at a given origin.
fn make_widget_with_bounds(
    cell_w: f32,
    cell_h: f32,
    cols: usize,
    rows: usize,
    origin_x: f32,
    origin_y: f32,
) -> TerminalGridWidget {
    let widget = TerminalGridWidget::new(cell_w, cell_h, cols, rows);
    let theme = UiTheme::dark();
    let measurer = TestMeasurer;
    let mut draw_list = DrawList::new();
    let animations_running = std::cell::Cell::new(false);
    let bounds = Rect::new(
        origin_x,
        origin_y,
        cols as f32 * cell_w,
        rows as f32 * cell_h,
    );
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: None,
        now: Instant::now(),
        animations_running: &animations_running,
        theme: &theme,
    };
    widget.draw(&mut ctx);
    widget
}

fn ctx_at_origin(
    cell_w: f32,
    cell_h: f32,
    cols: usize,
    rows: usize,
) -> (TerminalGridWidget, CellMetrics) {
    let widget = make_widget_with_bounds(cell_w, cell_h, cols, rows, 0.0, 0.0);
    let cell = test_cell_metrics(cell_w, cell_h);
    (widget, cell)
}

fn grid_ctx(widget: &TerminalGridWidget, cell: CellMetrics) -> GridCtx<'_> {
    GridCtx { widget, cell }
}

// --- pixel_to_cell ---

#[test]
fn cell_at_origin() {
    let (w, c) = ctx_at_origin(8.0, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    assert_eq!(
        pixel_to_cell(PhysicalPosition::new(0.0, 0.0), &ctx),
        Some((0, 0))
    );
}

#[test]
fn cell_mid_grid() {
    let (w, c) = ctx_at_origin(8.0, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    // Column 5, line 3: pixel (44.0, 52.0).
    assert_eq!(
        pixel_to_cell(PhysicalPosition::new(44.0, 52.0), &ctx),
        Some((5, 3))
    );
}

#[test]
fn cell_last() {
    let (w, c) = ctx_at_origin(8.0, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    // Last pixel of last cell: column 79, row 23.
    assert_eq!(
        pixel_to_cell(PhysicalPosition::new(639.0, 383.0), &ctx),
        Some((79, 23))
    );
}

#[test]
fn cell_negative_x_returns_none() {
    let (w, c) = ctx_at_origin(8.0, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    assert_eq!(pixel_to_cell(PhysicalPosition::new(-1.0, 5.0), &ctx), None);
}

#[test]
fn cell_negative_y_returns_none() {
    let (w, c) = ctx_at_origin(8.0, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    assert_eq!(pixel_to_cell(PhysicalPosition::new(5.0, -1.0), &ctx), None);
}

#[test]
fn cell_no_bounds_returns_none() {
    let widget = TerminalGridWidget::new(8.0, 16.0, 80, 24);
    let cell = test_cell_metrics(8.0, 16.0);
    let ctx = grid_ctx(&widget, cell);
    assert_eq!(pixel_to_cell(PhysicalPosition::new(40.0, 40.0), &ctx), None);
}

#[test]
fn cell_with_offset_origin() {
    let widget = make_widget_with_bounds(8.0, 16.0, 80, 24, 10.0, 50.0);
    let cell = test_cell_metrics(8.0, 16.0);
    let ctx = grid_ctx(&widget, cell);

    // Before grid origin: None.
    assert_eq!(pixel_to_cell(PhysicalPosition::new(5.0, 55.0), &ctx), None);
    assert_eq!(pixel_to_cell(PhysicalPosition::new(15.0, 45.0), &ctx), None);

    // At grid origin: (0, 0).
    assert_eq!(
        pixel_to_cell(PhysicalPosition::new(10.0, 50.0), &ctx),
        Some((0, 0))
    );

    // Column 2, line 1: pixel (26.0, 66.0).
    assert_eq!(
        pixel_to_cell(PhysicalPosition::new(26.0, 66.0), &ctx),
        Some((2, 1))
    );
}

// --- pixel_to_side ---

#[test]
fn side_left_half() {
    let (w, c) = ctx_at_origin(10.0, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    assert_eq!(
        pixel_to_side(PhysicalPosition::new(3.0, 0.0), &ctx),
        Side::Left
    );
}

#[test]
fn side_right_half() {
    let (w, c) = ctx_at_origin(10.0, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    assert_eq!(
        pixel_to_side(PhysicalPosition::new(7.0, 0.0), &ctx),
        Side::Right
    );
}

#[test]
fn side_midpoint_is_right() {
    let (w, c) = ctx_at_origin(10.0, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    // Exactly at midpoint (5.0 of 10.0) — right half.
    assert_eq!(
        pixel_to_side(PhysicalPosition::new(5.0, 0.0), &ctx),
        Side::Right
    );
}

#[test]
fn side_second_cell() {
    let (w, c) = ctx_at_origin(10.0, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    // Cell 1: pixels 10..20. Offset 2 within cell → left.
    assert_eq!(
        pixel_to_side(PhysicalPosition::new(12.0, 0.0), &ctx),
        Side::Left
    );
    // Offset 7 within cell → right.
    assert_eq!(
        pixel_to_side(PhysicalPosition::new(17.0, 0.0), &ctx),
        Side::Right
    );
}

#[test]
fn side_with_offset_origin() {
    let widget = make_widget_with_bounds(10.0, 16.0, 80, 24, 5.0, 0.0);
    let cell = test_cell_metrics(10.0, 16.0);
    let ctx = grid_ctx(&widget, cell);
    // X=7.0 with grid at 5.0: offset within cell = (7-5) % 10 = 2 → left.
    assert_eq!(
        pixel_to_side(PhysicalPosition::new(7.0, 0.0), &ctx),
        Side::Left
    );
    // X=12.0 with grid at 5.0: offset within cell = (12-5) % 10 = 7 → right.
    assert_eq!(
        pixel_to_side(PhysicalPosition::new(12.0, 0.0), &ctx),
        Side::Right
    );
}

// --- MouseState ---

#[test]
fn mouse_state_initial() {
    let m = MouseState::new();
    assert!(!m.is_dragging());
    assert_eq!(m.cursor_pos().x, 0.0);
    assert_eq!(m.cursor_pos().y, 0.0);
}

#[test]
fn mouse_state_cursor_tracking() {
    let mut m = MouseState::new();
    m.set_cursor_pos(PhysicalPosition::new(100.0, 200.0));
    assert_eq!(m.cursor_pos().x, 100.0);
    assert_eq!(m.cursor_pos().y, 200.0);
}

#[test]
fn mouse_state_not_dragging_when_not_down() {
    let m = MouseState::new();
    assert!(!m.is_dragging());
}

#[test]
fn mouse_state_release_clears_drag() {
    use super::handle_release;

    let mut m = MouseState::new();
    m.left_down = true;
    m.drag_active = true;
    assert!(m.is_dragging());

    handle_release(&mut m);
    assert!(!m.is_dragging());
}

// --- Off-grid / boundary edge cases ---

#[test]
fn cell_beyond_grid_still_returns_some() {
    // pixel_to_cell doesn't clamp — it returns raw col/row.
    // Clamping to grid bounds happens in handle_press/update_drag_endpoint.
    let (w, c) = ctx_at_origin(8.0, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    // col 100 > 80 grid cols — pixel_to_cell still returns the raw value.
    let result = pixel_to_cell(PhysicalPosition::new(800.0, 100.0), &ctx);
    assert_eq!(result, Some((100, 6)));
}

#[test]
fn cell_at_exact_boundary() {
    let (w, c) = ctx_at_origin(8.0, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    // Exactly at cell boundary: pixel 8.0 = start of column 1.
    assert_eq!(
        pixel_to_cell(PhysicalPosition::new(8.0, 0.0), &ctx),
        Some((1, 0))
    );
    // Pixel 7.999.. is still column 0.
    assert_eq!(
        pixel_to_cell(PhysicalPosition::new(7.99, 0.0), &ctx),
        Some((0, 0))
    );
}

#[test]
fn cell_at_exact_row_boundary() {
    let (w, c) = ctx_at_origin(8.0, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    // Exactly at row boundary: pixel 16.0 = start of row 1.
    assert_eq!(
        pixel_to_cell(PhysicalPosition::new(0.0, 16.0), &ctx),
        Some((0, 1))
    );
}

// --- Fractional cell sizes ---

#[test]
fn cell_fractional_cell_width() {
    // Non-integer cell width (common with real fonts).
    let (w, c) = ctx_at_origin(7.5, 15.5, 80, 24);
    let ctx = grid_ctx(&w, c);
    // Column 1 starts at 7.5px.
    assert_eq!(
        pixel_to_cell(PhysicalPosition::new(7.5, 0.0), &ctx),
        Some((1, 0))
    );
    // Column 0 at 7.4px.
    assert_eq!(
        pixel_to_cell(PhysicalPosition::new(7.4, 0.0), &ctx),
        Some((0, 0))
    );
}

#[test]
fn side_fractional_cell_width() {
    let (w, c) = ctx_at_origin(7.5, 16.0, 80, 24);
    let ctx = grid_ctx(&w, c);
    // Cell width 7.5. Midpoint at 3.75.
    // 3.0 < 3.75 → Left.
    assert_eq!(
        pixel_to_side(PhysicalPosition::new(3.0, 0.0), &ctx),
        Side::Left
    );
    // 4.0 >= 3.75 → Right.
    assert_eq!(
        pixel_to_side(PhysicalPosition::new(4.0, 0.0), &ctx),
        Side::Right
    );
}

// --- Drag threshold ---

#[test]
fn drag_threshold_constant_is_positive() {
    assert!(DRAG_THRESHOLD_PX > 0.0);
}

#[test]
fn drag_not_active_when_button_not_down() {
    let mut m = MouseState::new();
    // left_down is false — handle_drag would return false immediately.
    // We verify the guard via MouseState directly (can't call handle_drag
    // without a real Tab).
    assert!(!m.is_dragging());
    assert!(!m.left_down);

    // Both left_down AND drag_active must be true for is_dragging.
    m.left_down = false;
    m.drag_active = true;
    assert!(!m.is_dragging());
}

#[test]
fn drag_threshold_requires_distance() {
    // Verify the threshold computation: max(cell_width / 4, DRAG_THRESHOLD_PX).
    let cell_w = 8.0_f64;
    let threshold = (cell_w / 4.0).max(DRAG_THRESHOLD_PX);
    assert_eq!(threshold, 2.0); // 8/4 = 2, max(2, 2) = 2.

    // Small cell: threshold floors at DRAG_THRESHOLD_PX.
    let small_cell = 4.0_f64;
    let threshold = (small_cell / 4.0).max(DRAG_THRESHOLD_PX);
    assert_eq!(threshold, 2.0); // 4/4 = 1, max(1, 2) = 2.

    // Large cell: threshold scales with cell width.
    let large_cell = 20.0_f64;
    let threshold = (large_cell / 4.0).max(DRAG_THRESHOLD_PX);
    assert_eq!(threshold, 5.0); // 20/4 = 5, max(5, 2) = 5.
}

#[test]
fn drag_threshold_distance_check() {
    // Simulate the squared-distance check from handle_drag.
    let td = PhysicalPosition::new(50.0, 50.0);
    let threshold = 2.0_f64;

    // Sub-threshold move.
    let pos1 = PhysicalPosition::new(50.5, 50.5);
    let dx1 = pos1.x - td.x;
    let dy1 = pos1.y - td.y;
    assert!(dx1 * dx1 + dy1 * dy1 < threshold * threshold);

    // Over-threshold move.
    let pos2 = PhysicalPosition::new(55.0, 50.0);
    let dx2 = pos2.x - td.x;
    let dy2 = pos2.y - td.y;
    assert!(dx2 * dx2 + dy2 * dy2 >= threshold * threshold);

    // Exactly at threshold boundary.
    let pos3 = PhysicalPosition::new(52.0, 50.0);
    let dx3 = pos3.x - td.x;
    let dy3 = pos3.y - td.y;
    assert!(dx3 * dx3 + dy3 * dy3 >= threshold * threshold);
}

// --- redirect_spacer ---

#[test]
fn redirect_spacer_normal_cell() {
    use oriterm_core::grid::Grid;
    let grid = Grid::new(5, 10);
    // Col 3 on an empty grid has no WIDE_CHAR_SPACER flag.
    assert_eq!(redirect_spacer(&grid, 0, 3), 3);
}

#[test]
fn redirect_spacer_col_zero() {
    use oriterm_core::grid::Grid;
    let grid = Grid::new(5, 10);
    // Col 0 can never redirect (would go to -1).
    assert_eq!(redirect_spacer(&grid, 0, 0), 0);
}

#[test]
fn redirect_spacer_out_of_bounds_row() {
    use oriterm_core::grid::Grid;
    let grid = Grid::new(5, 10);
    // Absolute row 999 doesn't exist — should return col unchanged.
    assert_eq!(redirect_spacer(&grid, 999, 5), 5);
}

#[test]
fn redirect_spacer_wide_char() {
    use oriterm_core::grid::Grid;
    use oriterm_core::{CellFlags, Column, Line};

    let mut grid = Grid::new(5, 10);
    // Set up a wide char at col 2, spacer at col 3.
    // Grid is scrollback(0) + visible(5), so abs row 0 = visible row 0.
    grid[Line(0)][Column(2)].flags |= CellFlags::WIDE_CHAR;
    grid[Line(0)][Column(3)].flags |= CellFlags::WIDE_CHAR_SPACER;

    // Click on spacer at col 3 → redirected to col 2.
    assert_eq!(redirect_spacer(&grid, 0, 3), 2);
    // Click on base cell at col 2 → stays at col 2.
    assert_eq!(redirect_spacer(&grid, 0, 2), 2);
    // Click on normal cell at col 4 → stays at col 4.
    assert_eq!(redirect_spacer(&grid, 0, 4), 4);
}
