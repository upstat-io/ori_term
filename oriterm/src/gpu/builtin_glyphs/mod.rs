//! Built-in geometric glyph rasterization for box drawing, block elements,
//! braille patterns, and powerline symbols.
//!
//! These characters are rasterized from cell dimensions on the CPU into alpha
//! bitmaps, then inserted into the glyph atlas and rendered through the normal
//! texture-sampled pipeline. This produces pixel-perfect results regardless of
//! which font is loaded.

mod blocks;
mod box_drawing;
mod braille;
pub(crate) mod decorations;
mod powerline;

use wgpu::Queue;

use oriterm_core::CellFlags;

use crate::font::collection::RasterizedGlyph;
use crate::font::{CellMetrics, FaceIdx, GlyphFormat, RasterKey, SyntheticFlags, is_builtin};

use super::atlas::GlyphAtlas;
use super::frame_input::FrameInput;

/// Construct a [`RasterKey`] for a built-in glyph.
///
/// Uses [`FaceIdx::BUILTIN`] as the face index and the character's codepoint
/// as the glyph ID. All built-in codepoints fit in `u16`.
pub(crate) fn raster_key(ch: char, size_q6: u32) -> RasterKey {
    RasterKey {
        glyph_id: ch as u16,
        face_idx: FaceIdx::BUILTIN,
        size_q6,
        synthetic: SyntheticFlags::NONE,
    }
}

/// Rasterize a built-in glyph to an alpha bitmap.
///
/// Returns `None` if the character is not a recognized built-in or if cell
/// dimensions are zero. The returned glyph has `bearing_x = 0`, `bearing_y = 0`
/// and fills the entire cell.
pub(crate) fn rasterize(ch: char, cell_w: u32, cell_h: u32) -> Option<RasterizedGlyph> {
    if cell_w == 0 || cell_h == 0 {
        return None;
    }

    let mut canvas = Canvas::new(cell_w, cell_h);
    let handled = match ch {
        '\u{2500}'..='\u{257F}' => box_drawing::draw_box(&mut canvas, ch),
        '\u{2580}'..='\u{259F}' => blocks::draw_block(&mut canvas, ch),
        '\u{2800}'..='\u{28FF}' => braille::draw_braille(&mut canvas, ch),
        '\u{E0B0}'..='\u{E0B4}' | '\u{E0B6}' => powerline::draw_powerline(&mut canvas, ch),
        _ => false,
    };

    if handled {
        Some(canvas.into_rasterized_glyph())
    } else {
        None
    }
}

/// Rasterize and cache all built-in glyphs and decoration patterns in the frame.
///
/// Single-pass scan: checks each cell for built-in characters (box drawing,
/// block elements, braille, powerline) and patterned underline flags (curly,
/// dotted, dashed). Built-in glyphs are rasterized individually on cache miss;
/// decoration patterns are collected as flags and rasterized once after the scan.
pub(crate) fn ensure_builtins_cached(
    input: &FrameInput,
    size_q6: u32,
    atlas: &mut GlyphAtlas,
    queue: &Queue,
) {
    let cell_w = input.cell_size.width.round() as u32;
    let cell_h = input.cell_size.height.round() as u32;
    let metrics = &input.cell_size;

    let mut need_curly = false;
    let mut need_dotted = false;
    let mut need_dashed = false;

    for cell in &input.content.cells {
        // Built-in geometric glyphs.
        if is_builtin(cell.ch) {
            let key = raster_key(cell.ch, size_q6);
            if atlas.lookup_touch(key).is_none() && !atlas.is_known_empty(key) {
                if let Some(glyph) = rasterize(cell.ch, cell_w, cell_h) {
                    atlas.insert(key, &glyph, queue);
                } else {
                    atlas.mark_empty(key);
                }
            }
        }

        // Decoration pattern flags (early-exit when all 3 found).
        if !(need_curly && need_dotted && need_dashed) {
            need_curly = need_curly || cell.flags.contains(CellFlags::CURLY_UNDERLINE);
            need_dotted = need_dotted || cell.flags.contains(CellFlags::DOTTED_UNDERLINE);
            need_dashed = need_dashed || cell.flags.contains(CellFlags::DASHED_UNDERLINE);
        }
    }

    if need_curly {
        cache_decoration(
            decorations::CURLY_GLYPH_ID,
            size_q6,
            metrics,
            atlas,
            queue,
            decorations::rasterize_curly,
        );
    }
    if need_dotted {
        cache_decoration(
            decorations::DOTTED_GLYPH_ID,
            size_q6,
            metrics,
            atlas,
            queue,
            decorations::rasterize_dotted,
        );
    }
    if need_dashed {
        cache_decoration(
            decorations::DASHED_GLYPH_ID,
            size_q6,
            metrics,
            atlas,
            queue,
            decorations::rasterize_dashed,
        );
    }
}

/// Cache a single decoration pattern if not already present.
fn cache_decoration(
    glyph_id: u16,
    size_q6: u32,
    metrics: &CellMetrics,
    atlas: &mut GlyphAtlas,
    queue: &Queue,
    rasterize_fn: fn(&CellMetrics) -> Option<RasterizedGlyph>,
) {
    let key = decorations::decoration_key(glyph_id, size_q6);
    if atlas.lookup_touch(key).is_some() || atlas.is_known_empty(key) {
        return;
    }
    if let Some(glyph) = rasterize_fn(metrics) {
        atlas.insert(key, &glyph, queue);
    } else {
        atlas.mark_empty(key);
    }
}

// ── Canvas ──

/// Simple alpha bitmap for rasterizing built-in glyphs.
///
/// Coordinates are in pixel space relative to the cell origin (0, 0).
/// All drawing operations clip to canvas bounds.
pub(super) struct Canvas {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

impl Canvas {
    /// Create a zero-filled canvas of the given pixel dimensions.
    fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![0; (width * height) as usize],
            width,
            height,
        }
    }

    /// Canvas width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Canvas height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Fill a rectangle with a solid alpha value.
    ///
    /// Uses `floor()` for the start edge and `ceil()` for the end edge to
    /// ensure complete coverage of the specified area. Out-of-bounds regions
    /// are clipped.
    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, alpha: u8) {
        let x0 = (x.floor() as i32).max(0) as u32;
        let y0 = (y.floor() as i32).max(0) as u32;
        let x1 = ((x + w).ceil() as u32).min(self.width);
        let y1 = ((y + h).ceil() as u32).min(self.height);

        for py in y0..y1 {
            let row_start = (py * self.width) as usize;
            for px in x0..x1 {
                self.data[row_start + px as usize] = alpha;
            }
        }
    }

    /// Blend an alpha value at a single pixel (saturating add, clamped to 255).
    ///
    /// Out-of-bounds coordinates are silently ignored.
    pub fn blend_pixel(&mut self, x: i32, y: i32, alpha: u8) {
        if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
            let idx = (y as u32 * self.width + x as u32) as usize;
            self.data[idx] = self.data[idx].saturating_add(alpha);
        }
    }

    /// Draw an anti-aliased line segment with the given thickness.
    ///
    /// Uses signed-distance-field evaluation: each pixel's alpha is determined
    /// by its perpendicular distance to the line segment, with a 1px anti-alias
    /// transition zone at the edges.
    pub fn fill_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = dx.hypot(dy);
        if len < f32::EPSILON {
            return;
        }

        // Unit normal perpendicular to the line.
        let nx = -dy / len;
        let ny = dx / len;

        let half_t = thickness / 2.0;
        let max_dist = half_t + 0.5;

        // Bounding box expanded by thickness.
        let min_x = (x0.min(x1) - max_dist).floor().max(0.0) as u32;
        let min_y = (y0.min(y1) - max_dist).floor().max(0.0) as u32;
        let max_x = ((x0.max(x1) + max_dist).ceil() as u32).min(self.width);
        let max_y = ((y0.max(y1) + max_dist).ceil() as u32).min(self.height);

        let inv_len = 1.0 / len;

        for py in min_y..max_y {
            for px in min_x..max_x {
                let pc_x = px as f32 + 0.5;
                let pc_y = py as f32 + 0.5;

                // Perpendicular distance from pixel center to infinite line.
                let d = ((pc_x - x0) * nx + (pc_y - y0) * ny).abs();

                // Longitudinal parameter along the line (0..len).
                let along = (pc_x - x0) * dx * inv_len + (pc_y - y0) * dy * inv_len;

                // Clip to line segment with 0.5px extension for clean endpoints.
                if along < -0.5 || along > len + 0.5 {
                    continue;
                }

                let alpha = if d <= half_t - 0.5 {
                    255u8
                } else if d <= half_t + 0.5 {
                    ((half_t + 0.5 - d) * 255.0) as u8
                } else {
                    continue;
                };

                self.blend_pixel(px as i32, py as i32, alpha);
            }
        }
    }

    /// Consume the canvas and produce a [`RasterizedGlyph`].
    ///
    /// The glyph fills the entire cell: `bearing_x = 0`, `bearing_y = 0`.
    pub(super) fn into_rasterized_glyph(self) -> RasterizedGlyph {
        RasterizedGlyph {
            width: self.width,
            height: self.height,
            bearing_x: 0,
            bearing_y: 0,
            advance: self.width as f32,
            format: GlyphFormat::Alpha,
            bitmap: self.data,
        }
    }
}

#[cfg(test)]
mod tests;
