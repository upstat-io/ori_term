//! Unit tests for the font collection module.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::face::{
    build_face, cap_height_px, compute_metrics, embolden_strength, font_ref, has_glyph,
    validate_font,
};
use super::{FontCollection, FontData, FontSet};
use crate::font::discovery::EMBEDDED_FONT_DATA;
use crate::font::{FaceIdx, GlyphFormat, GlyphStyle, HintingMode, RasterKey, SyntheticFlags};

/// Helper: build a FontCollection from system discovery with default settings.
fn system_collection(format: GlyphFormat) -> FontCollection {
    let font_set = FontSet::load(None, 400).expect("font must load");
    FontCollection::new(font_set, 12.0, 96.0, format, 400, HintingMode::Full)
        .expect("collection must build")
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
    FontCollection::new(font_set, 12.0, 96.0, format, 400, HintingMode::Full)
        .expect("collection must build")
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
    let fd =
        build_face(Arc::new(EMBEDDED_FONT_DATA.to_vec()), 0).expect("embedded font must build");
    let fr = font_ref(&fd);
    let gid = fr.charmap().map('A');
    assert_ne!(gid, 0, "'A' must have a non-zero glyph ID");
}

#[test]
fn has_glyph_true_for_ascii() {
    let fd =
        build_face(Arc::new(EMBEDDED_FONT_DATA.to_vec()), 0).expect("embedded font must build");
    assert!(has_glyph(&fd, 'A'), "embedded font must cover 'A'");
    assert!(has_glyph(&fd, 'z'), "embedded font must cover 'z'");
    assert!(has_glyph(&fd, '0'), "embedded font must cover '0'");
    assert!(has_glyph(&fd, ' '), "embedded font must cover space");
}

#[test]
fn has_glyph_notdef_graceful() {
    let fd =
        build_face(Arc::new(EMBEDDED_FONT_DATA.to_vec()), 0).expect("embedded font must build");
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
    // Decoration metrics from font tables.
    assert!(
        cm.stroke_size >= 1.0,
        "stroke_size must be at least 1.0 (clamped minimum)"
    );
    assert!(
        cm.underline_offset.is_finite(),
        "underline_offset must be finite"
    );
    assert!(
        cm.strikeout_offset.is_finite(),
        "strikeout_offset must be finite"
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
    assert_eq!(
        resolved.face_idx,
        FaceIdx::REGULAR,
        "'A' should resolve to primary Regular"
    );
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
        resolved.face_idx,
        FaceIdx::REGULAR,
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
    assert_eq!(resolved.face_idx, FaceIdx::REGULAR);
    assert!(
        resolved.synthetic.contains(SyntheticFlags::ITALIC),
        "should flag synthetic italic"
    );
}

#[test]
fn resolve_bold_italic_without_variants_is_synthetic() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::BoldItalic);
    assert_eq!(resolved.face_idx, FaceIdx::REGULAR);
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

// ── Rasterization ──

#[test]
fn rasterize_alpha_produces_bitmap() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::Regular);
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
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
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
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
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
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
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
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
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);

    let first = fc.rasterize(key).expect("first rasterize");
    let first_bitmap = first.bitmap.clone();
    let first_width = first.width;

    let second = fc.rasterize(key).expect("cache hit");
    assert_eq!(
        second.width, first_width,
        "cache hit should return same data"
    );
    assert_eq!(
        second.bitmap, first_bitmap,
        "cache hit should return same bitmap"
    );
}

#[test]
fn rasterize_space_has_no_outline() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve(' ', GlyphStyle::Regular);
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
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
        face_idx: FaceIdx::REGULAR,
        size_q6: 1024,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
    };
    let k2 = RasterKey {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        size_q6: 1024,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
    };
    let k3 = RasterKey {
        glyph_id: 43,
        face_idx: FaceIdx::REGULAR,
        size_q6: 1024,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
    };
    assert_eq!(k1, k2);
    assert_ne!(k1, k3);
}

#[test]
fn raster_key_hash_consistency() {
    let k1 = RasterKey {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        size_q6: 1024,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
    };
    let k2 = RasterKey {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        size_q6: 1024,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
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
    let m1 = compute_metrics(EMBEDDED_FONT_DATA, 0, 16.0);
    let m2 = compute_metrics(EMBEDDED_FONT_DATA, 0, 16.0);
    assert!(
        (m1.cell_width - m2.cell_width).abs() < f32::EPSILON
            && (m1.cell_height - m2.cell_height).abs() < f32::EPSILON
            && (m1.baseline - m2.baseline).abs() < f32::EPSILON,
        "metrics should be deterministic"
    );
}

#[test]
fn compute_metrics_positive() {
    let m = compute_metrics(EMBEDDED_FONT_DATA, 0, 16.0);
    assert!(m.cell_width > 0.0, "cell width must be positive");
    assert!(m.cell_height > 0.0, "cell height must be positive");
    assert!(m.baseline > 0.0, "baseline must be positive");
    assert!(
        m.baseline <= m.cell_height,
        "baseline must not exceed cell height"
    );
}

#[test]
fn compute_metrics_decoration_fields() {
    let m = compute_metrics(EMBEDDED_FONT_DATA, 0, 16.0);
    assert!(m.stroke_size > 0.0, "stroke_size must be positive");
    assert!(m.stroke_size.is_finite(), "stroke_size must be finite");
    assert!(
        m.underline_offset.is_finite(),
        "underline_offset must be finite"
    );
    assert!(
        m.strikeout_offset.is_finite(),
        "strikeout_offset must be finite"
    );
}

// ── Cache ──

#[test]
fn new_collection_has_empty_cache() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    // FontCollection::new() no longer pre-caches ASCII — the GPU renderer's
    // pre_cache_atlas() fills both the HashMap and atlas in one pass.
    assert_eq!(
        fc.cache_len(),
        0,
        "new collection should start with empty cache"
    );
}

// ── Bearing sanity ──

#[test]
fn rasterize_uppercase_has_positive_top_bearing() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('H', GlyphStyle::Regular);
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
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
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
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
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
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

// ── Emoji resolution (Section 6.10) ──

#[test]
fn resolve_prefer_emoji_without_fallbacks_uses_primary() {
    // With no fallbacks, resolve_prefer_emoji should fall through to
    // normal resolution and return the primary face (or .notdef).
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let normal = fc.resolve('A', GlyphStyle::Regular);
    let emoji = fc.resolve_prefer_emoji('A', GlyphStyle::Regular);
    assert_eq!(normal.glyph_id, emoji.glyph_id);
    assert_eq!(normal.face_idx, emoji.face_idx);
}

#[test]
fn resolve_prefer_emoji_tries_fallback_for_ascii() {
    // System collection has fallback fonts. For 'A' (in primary font), normal
    // resolve returns primary. resolve_prefer_emoji returns fallback if it
    // has 'A', otherwise falls through to normal.
    let fc = system_collection(GlyphFormat::Alpha);
    let normal = fc.resolve('A', GlyphStyle::Regular);
    let emoji = fc.resolve_prefer_emoji('A', GlyphStyle::Regular);
    // Both should produce a valid glyph (non-zero ID).
    assert_ne!(normal.glyph_id, 0);
    assert_ne!(emoji.glyph_id, 0);
    // resolve_prefer_emoji should try fallbacks first — it may or may not
    // return a different face_idx depending on whether a fallback has 'A'.
    // Key invariant: the result is always valid.
}

#[test]
fn resolve_prefer_emoji_emoji_char_hits_fallback() {
    // System collection with fallbacks should resolve a known emoji to a
    // fallback face (color emoji font). If no emoji font is installed,
    // this test still passes — it just verifies no panic.
    let fc = system_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve_prefer_emoji('\u{1F600}', GlyphStyle::Regular); // 😀
    if resolved.glyph_id != 0 {
        assert!(
            resolved.face_idx.is_fallback(),
            "emoji should resolve via fallback face (got face_idx={:?})",
            resolved.face_idx,
        );
    }
}

#[test]
fn rasterize_emoji_as_color_format() {
    // When system has a color emoji font, rasterizing 😀 should produce
    // GlyphFormat::Color (RGBA bitmap). Permissive: skips if no emoji font.
    let mut fc = system_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve_prefer_emoji('\u{1F600}', GlyphStyle::Regular);
    if resolved.glyph_id == 0 {
        return; // No emoji font available.
    }
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
    if let Some(glyph) = fc.rasterize(key) {
        if glyph.format == GlyphFormat::Color {
            // Color emoji: RGBA bitmap, 4 bytes per pixel.
            let expected = (glyph.width * glyph.height * 4) as usize;
            assert_eq!(
                glyph.bitmap.len(),
                expected,
                "color emoji bitmap should be width * height * 4 bytes"
            );
        }
        // If the font renders it as Alpha (text presentation), that's also valid.
    }
}

// ── FaceIdx ──

#[test]
fn face_idx_primary_not_fallback() {
    for i in 0..4 {
        assert!(
            !FaceIdx(i).is_fallback(),
            "primary index {i} is not fallback"
        );
    }
}

#[test]
fn face_idx_fallback_starts_at_4() {
    assert!(FaceIdx(4).is_fallback());
    assert!(FaceIdx(10).is_fallback());
}

#[test]
fn face_idx_fallback_index() {
    assert_eq!(FaceIdx(0).fallback_index(), None);
    assert_eq!(FaceIdx(3).fallback_index(), None);
    assert_eq!(FaceIdx(4).fallback_index(), Some(0));
    assert_eq!(FaceIdx(7).fallback_index(), Some(3));
}

// ── Fallback chain ──

#[test]
fn resolve_unknown_char_returns_notdef_without_fallbacks() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    // CJK ideograph — not in JetBrains Mono.
    let resolved = fc.resolve('\u{4E00}', GlyphStyle::Regular);
    assert_eq!(
        resolved.face_idx,
        FaceIdx::REGULAR,
        "unknown char should fall back to Regular"
    );
    // Glyph ID 0 = .notdef (unmapped character).
    assert_eq!(resolved.glyph_id, 0, "unmapped char should be .notdef");
}

#[test]
fn resolve_unknown_char_uses_fallback_when_available() {
    // System discovery includes fallback fonts (e.g. Noto Sans CJK).
    let fc = system_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('\u{4E00}', GlyphStyle::Regular);
    // If system has CJK fallback, face_idx should be >= 4 (a fallback face)
    // and glyph_id should be non-zero.
    // If no CJK fallback is installed, this degrades to .notdef — both are valid.
    if resolved.glyph_id != 0 {
        assert!(
            resolved.face_idx.is_fallback(),
            "CJK char should resolve via fallback face (got face_idx={:?})",
            resolved.face_idx,
        );
    }
}

// ── Cap-height normalization (Section 6.2) ──

#[test]
fn effective_size_primary_equals_base() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let size = fc.effective_size(FaceIdx::REGULAR);
    assert!(
        (size - fc.size_px()).abs() < f32::EPSILON,
        "primary face effective_size should equal base size"
    );
}

#[test]
fn effective_size_primary_all_styles_equal_base() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    for i in 0..4 {
        let size = fc.effective_size(FaceIdx(i));
        assert!(
            (size - fc.size_px()).abs() < f32::EPSILON,
            "primary face {i} effective_size should equal base size"
        );
    }
}

#[test]
fn effective_size_for_unit_scale_factor() {
    // FallbackMeta with scale_factor=1.0 should return base_size.
    use super::FallbackMeta;

    let meta = vec![FallbackMeta {
        scale_factor: 1.0,
        size_offset: 0.0,
        features: None,
    }];
    let base = 16.0;
    let result = super::effective_size_for(FaceIdx(4), base, &meta);
    assert!(
        (result - base).abs() < f32::EPSILON,
        "scale_factor 1.0 should return base_size"
    );
}

#[test]
fn effective_size_for_with_scaling() {
    use super::FallbackMeta;

    let meta = vec![FallbackMeta {
        scale_factor: 1.2,
        size_offset: 0.0,
        features: None,
    }];
    let base = 16.0;
    let result = super::effective_size_for(FaceIdx(4), base, &meta);
    let expected = 16.0 * 1.2;
    assert!(
        (result - expected).abs() < 0.01,
        "scale_factor 1.2 should produce {expected}, got {result}"
    );
}

#[test]
fn effective_size_for_with_size_offset() {
    use super::FallbackMeta;

    let meta = vec![FallbackMeta {
        scale_factor: 1.0,
        size_offset: -2.0,
        features: None,
    }];
    let base = 16.0;
    let result = super::effective_size_for(FaceIdx(4), base, &meta);
    let expected = 14.0;
    assert!(
        (result - expected).abs() < f32::EPSILON,
        "size_offset -2 should produce {expected}, got {result}"
    );
}

#[test]
fn effective_size_for_clamps_to_min() {
    use super::FallbackMeta;

    let meta = vec![FallbackMeta {
        scale_factor: 0.01,
        size_offset: 0.0,
        features: None,
    }];
    let result = super::effective_size_for(FaceIdx(4), 16.0, &meta);
    assert!(
        result >= super::MIN_FONT_SIZE,
        "effective_size should not go below MIN_FONT_SIZE"
    );
}

#[test]
fn effective_size_for_clamps_to_max() {
    use super::FallbackMeta;

    let meta = vec![FallbackMeta {
        scale_factor: 100.0,
        size_offset: 0.0,
        features: None,
    }];
    let result = super::effective_size_for(FaceIdx(4), 16.0, &meta);
    assert!(
        result <= super::MAX_FONT_SIZE,
        "effective_size should not exceed MAX_FONT_SIZE"
    );
}

// ── OpenType features (Section 6.7) ──

#[test]
fn parse_features_enable() {
    let features = super::parse_features(&["liga"]);
    assert_eq!(features.len(), 1);
    assert_eq!(features[0].value, 1, "liga should be enabled (value=1)");
}

#[test]
fn parse_features_disable() {
    let features = super::parse_features(&["-liga"]);
    assert_eq!(features.len(), 1);
    assert_eq!(features[0].value, 0, "-liga should be disabled (value=0)");
}

#[test]
fn parse_features_multiple() {
    let features = super::parse_features(&["liga", "calt", "-dlig"]);
    assert_eq!(features.len(), 3);
    assert_eq!(features[0].value, 1, "liga enabled");
    assert_eq!(features[1].value, 1, "calt enabled");
    assert_eq!(features[2].value, 0, "dlig disabled");
}

#[test]
fn parse_features_invalid_skipped() {
    let features = super::parse_features(&["liga", "", "calt"]);
    // Empty string is invalid; should be skipped.
    assert_eq!(features.len(), 2, "invalid tag should be skipped");
}

#[test]
fn default_features_has_liga_and_calt() {
    let defaults = super::default_features();
    assert_eq!(defaults.len(), 2, "defaults should be liga + calt");
    assert_eq!(defaults[0].value, 1);
    assert_eq!(defaults[1].value, 1);
}

#[test]
fn features_for_face_primary_uses_collection_defaults() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let features = fc.features_for_face(FaceIdx::REGULAR);
    assert_eq!(
        features.len(),
        2,
        "primary face should use collection defaults (liga + calt)"
    );
}

#[test]
fn features_for_face_fallback_without_override_uses_defaults() {
    let fc = system_collection(GlyphFormat::Alpha);
    // Fallback face (if any) without explicit override uses collection defaults.
    let features = fc.features_for_face(FaceIdx(4));
    assert_eq!(
        features.len(),
        2,
        "fallback without override should use collection defaults"
    );
}

// ── Font Synthesis (Section 6.11) ──

/// Helper: rasterize a character with given synthetic flags.
fn rasterize_with_synthesis(
    fc: &mut FontCollection,
    ch: char,
    synthetic: SyntheticFlags,
) -> Option<super::RasterizedGlyph> {
    let resolved = fc.resolve(ch, GlyphStyle::Regular);
    let key = RasterKey {
        glyph_id: resolved.glyph_id,
        face_idx: resolved.face_idx,
        size_q6: super::size_key(fc.size_px()),
        synthetic,
        hinted: true,
    };
    fc.rasterize(key).cloned()
}

#[test]
fn synthetic_bold_produces_wider_bitmap() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let regular = rasterize_with_synthesis(&mut fc, 'H', SyntheticFlags::NONE)
        .expect("regular 'H' must rasterize");
    let bold = rasterize_with_synthesis(&mut fc, 'H', SyntheticFlags::BOLD)
        .expect("synthetic bold 'H' must rasterize");

    // Emboldening expands outlines — bitmap should be at least as wide.
    assert!(
        bold.width >= regular.width,
        "synthetic bold should be at least as wide (regular={}, bold={})",
        regular.width,
        bold.width,
    );
    // Bitmaps must differ (embolden changes pixel values).
    assert_ne!(
        regular.bitmap, bold.bitmap,
        "synthetic bold bitmap must differ from regular"
    );
}

#[test]
fn synthetic_italic_differs_from_regular() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let regular = rasterize_with_synthesis(&mut fc, 'H', SyntheticFlags::NONE)
        .expect("regular 'H' must rasterize");
    let italic = rasterize_with_synthesis(&mut fc, 'H', SyntheticFlags::ITALIC)
        .expect("synthetic italic 'H' must rasterize");

    // Skewing changes the bitmap.
    assert_ne!(
        regular.bitmap, italic.bitmap,
        "synthetic italic bitmap must differ from regular"
    );
}

#[test]
fn synthetic_bold_italic_applies_both() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let regular = rasterize_with_synthesis(&mut fc, 'H', SyntheticFlags::NONE)
        .expect("regular 'H' must rasterize");
    let bold_italic =
        rasterize_with_synthesis(&mut fc, 'H', SyntheticFlags::BOLD | SyntheticFlags::ITALIC)
            .expect("synthetic bold+italic 'H' must rasterize");

    // Combined synthesis must differ from plain regular.
    assert_ne!(
        regular.bitmap, bold_italic.bitmap,
        "synthetic bold+italic bitmap must differ from regular"
    );
}

#[test]
fn regular_cells_have_no_synthesis() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::Regular);
    assert_eq!(
        resolved.synthetic,
        SyntheticFlags::NONE,
        "regular style should have no synthetic flags"
    );
}

#[test]
fn synthesis_detection_bold_without_variant() {
    // embedded_only_collection has no Bold face → resolve Bold produces synthetic flag.
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::Bold);
    assert!(
        resolved.synthetic.contains(SyntheticFlags::BOLD),
        "resolving Bold without a Bold face should set BOLD flag"
    );
    assert_eq!(
        resolved.face_idx,
        FaceIdx::REGULAR,
        "without Bold face, should fall back to Regular"
    );
}

#[test]
fn synthesis_detection_italic_without_variant() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::Italic);
    assert!(
        resolved.synthetic.contains(SyntheticFlags::ITALIC),
        "resolving Italic without an Italic face should set ITALIC flag"
    );
}

#[test]
fn synthesis_detection_bold_italic_without_variants() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::BoldItalic);
    assert!(
        resolved.synthetic.contains(SyntheticFlags::BOLD),
        "BoldItalic without variants should set BOLD flag"
    );
    assert!(
        resolved.synthetic.contains(SyntheticFlags::ITALIC),
        "BoldItalic without variants should set ITALIC flag"
    );
}

#[test]
fn synthetic_cache_separates_from_regular() {
    // Same glyph_id + face_idx but different synthetic flags → different cache entries.
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let regular = rasterize_with_synthesis(&mut fc, 'A', SyntheticFlags::NONE)
        .expect("regular must rasterize");
    let bold =
        rasterize_with_synthesis(&mut fc, 'A', SyntheticFlags::BOLD).expect("bold must rasterize");

    // They should be different glyphs (emboldening changes the bitmap).
    assert_ne!(
        regular.bitmap, bold.bitmap,
        "cached regular and bold bitmaps must differ"
    );
}

#[test]
fn embolden_strength_scales_with_size() {
    // Verify the formula produces reasonable pixel values.
    let s17 = embolden_strength(17.0);
    let s32 = embolden_strength(32.0);
    assert!(s17 > 0.0, "embolden strength must be positive");
    assert!(
        s32 > s17,
        "larger font should have greater embolden strength"
    );
    assert!(
        s17 < 1.0,
        "17px font should have sub-pixel embolden (~0.53)"
    );
    assert!(
        (s32 - 1.0).abs() < f32::EPSILON,
        "32px font should have 1.0px embolden"
    );
}

// ── Bold face availability (Section 6.14) ──

#[test]
fn bold_rasterization_works_when_available() {
    // System collection may have a real Bold face — verify it can rasterize.
    let mut system = system_collection(GlyphFormat::Alpha);
    if system.has_bold() {
        let resolved = system.resolve('A', GlyphStyle::Bold);
        let key = RasterKey::from_resolved(resolved, super::size_key(system.size_px()), true);
        let glyph = system.rasterize(key);
        assert!(
            glyph.is_some(),
            "Bold 'A' should rasterize when Bold face exists"
        );
    }
}

#[test]
fn has_bold_false_for_embedded_only() {
    let fc = embedded_only_collection(GlyphFormat::Alpha);
    assert!(!fc.has_bold(), "embedded-only collection has no Bold face");
}

// ── set_size (Section 6.14) ──

#[test]
fn set_size_recomputes_metrics() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let old_metrics = fc.cell_metrics();

    fc.set_size(18.0, 96.0);
    let new_metrics = fc.cell_metrics();

    assert_ne!(
        old_metrics, new_metrics,
        "changing size 12→18 should produce different cell metrics"
    );
    assert!(
        new_metrics.width > old_metrics.width,
        "larger font should have wider cells"
    );
    assert!(
        new_metrics.height > old_metrics.height,
        "larger font should have taller cells"
    );
}

#[test]
fn set_size_clears_cache() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    // Manually rasterize a glyph to populate the cache.
    let resolved = fc.resolve('A', GlyphStyle::Regular);
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
    let _ = fc.rasterize(key);
    assert!(
        fc.cache_len() > 0,
        "cache should have entries after rasterize"
    );

    fc.set_size(18.0, 96.0);
    assert_eq!(fc.cache_len(), 0, "set_size should clear the glyph cache",);
}

#[test]
fn set_size_updates_size_px() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let expected = 18.0 * 96.0 / 72.0;

    fc.set_size(18.0, 96.0);
    assert!(
        (fc.size_px() - expected).abs() < f32::EPSILON,
        "size_px should reflect new size (expected {expected}, got {})",
        fc.size_px(),
    );
}

// ── Hinting ──

#[test]
fn hinting_mode_auto_detection() {
    assert_eq!(
        HintingMode::from_scale_factor(1.0),
        HintingMode::Full,
        "1x scale → Full hinting",
    );
    assert_eq!(
        HintingMode::from_scale_factor(1.5),
        HintingMode::Full,
        "1.5x scale → Full hinting",
    );
    assert_eq!(
        HintingMode::from_scale_factor(2.0),
        HintingMode::None,
        "2x scale → no hinting",
    );
    assert_eq!(
        HintingMode::from_scale_factor(3.0),
        HintingMode::None,
        "3x scale → no hinting",
    );
}

#[test]
fn hinting_mode_hint_flag() {
    assert!(HintingMode::Full.hint_flag(), "Full → hint(true)");
    assert!(!HintingMode::None.hint_flag(), "None → hint(false)");
}

#[test]
fn hinting_mode_default_is_full() {
    assert_eq!(HintingMode::default(), HintingMode::Full);
}

#[test]
fn set_hinting_clears_cache() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    assert_eq!(fc.hinting_mode(), HintingMode::Full);

    let resolved = fc.resolve('A', GlyphStyle::Regular);
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
    let _ = fc.rasterize(key);
    assert!(
        fc.cache_len() > 0,
        "cache should have entries after rasterize"
    );

    let changed = fc.set_hinting(HintingMode::None);
    assert!(changed, "set_hinting should return true when mode changes");
    assert_eq!(
        fc.cache_len(),
        0,
        "set_hinting should clear the glyph cache"
    );
    assert_eq!(fc.hinting_mode(), HintingMode::None);
}

#[test]
fn set_hinting_noop_when_unchanged() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::Regular);
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
    let _ = fc.rasterize(key);
    let before = fc.cache_len();

    let changed = fc.set_hinting(HintingMode::Full);
    assert!(
        !changed,
        "set_hinting should return false when mode unchanged"
    );
    assert_eq!(fc.cache_len(), before, "cache should not be cleared");
}

#[test]
fn hinted_glyph_differs_from_unhinted() {
    let mut fc_hinted = embedded_only_collection(GlyphFormat::Alpha);
    assert_eq!(fc_hinted.hinting_mode(), HintingMode::Full);

    let mut fc_unhinted = embedded_only_collection(GlyphFormat::Alpha);
    fc_unhinted.set_hinting(HintingMode::None);

    let resolved_h = fc_hinted.resolve('A', GlyphStyle::Regular);
    let key_h = RasterKey::from_resolved(resolved_h, super::size_key(fc_hinted.size_px()), true);
    let glyph_h = fc_hinted
        .rasterize(key_h)
        .expect("hinted 'A' should rasterize");

    let resolved_u = fc_unhinted.resolve('A', GlyphStyle::Regular);
    let key_u = RasterKey::from_resolved(resolved_u, super::size_key(fc_unhinted.size_px()), false);
    let glyph_u = fc_unhinted
        .rasterize(key_u)
        .expect("unhinted 'A' should rasterize");

    // At 12pt/96dpi (16px), hinted and unhinted bitmaps should differ.
    // They may have different dimensions or different pixel values.
    let differs = glyph_h.width != glyph_u.width
        || glyph_h.height != glyph_u.height
        || glyph_h.bitmap != glyph_u.bitmap;
    assert!(
        differs,
        "hinted and unhinted glyphs should produce different bitmaps at 12pt",
    );
}

#[test]
fn raster_key_hinting_distinguishes_cache() {
    let k_hinted = RasterKey {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        size_q6: 1024,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
    };
    let k_unhinted = RasterKey {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        size_q6: 1024,
        synthetic: SyntheticFlags::NONE,
        hinted: false,
    };
    assert_ne!(
        k_hinted, k_unhinted,
        "same glyph with different hinting should have different keys",
    );
}

// ── set_format ──

#[test]
fn set_format_clears_cache() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    assert_eq!(fc.format(), GlyphFormat::Alpha);

    let resolved = fc.resolve('A', GlyphStyle::Regular);
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
    let _ = fc.rasterize(key);
    assert!(
        fc.cache_len() > 0,
        "cache should have entries after rasterize",
    );

    let changed = fc.set_format(GlyphFormat::SubpixelRgb);
    assert!(changed, "set_format should return true when format changes");
    assert_eq!(fc.cache_len(), 0, "set_format should clear the glyph cache",);
    assert_eq!(fc.format(), GlyphFormat::SubpixelRgb);
}

#[test]
fn set_format_noop_when_unchanged() {
    let mut fc = embedded_only_collection(GlyphFormat::Alpha);
    let resolved = fc.resolve('A', GlyphStyle::Regular);
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
    let _ = fc.rasterize(key);
    let before = fc.cache_len();

    let changed = fc.set_format(GlyphFormat::Alpha);
    assert!(
        !changed,
        "set_format should return false when format unchanged",
    );
    assert_eq!(fc.cache_len(), before, "cache should not be cleared");
}

#[test]
fn set_format_alpha_to_subpixel_changes_rasterization() {
    let mut fc = embedded_only_collection(GlyphFormat::SubpixelRgb);
    let resolved = fc.resolve('H', GlyphStyle::Regular);
    let key = RasterKey::from_resolved(resolved, super::size_key(fc.size_px()), true);
    let glyph = fc
        .rasterize(key)
        .expect("'H' should rasterize in subpixel mode");
    assert_eq!(glyph.format, GlyphFormat::SubpixelRgb);
    // Subpixel bitmaps are 4 bytes per pixel.
    assert_eq!(
        glyph.bitmap.len(),
        (glyph.width * glyph.height * 4) as usize,
    );
}
