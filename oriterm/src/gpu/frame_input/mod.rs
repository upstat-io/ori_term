//! Frame input types for the Extract phase of the render pipeline.
//!
//! [`FrameInput`] composes `oriterm_core::RenderableContent` (the terminal
//! snapshot) with rendering context: viewport pixel dimensions, cell metrics,
//! and semantic palette colors. The Prepare phase consumes a `FrameInput` and
//! produces a [`PreparedFrame`](super::prepared_frame::PreparedFrame).

use oriterm_core::grid::StableRowIndex;
use oriterm_core::selection::{Selection, SelectionBounds};
use oriterm_core::{Column, CursorShape, RenderableContent, Rgb};

use crate::font::CellMetrics;

/// Placeholder for search match — replaced in Section 11.
pub type SearchMatch = ();

/// Selection state snapshotted for one frame.
///
/// Encapsulates [`SelectionBounds`] with the viewport→stable row mapping
/// so the Prepare phase can test containment without terminal access.
#[derive(Debug)]
pub struct FrameSelection {
    bounds: SelectionBounds,
    /// Stable row index of viewport line 0.
    base_stable: u64,
}

impl FrameSelection {
    /// Build from an active selection and the viewport's stable row base.
    ///
    /// `stable_row_base` is `RenderableContent::stable_row_base` — the
    /// `StableRowIndex` value of viewport line 0.
    pub fn new(selection: &Selection, stable_row_base: u64) -> Self {
        Self {
            bounds: selection.bounds(),
            base_stable: stable_row_base,
        }
    }

    /// Test whether a visible cell at (`viewport_line`, `col`) is selected.
    pub fn contains(&self, viewport_line: usize, col: usize) -> bool {
        let stable = StableRowIndex(self.base_stable + viewport_line as u64);
        self.bounds.contains(stable, col)
    }
}

/// Mark-mode cursor override for the Prepare phase.
///
/// When mark mode is active, the app sets this on [`FrameInput`] so the
/// Prepare phase renders a hollow block at the mark position instead of
/// the terminal's real cursor. The extract snapshot (`content.cursor`)
/// is never mutated — this override is a separate rendering concern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkCursorOverride {
    /// Viewport line (0 = top of visible area).
    pub line: usize,
    /// Column (0-based).
    pub column: Column,
    /// Cursor shape to render (always `HollowBlock` for mark mode).
    pub shape: CursorShape,
}

/// Pixel dimensions of the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportSize {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
}

impl ViewportSize {
    /// Create a viewport size from pixel dimensions.
    ///
    /// Dimensions are clamped to a minimum of 1 to avoid zero-size surfaces.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
        }
    }
}

/// Semantic colors needed beyond per-cell resolved colors.
///
/// Per-cell fg/bg are already resolved in `RenderableCell`. This captures
/// only the three global colors the renderer needs: clear color, cursor
/// fill, and text-under-cursor inversion color — plus the window opacity
/// for transparent rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FramePalette {
    /// Window clear color (terminal background).
    pub background: Rgb,
    /// Default foreground (used for cursor text inversion).
    pub foreground: Rgb,
    /// Cursor rectangle fill color.
    pub cursor_color: Rgb,
    /// Window opacity (0.0 = fully transparent, 1.0 = fully opaque).
    pub opacity: f32,
}

/// Complete input for one render frame.
///
/// Composes the terminal snapshot ([`RenderableContent`]) with the rendering
/// context needed to convert logical cells into pixel geometry. Built during
/// the Extract phase, consumed by the Prepare phase.
#[derive(Debug)]
pub struct FrameInput {
    /// Terminal content snapshot (cells, cursor, damage, mode).
    pub content: RenderableContent,
    /// Viewport pixel dimensions.
    pub viewport: ViewportSize,
    /// Cell pixel dimensions from font metrics.
    pub cell_size: CellMetrics,
    /// Semantic colors for background clear and cursor.
    pub palette: FramePalette,
    /// Active selection for highlight rendering.
    pub selection: Option<FrameSelection>,
    /// Active search matches (placeholder until Section 11).
    #[allow(dead_code, reason = "search highlight rendering in Section 11")]
    pub search_matches: Vec<SearchMatch>,
    /// Viewport cell under the mouse cursor for hyperlink hover detection.
    ///
    /// `(viewport_line, column)`. Set from mouse state after extraction;
    /// `None` when the cursor is outside the grid.
    pub hovered_cell: Option<(usize, usize)>,
    /// Mark-mode cursor override.
    ///
    /// When set, the Prepare phase renders this cursor instead of
    /// `content.cursor`. Set by the app layer after extraction when mark
    /// mode is active; the extracted content is never mutated.
    pub mark_cursor: Option<MarkCursorOverride>,
}

impl FrameInput {
    /// Number of grid columns based on viewport and cell size.
    pub fn columns(&self) -> usize {
        self.cell_size.columns(self.viewport.width)
    }

    /// Number of grid rows based on viewport and cell size.
    pub fn rows(&self) -> usize {
        self.cell_size.rows(self.viewport.height)
    }

    /// Whether the entire viewport needs a full repaint.
    #[allow(dead_code, reason = "damage tracking optimization for later sections")]
    pub fn needs_full_repaint(&self) -> bool {
        self.content.all_dirty
    }

    /// Build a test frame from a text string.
    ///
    /// Creates a grid of `cols × rows` cells. `text` is laid out left-to-right,
    /// top-to-bottom; cells beyond the text length are filled with spaces. All
    /// cells use default dark-theme colors. Cell size is 8×16 px.
    #[cfg(test)]
    pub fn test_grid(cols: usize, rows: usize, text: &str) -> Self {
        use oriterm_core::{
            CellFlags, Column, CursorShape, RenderableCell, RenderableContent, RenderableCursor,
            TermMode,
        };

        let fg = Rgb {
            r: 211,
            g: 215,
            b: 207,
        };
        let bg = Rgb { r: 0, g: 0, b: 0 };

        let mut cells = Vec::with_capacity(cols * rows);
        let mut chars = text.chars();

        for row in 0..rows {
            for col in 0..cols {
                let ch = chars.next().unwrap_or(' ');
                cells.push(RenderableCell {
                    line: row,
                    column: Column(col),
                    ch,
                    fg,
                    bg,
                    flags: CellFlags::empty(),
                    underline_color: None,
                    has_hyperlink: false,
                    zerowidth: Vec::new(),
                });
            }
        }

        Self {
            content: RenderableContent {
                cells,
                cursor: RenderableCursor {
                    line: 0,
                    column: Column(0),
                    shape: CursorShape::default(),
                    visible: true,
                },
                display_offset: 0,
                stable_row_base: 0,
                mode: TermMode::SHOW_CURSOR,
                all_dirty: true,
                damage: Vec::new(),
            },
            viewport: ViewportSize::new(cols as u32 * 8, rows as u32 * 16),
            cell_size: CellMetrics::new(8.0, 16.0, 12.0, 2.0, 1.0, 4.0),
            palette: FramePalette {
                background: bg,
                foreground: fg,
                cursor_color: Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                },
                opacity: 1.0,
            },
            selection: None,
            search_matches: Vec::new(),
            hovered_cell: None,
            mark_cursor: None,
        }
    }
}

#[cfg(test)]
mod tests;
