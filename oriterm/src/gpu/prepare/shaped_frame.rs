//! Pre-shaped frame: per-row glyph data for shaped text rendering.
//!
//! [`ShapedFrame`] stores the output of shaping all visible rows into a single
//! flat buffer with per-row span indices and column-to-glyph maps. Built once
//! per frame by the renderer, consumed by the prepare phase to emit glyph
//! instances from shaped data instead of per-cell character lookups.

use crate::font::ShapedGlyph;

/// Pre-shaped glyph data for all visible rows.
///
/// Flat storage with per-row indexing to avoid per-row allocations. The
/// renderer builds this by shaping each row and calling [`push_row`](Self::push_row);
/// the prepare phase reads it via [`row_glyphs`](Self::row_glyphs) and
/// [`col_map`](Self::col_map).
pub(crate) struct ShapedFrame {
    /// Flat buffer of all shaped glyphs, rows concatenated.
    glyphs: Vec<ShapedGlyph>,
    /// `row_spans[row] = (start, end)` indices into `glyphs`.
    row_spans: Vec<(usize, usize)>,
    /// Flat col-to-glyph map: `col_maps[row * cols + col] = Option<usize>`
    /// where the `usize` is **relative** to the row's span start.
    col_maps: Vec<Option<usize>>,
    /// Number of columns per row.
    cols: usize,
    /// Font size in 26.6 fixed-point for `RasterKey` construction.
    size_q6: u32,
    /// Whether glyphs were rasterized with hinting enabled.
    hinted: bool,
}

impl ShapedFrame {
    /// Create an empty frame for the given column count and font size.
    pub fn new(cols: usize, size_q6: u32) -> Self {
        Self {
            glyphs: Vec::new(),
            row_spans: Vec::new(),
            col_maps: Vec::new(),
            cols,
            size_q6,
            hinted: true,
        }
    }

    /// Append a shaped row's glyphs and column map.
    ///
    /// `glyphs` are the shaped output for one row. `col_map` maps columns
    /// to indices within `glyphs` (produced by [`build_col_glyph_map`]).
    ///
    /// [`build_col_glyph_map`]: crate::font::build_col_glyph_map
    pub fn push_row(&mut self, glyphs: &[ShapedGlyph], col_map: &[Option<usize>]) {
        debug_assert_eq!(
            col_map.len(),
            self.cols,
            "col_map length must match frame column count",
        );

        let start = self.glyphs.len();
        self.glyphs.extend_from_slice(glyphs);
        let end = self.glyphs.len();
        self.row_spans.push((start, end));
        self.col_maps.extend_from_slice(col_map);
    }

    /// Glyph slice for a given row.
    pub fn row_glyphs(&self, row: usize) -> &[ShapedGlyph] {
        let (start, end) = self.row_spans[row];
        &self.glyphs[start..end]
    }

    /// Column-to-glyph index for a given row and column.
    ///
    /// Returns `Some(idx)` where `idx` is relative to the row's glyph slice
    /// (i.e. index into `row_glyphs(row)`), or `None` if the column has no
    /// glyph (space, null, or ligature continuation).
    pub fn col_map(&self, row: usize, col: usize) -> Option<usize> {
        self.col_maps[row * self.cols + col]
    }

    /// Number of columns per row.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Number of shaped rows.
    pub fn rows(&self) -> usize {
        self.row_spans.len()
    }

    /// All glyphs across all rows (for atlas pre-caching).
    pub fn all_glyphs(&self) -> &[ShapedGlyph] {
        &self.glyphs
    }

    /// Font size in 26.6 fixed-point.
    pub fn size_q6(&self) -> u32 {
        self.size_q6
    }

    /// Whether glyphs in this frame were rasterized with hinting.
    pub fn hinted(&self) -> bool {
        self.hinted
    }

    /// Reset for reuse on the next frame, updating metadata.
    ///
    /// Clears all glyph and mapping data while retaining allocations.
    /// `cols` and `size_q6` are updated to match the new frame's parameters.
    pub fn clear(&mut self, cols: usize, size_q6: u32, hinted: bool) {
        self.glyphs.clear();
        self.row_spans.clear();
        self.col_maps.clear();
        self.cols = cols;
        self.size_q6 = size_q6;
        self.hinted = hinted;
    }
}
