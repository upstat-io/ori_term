//! Instance emission helpers: shaped glyph emitter, cursor, and overlay markers.
//!
//! Extracted from `prepare/mod.rs` to keep the main module under 500 lines.
//! All functions emit instances into a [`PreparedFrame`].

use oriterm_core::{CursorShape, Rgb};

use super::super::atlas::AtlasKind;
use super::super::frame_input::FrameInput;
use super::super::prepared_frame::PreparedFrame;
use super::AtlasLookup;
use crate::font::{FaceIdx, FontRealm, RasterKey, SyntheticFlags, subpx_bin, subpx_offset};
use crate::gpu::instance_writer::ScreenRect;
use oriterm_ui::text::ShapedGlyph;

/// Prompt marker bar color: subtle blue accent.
const PROMPT_MARKER_COLOR: Rgb = Rgb {
    r: 80,
    g: 140,
    b: 220,
};

/// Prompt marker bar width in pixels.
const PROMPT_MARKER_WIDTH: f32 = 2.0;

/// Frame-level context for shaped glyph emission.
///
/// Bundles the atlas, size key, baseline, and output frame that are invariant
/// across cells. Per-cell parameters (row glyphs, column, position, color)
/// are passed to [`emit`](Self::emit).
pub(super) struct GlyphEmitter<'a> {
    pub baseline: f32,
    pub size_q6: u32,
    pub hinted: bool,
    pub fg_dim: f32,
    pub atlas: &'a dyn AtlasLookup,
    pub frame: &'a mut PreparedFrame,
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
        reason = "per-cell shaped glyph params: glyph source, col_starts, grid column, screen position, color"
    )]
    pub fn emit(
        &mut self,
        row_glyphs: &[ShapedGlyph],
        col_starts: &[usize],
        start_idx: usize,
        col: usize,
        x: f32,
        y: f32,
        fg: Rgb,
        bg: Rgb,
    ) {
        let mut is_first = true;
        for (sg, &cs) in row_glyphs[start_idx..].iter().zip(&col_starts[start_idx..]) {
            // Stop at the first glyph in a different column (combining marks are contiguous).
            if !is_first && cs != col {
                break;
            }
            is_first = false;

            let subpx = subpx_bin(sg.x_offset);
            let key = RasterKey {
                glyph_id: sg.glyph_id,
                face_idx: FaceIdx(sg.face_index),
                size_q6: self.size_q6,
                synthetic: SyntheticFlags::from_bits_truncate(sg.synthetic),
                hinted: self.hinted,
                subpx_x: subpx,
                font_realm: FontRealm::Terminal,
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
                writer.push_glyph_with_bg(rect, uv, fg, bg, self.fg_dim, entry.page);
            }
        }
    }
}

/// Draw visual prompt markers as thin colored bars at the left margin.
///
/// For each viewport row in `prompt_marker_rows`, emits a 2px-wide
/// colored rectangle at the left edge of the row. Renders into the
/// cursor layer so it appears above cell backgrounds.
pub(super) fn draw_prompt_markers(input: &FrameInput, frame: &mut PreparedFrame, ox: f32, oy: f32) {
    if input.prompt_marker_rows.is_empty() {
        return;
    }
    let ch = input.cell_size.height;
    for &row in &input.prompt_marker_rows {
        let x = ox;
        let y = oy + row as f32 * ch;
        let rect = ScreenRect {
            x,
            y,
            w: PROMPT_MARKER_WIDTH,
            h: ch,
        };
        frame.cursors.push_cursor(rect, PROMPT_MARKER_COLOR, 0.7);
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
    reason = "cursor geometry: frame, shape, grid position, cell size, origin offset, color"
)]
pub(super) fn build_cursor(
    frame: &mut PreparedFrame,
    shape: CursorShape,
    col: usize,
    row: usize,
    cw: f32,
    ch: f32,
    ox: f32,
    oy: f32,
    color: Rgb,
) {
    let x = ox + col as f32 * cw;
    let y = oy + row as f32 * ch;
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

/// Draw implicit URL hover underlines as continuous rects per segment.
///
/// Renders into the cursor layer (on top of glyphs) so the underline is
/// not obscured by character pixels that extend into the underline zone
/// (e.g. `/` descenders in `https://`).
pub(super) fn draw_url_hover_underline(
    input: &FrameInput,
    frame: &mut PreparedFrame,
    ox: f32,
    oy: f32,
) {
    if input.hovered_url_segments.is_empty() {
        return;
    }
    let cw = input.cell_size.width;
    let ch = input.cell_size.height;
    let underline_y_offset = input.cell_size.baseline + input.cell_size.underline_offset;
    let t = input.cell_size.stroke_size;
    let fg = input.palette.foreground;

    for &(line, start_col, end_col) in &input.hovered_url_segments {
        let x = ox + start_col as f32 * cw;
        let y = oy + line as f32 * ch + underline_y_offset;
        let w = (end_col - start_col + 1) as f32 * cw;
        frame
            .cursors
            .push_cursor(ScreenRect { x, y, w, h: t }, fg, 1.0);
    }
}

/// Emit image quads from `RenderableContent`, splitting by z-index.
///
/// Images with negative z-index go to `image_quads_below` (drawn before text),
/// others go to `image_quads_above` (drawn after text).
pub(super) fn emit_image_quads(input: &FrameInput, frame: &mut PreparedFrame, ox: f32, oy: f32) {
    for img in &input.content.images {
        let quad = super::super::prepared_frame::ImageQuad {
            image_id: img.image_id,
            x: ox + img.viewport_x,
            y: oy + img.viewport_y,
            w: img.display_width,
            h: img.display_height,
            uv_x: img.source_x,
            uv_y: img.source_y,
            uv_w: img.source_w,
            uv_h: img.source_h,
            opacity: img.opacity,
        };
        if img.z_index < 0 {
            frame.image_quads_below.push(quad);
        } else {
            frame.image_quads_above.push(quad);
        }
    }
}
