//! Text shaping via rustybuzz — segments grid rows into runs, shapes each run,
//! and maps shaped glyphs back to grid columns.
//!
//! # Two-Phase API
//!
//! 1. [`prepare_line`] — segment a row of cells into [`ShapingRun`]s (immutable).
//! 2. [`shape_prepared_runs`] — shape each run through rustybuzz → [`ShapedGlyph`]s.
//!
//! Splitting into two phases lets callers create rustybuzz `Face` objects once
//! per frame and reuse them across all rows.

use oriterm_core::{Cell, CellFlags, RenderableCell};

use super::collection::FontCollection;
use super::{FaceIdx, GlyphStyle};

// ── ShapableCell trait ──

/// Abstraction over cell types that the shaper can operate on.
///
/// Both [`Cell`] (grid storage) and [`RenderableCell`] (extract-phase output)
/// carry the same shaping-relevant data — character, flags, zero-width marks —
/// but store them differently. This trait provides uniform access so the shaper
/// can work with either type without copying.
pub(crate) trait ShapableCell {
    /// The display character.
    fn ch(&self) -> char;
    /// Cell attribute flags (bold, italic, wide, etc.).
    fn flags(&self) -> CellFlags;
    /// Zero-width combining characters appended to this cell.
    fn zerowidth(&self) -> &[char];
}

impl ShapableCell for Cell {
    fn ch(&self) -> char {
        self.ch
    }

    fn flags(&self) -> CellFlags {
        self.flags
    }

    fn zerowidth(&self) -> &[char] {
        self.extra.as_ref().map_or(&[], |e| &e.zerowidth)
    }
}

impl ShapableCell for RenderableCell {
    fn ch(&self) -> char {
        self.ch
    }

    fn flags(&self) -> CellFlags {
        self.flags
    }

    fn zerowidth(&self) -> &[char] {
        &self.zerowidth
    }
}

// ── Types ──

/// A contiguous run of characters sharing the same font face.
///
/// Produced by [`prepare_line`], consumed by [`shape_prepared_runs`].
pub struct ShapingRun {
    /// Text to shape (base characters + combining marks).
    pub(crate) text: String,
    /// Which font face to shape this run with.
    pub(crate) face_idx: FaceIdx,
    /// Starting grid column of this run.
    pub(crate) col_start: usize,
    /// Maps byte offset in `text` to grid column index.
    ///
    /// Critical for mapping rustybuzz cluster indices back to grid positions.
    pub(crate) byte_to_col: Vec<usize>,
}

/// A shaped glyph positioned on the grid.
///
/// Produced by [`shape_prepared_runs`]. Each glyph occupies one or more grid
/// columns (ligatures span multiple).
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// Glyph ID within the font face (post-shaping, NOT codepoint).
    pub glyph_id: u16,
    /// Which font face this was shaped from.
    pub face_idx: FaceIdx,
    /// First grid column this glyph occupies.
    pub col_start: usize,
    /// Number of grid columns (1 = normal, 2+ = ligature or wide char).
    #[allow(dead_code, reason = "informational field consumed by tests and diagnostics")]
    pub col_span: usize,
    /// Shaper X positioning offset in pixels.
    pub x_offset: f32,
    /// Shaper Y positioning offset in pixels.
    pub y_offset: f32,
}

/// Variation Selector 16: forces emoji presentation (U+FE0F).
const VS16: char = '\u{FE0F}';

// ── Phase 1: Run Segmentation ──

/// Segment a row of cells into shaping runs.
///
/// Clears and fills `runs_out`. Each run contains characters that share the
/// same font face and can be shaped together. Run boundaries occur at:
/// - Space (`' '`) or null (`'\0'`)
/// - Font face changes (different glyph found in a different face)
/// - Wide char spacers (part of the preceding wide char)
///
/// Combining marks (zero-width characters) are appended to the same column
/// as their base character within the current run.
///
/// Inner allocations (`text`, `byte_to_col`) are retained across calls to
/// avoid per-run heap churn.
pub fn prepare_line<C: ShapableCell>(
    row: &[C],
    cols: usize,
    collection: &FontCollection,
    runs_out: &mut Vec<ShapingRun>,
) {
    // Clear inner buffers but keep the ShapingRun objects so their String
    // and Vec capacities survive for reuse.
    for run in runs_out.iter_mut() {
        run.text.clear();
        run.byte_to_col.clear();
    }
    segment_runs(row, cols, collection, runs_out);
}

/// Internal segmentation logic.
///
/// Reuses existing `ShapingRun` slots (whose inner buffers were cleared by
/// the caller) when possible, only allocating new runs when the vec grows.
/// Truncates to the final run count at the end.
fn segment_runs<C: ShapableCell>(
    row: &[C],
    cols: usize,
    collection: &FontCollection,
    runs: &mut Vec<ShapingRun>,
) {
    let mut col = 0;
    // Index of the next run slot to write into.
    let mut run_count = 0;

    while col < cols {
        let cell = &row[col];

        // Skip wide char spacers (part of preceding wide char).
        if cell.flags().contains(CellFlags::WIDE_CHAR_SPACER) {
            col += 1;
            continue;
        }

        // Run boundaries: space, null, or built-in geometric glyphs.
        if cell.ch() == ' ' || cell.ch() == '\0' || super::is_builtin(cell.ch()) {
            col += 1;
            continue;
        }

        let style = GlyphStyle::from_cell_flags(cell.flags());
        let resolved = if cell.zerowidth().contains(&VS16) {
            collection.resolve_prefer_emoji(cell.ch(), style)
        } else {
            collection.resolve(cell.ch(), style)
        };
        let face_idx = resolved.face_idx;

        // Check if we can extend the current run (same face).
        let extend = run_count > 0
            && runs
                .get(run_count - 1)
                .is_some_and(|r: &ShapingRun| r.face_idx == face_idx);

        if extend {
            append_cell_to_run(&mut runs[run_count - 1], cell, col);
        } else {
            // Recycle existing slot or push a new one.
            if run_count < runs.len() {
                runs[run_count].face_idx = face_idx;
                runs[run_count].col_start = col;
                // text and byte_to_col already cleared by caller.
            } else {
                runs.push(ShapingRun {
                    text: String::new(),
                    face_idx,
                    col_start: col,
                    byte_to_col: Vec::new(),
                });
            }
            append_cell_to_run(&mut runs[run_count], cell, col);
            run_count += 1;
        }

        col += if cell.flags().contains(CellFlags::WIDE_CHAR) {
            2
        } else {
            1
        };
    }

    runs.truncate(run_count);
}

/// Append a cell's character and zero-width marks to a shaping run.
fn append_cell_to_run<C: ShapableCell>(run: &mut ShapingRun, cell: &C, col: usize) {
    // Base character.
    let byte_start = run.text.len();
    run.text.push(cell.ch());
    for _ in byte_start..run.text.len() {
        run.byte_to_col.push(col);
    }

    // Combining marks / zero-width characters.
    for &zw in cell.zerowidth() {
        let zw_start = run.text.len();
        run.text.push(zw);
        for _ in zw_start..run.text.len() {
            run.byte_to_col.push(col);
        }
    }
}

// ── Phase 2: Shaping ──

/// Shape pre-segmented runs using pre-created rustybuzz faces.
///
/// Clears and fills `output`. Faces should be created once per frame via
/// [`FontCollection::create_shaping_faces`].
///
/// Reuses a single `UnicodeBuffer` across all runs to avoid per-run heap
/// allocation. The buffer is returned from each `GlyphBuffer::clear()`.
/// Pass `buffer_slot` to persist the buffer across frames — the first call
/// allocates, subsequent calls reuse the existing capacity.
pub fn shape_prepared_runs(
    runs: &[ShapingRun],
    faces: &[Option<rustybuzz::Face<'_>>],
    collection: &FontCollection,
    output: &mut Vec<ShapedGlyph>,
    buffer_slot: &mut Option<rustybuzz::UnicodeBuffer>,
) {
    output.clear();
    let mut buffer = buffer_slot
        .take()
        .unwrap_or_default();
    for run in runs {
        buffer = shape_run(run, faces, collection, output, buffer);
    }
    *buffer_slot = Some(buffer);
}

/// Shape a single run and append results to the output vec.
///
/// Returns the `UnicodeBuffer` for reuse by the next run. When a face is
/// missing, the buffer is returned unchanged (unshaped fallback path).
fn shape_run(
    run: &ShapingRun,
    faces: &[Option<rustybuzz::Face<'_>>],
    collection: &FontCollection,
    output: &mut Vec<ShapedGlyph>,
    mut buffer: rustybuzz::UnicodeBuffer,
) -> rustybuzz::UnicodeBuffer {
    let face_i = run.face_idx.as_usize();
    let Some(face) = faces.get(face_i).and_then(|f| f.as_ref()) else {
        emit_unshaped_fallback(run, output);
        return buffer;
    };

    buffer.push_str(&run.text);
    buffer.set_direction(rustybuzz::Direction::LeftToRight);

    let features = collection.features_for_face(run.face_idx);
    let glyph_buffer = rustybuzz::shape(face, features, buffer);
    let infos = glyph_buffer.glyph_infos();
    let positions = glyph_buffer.glyph_positions();

    let upem = face.units_per_em() as f32;
    let eff_size = collection.effective_size(run.face_idx);
    let scale = eff_size / upem;
    let cell_w = collection.cell_metrics().width;

    for (info, pos) in infos.iter().zip(positions.iter()) {
        let cluster = info.cluster as usize;

        // Map cluster (byte offset) → grid column.
        let col = run
            .byte_to_col
            .get(cluster)
            .copied()
            .unwrap_or(run.col_start);

        // Compute col_span from advance width.
        let advance_px = pos.x_advance as f32 * scale;
        let col_span = (advance_px / cell_w).round().max(1.0) as usize;

        let x_offset = pos.x_offset as f32 * scale;
        let y_offset = pos.y_offset as f32 * scale;

        output.push(ShapedGlyph {
            glyph_id: info.glyph_id as u16,
            face_idx: run.face_idx,
            col_start: col,
            col_span,
            x_offset,
            y_offset,
        });
    }

    // Return the cleared buffer for reuse by the next run.
    glyph_buffer.clear()
}

/// Fallback for when no rustybuzz face is available — emit one glyph per char.
fn emit_unshaped_fallback(run: &ShapingRun, output: &mut Vec<ShapedGlyph>) {
    let mut col = run.col_start;
    for ch in run.text.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if w == 0 {
            continue;
        }
        output.push(ShapedGlyph {
            glyph_id: 0,
            face_idx: run.face_idx,
            col_start: col,
            col_span: w,
            x_offset: 0.0,
            y_offset: 0.0,
        });
        col += w;
    }
}

// ── Phase 3: Column ↔ Glyph Mapping ──

/// Build a column-to-glyph map from shaped output.
///
/// Clears and fills `map_out` with `cols` entries. Each entry is either:
/// - `Some(glyph_index)` — the **first** glyph that starts at this column
/// - `None` — this column is a continuation of a multi-column glyph (ligature
///   or wide char), or has no glyph (space, null)
///
/// Uses first-wins semantics: the first glyph at a column (the base character)
/// claims the slot. Combining marks at the same `col_start` are contiguous in
/// the glyph vec and found by iterating forward from the base index.
///
/// The renderer uses this map to decide: render the glyph at `Some` columns
/// (plus any combining marks that follow), skip `None` columns.
pub fn build_col_glyph_map(
    glyphs: &[ShapedGlyph],
    cols: usize,
    map_out: &mut Vec<Option<usize>>,
) {
    map_out.clear();
    map_out.resize(cols, None);

    for (i, glyph) in glyphs.iter().enumerate() {
        if glyph.col_start < cols && map_out[glyph.col_start].is_none() {
            map_out[glyph.col_start] = Some(i);
        }
        // Continuation columns (col_start+1 .. col_start+col_span) stay None.
    }
}

#[cfg(test)]
mod tests;
