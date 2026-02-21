//! Terminal grid widget — the terminal as a participant in the UI layout.
//!
//! Implements the [`Widget`] trait with `Fill × Fill` sizing so the grid
//! expands to fill remaining space after tab bar, status bar, etc. Rendering
//! is handled by the existing GPU prepare pipeline (not `DrawList`); this
//! widget participates in layout and event routing only.

use std::cell::Cell;

use oriterm_ui::geometry::Rect;
use oriterm_ui::input::{HoverEvent, KeyEvent, MouseEvent};
use oriterm_ui::layout::{LayoutBox, SizeSpec};
use oriterm_ui::widget_id::WidgetId;
use oriterm_ui::widgets::{DrawCtx, EventCtx, LayoutCtx, Widget, WidgetResponse};

/// The terminal grid as a UI widget.
///
/// Does not render cells via `DrawList` — the existing prepare pipeline
/// handles cell rendering. This widget exists to participate in layout
/// (reporting `Fill × Fill` sizing) and event routing (claiming keyboard
/// and mouse events when focused).
pub(crate) struct TerminalGridWidget {
    /// Unique widget ID.
    id: WidgetId,
    /// Cell width in pixels (from font metrics).
    cell_width: f32,
    /// Cell height in pixels (from font metrics).
    cell_height: f32,
    /// Number of grid columns.
    cols: usize,
    /// Number of grid rows.
    rows: usize,
    /// Computed layout bounds, stored during `draw()`.
    bounds: Cell<Option<Rect>>,
}

impl TerminalGridWidget {
    /// Creates a new terminal grid widget with the given cell metrics.
    pub(crate) fn new(cell_width: f32, cell_height: f32, cols: usize, rows: usize) -> Self {
        Self {
            id: WidgetId::next(),
            cell_width,
            cell_height,
            cols,
            rows,
            bounds: Cell::new(None),
        }
    }

    /// Returns the computed layout bounds, or `None` if `draw()` hasn't
    /// been called yet.
    pub(crate) fn bounds(&self) -> Option<Rect> {
        self.bounds.get()
    }

    /// Updates cell dimensions from font metrics.
    pub(crate) fn set_cell_metrics(&mut self, cell_width: f32, cell_height: f32) {
        self.cell_width = cell_width;
        self.cell_height = cell_height;
    }

    /// Updates the grid size (columns and rows).
    pub(crate) fn set_grid_size(&mut self, cols: usize, rows: usize) {
        self.cols = cols;
        self.rows = rows;
    }

    /// Directly set the layout bounds (bypasses the full layout engine).
    ///
    /// Used during init and resize before the layout engine is wired.
    /// Once the layout engine drives widget placement this can be removed.
    pub(crate) fn set_bounds(&self, bounds: Rect) {
        self.bounds.set(Some(bounds));
    }

    /// Current grid columns.
    #[cfg(test)]
    pub(crate) fn cols(&self) -> usize {
        self.cols
    }

    /// Current grid rows.
    #[cfg(test)]
    pub(crate) fn rows(&self) -> usize {
        self.rows
    }

    /// Current cell width.
    #[cfg(test)]
    pub(crate) fn cell_width(&self) -> f32 {
        self.cell_width
    }

    /// Current cell height.
    #[cfg(test)]
    pub(crate) fn cell_height(&self) -> f32 {
        self.cell_height
    }
}

impl Widget for TerminalGridWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn layout(&self, _ctx: &LayoutCtx<'_>) -> LayoutBox {
        LayoutBox::leaf(
            self.cols as f32 * self.cell_width,
            self.rows as f32 * self.cell_height,
        )
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_widget_id(self.id)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        // Store bounds for the app to read and pass as origin to the
        // prepare pipeline. No DrawCommands — cell rendering is handled
        // by the GPU prepare phase.
        self.bounds.set(Some(ctx.bounds));
    }

    fn handle_mouse(&mut self, _event: &MouseEvent, _ctx: &EventCtx<'_>) -> WidgetResponse {
        // Claim all mouse events within the grid area.
        WidgetResponse::handled()
    }

    fn handle_hover(&mut self, _event: HoverEvent, _ctx: &EventCtx<'_>) -> WidgetResponse {
        WidgetResponse::ignored()
    }

    fn handle_key(&mut self, _event: KeyEvent, _ctx: &EventCtx<'_>) -> WidgetResponse {
        // Claim all key events when focused — they go to the PTY.
        WidgetResponse::handled()
    }
}

#[cfg(test)]
mod tests;
