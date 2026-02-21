//! Mouse-driven text selection: click detection, drag, and auto-scroll.
//!
//! Handles the full mouse selection lifecycle: press (single/double/triple
//! click, shift-extend, alt-block), drag (mode-aware endpoint updates with
//! word/line boundary snapping), and auto-scroll when dragging outside the
//! grid viewport.

use winit::dpi::PhysicalPosition;
use winit::keyboard::ModifiersState;

use oriterm_core::grid::Grid;
use oriterm_core::grid::StableRowIndex;
use oriterm_core::selection::{logical_line_end, logical_line_start, word_boundaries};
use oriterm_core::{
    CellFlags, ClickDetector, Column, Selection, SelectionMode, SelectionPoint, Side,
};

use crate::font::CellMetrics;
use crate::tab::Tab;
use crate::widgets::terminal_grid::TerminalGridWidget;

/// Tracks mouse state for selection operations.
///
/// Stored on [`super::App`] and updated on `CursorMoved` / `MouseInput`
/// window events. Owns the click detector and drag state.
pub(crate) struct MouseState {
    /// Whether the left mouse button is currently held.
    left_down: bool,
    /// Pixel position of the initial press (for drag threshold).
    touchdown: Option<PhysicalPosition<f64>>,
    /// Whether the drag threshold has been exceeded (selection started).
    drag_active: bool,
    /// Multi-click detector (1 → char, 2 → word, 3 → line).
    click_detector: ClickDetector,
    /// Last known cursor position (for drag events).
    cursor_pos: PhysicalPosition<f64>,
}

impl MouseState {
    /// Create a new idle mouse state.
    pub(crate) fn new() -> Self {
        Self {
            left_down: false,
            touchdown: None,
            drag_active: false,
            click_detector: ClickDetector::new(),
            cursor_pos: PhysicalPosition::new(0.0, 0.0),
        }
    }

    /// Whether the left button is held (potential or active drag).
    pub(crate) fn left_down(&self) -> bool {
        self.left_down
    }

    /// Whether a drag is currently active (threshold exceeded).
    #[allow(dead_code, reason = "used for cursor shape changes and tests")]
    pub(crate) fn is_dragging(&self) -> bool {
        self.left_down && self.drag_active
    }

    /// Update the cursor position (called on every `CursorMoved`).
    pub(crate) fn set_cursor_pos(&mut self, pos: PhysicalPosition<f64>) {
        self.cursor_pos = pos;
    }

    /// Current cursor position.
    pub(crate) fn cursor_pos(&self) -> PhysicalPosition<f64> {
        self.cursor_pos
    }
}

/// Minimum pixel distance before a click becomes a drag.
///
/// Set to 1/4 cell width at runtime; this is the fallback.
const DRAG_THRESHOLD_PX: f64 = 2.0;

/// Grid layout context needed for pixel-to-cell conversion.
///
/// Bundles the terminal grid widget and cell metrics to avoid passing
/// many individual parameters to mouse handling functions.
pub(crate) struct GridCtx<'a> {
    /// The terminal grid widget (provides layout bounds).
    pub(crate) widget: &'a TerminalGridWidget,
    /// Cell metrics (provides cell width/height).
    pub(crate) cell: CellMetrics,
}

/// Convert a pixel position to grid cell coordinates (col, `viewport_line`).
///
/// Returns `None` if the position is outside the grid area. Uses the
/// terminal grid widget's layout bounds and cell metrics for conversion.
pub(crate) fn pixel_to_cell(
    pos: PhysicalPosition<f64>,
    ctx: &GridCtx<'_>,
) -> Option<(usize, usize)> {
    let bounds = ctx.widget.bounds()?;
    let x = pos.x;
    let y = pos.y;

    if x < f64::from(bounds.x()) || y < f64::from(bounds.y()) {
        return None;
    }

    let cw = f64::from(ctx.cell.width);
    let ch = f64::from(ctx.cell.height);
    if cw <= 0.0 || ch <= 0.0 {
        return None;
    }

    let col = ((x - f64::from(bounds.x())) / cw) as usize;
    let line = ((y - f64::from(bounds.y())) / ch) as usize;
    Some((col, line))
}

/// Determine which half of the cell the cursor is on.
pub(crate) fn pixel_to_side(pos: PhysicalPosition<f64>, ctx: &GridCtx<'_>) -> Side {
    let cw = f64::from(ctx.cell.width);
    if cw <= 0.0 {
        return Side::Left;
    }
    let grid_x = ctx.widget.bounds().map_or(0.0, |b| f64::from(b.x()));
    let cell_x = (pos.x - grid_x).rem_euclid(cw);
    if cell_x < cw / 2.0 {
        Side::Left
    } else {
        Side::Right
    }
}

/// Handle a left mouse button press in the grid area.
///
/// Creates or extends a selection based on click count and modifiers.
/// Returns `true` if the press was handled (redraw needed).
pub(crate) fn handle_press(
    mouse: &mut MouseState,
    tab: &mut Tab,
    ctx: &GridCtx<'_>,
    pos: PhysicalPosition<f64>,
    modifiers: ModifiersState,
) -> bool {
    let Some((col, line)) = pixel_to_cell(pos, ctx) else {
        return false;
    };
    let side = pixel_to_side(pos, ctx);

    // Clamp to grid bounds, redirect wide-char spacers, compute stable row.
    let (col, grid_cols, stable_row, abs_row) = {
        let term = tab.terminal().lock();
        let g = term.grid();
        let c = col.min(g.cols().saturating_sub(1));
        let l = line.min(g.lines().saturating_sub(1));
        let abs = g.scrollback().len().saturating_sub(g.display_offset()) + l;
        let c = redirect_spacer(g, abs, c);
        let stable = StableRowIndex::from_absolute(g, abs);
        (c, g.cols(), stable, abs)
    };

    // Record touchdown for drag threshold.
    mouse.touchdown = Some(pos);
    mouse.left_down = true;
    mouse.drag_active = false;

    let click_count = mouse.click_detector.click(col, line);
    let shift = modifiers.shift_key();
    let alt = modifiers.alt_key();

    // Shift+click: extend existing selection.
    if shift && tab.selection().is_some() {
        tab.update_selection_end(SelectionPoint {
            row: stable_row,
            col,
            side,
        });
        mouse.drag_active = true;
        return true;
    }

    // Create new selection based on click count.
    let selection = match click_count {
        2 => {
            // Double-click: word selection.
            let term = tab.terminal().lock();
            let g = term.grid();
            let (ws, we) = word_boundaries(g, abs_row, col);
            Selection::new_word(
                SelectionPoint {
                    row: stable_row,
                    col: ws,
                    side: Side::Left,
                },
                SelectionPoint {
                    row: stable_row,
                    col: we,
                    side: Side::Right,
                },
            )
        }
        3 => {
            // Triple-click: line selection (follows wrapped lines).
            let term = tab.terminal().lock();
            let g = term.grid();
            let ls = logical_line_start(g, abs_row);
            let le = logical_line_end(g, abs_row);
            Selection::new_line(
                SelectionPoint {
                    row: StableRowIndex::from_absolute(g, ls),
                    col: 0,
                    side: Side::Left,
                },
                SelectionPoint {
                    row: StableRowIndex::from_absolute(g, le),
                    col: grid_cols.saturating_sub(1),
                    side: Side::Right,
                },
            )
        }
        _ => {
            // Single click: char selection. Alt toggles block mode.
            let mut sel = Selection::new_char(stable_row, col, side);
            if alt {
                let was_block = tab
                    .selection()
                    .is_some_and(|s| s.mode == SelectionMode::Block);
                sel.mode = if was_block {
                    SelectionMode::Char
                } else {
                    SelectionMode::Block
                };
            }
            sel
        }
    };

    tab.set_selection(selection);
    // For double/triple clicks, drag is immediately active (no threshold).
    if click_count >= 2 {
        mouse.drag_active = true;
    }
    true
}

/// Handle mouse drag (cursor moved while button held).
///
/// Updates the selection endpoint. For word/line modes, snaps the endpoint
/// to the nearest boundary in the drag direction. Returns `true` if the
/// selection changed (redraw needed).
pub(crate) fn handle_drag(
    mouse: &mut MouseState,
    tab: &mut Tab,
    ctx: &GridCtx<'_>,
    pos: PhysicalPosition<f64>,
) -> bool {
    if !mouse.left_down {
        return false;
    }

    // Check drag threshold before first activation.
    if !mouse.drag_active {
        if let Some(td) = mouse.touchdown {
            let threshold = (f64::from(ctx.cell.width) / 4.0).max(DRAG_THRESHOLD_PX);
            let dx = pos.x - td.x;
            let dy = pos.y - td.y;
            if dx * dx + dy * dy < threshold * threshold {
                return false;
            }
        }
        mouse.drag_active = true;
    }

    // Try to convert pixel to cell within the grid area.
    if let Some((col, line)) = pixel_to_cell(pos, ctx) {
        let side = pixel_to_side(pos, ctx);
        update_drag_endpoint(tab, col, line, side);
        return true;
    }

    // Mouse is outside the grid — handle auto-scroll.
    handle_auto_scroll(tab, pos, ctx);
    true
}

/// Handle left mouse button release.
///
/// Clears the drag state. The selection (if any) remains on the tab.
pub(crate) fn handle_release(mouse: &mut MouseState) {
    mouse.left_down = false;
    mouse.drag_active = false;
    mouse.touchdown = None;
}

/// Update the selection endpoint during drag, respecting mode-aware snapping.
fn update_drag_endpoint(tab: &mut Tab, col: usize, line: usize, side: Side) {
    // Read selection state before locking the terminal.
    let sel_mode = tab.selection().map(|s| s.mode);
    let sel_anchor = tab.selection().map(|s| s.anchor);

    let new_end = {
        let term = tab.terminal().lock();
        let g = term.grid();
        let col = col.min(g.cols().saturating_sub(1));
        let line = line.min(g.lines().saturating_sub(1));
        let abs_row = g.scrollback().len().saturating_sub(g.display_offset()) + line;
        let col = redirect_spacer(g, abs_row, col);
        let stable_row = StableRowIndex::from_absolute(g, abs_row);

        match sel_mode {
            Some(SelectionMode::Word) => {
                let (ws, we) = word_boundaries(g, abs_row, col);
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
                    start_pt
                } else {
                    end_pt
                }
            }
            Some(SelectionMode::Line) => {
                let ls = logical_line_start(g, abs_row);
                let le = logical_line_end(g, abs_row);
                let grid_cols = g.cols();
                // Snap to line boundary in the drag direction.
                if sel_anchor.is_some_and(|a| stable_row < a.row) {
                    SelectionPoint {
                        row: StableRowIndex::from_absolute(g, ls),
                        col: 0,
                        side: Side::Left,
                    }
                } else {
                    SelectionPoint {
                        row: StableRowIndex::from_absolute(g, le),
                        col: grid_cols.saturating_sub(1),
                        side: Side::Right,
                    }
                }
            }
            Some(_) => SelectionPoint {
                row: stable_row,
                col,
                side,
            },
            None => return,
        }
    };

    tab.update_selection_end(new_end);
}

/// Redirect a column to the base cell if it lands on a wide char spacer.
///
/// Wide characters occupy two cells: the base cell and a trailing spacer.
/// Clicking on the spacer should act as if the user clicked on the base cell.
fn redirect_spacer(grid: &Grid, abs_row: usize, col: usize) -> usize {
    if col == 0 {
        return col;
    }
    let Some(row) = grid.absolute_row(abs_row) else {
        return col;
    };
    if col < row.cols() && row[Column(col)].flags.contains(CellFlags::WIDE_CHAR_SPACER) {
        col - 1
    } else {
        col
    }
}

/// Auto-scroll the viewport when the mouse is above or below the grid.
fn handle_auto_scroll(tab: &Tab, pos: PhysicalPosition<f64>, ctx: &GridCtx<'_>) {
    let Some(bounds) = ctx.widget.bounds() else {
        return;
    };
    let y = pos.y;
    let grid_top = f64::from(bounds.y());
    let ch = f64::from(ctx.cell.height);
    if ch <= 0.0 {
        return;
    }

    let mut term = tab.terminal().lock();
    let g = term.grid_mut();

    if y < grid_top {
        // Mouse above grid: scroll into history.
        g.scroll_display(1);
    } else {
        let grid_bottom = grid_top + g.lines() as f64 * ch;
        if y >= grid_bottom && g.display_offset() > 0 {
            // Mouse below grid: scroll toward live.
            g.scroll_display(-1);
        }
    }
}

#[cfg(test)]
mod tests;
