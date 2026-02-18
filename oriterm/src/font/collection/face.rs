//! Internal face data and rasterization helpers.
//!
//! [`FaceData`] stores validated font bytes with swash metadata for transient
//! [`FontRef`](swash::FontRef) creation. Free functions handle validation,
//! glyph lookup, rasterization, and metrics computation.

use std::sync::Arc;

use swash::scale::{Render, ScaleContext, Source, StrikeWith, image::Content};
use swash::zeno::{Angle, Format, Transform};
use swash::{CacheKey, FontRef};

use super::RasterizedGlyph;
use crate::font::{GlyphFormat, SyntheticFlags};

/// Validated font data with swash metadata.
///
/// Raw bytes are kept in [`Arc<Vec<u8>>`] for shared ownership with rustybuzz
/// faces (Section 6). The `offset` and `cache_key` enable fast transient
/// [`FontRef`] construction without re-parsing.
pub(super) struct FaceData {
    /// Raw font file bytes.
    pub(super) bytes: Arc<Vec<u8>>,
    /// Index within a `.ttc` collection file (0 for standalone `.ttf`).
    pub(super) face_index: u32,
    /// Byte offset to the font table directory.
    offset: u32,
    /// Unique cache key for [`ScaleContext`] reuse.
    cache_key: CacheKey,
}

/// Validate font bytes and extract swash metadata.
///
/// Returns `(byte_offset, cache_key)` on success, `None` for invalid data.
pub(super) fn validate_font(data: &[u8], face_index: u32) -> Option<(u32, CacheKey)> {
    let fr = FontRef::from_index(data, face_index as usize)?;
    Some((fr.offset, fr.key))
}

/// Build a [`FaceData`] from an [`Arc<Vec<u8>>`] and face index.
///
/// Returns `None` if the font bytes are invalid.
pub(super) fn build_face(bytes: Arc<Vec<u8>>, face_index: u32) -> Option<FaceData> {
    let (offset, cache_key) = validate_font(&bytes, face_index)?;
    Some(FaceData {
        bytes,
        face_index,
        offset,
        cache_key,
    })
}

/// Create a transient swash [`FontRef`] from stored face data.
///
/// This is cheap (no parsing) because offset and `cache_key` are pre-computed.
pub(super) fn font_ref(fd: &FaceData) -> FontRef<'_> {
    FontRef {
        data: &fd.bytes,
        offset: fd.offset,
        key: fd.cache_key,
    }
}

/// Check whether a face covers a given character.
#[allow(dead_code, reason = "font face helpers consumed in later sections")]
pub(super) fn has_glyph(fd: &FaceData, ch: char) -> bool {
    font_ref(fd).charmap().map(ch) != 0
}

/// Map a character to its glyph ID in the given face.
///
/// Returns 0 (.notdef) if the character is not covered.
pub(super) fn glyph_id(fd: &FaceData, ch: char) -> u16 {
    font_ref(fd).charmap().map(ch)
}

/// Rasterize a glyph from face data into a bitmap.
///
/// Tries color sources first (COLR outlines, CBDT/sbix bitmaps), then falls
/// back to the configured outline format. Returns `None` for empty glyphs.
///
/// When `synthetic` flags are set, applies outline manipulation before
/// rasterization: emboldening for [`SyntheticFlags::BOLD`] and a 14-degree
/// skew for [`SyntheticFlags::ITALIC`]. Synthesis is skipped for color
/// glyphs (bitmaps can't be outline-manipulated).
///
/// When a color source produces output, the returned [`RasterizedGlyph`] has
/// `format: GlyphFormat::Color` with RGBA premultiplied data regardless of
/// the requested `format`. Callers must route color glyphs to a separate
/// RGBA atlas.
#[allow(
    clippy::too_many_arguments,
    reason = "rasterization requires all these parameters"
)]
pub(super) fn rasterize_from_face(
    fd: &FaceData,
    glyph_id: u16,
    size_px: f32,
    wght: Option<f32>,
    synthetic: SyntheticFlags,
    cell_height: f32,
    format: GlyphFormat,
    ctx: &mut ScaleContext,
) -> Option<RasterizedGlyph> {
    let fr = font_ref(fd);
    let advance = fr.glyph_metrics(&[]).scale(size_px).advance_width(glyph_id);

    let builder = ctx.builder(fr).size(size_px).hint(true);
    let mut scaler = match wght {
        Some(w) => builder.variations(&[("wght", w)]).build(),
        None => builder.build(),
    };

    let zeno_fmt = match format {
        GlyphFormat::SubpixelRgb => Format::Subpixel,
        GlyphFormat::SubpixelBgr => Format::subpixel_bgra(),
        GlyphFormat::Alpha | GlyphFormat::Color => Format::Alpha,
    };

    // Try color sources first, then fall back to outline.
    let mut render = Render::new(&[
        Source::ColorOutline(0),
        Source::ColorBitmap(StrikeWith::BestFit),
        Source::Outline,
    ]);
    render.format(zeno_fmt);

    // Apply synthesis (outline manipulation before rasterization).
    // Order: embolden first, then transform — matches swash's internal
    // application order in Render::render().
    if synthetic.contains(SyntheticFlags::BOLD) {
        render.embolden(embolden_strength(cell_height));
    }
    if synthetic.contains(SyntheticFlags::ITALIC) {
        render.transform(Some(Transform::skew(
            Angle::from_degrees(SYNTHETIC_ITALIC_ANGLE),
            Angle::from_degrees(0.0),
        )));
    }

    let image = render.render(&mut scaler, glyph_id)?;

    let out_format = match image.content {
        Content::Color => GlyphFormat::Color,
        Content::SubpixelMask | Content::Mask => format,
    };

    Some(RasterizedGlyph {
        width: image.placement.width,
        height: image.placement.height,
        bearing_x: image.placement.left,
        bearing_y: image.placement.top,
        advance,
        format: out_format,
        bitmap: image.data,
    })
}

/// Embolden strength in pixels for synthetic bold.
///
/// Adapted from Ghostty's formula: `ceil(height_26dot6 * 64 / 2048)` converted
/// from `FreeType` 26.6 fixed-point to swash pixel coordinates. Scales with
/// font size so bold looks consistent from 8pt to 24pt.
///
/// Examples: 17px → 0.53px, 20px → 0.63px, 32px → 1.0px.
pub(super) fn embolden_strength(cell_height: f32) -> f32 {
    (cell_height * 2.0).ceil() / 64.0
}

/// Standard synthetic italic angle in degrees.
///
/// 14° matches the CSS oblique spec and is used by Ghostty (12°) and
/// cosmic-text (14°). We use 14° as the more standard value.
const SYNTHETIC_ITALIC_ANGLE: f32 = 14.0;

/// Computed font metrics for cell layout and text decorations.
pub(super) struct FontMetrics {
    pub cell_width: f32,
    pub cell_height: f32,
    pub baseline: f32,
    /// Distance below baseline to underline center (positive = below).
    pub underline_offset: f32,
    /// Thickness of underline and strikethrough strokes.
    pub stroke_size: f32,
    /// Distance above baseline to strikeout center (positive = above).
    pub strikeout_offset: f32,
}

/// Compute cell metrics from font bytes at the given pixel size.
///
/// Cell height = `ceil(ascent + |descent|)`, cell width = `ceil(advance of 'M')`,
/// baseline = `ceil(ascent)`. Decoration metrics (underline, strikeout) are
/// extracted from the font's OS/2 and post tables via swash.
///
/// # Panics
///
/// Panics if `bytes` does not contain a valid font at `face_index`.
/// Callers must validate before calling.
pub(super) fn compute_metrics(bytes: &[u8], face_index: u32, size_px: f32) -> FontMetrics {
    let fr = FontRef::from_index(bytes, face_index as usize).expect("pre-validated font");
    let metrics = fr.metrics(&[]).scale(size_px);
    let cell_height = (metrics.ascent + metrics.descent.abs()).ceil();
    let baseline = metrics.ascent.ceil();
    let gid = fr.charmap().map('M');
    let cell_width = fr
        .glyph_metrics(&[])
        .scale(size_px)
        .advance_width(gid)
        .ceil();

    // swash underline_offset is negative (below baseline), negate for our
    // convention where positive = pixels below baseline.
    let underline_offset = -metrics.underline_offset;
    let stroke_size = metrics.stroke_size;
    let strikeout_offset = metrics.strikeout_offset;

    FontMetrics {
        cell_width,
        cell_height,
        baseline,
        underline_offset,
        stroke_size,
        strikeout_offset,
    }
}

/// Compute the cap height in pixels for a font at the given pixel size.
///
/// Reads `capital_height` from the OS/2 table via `ttf-parser`. Falls back to
/// `0.75 * ascender` when the metric is missing.
pub(super) fn cap_height_px(bytes: &[u8], face_index: u32, size_px: f32) -> f32 {
    let Some(face) = rustybuzz::Face::from_slice(bytes, face_index) else {
        return 0.0;
    };
    let upem = face.units_per_em() as f32;
    if upem == 0.0 {
        return 0.0;
    }
    let cap_units = face
        .tables()
        .os2
        .and_then(|os2| {
            let h = os2.capital_height()?;
            Some(h as f32)
        })
        .unwrap_or_else(|| face.ascender() as f32 * 0.75);
    cap_units / upem * size_px
}

/// Convert a font size in pixels to 26.6 fixed-point for use as a cache key.
pub fn size_key(size: f32) -> u32 {
    (size * 64.0).round() as u32
}
