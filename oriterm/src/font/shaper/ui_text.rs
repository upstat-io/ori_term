//! UI text shaping — free-form pixel positioning for non-grid text.
//!
//! Tab bar titles, search bar content, and overlays need text that isn't
//! tied to grid columns. This module provides [`shape_text_string`] to shape
//! arbitrary strings into [[`ShapedGlyph`]]s with `x_advance` positioning,
//! plus [`measure_text`], [`shape_text`], and [`truncate_with_ellipsis`] for layout.

use std::borrow::Cow;

use oriterm_ui::text::{FontWeight, ShapedGlyph, ShapedText, TextMetrics, TextOverflow, TextStyle};

use crate::font::collection::FontCollection;
use crate::font::{FaceIdx, GlyphStyle};

/// Shape a plain text string for UI rendering (tab titles, search bar, overlays).
///
/// Segments text into runs by font face, shapes each run through rustybuzz,
/// and emits [[`ShapedGlyph`]]s with pixel-based `x_advance` positioning.
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
    output: &mut Vec<ShapedGlyph>,
    buffer_slot: &mut Option<rustybuzz::UnicodeBuffer>,
) {
    output.clear();
    if text.is_empty() {
        return;
    }

    let mut buffer = buffer_slot.take().unwrap_or_default();

    let mut run_start: Option<usize> = None;
    let mut run_face = FaceIdx::REGULAR;

    for (byte_idx, ch) in text.char_indices() {
        // Resolve face for non-space characters. Spaces stay in the
        // current run so rustybuzz computes the font's actual space
        // advance (proportional, not monospace cell_w).
        let face_idx = if ch == ' ' {
            run_face
        } else if is_likely_emoji(ch) {
            collection
                .resolve_prefer_emoji(ch, GlyphStyle::Regular)
                .face_idx
        } else {
            collection.resolve(ch, GlyphStyle::Regular).face_idx
        };

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

/// Shape text into a [`ShapedText`] block using the given style.
///
/// Higher-level API that handles font weight selection, overflow (clip,
/// ellipsis, wrap), and returns a complete [`ShapedText`] with layout metrics.
///
/// `max_width` limits the text width for overflow handling. Pass `f32::INFINITY`
/// for unconstrained shaping.
pub fn shape_text(
    text: &str,
    style: &TextStyle,
    max_width: f32,
    collection: &FontCollection,
) -> ShapedText {
    let glyph_style = match style.weight {
        FontWeight::Regular => GlyphStyle::Regular,
        FontWeight::Bold => GlyphStyle::Bold,
    };
    let _ = glyph_style; // Weight selection deferred until multi-weight UI fonts.

    match style.overflow {
        TextOverflow::Ellipsis => {
            let truncated = truncate_with_ellipsis(text, max_width, collection);
            shape_to_shaped_text(&truncated, collection)
        }
        TextOverflow::Clip | TextOverflow::Wrap => {
            // Clip: shape full text, let renderer clip at bounding box.
            // Wrap: full shaping for now (word-wrap deferred).
            shape_to_shaped_text(text, collection)
        }
    }
}

/// Shape text into a [`ShapedText`] block with computed metrics.
fn shape_to_shaped_text(text: &str, collection: &FontCollection) -> ShapedText {
    let faces = collection.create_shaping_faces();
    let mut glyphs = Vec::new();
    let mut buffer_slot = None;
    shape_text_string(text, &faces, collection, &mut glyphs, &mut buffer_slot);

    let width: f32 = glyphs.iter().map(|g| g.x_advance).sum();
    let metrics = collection.cell_metrics();

    ShapedText::new(glyphs, width, metrics.height, metrics.baseline)
}

/// Measure text dimensions using the given style.
///
/// Returns [`TextMetrics`] with width, height, and line count. Shapes the
/// text to compute exact proportional width. For short UI strings (dialog
/// titles, labels, button text) the cost is negligible.
pub fn measure_text_styled(
    text: &str,
    _style: &TextStyle,
    collection: &FontCollection,
) -> TextMetrics {
    let shaped = shape_to_shaped_text(text, collection);
    TextMetrics {
        width: shaped.width,
        height: shaped.height,
        line_count: 1,
    }
}

/// Measure the total pixel width of a text string using unicode widths.
///
/// Uses `unicode_width * cell_width` for measurement, consistent with
/// [`truncate_with_ellipsis`]. Exact for monospace fonts.
#[cfg(test)]
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
    output: &mut Vec<ShapedGlyph>,
    mut buffer: rustybuzz::UnicodeBuffer,
) -> rustybuzz::UnicodeBuffer {
    let Some(face) = faces.get(face_idx.as_usize()).and_then(|f| f.as_ref()) else {
        // No rustybuzz face — try cmap + font metrics for each character.
        // This handles color emoji fonts that ttf-parser can't parse for
        // shaping but that swash can still rasterize via cmap lookup.
        let cell_w = collection.cell_metrics().width;
        for ch in text.chars() {
            if let Some((gid, advance)) = collection.cmap_glyph(ch, face_idx) {
                output.push(ShapedGlyph {
                    glyph_id: gid,
                    face_index: face_idx.0,
                    synthetic: 0,
                    x_advance: advance,
                    x_offset: 0.0,
                    y_offset: 0.0,
                });
            } else {
                let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                if w == 0 {
                    continue;
                }
                output.push(ShapedGlyph {
                    glyph_id: 0,
                    face_index: face_idx.0,
                    synthetic: 0,
                    x_advance: w as f32 * cell_w,
                    x_offset: 0.0,
                    y_offset: 0.0,
                });
            }
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
        output.push(ShapedGlyph {
            glyph_id: info.glyph_id as u16,
            face_index: face_idx.0,
            synthetic: 0,
            x_advance: pos.x_advance as f32 * scale,
            x_offset: pos.x_offset as f32 * scale,
            y_offset: pos.y_offset as f32 * scale,
        });
    }

    glyph_buffer.clear()
}

/// Whether a codepoint is likely emoji and should prefer emoji font resolution.
///
/// Delegates to [`oriterm_core::is_emoji_presentation`] for the base ranges,
/// then adds ZWJ and variation selectors used in emoji sequences.
fn is_likely_emoji(cp: char) -> bool {
    oriterm_core::is_emoji_presentation(cp) || matches!(cp, '\u{200D}' | '\u{FE0F}')
}
