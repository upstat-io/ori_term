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
    curly_amplitude, decoration_key, CURLY_GLYPH_ID, DASHED_GLYPH_ID, DOTTED_GLYPH_ID,
};
use crate::gpu::instance_writer::InstanceWriter;

use super::AtlasLookup;

/// Frame-level context for decoration rendering.
///
/// Bundles the instance writers, atlas, size key, and font metrics that are
/// invariant across cells within a single frame. Per-cell parameters (flags,
/// colors, position) are passed to [`draw`](Self::draw).
pub(super) struct DecorationContext<'a> {
    pub(super) backgrounds: &'a mut InstanceWriter,
    pub(super) glyphs: &'a mut InstanceWriter,
    pub(super) atlas: &'a dyn AtlasLookup,
    pub(super) size_q6: u32,
    pub(super) metrics: &'a CellMetrics,
}

impl DecorationContext<'_> {
    /// Emit underline and strikethrough decorations for a single cell.
    ///
    /// Fast-path: returns immediately when no decoration flags are set.
    /// Underlines and strikethrough are independent — both can coexist on
    /// the same cell.
    ///
    /// Patterned underlines (curly, dotted, dashed) are emitted as glyph
    /// instances from the atlas. If the atlas entry is missing (e.g. in tests),
    /// falls back to per-pixel rect emission.
    pub(super) fn draw(
        &mut self,
        flags: CellFlags,
        underline_color: Option<Rgb>,
        fg: Rgb,
        x: f32,
        y: f32,
        cell_width: f32,
    ) {
        let has_underline = flags.intersects(CellFlags::ALL_UNDERLINES);
        let has_strikethrough = flags.contains(CellFlags::STRIKETHROUGH);

        if !has_underline && !has_strikethrough {
            return;
        }

        let t = self.metrics.stroke_size;

        if has_underline {
            let color = underline_color.unwrap_or(fg);
            let underline_y = y + self.metrics.baseline + self.metrics.underline_offset;
            self.draw_underline(flags, color, x, underline_y, cell_width, t);
        }

        if has_strikethrough {
            let strike_y = y + self.metrics.baseline - self.metrics.strikeout_offset;
            self.backgrounds.push_rect(x, strike_y, cell_width, t, fg, 1.0);
        }
    }

    /// Dispatch to the appropriate underline style.
    ///
    /// Priority: curly > double > dotted > dashed > single.
    fn draw_underline(
        &mut self,
        flags: CellFlags,
        color: Rgb,
        x: f32,
        y: f32,
        w: f32,
        t: f32,
    ) {
        if flags.contains(CellFlags::CURLY_UNDERLINE) {
            if !self.try_atlas_decoration(CURLY_GLYPH_ID, color, x, y) {
                draw_curly_underline_rects(self.backgrounds, color, x, y, w, t);
            }
        } else if flags.contains(CellFlags::DOUBLE_UNDERLINE) {
            draw_double_underline(self.backgrounds, color, x, y, w, t);
        } else if flags.contains(CellFlags::DOTTED_UNDERLINE) {
            if !self.try_atlas_decoration(DOTTED_GLYPH_ID, color, x, y) {
                draw_dotted_underline_rects(self.backgrounds, color, x, y, w, t);
            }
        } else if flags.contains(CellFlags::DASHED_UNDERLINE) {
            if !self.try_atlas_decoration(DASHED_GLYPH_ID, color, x, y) {
                draw_dashed_underline_rects(self.backgrounds, color, x, y, w, t);
            }
        } else {
            // Single underline (plain UNDERLINE flag).
            self.backgrounds.push_rect(x, y, w, t, color, 1.0);
        }
    }

    /// Try to emit a patterned decoration as a single atlas glyph instance.
    ///
    /// Returns `true` if the atlas had the entry and the glyph was emitted,
    /// `false` to signal the caller should fall back to rect emission.
    fn try_atlas_decoration(
        &mut self,
        glyph_id: u16,
        color: Rgb,
        x: f32,
        y: f32,
    ) -> bool {
        let key = decoration_key(glyph_id, self.size_q6);
        if let Some(entry) = self.atlas.lookup_key(key) {
            // Curly decorations are taller than the underline position —
            // center the bitmap vertically on the underline Y coordinate.
            let glyph_y = if glyph_id == CURLY_GLYPH_ID {
                y - curly_amplitude(self.metrics.stroke_size)
            } else {
                y
            };
            let uv = [entry.uv_x, entry.uv_y, entry.uv_w, entry.uv_h];
            self.glyphs.push_glyph(
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
    let amplitude = curly_amplitude(t);
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
