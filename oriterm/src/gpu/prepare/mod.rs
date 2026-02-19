//! Prepare phase: convert a [`FrameInput`] into GPU-ready instance buffers.
//!
//! [`prepare_frame`] is a pure CPU function — no wgpu types, no device, no
//! queue. Given a terminal snapshot and an atlas lookup, it produces a
//! [`PreparedFrame`] containing three [`InstanceWriter`] buffers (backgrounds,
//! glyphs, cursors) ready for GPU upload.
//!
//! The [`AtlasLookup`] trait abstracts glyph lookup for testability: production
//! wraps `FontCollection::resolve` + `GlyphAtlas::lookup`; tests use a simple
//! `HashMap`.

mod decorations;
pub(crate) mod shaped_frame;

use oriterm_core::{CellFlags, CursorShape, Rgb};

use super::atlas::{AtlasEntry, AtlasKind};
use super::frame_input::FrameInput;
use super::prepared_frame::PreparedFrame;
use crate::font::{GlyphStyle, RasterKey, ShapedGlyph, subpx_bin, subpx_offset};
use crate::gpu::instance_writer::ScreenRect;

pub(crate) use shaped_frame::ShapedFrame;

/// Abstracts glyph atlas lookup for testability.
///
/// Production: the shaped path uses [`lookup_key`](Self::lookup_key) for
/// direct `RasterKey` → `AtlasEntry` lookups. Tests may override `lookup`
/// for the per-cell unshaped path.
pub trait AtlasLookup {
    /// Look up a cached glyph entry by character and style.
    ///
    /// Used by the unshaped [`prepare_frame`] test path. Default returns
    /// `None` — production implementations only need [`lookup_key`](Self::lookup_key).
    #[allow(dead_code, reason = "used by test-only unshaped prepare_frame path")]
    fn lookup(&self, _ch: char, _style: GlyphStyle) -> Option<&AtlasEntry> {
        None
    }

    /// Look up a cached glyph entry by [`RasterKey`] (shaped path).
    fn lookup_key(&self, key: RasterKey) -> Option<&AtlasEntry>;
}

/// Convert cell flags to the corresponding glyph style.
#[cfg(test)]
fn glyph_style(flags: CellFlags) -> GlyphStyle {
    GlyphStyle::from_cell_flags(flags)
}

/// Convert a [`FrameInput`] into a GPU-ready [`PreparedFrame`] using per-cell
/// character lookups (unshaped path).
///
/// Used by tests to verify prepare logic without shaping complexity. Production
/// rendering uses [`prepare_frame_shaped`] instead.
#[cfg(test)]
pub fn prepare_frame(input: &FrameInput, atlas: &dyn AtlasLookup) -> PreparedFrame {
    let cols = input.columns();
    let rows = input.rows();
    let opacity = f64::from(input.palette.opacity);
    let mut frame = PreparedFrame::with_capacity(
        input.viewport,
        cols,
        rows,
        input.palette.background,
        opacity,
    );
    fill_frame(input, atlas, &mut frame);
    frame
}

/// Convert a [`FrameInput`] into a pre-existing [`PreparedFrame`], reusing
/// its buffer allocations (unshaped path).
///
/// Used by tests. Production rendering uses [`prepare_frame_shaped`] instead.
#[cfg(test)]
pub fn prepare_frame_into(input: &FrameInput, atlas: &dyn AtlasLookup, out: &mut PreparedFrame) {
    out.clear();
    out.viewport = input.viewport;
    out.set_clear_color(input.palette.background, f64::from(input.palette.opacity));
    fill_frame(input, atlas, out);
}

/// Convert a [`FrameInput`] into a GPU-ready [`PreparedFrame`] using shaped
/// glyph data.
///
/// Like [`prepare_frame`] but uses pre-shaped glyph positions from a
/// [`ShapedFrame`] instead of per-cell character lookups. This enables
/// ligatures, combining marks, and shaper-driven positioning.
///
/// Used by tests to get a fresh frame. Production uses
/// [`prepare_frame_shaped_into`] for buffer reuse.
#[cfg(test)]
pub fn prepare_frame_shaped(
    input: &FrameInput,
    atlas: &dyn AtlasLookup,
    shaped: &ShapedFrame,
) -> PreparedFrame {
    let cols = input.columns();
    let rows = input.rows();
    let opacity = f64::from(input.palette.opacity);
    let mut frame = PreparedFrame::with_capacity(
        input.viewport,
        cols,
        rows,
        input.palette.background,
        opacity,
    );
    fill_frame_shaped(input, atlas, shaped, &mut frame);
    frame
}

/// Convert a [`FrameInput`] into a pre-existing [`PreparedFrame`], reusing
/// its buffer allocations (shaped path).
///
/// Like [`prepare_frame_shaped`] but clears and refills `out` instead of
/// allocating a new frame.
pub fn prepare_frame_shaped_into(
    input: &FrameInput,
    atlas: &dyn AtlasLookup,
    shaped: &ShapedFrame,
    out: &mut PreparedFrame,
) {
    out.clear();
    out.viewport = input.viewport;
    out.set_clear_color(input.palette.background, f64::from(input.palette.opacity));
    fill_frame_shaped(input, atlas, shaped, out);
}

/// Unshaped per-cell rendering: emit instances into `frame`.
///
/// Iterates every visible cell, emits background and glyph instances via
/// character lookup, then builds cursor instances. Used by tests; production
/// rendering uses [`fill_frame_shaped`].
#[cfg(test)]
fn fill_frame(input: &FrameInput, atlas: &dyn AtlasLookup, frame: &mut PreparedFrame) {
    let cw = input.cell_size.width;
    let ch = input.cell_size.height;
    let baseline = input.cell_size.baseline;

    for cell in &input.content.cells {
        // Wide char spacers are handled by the primary wide char cell.
        if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
            continue;
        }

        let col = cell.column.0;
        let row = cell.line;
        let x = col as f32 * cw;
        let y = row as f32 * ch;

        // Background: wide chars span 2 cell widths.
        let bg_w = if cell.flags.contains(CellFlags::WIDE_CHAR) {
            2.0 * cw
        } else {
            cw
        };
        frame.backgrounds.push_rect(
            ScreenRect {
                x,
                y,
                w: bg_w,
                h: ch,
            },
            cell.bg,
            1.0,
        );

        decorations::DecorationContext {
            backgrounds: &mut frame.backgrounds,
            glyphs: &mut frame.glyphs,
            atlas,
            size_q6: 0,
            metrics: &input.cell_size,
        }
        .draw(cell.flags, cell.underline_color, cell.fg, x, y, bg_w);

        // Foreground glyph (skip spaces).
        if cell.ch != ' ' {
            let style = glyph_style(cell.flags);
            if let Some(entry) = atlas.lookup(cell.ch, style) {
                let glyph_x = x + entry.bearing_x as f32;
                let glyph_y = y + baseline - entry.bearing_y as f32;
                let uv = [entry.uv_x, entry.uv_y, entry.uv_w, entry.uv_h];
                let rect = ScreenRect {
                    x: glyph_x,
                    y: glyph_y,
                    w: entry.width as f32,
                    h: entry.height as f32,
                };
                frame.glyphs.push_glyph(rect, uv, cell.fg, 1.0, entry.page);
            }
        }
    }

    // Cursor instances.
    let cursor = &input.content.cursor;
    if cursor.visible {
        build_cursor(
            frame,
            cursor.shape,
            cursor.column.0,
            cursor.line,
            cw,
            ch,
            input.palette.cursor_color,
        );
    }
}

/// Shaped rendering: emit background, glyph, and cursor instances from shaped data.
///
/// Backgrounds and cursors use the same per-cell logic as [`fill_frame`].
/// Glyphs are driven by the [`ShapedFrame`] col-to-glyph map instead of
/// per-cell character lookups, enabling ligatures and combining marks.
fn fill_frame_shaped(
    input: &FrameInput,
    atlas: &dyn AtlasLookup,
    shaped: &ShapedFrame,
    frame: &mut PreparedFrame,
) {
    let cw = input.cell_size.width;
    let ch = input.cell_size.height;
    let baseline = input.cell_size.baseline;

    for cell in &input.content.cells {
        if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
            continue;
        }

        let col = cell.column.0;
        let row = cell.line;
        let x = col as f32 * cw;
        let y = row as f32 * ch;

        // Background (identical to unshaped path).
        let bg_w = if cell.flags.contains(CellFlags::WIDE_CHAR) {
            2.0 * cw
        } else {
            cw
        };
        frame.backgrounds.push_rect(
            ScreenRect {
                x,
                y,
                w: bg_w,
                h: ch,
            },
            cell.bg,
            1.0,
        );

        decorations::DecorationContext {
            backgrounds: &mut frame.backgrounds,
            glyphs: &mut frame.glyphs,
            atlas,
            size_q6: shaped.size_q6(),
            metrics: &input.cell_size,
        }
        .draw(cell.flags, cell.underline_color, cell.fg, x, y, bg_w);

        // Built-in geometric glyphs: bypass shaping, render from atlas.
        if crate::font::is_builtin(cell.ch) {
            let key = super::builtin_glyphs::raster_key(cell.ch, shaped.size_q6());
            if let Some(entry) = atlas.lookup_key(key) {
                let uv = [entry.uv_x, entry.uv_y, entry.uv_w, entry.uv_h];
                let rect = ScreenRect {
                    x,
                    y,
                    w: entry.width as f32,
                    h: entry.height as f32,
                };
                frame.glyphs.push_glyph(rect, uv, cell.fg, 1.0, entry.page);
            }
            continue;
        }

        // Foreground: emit shaped glyphs via col-to-glyph map.
        // Guard: viewport cells may exceed shaped frame during async resize.
        if row >= shaped.rows() || col >= shaped.cols() {
            continue;
        }
        if let Some(start_idx) = shaped.col_map(row, col) {
            let row_glyphs = shaped.row_glyphs(row);
            GlyphEmitter {
                baseline,
                size_q6: shaped.size_q6(),
                hinted: shaped.hinted(),
                atlas,
                frame,
            }
            .emit(row_glyphs, start_idx, col, x, y, cell.fg, cell.bg);
        }
    }

    // Cursor (identical to unshaped path).
    let cursor = &input.content.cursor;
    if cursor.visible {
        build_cursor(
            frame,
            cursor.shape,
            cursor.column.0,
            cursor.line,
            cw,
            ch,
            input.palette.cursor_color,
        );
    }
}

/// Frame-level context for shaped glyph emission.
///
/// Bundles the atlas, size key, baseline, and output frame that are invariant
/// across cells. Per-cell parameters (row glyphs, column, position, color)
/// are passed to [`emit`](Self::emit).
struct GlyphEmitter<'a> {
    baseline: f32,
    size_q6: u32,
    hinted: bool,
    atlas: &'a dyn AtlasLookup,
    frame: &'a mut PreparedFrame,
}

impl GlyphEmitter<'_> {
    /// Emit glyph instances for a shaped cell: base glyph + any combining marks.
    ///
    /// Starts at `start_idx` in `row_glyphs` (the base glyph from the col map),
    /// then iterates forward while subsequent glyphs share the same `col_start`
    /// (combining marks are contiguous in the shaper output).
    ///
    /// Routing by [`AtlasKind`]:
    /// - `Mono` → `frame.glyphs` (R8 atlas, tinted by `fg_color`).
    /// - `Subpixel` → `frame.subpixel_glyphs` (RGBA atlas, per-channel blend).
    /// - `Color` → `frame.color_glyphs` (RGBA atlas, rendered as-is).
    #[expect(
        clippy::too_many_arguments,
        reason = "per-cell shaped glyph params: glyph source, grid column, screen position, color"
    )]
    fn emit(
        &mut self,
        row_glyphs: &[ShapedGlyph],
        start_idx: usize,
        col: usize,
        x: f32,
        y: f32,
        fg: Rgb,
        bg: Rgb,
    ) {
        let mut is_first = true;
        for sg in &row_glyphs[start_idx..] {
            // Stop at the first glyph in a different column (combining marks are contiguous).
            if !is_first && sg.col_start != col {
                break;
            }
            is_first = false;

            let subpx = subpx_bin(sg.x_offset);
            let key = RasterKey {
                glyph_id: sg.glyph_id,
                face_idx: sg.face_idx,
                size_q6: self.size_q6,
                synthetic: sg.synthetic,
                hinted: self.hinted,
                subpx_x: subpx,
            };
            if let Some(entry) = self.atlas.lookup_key(key) {
                // Apply shaper offsets: x_offset shifts horizontally,
                // y_offset shifts vertically (positive = up in font coords = subtract in screen).
                // Subtract the absorbed subpixel offset to avoid double-counting
                // (once in the rasterized bitmap, once in positioning).
                let absorbed = subpx_offset(subpx);
                let gx = x + entry.bearing_x as f32 + sg.x_offset - absorbed;
                let gy = y + self.baseline - entry.bearing_y as f32 - sg.y_offset;
                let uv = [entry.uv_x, entry.uv_y, entry.uv_w, entry.uv_h];
                let rect = ScreenRect {
                    x: gx,
                    y: gy,
                    w: entry.width as f32,
                    h: entry.height as f32,
                };
                let writer = match entry.kind {
                    AtlasKind::Color => &mut self.frame.color_glyphs,
                    AtlasKind::Subpixel => &mut self.frame.subpixel_glyphs,
                    AtlasKind::Mono => &mut self.frame.glyphs,
                };
                writer.push_glyph_with_bg(rect, uv, fg, bg, 1.0, entry.page);
            }
        }
    }
}

/// Emit cursor instances into the prepared frame.
///
/// The cursor shape determines the geometry:
/// - `Block` — full cell rectangle.
/// - `Bar` — 2px-wide vertical line at the left edge.
/// - `Underline` — 2px-tall horizontal line at the bottom.
/// - `HollowBlock` — 4 thin outline rectangles (top, bottom, left, right).
/// - `Hidden` — no instances.
#[expect(
    clippy::too_many_arguments,
    reason = "cursor geometry: frame, shape, grid position, cell size, color"
)]
fn build_cursor(
    frame: &mut PreparedFrame,
    shape: CursorShape,
    col: usize,
    row: usize,
    cw: f32,
    ch: f32,
    color: Rgb,
) {
    let x = col as f32 * cw;
    let y = row as f32 * ch;
    let t = 2.0_f32;

    match shape {
        CursorShape::Block => {
            frame
                .cursors
                .push_cursor(ScreenRect { x, y, w: cw, h: ch }, color, 1.0);
        }
        CursorShape::Bar => {
            frame
                .cursors
                .push_cursor(ScreenRect { x, y, w: t, h: ch }, color, 1.0);
        }
        CursorShape::Underline => {
            let rect = ScreenRect {
                x,
                y: y + ch - t,
                w: cw,
                h: t,
            };
            frame.cursors.push_cursor(rect, color, 1.0);
        }
        CursorShape::HollowBlock => {
            // Top edge.
            frame
                .cursors
                .push_cursor(ScreenRect { x, y, w: cw, h: t }, color, 1.0);
            // Bottom edge.
            let rect = ScreenRect {
                x,
                y: y + ch - t,
                w: cw,
                h: t,
            };
            frame.cursors.push_cursor(rect, color, 1.0);
            // Left edge.
            frame
                .cursors
                .push_cursor(ScreenRect { x, y, w: t, h: ch }, color, 1.0);
            // Right edge.
            let rect = ScreenRect {
                x: x + cw - t,
                y,
                w: t,
                h: ch,
            };
            frame.cursors.push_cursor(rect, color, 1.0);
        }
        CursorShape::Hidden => {}
    }
}

#[cfg(test)]
mod tests;
