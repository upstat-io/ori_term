//! Tests for font types defined in `font/mod.rs`.

use super::{GlyphFormat, HintingMode, SubpixelMode, subpx_bin, subpx_offset};

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

#[test]
fn subpixel_none_forces_alpha_regardless_of_scale() {
    // Config "none" overrides scale factor — always produces Alpha (grayscale).
    assert_eq!(
        SubpixelMode::None.glyph_format(),
        GlyphFormat::Alpha,
        "None at any scale → Alpha",
    );
    // Even though 1x scale would normally give RGB, explicit None wins.
    assert_ne!(
        SubpixelMode::None.glyph_format(),
        SubpixelMode::from_scale_factor(1.0).glyph_format(),
        "explicit None differs from auto-detected 1x",
    );
}

#[test]
fn subpixel_rgb_and_bgr_are_distinct() {
    let rgb = SubpixelMode::Rgb.glyph_format();
    let bgr = SubpixelMode::Bgr.glyph_format();

    // Both are subpixel formats.
    assert!(rgb.is_subpixel());
    assert!(bgr.is_subpixel());

    // But they are not equal — channel order differs.
    assert_ne!(
        rgb, bgr,
        "RGB and BGR produce different GlyphFormat variants"
    );
}

// ── SubpixelMode::for_display (transparent background fallback) ──

#[test]
fn subpixel_for_display_opaque_uses_scale_factor() {
    // Fully opaque background delegates to scale factor logic.
    assert_eq!(
        SubpixelMode::for_display(1.0, 1.0),
        SubpixelMode::Rgb,
        "opaque + 1x → RGB",
    );
    assert_eq!(
        SubpixelMode::for_display(2.0, 1.0),
        SubpixelMode::None,
        "opaque + 2x → None (HiDPI)",
    );
}

#[test]
fn subpixel_for_display_transparent_forces_none() {
    // Transparent background disables subpixel regardless of scale factor.
    assert_eq!(
        SubpixelMode::for_display(1.0, 0.9),
        SubpixelMode::None,
        "transparent + 1x → None (fringing prevention)",
    );
    assert_eq!(
        SubpixelMode::for_display(1.0, 0.5),
        SubpixelMode::None,
        "half-transparent + 1x → None",
    );
    assert_eq!(
        SubpixelMode::for_display(1.0, 0.0),
        SubpixelMode::None,
        "fully transparent + 1x → None",
    );
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

// ── SubpixelMode::for_display edge cases ──

#[test]
fn subpixel_for_display_opacity_boundary() {
    // opacity 0.999 is below 1.0 → None (transparent bg disables subpixel).
    assert_eq!(
        SubpixelMode::for_display(1.0, 0.999),
        SubpixelMode::None,
        "opacity < 1.0 → None",
    );
    // opacity exactly 1.0 → delegates to scale factor.
    assert_eq!(
        SubpixelMode::for_display(1.0, 1.0),
        SubpixelMode::Rgb,
        "opacity 1.0 + 1x scale → Rgb",
    );
}

#[test]
fn subpixel_mode_edge_cases() {
    // Very low scale → subpixel enabled.
    assert_eq!(
        SubpixelMode::from_scale_factor(0.5),
        SubpixelMode::Rgb,
        "0.5x scale → Rgb",
    );
    // Very high scale → subpixel disabled.
    assert_eq!(
        SubpixelMode::from_scale_factor(10.0),
        SubpixelMode::None,
        "10x scale → None",
    );
}

#[test]
fn subpixel_for_display_high_dpi_opaque() {
    // High DPI with opaque bg → None (HiDPI wins even when opaque).
    assert_eq!(
        SubpixelMode::for_display(3.0, 1.0),
        SubpixelMode::None,
        "3x scale + opaque → None (HiDPI)",
    );
}

// ── SubpixelMode threshold boundary ──

#[test]
fn subpixel_mode_threshold_boundary() {
    // Just below threshold → Rgb.
    assert_eq!(
        SubpixelMode::from_scale_factor(1.99),
        SubpixelMode::Rgb,
        "1.99x scale → Rgb (just below 2.0 threshold)",
    );
    // Exactly at threshold → None.
    assert_eq!(
        SubpixelMode::from_scale_factor(2.0),
        SubpixelMode::None,
        "2.0x scale → None (at threshold)",
    );
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

#[test]
fn hinting_mode_threshold_boundary() {
    // Just below threshold → Full.
    assert_eq!(
        HintingMode::from_scale_factor(1.99),
        HintingMode::Full,
        "1.99x scale → Full (just below 2.0 threshold)",
    );
    // Exactly at threshold → None.
    assert_eq!(
        HintingMode::from_scale_factor(2.0),
        HintingMode::None,
        "2.0x scale → None (at threshold)",
    );
}

#[test]
fn hinting_mode_edge_cases() {
    // Very low scale → Full.
    assert_eq!(
        HintingMode::from_scale_factor(0.5),
        HintingMode::Full,
        "0.5x scale → Full",
    );
    // Very high scale → None.
    assert_eq!(
        HintingMode::from_scale_factor(10.0),
        HintingMode::None,
        "10x scale → None",
    );
}

// ── subpx_bin ──

#[test]
fn subpx_bin_exact_centers() {
    assert_eq!(subpx_bin(0.0), 0, "0.00 → phase 0");
    assert_eq!(subpx_bin(0.25), 1, "0.25 → phase 1");
    assert_eq!(subpx_bin(0.50), 2, "0.50 → phase 2");
    assert_eq!(subpx_bin(0.75), 3, "0.75 → phase 3");
}

#[test]
fn subpx_bin_boundaries() {
    // Just below 0.125 boundary → phase 0.
    assert_eq!(subpx_bin(0.124), 0, "0.124 → phase 0");
    // At 0.125 boundary → phase 1.
    assert_eq!(subpx_bin(0.125), 1, "0.125 → phase 1");
    // Just below 0.375 boundary → phase 1.
    assert_eq!(subpx_bin(0.374), 1, "0.374 → phase 1");
    // At 0.375 boundary → phase 2.
    assert_eq!(subpx_bin(0.375), 2, "0.375 → phase 2");
    // Just below 0.625 boundary → phase 2.
    assert_eq!(subpx_bin(0.624), 2, "0.624 → phase 2");
    // At 0.625 boundary → phase 3.
    assert_eq!(subpx_bin(0.625), 3, "0.625 → phase 3");
    // Just below 0.875 boundary → phase 3.
    assert_eq!(subpx_bin(0.874), 3, "0.874 → phase 3");
    // At 0.875 → wraps to phase 0 (next integer).
    assert_eq!(subpx_bin(0.875), 0, "0.875 → phase 0 (wrap)");
}

#[test]
fn subpx_bin_integer_values() {
    assert_eq!(subpx_bin(1.0), 0, "1.0 → phase 0");
    assert_eq!(subpx_bin(5.0), 0, "5.0 → phase 0");
}

#[test]
fn subpx_bin_large_fractional_values() {
    assert_eq!(subpx_bin(3.37), subpx_bin(0.37), "3.37 matches 0.37");
    assert_eq!(subpx_bin(10.62), subpx_bin(0.62), "10.62 matches 0.62");
}

// ── subpx_offset ──

#[test]
fn subpx_offset_values() {
    assert_eq!(subpx_offset(0), 0.0);
    assert_eq!(subpx_offset(1), 0.25);
    assert_eq!(subpx_offset(2), 0.50);
    assert_eq!(subpx_offset(3), 0.75);
}

#[test]
fn subpx_offset_out_of_range_defaults_to_zero() {
    assert_eq!(subpx_offset(4), 0.0);
    assert_eq!(subpx_offset(255), 0.0);
}

#[test]
fn subpx_round_trip() {
    for phase in 0..4u8 {
        assert_eq!(
            subpx_bin(subpx_offset(phase)),
            phase,
            "round-trip for phase {phase}",
        );
    }
}
