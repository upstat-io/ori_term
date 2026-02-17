//! Unit tests for the font collection module.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::face::{build_face, cap_height_px, compute_metrics, font_ref, has_glyph, validate_font};
use super::{FontCollection, FontData, FontSet};
use crate::font::discovery::EMBEDDED_FONT_DATA;
use crate::font::{GlyphFormat, GlyphStyle, RasterKey, SyntheticFlags};

/// Helper: build a FontCollection from system discovery with default settings.
fn system_collection(format: GlyphFormat) -> FontCollection {
    let font_set = FontSet::load(None, 400).expect("font must load");
    FontCollection::new(font_set, 12.0, 96.0, format, 400).expect("collection must build")
}

/// Helper: build a FontCollection from ONLY the embedded Regular font.
///
/// Guarantees no Bold/Italic/BoldItalic variants and no fallbacks, so
/// style substitution tests behave deterministically.
fn embedded_only_collection(format: GlyphFormat) -> FontCollection {
    let font_set = FontSet {
        family_name: "JetBrains Mono (embedded)".to_owned(),
        regular: FontData {
            data: Arc::new(EMBEDDED_FONT_DATA.to_vec()),
            index: 0,
        },
        bold: None,
        italic: None,
        bold_italic: None,
        has_variant: [true, false, false, false],
        fallbacks: Vec::new(),
    };
    FontCollection::new(font_set, 12.0, 96.0, format, 400).expect("collection must build")
}

// ── Face helpers ──

#[test]
fn validate_font_accepts_embedded() {
    let result = validate_font(EMBEDDED_FONT_DATA, 0);
    assert!(result.is_some(), "embedded JetBrains Mono must validate");
}

#[test]
fn validate_font_rejects_garbage() {
    let garbage = b"this is not a font file at all";
    assert!(
        validate_font(garbage, 0).is_none(),
        "garbage bytes must fail validation"
    );
}

#[test]
fn validate_font_rejects_empty() {
    assert!(
        validate_font(&[], 0).is_none(),
        "empty bytes must fail validation"
    );
}

#[test]
fn font_ref_produces_working_charmap() {
    let fd = build_face(Arc::new(EMBEDDED_FONT_DATA.to_vec()), 0)
        .expect("embedded font must build");
    let fr = font_ref(&fd);
    let gid = fr.charmap().map('A');
    assert_ne!(gid, 0, "'A' must have a non-zero glyph ID");
}

#[test]
fn has_glyph_true_for_ascii() {
    let fd = build_face(Arc::new(EMBEDDED_FONT_DATA.to_vec()), 0)
        .expect("embedded font must build");
    assert!(has_glyph(&fd, 'A'), "embedded font must cover 'A'");
    assert!(has_glyph(&fd, 'z'), "embedded font must cover 'z'");
    assert!(has_glyph(&fd, '0'), "embedded font must cover '0'");
    assert!(has_glyph(&fd, ' '), "embedded font must cover space");
}

#[test]
fn has_glyph_notdef_graceful() {
    let fd = build_face(Arc::new(EMBEDDED_FONT_DATA.to_vec()), 0)
        .expect("embedded font must build");
    // CJK character unlikely in JetBrains Mono — just checking it doesn't panic.
    let _ = has_glyph(&fd, '\u{4E00}');
}

// ── FontSet ──

#[test]
fn font_set_load_default_succeeds() {
    let result = FontSet::load(None, 400);
    assert!(result.is_ok(), "FontSet::load(None, 400) must succeed");
}

// ── FontCollection construction ──

#[test]
fn collection_new_produces_positive_metrics() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    assert!(fc.cell_width > 0.0, "cell_width must be positive");
    assert!(fc.cell_height > 0.0, "cell_height must be positive");
    assert!(fc.baseline > 0.0, "baseline must be positive");
}

#[test]
fn cell_metrics_valid() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let cm = fc.cell_metrics();
    assert!(cm.width > 0.0, "CellMetrics width must be positive");
    assert!(cm.height > 0.0, "CellMetrics height must be positive");
    assert!(cm.baseline > 0.0, "CellMetrics baseline must be positive");
    assert!(
        cm.baseline <= cm.height,
        "baseline must not exceed cell height"
    );
}

#[test]
fn size_px_matches_computation() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let expected = 12.0 * 96.0 / 72.0;
    assert!(
        (fc.size_px() - expected).abs() < f32::EPSILON,
        "size_px should be size_pt * dpi / 72"
    );
}

// ── Resolve ──

#[test]
fn resolve_ascii_regular() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::Regular);
    assert_eq!(resolved.face_idx, 0, "'A' should resolve to primary Regular");
    assert_ne!(resolved.glyph_id, 0, "'A' must have a non-zero glyph ID");
    assert_eq!(
        resolved.synthetic,
        SyntheticFlags::NONE,
        "Regular should need no synthesis"
    );
}

#[test]
fn resolve_bold_without_bold_face_is_synthetic() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::Bold);
    assert_eq!(
        resolved.face_idx, 0,
        "should fall back to Regular face"
    );
    assert_ne!(resolved.glyph_id, 0);
    assert!(
        resolved.synthetic.contains(SyntheticFlags::BOLD),
        "should flag synthetic bold"
    );
}

#[test]
fn resolve_italic_without_italic_face_is_synthetic() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::Italic);
    assert_eq!(resolved.face_idx, 0);
    assert!(
        resolved.synthetic.contains(SyntheticFlags::ITALIC),
        "should flag synthetic italic"
    );
}

#[test]
fn resolve_bold_italic_without_variants_is_synthetic() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::BoldItalic);
    assert_eq!(resolved.face_idx, 0);
    assert!(
        resolved
            .synthetic
            .contains(SyntheticFlags::BOLD | SyntheticFlags::ITALIC),
        "should flag both synthetic bold and italic"
    );
}

#[test]
fn resolve_bold_with_system_fonts() {
    // System discovery may find real Bold variants — verify non-zero glyph.
    let fc = system_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::Bold);
    assert_ne!(resolved.glyph_id, 0, "'A' Bold should resolve to something");
}

#[test]
fn find_face_for_char_matches_resolve() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let r1 = fc.resolve('X', GlyphStyle::Regular);
    let r2 = fc.find_face_for_char('X', GlyphStyle::Regular);
    assert_eq!(r1.glyph_id, r2.glyph_id);
    assert_eq!(r1.face_idx, r2.face_idx);
    assert_eq!(r1.synthetic, r2.synthetic);
}

// ── Rasterization ──

#[test]
fn rasterize_alpha_produces_bitmap() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::Regular);
    let key = RasterKey {
        glyph_id: resolved.glyph_id,
        face_idx: resolved.face_idx,
        size_q6: super::size_key(fc.size_px()),
    };
    let glyph = fc.rasterize(key).expect("'A' must rasterize");
    assert!(glyph.width > 0, "bitmap width must be positive");
    assert!(glyph.height > 0, "bitmap height must be positive");
    assert!(!glyph.bitmap.is_empty(), "bitmap data must not be empty");
    assert_eq!(glyph.format, GlyphFormat::Alpha);
}

#[test]
fn rasterize_alpha_bitmap_size() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('H', GlyphStyle::Regular);
    let key = RasterKey {
        glyph_id: resolved.glyph_id,
        face_idx: resolved.face_idx,
        size_q6: super::size_key(fc.size_px()),
    };
    let glyph = fc.rasterize(key).expect("'H' must rasterize");
    let expected_len = (glyph.width * glyph.height) as usize;
    assert_eq!(
        glyph.bitmap.len(),
        expected_len,
        "Alpha bitmap should be width * height bytes"
    );
}

#[test]
fn rasterize_subpixel_rgb_bitmap_size() {
    let mut fc = embedded_only_collection(GlyphFormat::SubpixelRgb);
    let resolved = fc.resolve('H', GlyphStyle::Regular);
    let key = RasterKey {
        glyph_id: resolved.glyph_id,
        face_idx: resolved.face_idx,
        size_q6: super::size_key(fc.size_px()),
    };
    let glyph = fc.rasterize(key).expect("'H' must rasterize");
    let expected_len = (glyph.width * glyph.height * 4) as usize;
    assert_eq!(
        glyph.bitmap.len(),
        expected_len,
        "SubpixelRgb bitmap should be width * height * 4 bytes"
    );
}

#[test]
fn rasterize_bitmap_has_nonzero_pixels() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('H', GlyphStyle::Regular);
    let key = RasterKey {
        glyph_id: resolved.glyph_id,
        face_idx: resolved.face_idx,
        size_q6: super::size_key(fc.size_px()),
    };
    let glyph = fc.rasterize(key).expect("'H' must rasterize");
    assert!(
        glyph.bitmap.iter().any(|&b| b > 0),
        "bitmap should have non-zero pixels"
    );
}

#[test]
fn rasterize_cache_hit() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('B', GlyphStyle::Regular);
    let key = RasterKey {
        glyph_id: resolved.glyph_id,
        face_idx: resolved.face_idx,
        size_q6: super::size_key(fc.size_px()),
    };

    let first = fc.rasterize(key).expect("first rasterize");
    let first_bitmap = first.bitmap.clone();
    let first_width = first.width;

    let second = fc.rasterize(key).expect("cache hit");
    assert_eq!(second.width, first_width, "cache hit should return same data");
    assert_eq!(
        second.bitmap, first_bitmap,
        "cache hit should return same bitmap"
    );
}

#[test]
fn rasterize_space_has_no_outline() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve(' ', GlyphStyle::Regular);
    let key = RasterKey {
        glyph_id: resolved.glyph_id,
        face_idx: resolved.face_idx,
        size_q6: super::size_key(fc.size_px()),
    };
    // Space typically has no outline, so rasterize returns None. But some
    // fonts may define an outline; either result is acceptable.
    let result = fc.rasterize(key);
    if let Some(g) = result {
        // If it does rasterize, it should be valid (non-negative dimensions).
        assert!(g.width == 0 || g.bitmap.len() > 0);
    }
}

// ── RasterKey hashing/equality ──

#[test]
fn raster_key_equality() {
    let k1 = RasterKey {
        glyph_id: 42,
        face_idx: 0,
        size_q6: 1024,
    };
    let k2 = RasterKey {
        glyph_id: 42,
        face_idx: 0,
        size_q6: 1024,
    };
    let k3 = RasterKey {
        glyph_id: 43,
        face_idx: 0,
        size_q6: 1024,
    };
    assert_eq!(k1, k2);
    assert_ne!(k1, k3);
}

#[test]
fn raster_key_hash_consistency() {
    let k1 = RasterKey {
        glyph_id: 42,
        face_idx: 0,
        size_q6: 1024,
    };
    let k2 = RasterKey {
        glyph_id: 42,
        face_idx: 0,
        size_q6: 1024,
    };
    let h1 = {
        let mut h = DefaultHasher::new();
        k1.hash(&mut h);
        h.finish()
    };
    let h2 = {
        let mut h = DefaultHasher::new();
        k2.hash(&mut h);
        h.finish()
    };
    assert_eq!(h1, h2, "equal keys must produce equal hashes");
}

// ── size_key ──

#[test]
fn size_key_16_is_1024() {
    assert_eq!(super::size_key(16.0), 1024, "16.0 * 64 = 1024");
}

#[test]
fn size_key_fractional() {
    assert_eq!(super::size_key(12.5), 800, "12.5 * 64 = 800");
}

// ── cap_height_px ──

#[test]
fn cap_height_px_positive() {
    let cap = cap_height_px(EMBEDDED_FONT_DATA, 0, 16.0);
    assert!(cap > 0.0, "cap height should be positive for embedded font");
}

// ── compute_metrics ──

#[test]
fn compute_metrics_stable() {
    let (w1, h1, b1) = compute_metrics(EMBEDDED_FONT_DATA, 0, 16.0);
    let (w2, h2, b2) = compute_metrics(EMBEDDED_FONT_DATA, 0, 16.0);
    assert!(
        (w1 - w2).abs() < f32::EPSILON
            && (h1 - h2).abs() < f32::EPSILON
            && (b1 - b2).abs() < f32::EPSILON,
        "metrics should be deterministic"
    );
}

#[test]
fn compute_metrics_positive() {
    let (w, h, b) = compute_metrics(EMBEDDED_FONT_DATA, 0, 16.0);
    assert!(w > 0.0, "cell width must be positive");
    assert!(h > 0.0, "cell height must be positive");
    assert!(b > 0.0, "baseline must be positive");
    assert!(b <= h, "baseline must not exceed cell height");
}

// ── Pre-cache ──

#[test]
fn pre_cache_populates_ascii() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    // Space has no outline, so cache count may be less than full 95 printable chars.
    // Most ASCII glyphs do have outlines.
    assert!(
        fc.cache_len() >= 90,
        "pre-cache should populate most ASCII glyphs (got {})",
        fc.cache_len()
    );
}

// ── Bearing sanity ──

#[test]
fn rasterize_uppercase_has_positive_top_bearing() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('H', GlyphStyle::Regular);
    let key = RasterKey {
        glyph_id: resolved.glyph_id,
        face_idx: resolved.face_idx,
        size_q6: super::size_key(fc.size_px()),
    };
    let glyph = fc.rasterize(key).expect("'H' must rasterize");
    assert!(
        glyph.bearing_y > 0,
        "top bearing should be positive for uppercase (got {})",
        glyph.bearing_y
    );
}

#[test]
fn rasterize_advance_positive() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('M', GlyphStyle::Regular);
    let key = RasterKey {
        glyph_id: resolved.glyph_id,
        face_idx: resolved.face_idx,
        size_q6: super::size_key(fc.size_px()),
    };
    let glyph = fc.rasterize(key).expect("'M' must rasterize");
    assert!(
        glyph.advance > 0.0,
        "advance width should be positive for 'M'"
    );
}

// ── Format propagation ──

#[test]
fn rasterized_glyph_carries_format_tag() {
    let mut fc = embedded_only_collection(GlyphFormat::SubpixelRgb);
    let resolved = fc.resolve('A', GlyphStyle::Regular);
    let key = RasterKey {
        glyph_id: resolved.glyph_id,
        face_idx: resolved.face_idx,
        size_q6: super::size_key(fc.size_px()),
    };
    let glyph = fc.rasterize(key).expect("'A' must rasterize");
    assert_eq!(
        glyph.format,
        GlyphFormat::SubpixelRgb,
        "rasterized glyph should carry the requested format"
    );
}

// ── Family name ──

#[test]
fn family_name_not_empty() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    assert!(
        !fc.family_name().is_empty(),
        "family name should not be empty"
    );
}
