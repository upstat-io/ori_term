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
use crate::widgets::terminal_grid::TerminalGridWidget;
use oriterm_mux::pane::Pane;

/// Compact bitfield tracking which mouse buttons are currently pressed.
#[derive(Debug, Clone, Copy, Default)]
struct ButtonsDown(u8);

impl ButtonsDown {
    const LEFT: u8 = 1;
    const MIDDLE: u8 = 2;
    const RIGHT: u8 = 4;

    /// Set or clear the pressed state for a button.
    fn set(&mut self, button: winit::event::MouseButton, pressed: bool) {
        let bit = match button {
            winit::event::MouseButton::Left => Self::LEFT,
            winit::event::MouseButton::Middle => Self::MIDDLE,
            winit::event::MouseButton::Right => Self::RIGHT,
            _ => return,
        };
        if pressed {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
    }

    /// Whether the left button is held.
    fn left(self) -> bool {
        self.0 & Self::LEFT != 0
    }

    /// Whether the middle button is held.
    fn middle(self) -> bool {
        self.0 & Self::MIDDLE != 0
    }

    /// Whether the right button is held.
    fn right(self) -> bool {
        self.0 & Self::RIGHT != 0
    }

    /// Whether any button is held.
    fn any(self) -> bool {
        self.0 != 0
    }
}

/// Tracks mouse state for selection operations.
///
/// Stored on [`super::App`] and updated on `CursorMoved` / `MouseInput`
/// window events. Owns the click detector and drag state.
pub(crate) struct MouseState {
    /// Which mouse buttons are currently held.
    buttons: ButtonsDown,
    /// Pixel position of the initial press (for drag threshold).
    touchdown: Option<PhysicalPosition<f64>>,
    /// Whether the drag threshold has been exceeded (selection started).
    drag_active: bool,
    /// Multi-click detector (1 → char, 2 → word, 3 → line).
    click_detector: ClickDetector,
    /// Last known cursor position (for drag events).
    cursor_pos: PhysicalPosition<f64>,
    /// Last cell reported to the PTY for motion deduplication.
    last_reported_cell: Option<(usize, usize)>,
}

impl MouseState {
    /// Create a new idle mouse state.
    pub(crate) fn new() -> Self {
        Self {
            buttons: ButtonsDown::default(),
            touchdown: None,
            drag_active: false,
            click_detector: ClickDetector::new(),
            cursor_pos: PhysicalPosition::new(0.0, 0.0),
            last_reported_cell: None,
        }
    }

    /// Whether the left button is held (potential or active drag).
    pub(crate) fn left_down(&self) -> bool {
        self.buttons.left()
    }

    /// Whether the middle button is held.
    pub(crate) fn middle_down(&self) -> bool {
        self.buttons.middle()
    }

    /// Whether the right button is held.
    pub(crate) fn right_down(&self) -> bool {
        self.buttons.right()
    }

    /// Set the button-down state for a given button.
    pub(crate) fn set_button_down(&mut self, button: winit::event::MouseButton, pressed: bool) {
        self.buttons.set(button, pressed);
    }

    /// Whether any mouse button is currently held.
    pub(crate) fn any_button_down(&self) -> bool {
        self.buttons.any()
    }

    /// Whether a drag is currently active (threshold exceeded).
    pub(crate) fn is_dragging(&self) -> bool {
        self.buttons.left() && self.drag_active
    }

    /// Update the cursor position (called on every `CursorMoved`).
    pub(crate) fn set_cursor_pos(&mut self, pos: PhysicalPosition<f64>) {
        self.cursor_pos = pos;
    }

    /// Current cursor position.
    pub(crate) fn cursor_pos(&self) -> PhysicalPosition<f64> {
        self.cursor_pos
    }

    /// Last cell reported to the PTY (for motion deduplication).
    pub(crate) fn last_reported_cell(&self) -> Option<(usize, usize)> {
        self.last_reported_cell
    }

    /// Update the last reported cell for motion deduplication.
    pub(crate) fn set_last_reported_cell(&mut self, cell: Option<(usize, usize)>) {
        self.last_reported_cell = cell;
    }
}

/// Minimum pixel distance before a click becomes a drag.
///
/// Set to 1/4 cell width at runtime; this is the fallback.
const DRAG_THRESHOLD_PX: f64 = 2.0;

/// Grid layout context needed for pixel-to-cell conversion.
///
/// Bundles the terminal grid widget, cell metrics, and selection config
/// to avoid passing many individual parameters to mouse handling functions.
pub(crate) struct GridCtx<'a> {
    /// The terminal grid widget (provides layout bounds).
    pub(crate) widget: &'a TerminalGridWidget,
    /// Cell metrics (provides cell width/height).
    pub(crate) cell: CellMetrics,
    /// Word boundary delimiter characters for double-click selection.
    pub(crate) word_delimiters: &'a str,
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

    if x < f64::from(bounds.x())
        || y < f64::from(bounds.y())
        || x >= f64::from(bounds.right())
        || y >= f64::from(bounds.bottom())
    {
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
    pane: &mut Pane,
    ctx: &GridCtx<'_>,
    pos: PhysicalPosition<f64>,
    modifiers: ModifiersState,
) -> bool {
    let Some((col, line)) = pixel_to_cell(pos, ctx) else {
        return false;
    };
    let side = pixel_to_side(pos, ctx);

    // Record touchdown for drag threshold.
    mouse.touchdown = Some(pos);
    mouse.buttons.set(winit::event::MouseButton::Left, true);
    mouse.drag_active = false;

    // Click detection uses pixel-derived coordinates (before grid clamping).
    let click_count = mouse.click_detector.click(col, line);
    let shift = modifiers.shift_key();
    let alt = modifiers.alt_key();

    // Single lock: clamp, compute stable row, and conditionally compute
    // word/line boundaries for multi-click selections.
    let (col, stable_row, word_bounds, line_bounds) = {
        let term = pane.terminal().lock();
        let g = term.grid();
        let c = col.min(g.cols().saturating_sub(1));
        let l = line.min(g.lines().saturating_sub(1));
        let abs = g.scrollback().len().saturating_sub(g.display_offset()) + l;
        let c = redirect_spacer(g, abs, c);
        let stable = StableRowIndex::from_absolute(g, abs);

        let wb = if click_count == 2 {
            Some(word_boundaries(g, abs, c, ctx.word_delimiters))
        } else {
            None
        };
        let lb = if click_count >= 3 {
            Some((
                StableRowIndex::from_absolute(g, logical_line_start(g, abs)),
                StableRowIndex::from_absolute(g, logical_line_end(g, abs)),
                g.cols(),
            ))
        } else {
            None
        };

        (c, stable, wb, lb)
    };

    let action = classify_press(&PressInput {
        click_count,
        shift,
        alt,
        col,
        side,
        stable_row,
        word_bounds,
        line_bounds,
        existing_mode: pane.selection().map(|s| s.mode),
    });

    match action {
        PressAction::Extend(point) => {
            pane.update_selection_end(point);
            mouse.drag_active = true;
        }
        PressAction::New(selection) => {
            pane.set_selection(selection);
            if click_count >= 2 {
                mouse.drag_active = true;
            }
        }
    }
    true
}

/// Result of classifying a mouse press for selection creation.
#[derive(Debug)]
pub(crate) enum PressAction {
    /// Extend an existing selection to a new endpoint.
    Extend(SelectionPoint),
    /// Replace the current selection with a new one.
    New(Selection),
}

/// Input state for classifying a mouse press.
///
/// Bundles the computed click state and grid-resolved coordinates
/// needed to determine the selection action.
pub(crate) struct PressInput {
    /// Multi-click count (1 = char, 2 = word, 3 = line).
    pub click_count: u8,
    /// Whether Shift was held.
    pub shift: bool,
    /// Whether Alt was held.
    pub alt: bool,
    /// Grid column (clamped, spacer-redirected).
    pub col: usize,
    /// Which half of the cell was clicked.
    pub side: Side,
    /// Stable row of the click.
    pub stable_row: StableRowIndex,
    /// Word boundaries (start, end) for double-click.
    pub word_bounds: Option<(usize, usize)>,
    /// Line boundaries (`start_row`, `end_row`, cols) for triple-click.
    pub line_bounds: Option<(StableRowIndex, StableRowIndex, usize)>,
    /// Selection mode of the existing selection, if any.
    pub existing_mode: Option<SelectionMode>,
}

/// Determine the selection action for a mouse press.
///
/// Pure logic: given the click state and grid-resolved coordinates,
/// returns the appropriate selection action without side effects.
pub(crate) fn classify_press(input: &PressInput) -> PressAction {
    // Shift+click: extend existing selection.
    if input.shift && input.existing_mode.is_some() {
        return PressAction::Extend(SelectionPoint {
            row: input.stable_row,
            col: input.col,
            side: input.side,
        });
    }

    // Create new selection based on click count.
    let selection = match (input.click_count, input.word_bounds, input.line_bounds) {
        (2, Some((ws, we)), _) => {
            // Double-click: word selection.
            Selection::new_word(
                SelectionPoint {
                    row: input.stable_row,
                    col: ws,
                    side: Side::Left,
                },
                SelectionPoint {
                    row: input.stable_row,
                    col: we,
                    side: Side::Right,
                },
            )
        }
        (c, _, Some((ls, le, cols))) if c >= 3 => {
            // Triple-click: line selection (follows wrapped lines).
            Selection::new_line(
                SelectionPoint {
                    row: ls,
                    col: 0,
                    side: Side::Left,
                },
                SelectionPoint {
                    row: le,
                    col: cols.saturating_sub(1),
                    side: Side::Right,
                },
            )
        }
        _ => {
            // Single click: char selection. Alt toggles block mode.
            let mut sel = Selection::new_char(input.stable_row, input.col, input.side);
            if input.alt {
                let was_block = input.existing_mode == Some(SelectionMode::Block);
                sel.mode = if was_block {
                    SelectionMode::Char
                } else {
                    SelectionMode::Block
                };
            }
            sel
        }
    };

    PressAction::New(selection)
}

/// Handle mouse drag (cursor moved while button held).
///
/// Updates the selection endpoint. For word/line modes, snaps the endpoint
/// to the nearest boundary in the drag direction. Returns `true` if the
/// selection changed (redraw needed).
pub(crate) fn handle_drag(
    mouse: &mut MouseState,
    pane: &mut Pane,
    ctx: &GridCtx<'_>,
    pos: PhysicalPosition<f64>,
) -> bool {
    if !mouse.buttons.left() {
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
        update_drag_endpoint(pane, col, line, side, ctx.word_delimiters);
        return true;
    }

    // Mouse is outside the grid — handle auto-scroll.
    handle_auto_scroll(pane, pos, ctx);
    true
}

/// Handle left mouse button release.
///
/// Clears the drag state. The selection (if any) remains on the pane.
pub(crate) fn handle_release(mouse: &mut MouseState) {
    mouse.buttons.set(winit::event::MouseButton::Left, false);
    mouse.drag_active = false;
    mouse.touchdown = None;
}

/// Update the selection endpoint during drag, respecting mode-aware snapping.
fn update_drag_endpoint(
    pane: &mut Pane,
    col: usize,
    line: usize,
    side: Side,
    word_delimiters: &str,
) {
    // Read selection state before locking the terminal.
    let (sel_mode, sel_anchor) = match pane.selection() {
        Some(s) => (Some(s.mode), Some(s.anchor)),
        None => (None, None),
    };

    let new_end = {
        let term = pane.terminal().lock();
        let g = term.grid();
        let col = col.min(g.cols().saturating_sub(1));
        let line = line.min(g.lines().saturating_sub(1));
        let abs_row = g.scrollback().len().saturating_sub(g.display_offset()) + line;
        let col = redirect_spacer(g, abs_row, col);
        let stable_row = StableRowIndex::from_absolute(g, abs_row);

        match sel_mode {
            Some(SelectionMode::Word) => {
                let (ws, we) = word_boundaries(g, abs_row, col, word_delimiters);
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

    pane.update_selection_end(new_end);
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
///
/// After scrolling, updates the selection endpoint to the visible edge row
/// at the mouse's X column so the highlight extends with the scroll.
fn handle_auto_scroll(pane: &mut Pane, pos: PhysicalPosition<f64>, ctx: &GridCtx<'_>) {
    let Some(bounds) = ctx.widget.bounds() else {
        return;
    };
    let y = pos.y;
    let grid_top = f64::from(bounds.y());
    let ch = f64::from(ctx.cell.height);
    if ch <= 0.0 {
        return;
    }

    let side = pixel_to_side(pos, ctx);
    let scrolling_up = y < grid_top;

    // Determine scroll direction; bail if mouse is inside the grid or
    // already at the bottom of history.
    if scrolling_up {
        pane.scroll_display(1);
    } else {
        let (lines, offset) = {
            let term = pane.terminal().lock();
            let g = term.grid();
            (g.lines(), g.display_offset())
        };
        let grid_bottom = grid_top + lines as f64 * ch;
        if y < grid_bottom || offset == 0 {
            return;
        }
        pane.scroll_display(-1);
    }

    // After scrolling, compute endpoint for the visible edge row.
    let cw = f64::from(ctx.cell.width);
    let endpoint = {
        let term = pane.terminal().lock();
        let g = term.grid();
        let edge_line = if scrolling_up {
            0
        } else {
            g.lines().saturating_sub(1)
        };
        let abs = g.scrollback().len().saturating_sub(g.display_offset()) + edge_line;
        let col = if cw > 0.0 {
            ((pos.x - f64::from(bounds.x())) / cw) as usize
        } else {
            0
        };
        let col = col.min(g.cols().saturating_sub(1));
        let col = redirect_spacer(g, abs, col);
        let stable = StableRowIndex::from_absolute(g, abs);
        SelectionPoint {
            row: stable,
            col,
            side,
        }
    };

    pane.update_selection_end(endpoint);
}

#[cfg(test)]
mod tests;
