//! UI text shaping — free-form pixel positioning for non-grid text.
//!
//! Tab bar titles, search bar content, and overlays need text that isn't
//! tied to grid columns. This module provides [`shape_text_string`] to shape
//! arbitrary strings into [`UiShapedGlyph`]s with `x_advance` positioning,
//! plus [`measure_text`] and [`truncate_with_ellipsis`] for layout.

use std::borrow::Cow;

use crate::font::collection::FontCollection;
use crate::font::{FaceIdx, GlyphStyle};

/// A shaped glyph for UI text rendering (not tied to grid columns).
///
/// Like [`super::ShapedGlyph`] but uses `x_advance` for free-form pixel
/// positioning instead of `col_start`/`col_span` grid mapping. Always uses
/// Regular style — UI text has no bold/italic variants.
#[derive(Debug, Clone, Copy)]
pub struct UiShapedGlyph {
    /// Glyph ID within the font face (0 for advance-only entries like spaces).
    pub glyph_id: u16,
    /// Which font face this glyph comes from.
    pub face_idx: FaceIdx,
    /// Horizontal advance in pixels (cursor moves right by this amount).
    pub x_advance: f32,
    /// Shaper X offset from glyph origin in pixels.
    pub x_offset: f32,
    /// Shaper Y offset from baseline in pixels.
    pub y_offset: f32,
}

/// Shape a plain text string for UI rendering (tab titles, search bar, overlays).
///
/// Segments text into runs by font face, shapes each run through rustybuzz,
/// and emits [`UiShapedGlyph`]s with pixel-based `x_advance` positioning.
/// Spaces produce advance-only glyphs (`glyph_id=0`) at cell width.
///
/// Pass `buffer_slot` to persist the rustybuzz buffer across frames.
#[expect(
    clippy::string_slice,
    reason = "byte indices from char_indices() are always valid char boundaries"
)]
pub fn shape_text_string(
    text: &str,
    faces: &[Option<rustybuzz::Face<'_>>],
    collection: &FontCollection,
    output: &mut Vec<UiShapedGlyph>,
    buffer_slot: &mut Option<rustybuzz::UnicodeBuffer>,
) {
    output.clear();
    if text.is_empty() {
        return;
    }

    let cell_w = collection.cell_metrics().width;
    let mut buffer = buffer_slot.take().unwrap_or_default();

    let mut run_start: Option<usize> = None;
    let mut run_face = FaceIdx::REGULAR;

    for (byte_idx, ch) in text.char_indices() {
        if ch == ' ' {
            // Flush current run before the space.
            if let Some(start) = run_start.take() {
                buffer = shape_ui_run(
                    &text[start..byte_idx],
                    run_face,
                    faces,
                    collection,
                    output,
                    buffer,
                );
            }
            output.push(UiShapedGlyph {
                glyph_id: 0,
                face_idx: FaceIdx::REGULAR,
                x_advance: cell_w,
                x_offset: 0.0,
                y_offset: 0.0,
            });
            continue;
        }

        let face_idx = collection.resolve(ch, GlyphStyle::Regular).face_idx;

        if let Some(start) = run_start {
            if face_idx != run_face {
                // Face changed — flush current run.
                buffer = shape_ui_run(
                    &text[start..byte_idx],
                    run_face,
                    faces,
                    collection,
                    output,
                    buffer,
                );
                run_start = Some(byte_idx);
                run_face = face_idx;
            }
        } else {
            run_start = Some(byte_idx);
            run_face = face_idx;
        }
    }

    // Flush last run.
    if let Some(start) = run_start {
        buffer = shape_ui_run(&text[start..], run_face, faces, collection, output, buffer);
    }

    *buffer_slot = Some(buffer);
}

/// Shape a single UI text run and append results.
///
/// Returns the cleared `UnicodeBuffer` for reuse by the next run. When no
/// face is available, emits advance-only glyphs based on unicode width.
#[expect(
    clippy::too_many_arguments,
    reason = "mirrors grid shape_run with separate text+face_idx instead of ShapingRun"
)]
fn shape_ui_run(
    text: &str,
    face_idx: FaceIdx,
    faces: &[Option<rustybuzz::Face<'_>>],
    collection: &FontCollection,
    output: &mut Vec<UiShapedGlyph>,
    mut buffer: rustybuzz::UnicodeBuffer,
) -> rustybuzz::UnicodeBuffer {
    let Some(face) = faces.get(face_idx.as_usize()).and_then(|f| f.as_ref()) else {
        // No face — emit advance-only glyphs based on unicode width.
        let cell_w = collection.cell_metrics().width;
        for ch in text.chars() {
            let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if w == 0 {
                continue;
            }
            let w = w as f32;
            output.push(UiShapedGlyph {
                glyph_id: 0,
                face_idx,
                x_advance: w * cell_w,
                x_offset: 0.0,
                y_offset: 0.0,
            });
        }
        return buffer;
    };

    buffer.push_str(text);
    buffer.set_direction(rustybuzz::Direction::LeftToRight);

    let features = collection.features_for_face(face_idx);
    let glyph_buffer = rustybuzz::shape(face, features, buffer);
    let infos = glyph_buffer.glyph_infos();
    let positions = glyph_buffer.glyph_positions();

    let upem = face.units_per_em() as f32;
    let eff_size = collection.effective_size(face_idx);
    let scale = eff_size / upem;

    for (info, pos) in infos.iter().zip(positions.iter()) {
        output.push(UiShapedGlyph {
            glyph_id: info.glyph_id as u16,
            face_idx,
            x_advance: pos.x_advance as f32 * scale,
            x_offset: pos.x_offset as f32 * scale,
            y_offset: pos.y_offset as f32 * scale,
        });
    }

    glyph_buffer.clear()
}

/// Measure the total pixel width of a text string.
///
/// Uses `unicode_width * cell_width` for measurement, consistent with
/// [`truncate_with_ellipsis`]. Exact for monospace fonts. Suitable for
/// layout of short UI strings (tab titles, labels).
pub fn measure_text(text: &str, collection: &FontCollection) -> f32 {
    let cell_w = collection.cell_metrics().width;
    text.chars()
        .map(|ch| unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) as f32 * cell_w)
        .sum()
}

/// Truncate text with ellipsis if it exceeds `max_width` pixels.
///
/// Returns the original text unchanged if it fits. Otherwise, truncates at
/// a character boundary and appends `\u{2026}` (…). Uses cell-width-based
/// measurement which is exact for monospace fonts.
#[expect(
    clippy::string_slice,
    reason = "end_byte is accumulated from char_indices() offsets + len_utf8()"
)]
pub fn truncate_with_ellipsis<'a>(
    text: &'a str,
    max_width: f32,
    collection: &FontCollection,
) -> Cow<'a, str> {
    let cell_w = collection.cell_metrics().width;

    // Sum unicode widths for exact cell count in monospace.
    let total_cells: usize = text
        .chars()
        .map(|ch| unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum();
    if total_cells as f32 * cell_w <= max_width {
        return Cow::Borrowed(text);
    }

    // Ellipsis (U+2026) is width 1 in monospace.
    let budget = max_width - cell_w;
    if budget <= 0.0 {
        return Cow::Owned(String::from("\u{2026}"));
    }

    let mut used = 0.0_f32;
    let mut end_byte = 0;
    for (byte_idx, ch) in text.char_indices() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) as f32 * cell_w;
        if used + w > budget {
            break;
        }
        used += w;
        end_byte = byte_idx + ch.len_utf8();
    }

    let mut result = String::with_capacity(end_byte + 3);
    result.push_str(&text[..end_byte]);
    result.push('\u{2026}');
    Cow::Owned(result)
}
