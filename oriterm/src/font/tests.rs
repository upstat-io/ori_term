//! Tests for font types defined in `font/mod.rs`.

use super::{GlyphFormat, HintingMode, SubpixelMode};

// ── SubpixelMode ──

#[test]
fn subpixel_mode_default_is_rgb() {
    assert_eq!(SubpixelMode::default(), SubpixelMode::Rgb);
}

#[test]
fn subpixel_mode_from_scale_factor_low_dpi() {
    assert_eq!(
        SubpixelMode::from_scale_factor(1.0),
        SubpixelMode::Rgb,
        "1x scale → RGB subpixel",
    );
    assert_eq!(
        SubpixelMode::from_scale_factor(1.5),
        SubpixelMode::Rgb,
        "1.5x scale → RGB subpixel",
    );
}

#[test]
fn subpixel_mode_from_scale_factor_high_dpi() {
    assert_eq!(
        SubpixelMode::from_scale_factor(2.0),
        SubpixelMode::None,
        "2x scale → disabled",
    );
    assert_eq!(
        SubpixelMode::from_scale_factor(3.0),
        SubpixelMode::None,
        "3x scale → disabled",
    );
}

#[test]
fn subpixel_mode_glyph_format() {
    assert_eq!(SubpixelMode::Rgb.glyph_format(), GlyphFormat::SubpixelRgb);
    assert_eq!(SubpixelMode::Bgr.glyph_format(), GlyphFormat::SubpixelBgr);
    assert_eq!(SubpixelMode::None.glyph_format(), GlyphFormat::Alpha);
}

// ── GlyphFormat ──

#[test]
fn glyph_format_bytes_per_pixel() {
    assert_eq!(GlyphFormat::Alpha.bytes_per_pixel(), 1);
    assert_eq!(GlyphFormat::SubpixelRgb.bytes_per_pixel(), 4);
    assert_eq!(GlyphFormat::SubpixelBgr.bytes_per_pixel(), 4);
    assert_eq!(GlyphFormat::Color.bytes_per_pixel(), 4);
}

#[test]
fn glyph_format_is_subpixel() {
    assert!(GlyphFormat::SubpixelRgb.is_subpixel());
    assert!(GlyphFormat::SubpixelBgr.is_subpixel());
    assert!(!GlyphFormat::Alpha.is_subpixel());
    assert!(!GlyphFormat::Color.is_subpixel());
}

// ── HintingMode ──

#[test]
fn hinting_mode_default_is_full() {
    assert_eq!(HintingMode::default(), HintingMode::Full);
}

#[test]
fn hinting_mode_hint_flag() {
    assert!(HintingMode::Full.hint_flag());
    assert!(!HintingMode::None.hint_flag());
}
