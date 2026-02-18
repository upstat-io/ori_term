//! Internal face data and rasterization helpers.
//!
//! [`FaceData`] stores validated font bytes with swash metadata for transient
//! [`FontRef`](swash::FontRef) creation. Free functions handle validation,
//! glyph lookup, rasterization, and metrics computation.

use std::sync::Arc;

use swash::scale::{image::Content, Render, ScaleContext, Source, StrikeWith};
use swash::zeno::Format;
use swash::{CacheKey, FontRef};

use super::RasterizedGlyph;
use crate::font::GlyphFormat;

/// Validated font data with swash metadata.
///
/// Raw bytes are kept in [`Arc<Vec<u8>>`] for shared ownership with rustybuzz
/// faces (Section 6). The `offset` and `cache_key` enable fast transient
/// [`FontRef`] construction without re-parsing.
pub(crate) struct FaceData {
    /// Raw font file bytes.
    pub(crate) bytes: Arc<Vec<u8>>,
    /// Index within a `.ttc` collection file (0 for standalone `.ttf`).
    #[allow(dead_code, reason = "font face helpers consumed in later sections")]
    pub(crate) face_index: u32,
    /// Byte offset to the font table directory.
    offset: u32,
    /// Unique cache key for [`ScaleContext`] reuse.
    cache_key: CacheKey,
}

/// Validate font bytes and extract swash metadata.
///
/// Returns `(byte_offset, cache_key)` on success, `None` for invalid data.
pub(crate) fn validate_font(data: &[u8], face_index: u32) -> Option<(u32, CacheKey)> {
    let fr = FontRef::from_index(data, face_index as usize)?;
    Some((fr.offset, fr.key))
}

/// Build a [`FaceData`] from an [`Arc<Vec<u8>>`] and face index.
///
/// Returns `None` if the font bytes are invalid.
pub(crate) fn build_face(bytes: Arc<Vec<u8>>, face_index: u32) -> Option<FaceData> {
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
pub(crate) fn font_ref(fd: &FaceData) -> FontRef<'_> {
    FontRef {
        data: &fd.bytes,
        offset: fd.offset,
        key: fd.cache_key,
    }
}

/// Check whether a face covers a given character.
#[allow(dead_code, reason = "font face helpers consumed in later sections")]
pub(crate) fn has_glyph(fd: &FaceData, ch: char) -> bool {
    font_ref(fd).charmap().map(ch) != 0
}

/// Map a character to its glyph ID in the given face.
///
/// Returns 0 (.notdef) if the character is not covered.
pub(crate) fn glyph_id(fd: &FaceData, ch: char) -> u16 {
    font_ref(fd).charmap().map(ch)
}

/// Rasterize a glyph from face data into a bitmap.
///
/// Tries color sources first (COLR outlines, CBDT/sbix bitmaps), then falls
/// back to the configured outline format. Returns `None` for empty glyphs.
///
/// When a color source produces output, the returned [`RasterizedGlyph`] has
/// `format: GlyphFormat::Color` with RGBA premultiplied data regardless of
/// the requested `format`. Callers must route color glyphs to a separate
/// RGBA atlas.
pub(crate) fn rasterize_from_face(
    fd: &FaceData,
    glyph_id: u16,
    size_px: f32,
    wght: Option<f32>,
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
    let image = Render::new(&[
        Source::ColorOutline(0),
        Source::ColorBitmap(StrikeWith::BestFit),
        Source::Outline,
    ])
    .format(zeno_fmt)
    .render(&mut scaler, glyph_id)?;

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

/// Compute cell metrics from font bytes at the given pixel size.
///
/// Returns `(cell_width, cell_height, baseline)` as f32 values.
/// Cell height = `ceil(ascent + |descent|)`, cell width = `ceil(advance of 'M')`,
/// baseline = `ceil(ascent)`.
///
/// # Panics
///
/// Panics if `bytes` does not contain a valid font at `face_index`.
/// Callers must validate before calling.
pub(crate) fn compute_metrics(bytes: &[u8], face_index: u32, size_px: f32) -> (f32, f32, f32) {
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
    (cell_width, cell_height, baseline)
}

/// Compute the cap height in pixels for a font at the given pixel size.
///
/// Reads `capital_height` from the OS/2 table via `ttf-parser`. Falls back to
/// `0.75 * ascender` when the metric is missing.
pub(crate) fn cap_height_px(bytes: &[u8], face_index: u32, size_px: f32) -> f32 {
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
