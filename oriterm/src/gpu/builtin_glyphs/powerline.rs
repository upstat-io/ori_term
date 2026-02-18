//! Powerline symbol rendering (U+E0B0–U+E0B4, U+E0B6).
//!
//! Solid and outline triangles for powerline status line separators.
//! Only the 6 geometric separator codepoints are handled; icon glyphs
//! (U+E0A0–U+E0A3 branch/lock) fall through to the font path.

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

/// Solid right-pointing triangle filling the entire cell.
///
/// Tip at the right-center, base along the left edge.
fn draw_triangle_right(canvas: &mut Canvas) {
    let w = canvas.width() as f32;
    let h = canvas.height() as f32;
    let mid = h / 2.0;

    for iy in 0..canvas.height() {
        let frac = (iy as f32 + 0.5 - mid).abs() / mid;
        let line_w = w * (1.0 - frac);
        if line_w > 0.0 {
            canvas.fill_rect(0.0, iy as f32, line_w, 1.0, 255);
        }
    }
}

/// Thin right-pointing triangle (outline only).
fn draw_triangle_right_thin(canvas: &mut Canvas) {
    let w = canvas.width() as f32;
    let h = canvas.height() as f32;
    let mid = h / 2.0;
    let thin = 1.0f32.max((w / 8.0).round());

    for iy in 0..canvas.height() {
        let frac = (iy as f32 + 0.5 - mid).abs() / mid;
        let edge_x = w * (1.0 - frac);
        if edge_x > 0.0 {
            canvas.fill_rect(edge_x - thin, iy as f32, thin, 1.0, 255);
        }
    }
}

/// Solid left-pointing triangle filling the entire cell.
///
/// Tip at the left-center, base along the right edge.
fn draw_triangle_left(canvas: &mut Canvas) {
    let w = canvas.width() as f32;
    let h = canvas.height() as f32;
    let mid = h / 2.0;

    for iy in 0..canvas.height() {
        let frac = (iy as f32 + 0.5 - mid).abs() / mid;
        let line_w = w * (1.0 - frac);
        if line_w > 0.0 {
            canvas.fill_rect(w - line_w, iy as f32, line_w, 1.0, 255);
        }
    }
}

/// Thin left-pointing triangle (outline only).
fn draw_triangle_left_thin(canvas: &mut Canvas) {
    let w = canvas.width() as f32;
    let h = canvas.height() as f32;
    let mid = h / 2.0;
    let thin = 1.0f32.max((w / 8.0).round());

    for iy in 0..canvas.height() {
        let frac = (iy as f32 + 0.5 - mid).abs() / mid;
        let edge_x = w * (1.0 - frac);
        if edge_x > thin {
            canvas.fill_rect(w - edge_x, iy as f32, thin, 1.0, 255);
        }
    }
}
