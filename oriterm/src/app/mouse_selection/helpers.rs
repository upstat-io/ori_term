//! Private helpers for mouse selection: drag snapping, spacer redirect,
//! and auto-scroll.
//!
//! Extracted from `mod.rs` to keep file sizes under the 500-line limit.
//! All grid queries operate on [`SnapshotGrid`] — no terminal lock required.

use winit::dpi::PhysicalPosition;

use oriterm_core::{Selection, SelectionMode, SelectionPoint, Side};

use super::{GridCtx, pixel_to_side};
use crate::app::snapshot_grid::SnapshotGrid;

/// Compute the selection endpoint during drag, respecting mode-aware snapping.
///
/// Returns `Some(endpoint)` if the selection has a mode, `None` otherwise.
/// The caller applies the endpoint to App selection state.
#[expect(
    clippy::too_many_arguments,
    reason = "drag endpoint: grid, selection, col, line, side, delimiters"
)]
pub(super) fn compute_drag_endpoint(
    grid: &SnapshotGrid<'_>,
    selection: Option<&Selection>,
    col: usize,
    line: usize,
    side: Side,
    word_delimiters: &str,
) -> Option<SelectionPoint> {
    let (sel_mode, sel_anchor) = match selection {
        Some(s) => (Some(s.mode), Some(s.anchor)),
        None => return None,
    };

    let col = col.min(grid.cols().saturating_sub(1));
    let line = line.min(grid.lines().saturating_sub(1));
    let col = grid.redirect_spacer(line, col);
    let stable_row = grid.viewport_to_stable_row(line);

    match sel_mode {
        Some(SelectionMode::Word) => {
            let (ws, we) = grid.word_boundaries(line, col, word_delimiters);
            let start_pt = SelectionPoint {
                row: stable_row,
                col: ws,
                side: Side::Left,
            };
            let end_pt = SelectionPoint {
                row: stable_row,
                col: we,
                side: Side::Right,
            };
            // Snap to word boundary in the drag direction.
            if sel_anchor.is_some_and(|a| start_pt < a) {
                Some(start_pt)
            } else {
                Some(end_pt)
            }
        }
        Some(SelectionMode::Line) => {
            let ls = grid.logical_line_start(line);
            let le = grid.logical_line_end(line);
            // Snap to line boundary in the drag direction.
            if sel_anchor.is_some_and(|a| stable_row < a.row) {
                Some(SelectionPoint {
                    row: grid.viewport_to_stable_row(ls),
                    col: 0,
                    side: Side::Left,
                })
            } else {
                Some(SelectionPoint {
                    row: grid.viewport_to_stable_row(le),
                    col: grid.cols().saturating_sub(1),
                    side: Side::Right,
                })
            }
        }
        Some(_) => Some(SelectionPoint {
            row: stable_row,
            col,
            side,
        }),
        None => None,
    }
}

/// Compute the auto-scroll delta when the mouse is above or below the grid.
///
/// Returns `Some((delta, scrolling_up))` if scrolling is needed, or `None`
/// if the mouse is inside the grid or already at the scroll limit.
/// The caller applies the scroll via `MuxBackend::scroll_display`, then
/// calls [`compute_auto_scroll_endpoint`] to update the selection.
pub(super) fn auto_scroll_delta(
    grid: &SnapshotGrid<'_>,
    pos: PhysicalPosition<f64>,
    ctx: &GridCtx<'_>,
) -> Option<(isize, bool)> {
    let bounds = ctx.widget.bounds()?;
    let y = pos.y;
    let grid_top = f64::from(bounds.y());
    let ch = f64::from(ctx.cell.height);
    if ch <= 0.0 {
        return None;
    }

    if y < grid_top {
        // Mouse above grid — scroll up into history.
        return Some((1, true));
    }

    // Mouse below grid — scroll down toward live.
    let grid_bottom = grid_top + grid.lines() as f64 * ch;
    if y >= grid_bottom && grid.display_offset() > 0 {
        return Some((-1, false));
    }

    None
}

/// Compute the selection endpoint after an auto-scroll has been applied.
///
/// The scroll has already been performed via `MuxBackend`. The grid argument
/// should be a freshly-constructed `SnapshotGrid` from the post-scroll snapshot.
/// Returns the endpoint at the visible edge row at the mouse's X column.
pub(crate) fn compute_auto_scroll_endpoint(
    grid: &SnapshotGrid<'_>,
    pos: PhysicalPosition<f64>,
    ctx: &GridCtx<'_>,
    scrolling_up: bool,
) -> SelectionPoint {
    let side = pixel_to_side(pos, ctx);
    let cw = f64::from(ctx.cell.width);
    let grid_x = ctx.widget.bounds().map_or(0.0, |b| f64::from(b.x()));

    let edge_line = if scrolling_up {
        0
    } else {
        grid.lines().saturating_sub(1)
    };
    let col = if cw > 0.0 {
        ((pos.x - grid_x) / cw) as usize
    } else {
        0
    };
    let col = col.min(grid.cols().saturating_sub(1));
    let col = grid.redirect_spacer(edge_line, col);
    let stable = grid.viewport_to_stable_row(edge_line);

    SelectionPoint {
        row: stable,
        col,
        side,
    }
}
