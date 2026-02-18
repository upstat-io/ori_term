//! Rasterization of patterned underline decorations (curly, dotted, dashed).
//!
//! Each pattern is rasterized once into an alpha bitmap at the current cell
//! width and stroke thickness, then inserted into the monochrome atlas. The
//! prepare phase emits 1 glyph instance per decorated cell instead of O(w)
//! rect instances.

use crate::font::collection::RasterizedGlyph;
use crate::font::{CellMetrics, FaceIdx, RasterKey, SyntheticFlags};

use super::Canvas;

/// Synthetic glyph ID for curly underline pattern.
pub(crate) const CURLY_GLYPH_ID: u16 = 0xFFF0;

/// Synthetic glyph ID for dotted underline pattern.
pub(crate) const DOTTED_GLYPH_ID: u16 = 0xFFF1;

/// Synthetic glyph ID for dashed underline pattern.
pub(crate) const DASHED_GLYPH_ID: u16 = 0xFFF2;

/// Construct a [`RasterKey`] for a decoration pattern glyph.
pub(crate) fn decoration_key(glyph_id: u16, size_q6: u32) -> RasterKey {
    RasterKey {
        glyph_id,
        face_idx: FaceIdx::BUILTIN,
        size_q6,
        synthetic: SyntheticFlags::NONE,
    }
}

/// Rasterize a curly underline pattern.
///
/// Produces a sine wave with period = `cell_width`, amplitude scaling with
/// thickness. The bitmap height accommodates the full wave plus stroke.
pub(crate) fn rasterize_curly(metrics: &CellMetrics) -> Option<RasterizedGlyph> {
    let w = metrics.width.round() as u32;
    let t = metrics.stroke_size;
    if w == 0 {
        return None;
    }

    let amplitude = (t * 2.0).max(2.0);
    // Height = 2 * amplitude + thickness (covers full sine wave extent).
    let h = (2.0 * amplitude + t).ceil() as u32;
    if h == 0 {
        return None;
    }

    let mut canvas = Canvas::new(w, h);
    let center_y = amplitude;

    for dx in 0..w {
        let phase = (dx as f32 / w as f32) * std::f32::consts::TAU;
        let offset = (phase.sin() * amplitude).round();
        let py = center_y + offset;
        canvas.fill_rect(dx as f32, py, 1.0, t, 255);
    }

    Some(canvas.into_rasterized_glyph())
}

/// Rasterize a dotted underline pattern (1px on, 1px off).
pub(crate) fn rasterize_dotted(metrics: &CellMetrics) -> Option<RasterizedGlyph> {
    let w = metrics.width.round() as u32;
    let t = metrics.stroke_size;
    let h = t.ceil() as u32;
    if w == 0 || h == 0 {
        return None;
    }

    let mut canvas = Canvas::new(w, h);
    let steps = w as usize;
    for dx in (0..steps).step_by(2) {
        canvas.fill_rect(dx as f32, 0.0, 1.0, t, 255);
    }

    Some(canvas.into_rasterized_glyph())
}

/// Rasterize a dashed underline pattern (3px on, 2px off).
pub(crate) fn rasterize_dashed(metrics: &CellMetrics) -> Option<RasterizedGlyph> {
    let w = metrics.width.round() as u32;
    let t = metrics.stroke_size;
    let h = t.ceil() as u32;
    if w == 0 || h == 0 {
        return None;
    }

    let mut canvas = Canvas::new(w, h);
    let steps = w as usize;
    for dx in 0..steps {
        if dx % 5 < 3 {
            canvas.fill_rect(dx as f32, 0.0, 1.0, t, 255);
        }
    }

    Some(canvas.into_rasterized_glyph())
}
