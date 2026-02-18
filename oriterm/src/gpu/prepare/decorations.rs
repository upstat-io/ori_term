//! Text decoration rendering: underlines (single, double, curly, dotted, dashed)
//! and strikethrough.
//!
//! Simple decorations (single, double, strikethrough) are emitted as solid-color
//! rectangles into the background buffer. Patterned decorations (curly, dotted,
//! dashed) are rendered as atlas-cached glyph instances — one instance per cell
//! instead of O(`cell_width`) rect instances.
//!
//! Geometry is derived from font metrics in [`CellMetrics`] — underline
//! position and thickness come from the font's OS/2 and post tables.

use oriterm_core::{CellFlags, Rgb};

use crate::font::CellMetrics;
use crate::gpu::builtin_glyphs::decorations::{
    decoration_key, CURLY_GLYPH_ID, DASHED_GLYPH_ID, DOTTED_GLYPH_ID,
};
use crate::gpu::instance_writer::InstanceWriter;

use super::AtlasLookup;

/// Emit underline and strikethrough decorations for a single cell.
///
/// Fast-path: returns immediately when no decoration flags are set.
/// Underlines and strikethrough are independent — both can coexist on
/// the same cell.
///
/// Patterned underlines (curly, dotted, dashed) are emitted as glyph
/// instances from the atlas. If the atlas entry is missing (e.g. in tests),
/// falls back to per-pixel rect emission.
pub(super) fn draw_decorations(
    backgrounds: &mut InstanceWriter,
    glyphs: &mut InstanceWriter,
    atlas: &dyn AtlasLookup,
    size_q6: u32,
    flags: CellFlags,
    underline_color: Option<Rgb>,
    fg: Rgb,
    x: f32,
    y: f32,
    cell_width: f32,
    metrics: &CellMetrics,
) {
    let has_underline = flags.intersects(CellFlags::ALL_UNDERLINES);
    let has_strikethrough = flags.contains(CellFlags::STRIKETHROUGH);

    if !has_underline && !has_strikethrough {
        return;
    }

    let t = metrics.stroke_size;

    if has_underline {
        let color = underline_color.unwrap_or(fg);
        let underline_y = y + metrics.baseline + metrics.underline_offset;
        draw_underline(
            backgrounds,
            glyphs,
            atlas,
            size_q6,
            flags,
            color,
            x,
            underline_y,
            cell_width,
            t,
            metrics,
        );
    }

    if has_strikethrough {
        let strike_y = y + metrics.baseline - metrics.strikeout_offset;
        backgrounds.push_rect(x, strike_y, cell_width, t, fg, 1.0);
    }
}

/// Dispatch to the appropriate underline style.
///
/// Priority: curly > double > dotted > dashed > single.
fn draw_underline(
    bg: &mut InstanceWriter,
    glyphs: &mut InstanceWriter,
    atlas: &dyn AtlasLookup,
    size_q6: u32,
    flags: CellFlags,
    color: Rgb,
    x: f32,
    y: f32,
    w: f32,
    t: f32,
    metrics: &CellMetrics,
) {
    if flags.contains(CellFlags::CURLY_UNDERLINE) {
        if !try_atlas_decoration(glyphs, atlas, CURLY_GLYPH_ID, size_q6, color, x, y, metrics) {
            draw_curly_underline_rects(bg, color, x, y, w, t);
        }
    } else if flags.contains(CellFlags::DOUBLE_UNDERLINE) {
        draw_double_underline(bg, color, x, y, w, t);
    } else if flags.contains(CellFlags::DOTTED_UNDERLINE) {
        if !try_atlas_decoration(glyphs, atlas, DOTTED_GLYPH_ID, size_q6, color, x, y, metrics) {
            draw_dotted_underline_rects(bg, color, x, y, w, t);
        }
    } else if flags.contains(CellFlags::DASHED_UNDERLINE) {
        if !try_atlas_decoration(glyphs, atlas, DASHED_GLYPH_ID, size_q6, color, x, y, metrics) {
            draw_dashed_underline_rects(bg, color, x, y, w, t);
        }
    } else {
        // Single underline (plain UNDERLINE flag).
        bg.push_rect(x, y, w, t, color, 1.0);
    }
}

/// Try to emit a patterned decoration as a single atlas glyph instance.
///
/// Returns `true` if the atlas had the entry and the glyph was emitted,
/// `false` to signal the caller should fall back to rect emission.
fn try_atlas_decoration(
    glyphs: &mut InstanceWriter,
    atlas: &dyn AtlasLookup,
    glyph_id: u16,
    size_q6: u32,
    color: Rgb,
    x: f32,
    y: f32,
    metrics: &CellMetrics,
) -> bool {
    let key = decoration_key(glyph_id, size_q6);
    if let Some(entry) = atlas.lookup_key(key) {
        // Curly decorations are taller than the underline position —
        // center the bitmap vertically on the underline Y coordinate.
        let glyph_y = if glyph_id == CURLY_GLYPH_ID {
            let amplitude = (metrics.stroke_size * 2.0).max(2.0);
            y - amplitude
        } else {
            y
        };
        let uv = [entry.uv_x, entry.uv_y, entry.uv_w, entry.uv_h];
        glyphs.push_glyph(
            x,
            glyph_y,
            entry.width as f32,
            entry.height as f32,
            uv,
            color,
            1.0,
            entry.page,
        );
        true
    } else {
        false
    }
}

// ── Rect-based fallbacks (used when atlas entries are unavailable) ──

/// Curly underline fallback: per-pixel sine wave rects.
fn draw_curly_underline_rects(
    bg: &mut InstanceWriter,
    color: Rgb,
    x: f32,
    y: f32,
    w: f32,
    t: f32,
) {
    let amplitude = (t * 2.0).max(2.0);
    let steps = w as usize;
    for dx in 0..steps {
        let phase = (dx as f32 / w) * std::f32::consts::TAU;
        let offset = (phase.sin() * amplitude).round();
        bg.push_rect(x + dx as f32, y + offset, 1.0, t, color, 1.0);
    }
}

/// Double underline: two lines separated by a gap that scales with thickness.
fn draw_double_underline(
    bg: &mut InstanceWriter,
    color: Rgb,
    x: f32,
    y: f32,
    w: f32,
    t: f32,
) {
    let gap = (t + 1.0).ceil();
    bg.push_rect(x, y, w, t, color, 1.0);
    bg.push_rect(x, y - gap, w, t, color, 1.0);
}

/// Dotted underline fallback: per-pixel alternating rects.
fn draw_dotted_underline_rects(
    bg: &mut InstanceWriter,
    color: Rgb,
    x: f32,
    y: f32,
    w: f32,
    t: f32,
) {
    let steps = w as usize;
    for dx in (0..steps).step_by(2) {
        bg.push_rect(x + dx as f32, y, 1.0, t, color, 1.0);
    }
}

/// Dashed underline fallback: per-pixel 3-on-2-off rects.
fn draw_dashed_underline_rects(
    bg: &mut InstanceWriter,
    color: Rgb,
    x: f32,
    y: f32,
    w: f32,
    t: f32,
) {
    let steps = w as usize;
    for dx in 0..steps {
        if dx % 5 < 3 {
            bg.push_rect(x + dx as f32, y, 1.0, t, color, 1.0);
        }
    }
}
