//! Tests for the color palette.

use vte::ansi::{Color, NamedColor};

use super::{Palette, Rgb};
use crate::theme::Theme;

#[test]
fn default_color_0_is_black() {
    let p = Palette::default();
    let black = p.resolve(Color::Indexed(0));
    assert_eq!(black, Rgb { r: 0, g: 0, b: 0 });
}

#[test]
fn default_color_7_is_white() {
    let p = Palette::default();
    let white = p.resolve(Color::Indexed(7));
    assert_eq!(white, Rgb { r: 0xd3, g: 0xd7, b: 0xcf });
}

#[test]
fn default_color_15_is_bright_white() {
    let p = Palette::default();
    let bright_white = p.resolve(Color::Indexed(15));
    assert_eq!(bright_white, Rgb { r: 0xee, g: 0xee, b: 0xec });
}

#[test]
fn cube_color_index_16_is_black() {
    let p = Palette::default();
    // Cube (0,0,0) = index 16.
    let c = p.resolve(Color::Indexed(16));
    assert_eq!(c, Rgb { r: 0, g: 0, b: 0 });
}

#[test]
fn cube_color_index_231_is_white() {
    let p = Palette::default();
    // Cube (5,5,5) = index 231.
    let c = p.resolve(Color::Indexed(231));
    assert_eq!(c, Rgb { r: 255, g: 255, b: 255 });
}

#[test]
fn cube_color_index_196_is_pure_red() {
    let p = Palette::default();
    // Cube (5,0,0) = index 16 + 5*36 = 196.
    let c = p.resolve(Color::Indexed(196));
    assert_eq!(c, Rgb { r: 255, g: 0, b: 0 });
}

#[test]
fn cube_formula_correct() {
    let p = Palette::default();
    // Cube (2,3,4) = index 16 + 2*36 + 3*6 + 4 = 110.
    let c = p.resolve(Color::Indexed(110));
    assert_eq!(c, Rgb { r: 135, g: 175, b: 215 });
}

#[test]
fn grayscale_index_232() {
    let p = Palette::default();
    let c = p.resolve(Color::Indexed(232));
    assert_eq!(c, Rgb { r: 8, g: 8, b: 8 });
}

#[test]
fn grayscale_index_255() {
    let p = Palette::default();
    let c = p.resolve(Color::Indexed(255));
    assert_eq!(c, Rgb { r: 238, g: 238, b: 238 });
}

#[test]
fn grayscale_ramp_correct() {
    let p = Palette::default();
    for i in 0..24u8 {
        let expected = 8 + i * 10;
        let c = p.resolve(Color::Indexed(232 + i));
        assert_eq!(c.r, expected, "grayscale index {} r", 232 + i);
        assert_eq!(c.g, expected, "grayscale index {} g", 232 + i);
        assert_eq!(c.b, expected, "grayscale index {} b", 232 + i);
    }
}

#[test]
fn resolve_named() {
    let p = Palette::default();
    let red = p.resolve(Color::Named(NamedColor::Red));
    assert_eq!(red, Rgb { r: 0xcc, g: 0x00, b: 0x00 });
}

#[test]
fn resolve_spec() {
    let p = Palette::default();
    let rgb = Rgb { r: 42, g: 128, b: 255 };
    let resolved = p.resolve(Color::Spec(rgb));
    assert_eq!(resolved, rgb);
}

#[test]
fn resolve_indexed() {
    let p = Palette::default();
    let c = p.resolve(Color::Indexed(1));
    assert_eq!(c, Rgb { r: 0xcc, g: 0x00, b: 0x00 });
}

#[test]
fn set_indexed_and_resolve() {
    let mut p = Palette::default();
    let new_color = Rgb { r: 0xff, g: 0x00, b: 0xff };
    p.set_indexed(1, new_color);
    assert_eq!(p.resolve(Color::Indexed(1)), new_color);
}

#[test]
fn reset_indexed_restores_default() {
    let mut p = Palette::default();
    let original = p.resolve(Color::Indexed(1));
    p.set_indexed(1, Rgb { r: 0, g: 0, b: 0 });
    assert_ne!(p.resolve(Color::Indexed(1)), original);
    p.reset_indexed(1);
    assert_eq!(p.resolve(Color::Indexed(1)), original);
}

#[test]
fn foreground_returns_named_foreground() {
    let p = Palette::default();
    assert_eq!(p.foreground(), p.resolve(Color::Named(NamedColor::Foreground)));
}

#[test]
fn background_returns_named_background() {
    let p = Palette::default();
    assert_eq!(p.background(), p.resolve(Color::Named(NamedColor::Background)));
}

#[test]
fn cursor_color_returns_named_cursor() {
    let p = Palette::default();
    assert_eq!(p.cursor_color(), p.resolve(Color::Named(NamedColor::Cursor)));
}

#[test]
fn set_indexed_out_of_bounds_is_noop() {
    let mut p = Palette::default();
    let before = p.foreground();
    p.set_indexed(300, Rgb { r: 255, g: 0, b: 0 });
    assert_eq!(p.foreground(), before);
}

#[test]
fn dim_variants_are_darker() {
    let p = Palette::default();
    let normal = p.resolve(Color::Named(NamedColor::Red));
    let dim = p.resolve(Color::Named(NamedColor::DimRed));
    assert!(dim.r <= normal.r, "dim red should be <= normal red");
    assert!(dim.g <= normal.g);
    assert!(dim.b <= normal.b);
}

// --- Theme-aware palette tests ---

#[test]
fn dark_theme_has_dark_background() {
    let p = Palette::for_theme(Theme::Dark);
    assert_eq!(p.background(), Rgb { r: 0x00, g: 0x00, b: 0x00 });
}

#[test]
fn dark_theme_has_light_foreground() {
    let p = Palette::for_theme(Theme::Dark);
    assert_eq!(p.foreground(), Rgb { r: 0xd3, g: 0xd7, b: 0xcf });
}

#[test]
fn light_theme_has_white_background() {
    let p = Palette::for_theme(Theme::Light);
    assert_eq!(p.background(), Rgb { r: 0xff, g: 0xff, b: 0xff });
}

#[test]
fn light_theme_has_dark_foreground() {
    let p = Palette::for_theme(Theme::Light);
    assert_eq!(p.foreground(), Rgb { r: 0x2e, g: 0x34, b: 0x36 });
}

#[test]
fn light_theme_cursor_is_black() {
    let p = Palette::for_theme(Theme::Light);
    assert_eq!(p.cursor_color(), Rgb { r: 0x00, g: 0x00, b: 0x00 });
}

#[test]
fn dark_theme_cursor_is_white() {
    let p = Palette::for_theme(Theme::Dark);
    assert_eq!(p.cursor_color(), Rgb { r: 0xff, g: 0xff, b: 0xff });
}

#[test]
fn unknown_theme_uses_dark_defaults() {
    let unknown = Palette::for_theme(Theme::Unknown);
    let dark = Palette::for_theme(Theme::Dark);
    assert_eq!(unknown.foreground(), dark.foreground());
    assert_eq!(unknown.background(), dark.background());
    assert_eq!(unknown.cursor_color(), dark.cursor_color());
}

#[test]
fn indexed_colors_same_across_themes() {
    let dark = Palette::for_theme(Theme::Dark);
    let light = Palette::for_theme(Theme::Light);
    // ANSI colors 0–15 are theme-independent.
    for i in 0..16 {
        assert_eq!(
            dark.resolve(Color::Indexed(i)),
            light.resolve(Color::Indexed(i)),
            "ANSI color {i} differs between themes",
        );
    }
    // Cube and grayscale are also theme-independent.
    for i in 16..=255 {
        assert_eq!(
            dark.resolve(Color::Indexed(i)),
            light.resolve(Color::Indexed(i)),
            "indexed color {i} differs between themes",
        );
    }
}

#[test]
fn default_palette_equals_dark_theme() {
    let default = Palette::default();
    let dark = Palette::for_theme(Theme::Dark);
    assert_eq!(default.foreground(), dark.foreground());
    assert_eq!(default.background(), dark.background());
    assert_eq!(default.cursor_color(), dark.cursor_color());
}
