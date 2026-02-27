//! Frame input types for the Extract phase of the render pipeline.
//!
//! [`FrameInput`] composes `oriterm_core::RenderableContent` (the terminal
//! snapshot) with rendering context: viewport pixel dimensions, cell metrics,
//! and semantic palette colors. The Prepare phase consumes a `FrameInput` and
//! produces a [`PreparedFrame`](super::prepared_frame::PreparedFrame).

use oriterm_core::grid::StableRowIndex;
use oriterm_core::search::MatchType;
use oriterm_core::selection::{Selection, SelectionBounds};
use oriterm_core::{Column, CursorShape, RenderableContent, Rgb, SearchMatch, SearchState};

use crate::font::CellMetrics;
use crate::url_detect::UrlSegment;

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

/// Search rendering snapshot for one frame.
///
/// Contains the match data and viewport mapping needed to classify cells
/// for search highlighting. Built from `SearchState` without cloning the
/// full state — copies the match list for frame-local access.
#[derive(Debug)]
pub struct FrameSearch {
    /// Matches from the search state (copied per frame).
    matches: Vec<SearchMatch>,
    /// Index of the focused match.
    focused: usize,
    /// Stable row index of viewport line 0.
    base_stable: u64,
    /// Total match count (for search bar "N of M" display).
    match_count: usize,
    /// Query string (for search bar display).
    query: String,
}

impl FrameSearch {
    /// Build from an active search state and the viewport's stable row base.
    pub fn new(state: &SearchState, stable_row_base: u64) -> Self {
        Self {
            matches: state.matches().to_vec(),
            focused: state.focused_index(),
            base_stable: stable_row_base,
            match_count: state.matches().len(),
            query: state.query().to_string(),
        }
    }

    /// Classify a visible cell for search match highlighting.
    pub fn cell_match_type(&self, viewport_line: usize, col: usize) -> MatchType {
        if self.matches.is_empty() {
            return MatchType::None;
        }
        let stable = StableRowIndex(self.base_stable + viewport_line as u64);

        // Binary search: find first match whose start is beyond (row, col).
        let idx = self
            .matches
            .partition_point(|m| (m.start_row, m.start_col) <= (stable, col));

        let start = idx.saturating_sub(1);
        let end = (idx + 1).min(self.matches.len());

        for i in start..end {
            if cell_in_search_match(&self.matches[i], stable, col) {
                return if i == self.focused {
                    MatchType::FocusedMatch
                } else {
                    MatchType::Match
                };
            }
        }
        MatchType::None
    }

    /// Total number of matches.
    pub fn match_count(&self) -> usize {
        self.match_count
    }

    /// 1-based focused match index (for "N of M" display).
    pub fn focused_display(&self) -> usize {
        if self.match_count == 0 {
            0
        } else {
            self.focused + 1
        }
    }

    /// The current query string.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Build a test search snapshot from manually constructed matches.
    ///
    /// `focused` is the index into `matches` of the focused match.
    /// `stable_row_base` maps viewport line 0 to stable row coordinates.
    #[cfg(test)]
    pub fn for_test(matches: Vec<SearchMatch>, focused: usize, stable_row_base: u64) -> Self {
        Self {
            match_count: matches.len(),
            matches,
            focused,
            base_stable: stable_row_base,
            query: String::from("test"),
        }
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
/// for transparent rendering and optional selection color overrides.
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
    /// Explicit selection foreground (from scheme or config override).
    pub selection_fg: Option<Rgb>,
    /// Explicit selection background (from scheme or config override).
    pub selection_bg: Option<Rgb>,
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
    /// Active search state for match highlighting.
    pub search: Option<FrameSearch>,
    /// Viewport cell under the mouse cursor for hyperlink hover detection.
    ///
    /// `(viewport_line, column)`. Set from mouse state after extraction;
    /// `None` when the cursor is outside the grid.
    pub hovered_cell: Option<(usize, usize)>,
    /// Viewport-relative segments of an implicitly detected URL being hovered.
    ///
    /// Each entry is `(viewport_line, start_col, end_col)` inclusive. Set when
    /// Ctrl is held and the cursor is over a detected URL. Empty when no
    /// implicit URL is hovered.
    pub hovered_url_segments: Vec<UrlSegment>,
    /// Mark-mode cursor override.
    ///
    /// When set, the Prepare phase renders this cursor instead of
    /// `content.cursor`. Set by the app layer after extraction when mark
    /// mode is active; the extracted content is never mutated.
    pub mark_cursor: Option<MarkCursorOverride>,
    /// Foreground alpha multiplier for inactive pane dimming.
    ///
    /// 1.0 = fully opaque (default, focused pane). Values < 1.0 dim glyph
    /// alpha proportionally for unfocused panes. Set by the multi-pane
    /// render path; single-pane rendering always uses 1.0.
    pub fg_dim: f32,
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
                selection_fg: None,
                selection_bg: None,
            },
            selection: None,
            search: None,
            hovered_cell: None,
            hovered_url_segments: Vec::new(),
            mark_cursor: None,
            fg_dim: 1.0,
        }
    }
}

/// Check if `(stable_row, col)` falls within a search match span.
fn cell_in_search_match(m: &SearchMatch, stable_row: StableRowIndex, col: usize) -> bool {
    if stable_row < m.start_row || stable_row > m.end_row {
        return false;
    }
    if m.start_row == m.end_row {
        return col >= m.start_col && col <= m.end_col;
    }
    if stable_row == m.start_row {
        col >= m.start_col
    } else if stable_row == m.end_row {
        col <= m.end_col
    } else {
        true
    }
}

#[cfg(test)]
mod tests;
