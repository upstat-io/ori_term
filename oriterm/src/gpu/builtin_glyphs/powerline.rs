//! Powerline symbol rendering (U+E0B0–U+E0B4, U+E0B6).
//!
//! Solid and outline triangles for powerline status line separators.
//! Only the 6 geometric separator codepoints are handled; icon glyphs
//! (U+E0A0–U+E0A3 branch/lock) fall through to the font path.
//!
//! Edge computation uses boundary-aligned interpolation (`(iy + 1) / half`
//! instead of center-sampled `(iy + 0.5 - mid) / mid`) so the triangle
//! reaches full cell width at the center rows. Anti-aliased sub-pixel
//! rendering on diagonal edges eliminates visible stagger.

use super::Canvas;

/// Draw a powerline symbol onto the canvas. Returns `true` if handled.
pub(super) fn draw_powerline(canvas: &mut Canvas, ch: char) -> bool {
    match ch {
        // Solid right-pointing triangle / rounded separator.
        '\u{E0B0}' | '\u{E0B4}' => draw_triangle_right(canvas),
        // Right-pointing triangle outline.
        '\u{E0B1}' => draw_triangle_right_thin(canvas),
        // Solid left-pointing triangle / rounded separator.
        '\u{E0B2}' | '\u{E0B6}' => draw_triangle_left(canvas),
        // Left-pointing triangle outline.
        '\u{E0B3}' => draw_triangle_left_thin(canvas),
        // Icons and unrecognized extra glyphs — fall through to font.
        _ => return false,
    }
    true
}

/// Fractional coverage from center toward tip for scanline `iy`.
///
/// Returns a value in `0.0..=1.0` where 1.0 is the center (full width)
/// and values near 0.0 are the tips. Uses row boundaries (not centers)
/// so the center rows evaluate to exactly 1.0.
fn coverage(iy: u32, h: u32) -> f32 {
    let half = h as f32 / 2.0;
    if iy < h / 2 {
        (iy + 1) as f32 / half
    } else {
        (h - iy) as f32 / half
    }
}

/// Solid right-pointing triangle filling the entire cell.
///
/// Tip at the right-center, base along the left edge. Anti-aliased
/// sub-pixel edge rendering prevents visible stagger on the diagonal.
fn draw_triangle_right(canvas: &mut Canvas) {
    let w = canvas.width();
    let h = canvas.height();

    for iy in 0..h {
        let edge = (w as f32 * coverage(iy, h)).min(w as f32);
        fill_aa_span_right(canvas, iy, edge);
    }
}

/// Thin right-pointing triangle (outline only).
fn draw_triangle_right_thin(canvas: &mut Canvas) {
    let w = canvas.width();
    let h = canvas.height();
    let thin = 1.0f32.max((w as f32 / 8.0).round());

    for iy in 0..h {
        let edge = (w as f32 * coverage(iy, h)).min(w as f32);
        if edge > 0.0 {
            let start = (edge - thin).max(0.0);
            fill_aa_band(canvas, iy, start, edge.min(w as f32));
        }
    }
}

/// Solid left-pointing triangle filling the entire cell.
///
/// Tip at the left-center, base along the right edge. Anti-aliased
/// sub-pixel edge rendering prevents visible stagger on the diagonal.
fn draw_triangle_left(canvas: &mut Canvas) {
    let w = canvas.width();
    let h = canvas.height();

    for iy in 0..h {
        let edge = (w as f32 * coverage(iy, h)).min(w as f32);
        fill_aa_span_left(canvas, iy, w, edge);
    }
}

/// Thin left-pointing triangle (outline only).
fn draw_triangle_left_thin(canvas: &mut Canvas) {
    let w = canvas.width();
    let h = canvas.height();
    let thin = 1.0f32.max((w as f32 / 8.0).round());

    for iy in 0..h {
        let edge = (w as f32 * coverage(iy, h)).min(w as f32);
        if edge > 0.0 {
            let outer = w as f32 - edge;
            let inner = (outer + thin).min(w as f32);
            fill_aa_band(canvas, iy, outer, inner);
        }
    }
}

// ── Anti-aliased fill helpers ──

/// Fill from x=0 to a fractional `edge` position on a single scanline.
///
/// Fully opaque pixels up to `floor(edge)`, then a proportional-alpha
/// pixel at the boundary. Guarantees the rightmost non-zero pixel
/// advances by at most 1 per scanline when edge step < 2.
fn fill_aa_span_right(canvas: &mut Canvas, iy: u32, edge: f32) {
    let full = edge as u32; // floor
    if full > 0 {
        canvas.fill_rect(0.0, iy as f32, full as f32, 1.0, 255);
    }
    let frac = edge - full as f32;
    if frac > 0.0 {
        let alpha = (frac * 255.0) as u8;
        canvas.blend_pixel(full as i32, iy as i32, alpha);
    }
}

/// Fill from a fractional `edge` position to x=`w` on a single scanline.
///
/// Mirror of [`fill_aa_span_right`] for left-pointing triangles.
fn fill_aa_span_left(canvas: &mut Canvas, iy: u32, w: u32, edge: f32) {
    let full = edge as u32; // floor
    let start = w - full;
    if full > 0 {
        canvas.fill_rect(start as f32, iy as f32, full as f32, 1.0, 255);
    }
    let frac = edge - full as f32;
    if frac > 0.0 {
        let alpha = (frac * 255.0) as u8;
        canvas.blend_pixel(start as i32 - 1, iy as i32, alpha);
    }
}

/// Fill a horizontal band from `start` to `end` with AA on both edges.
///
/// Used for thin/outline triangles where both the inner and outer edges
/// need sub-pixel rendering.
fn fill_aa_band(canvas: &mut Canvas, iy: u32, start: f32, end: f32) {
    let s_floor = start.floor() as u32;
    let e_floor = end as u32; // floor

    // Fully opaque interior pixels.
    let interior_start = start.ceil() as u32;
    if interior_start < e_floor {
        canvas.fill_rect(
            interior_start as f32,
            iy as f32,
            (e_floor - interior_start) as f32,
            1.0,
            255,
        );
    }

    // AA on the start-edge pixel.
    let s_frac = start - s_floor as f32;
    let s_alpha = ((1.0 - s_frac) * 255.0) as u8;
    if s_alpha > 0 {
        canvas.blend_pixel(s_floor as i32, iy as i32, s_alpha);
    }

    // AA on the end-edge pixel (only if it differs from the start pixel).
    let e_frac = end - e_floor as f32;
    if e_frac > 0.0 && e_floor != s_floor {
        let e_alpha = (e_frac * 255.0) as u8;
        canvas.blend_pixel(e_floor as i32, iy as i32, e_alpha);
    }
}
