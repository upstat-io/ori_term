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
    assert_eq!(
        white,
        Rgb {
            r: 0xd3,
            g: 0xd7,
            b: 0xcf
        }
    );
}

#[test]
fn default_color_15_is_bright_white() {
    let p = Palette::default();
    let bright_white = p.resolve(Color::Indexed(15));
    assert_eq!(
        bright_white,
        Rgb {
            r: 0xee,
            g: 0xee,
            b: 0xec
        }
    );
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
    assert_eq!(
        c,
        Rgb {
            r: 255,
            g: 255,
            b: 255
        }
    );
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
    assert_eq!(
        c,
        Rgb {
            r: 135,
            g: 175,
            b: 215
        }
    );
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
    assert_eq!(
        c,
        Rgb {
            r: 238,
            g: 238,
            b: 238
        }
    );
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
    assert_eq!(
        red,
        Rgb {
            r: 0xcc,
            g: 0x00,
            b: 0x00
        }
    );
}

#[test]
fn resolve_spec() {
    let p = Palette::default();
    let rgb = Rgb {
        r: 42,
        g: 128,
        b: 255,
    };
    let resolved = p.resolve(Color::Spec(rgb));
    assert_eq!(resolved, rgb);
}

#[test]
fn resolve_indexed() {
    let p = Palette::default();
    let c = p.resolve(Color::Indexed(1));
    assert_eq!(
        c,
        Rgb {
            r: 0xcc,
            g: 0x00,
            b: 0x00
        }
    );
}

#[test]
fn set_indexed_and_resolve() {
    let mut p = Palette::default();
    let new_color = Rgb {
        r: 0xff,
        g: 0x00,
        b: 0xff,
    };
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
    assert_eq!(
        p.foreground(),
        p.resolve(Color::Named(NamedColor::Foreground))
    );
}

#[test]
fn background_returns_named_background() {
    let p = Palette::default();
    assert_eq!(
        p.background(),
        p.resolve(Color::Named(NamedColor::Background))
    );
}

#[test]
fn cursor_color_returns_named_cursor() {
    let p = Palette::default();
    assert_eq!(
        p.cursor_color(),
        p.resolve(Color::Named(NamedColor::Cursor))
    );
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
    assert_eq!(
        p.background(),
        Rgb {
            r: 0x00,
            g: 0x00,
            b: 0x00
        }
    );
}

#[test]
fn dark_theme_has_light_foreground() {
    let p = Palette::for_theme(Theme::Dark);
    assert_eq!(
        p.foreground(),
        Rgb {
            r: 0xcc,
            g: 0xcc,
            b: 0xcc
        }
    );
}

#[test]
fn light_theme_has_white_background() {
    let p = Palette::for_theme(Theme::Light);
    assert_eq!(
        p.background(),
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff
        }
    );
}

#[test]
fn light_theme_has_dark_foreground() {
    let p = Palette::for_theme(Theme::Light);
    assert_eq!(
        p.foreground(),
        Rgb {
            r: 0x2e,
            g: 0x34,
            b: 0x36
        }
    );
}

#[test]
fn light_theme_cursor_is_black() {
    let p = Palette::for_theme(Theme::Light);
    assert_eq!(
        p.cursor_color(),
        Rgb {
            r: 0x00,
            g: 0x00,
            b: 0x00
        }
    );
}

#[test]
fn dark_theme_cursor_is_white() {
    let p = Palette::for_theme(Theme::Dark);
    assert_eq!(
        p.cursor_color(),
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff
        }
    );
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

// --- Selection color tests ---

#[test]
fn selection_defaults_to_none() {
    let p = Palette::default();
    assert_eq!(p.selection_fg(), None);
    assert_eq!(p.selection_bg(), None);
}

#[test]
fn selection_accessors_roundtrip() {
    let mut p = Palette::default();
    let fg = Rgb {
        r: 0x11,
        g: 0x22,
        b: 0x33,
    };
    let bg = Rgb {
        r: 0xaa,
        g: 0xbb,
        b: 0xcc,
    };
    p.set_selection_fg(Some(fg));
    p.set_selection_bg(Some(bg));
    assert_eq!(p.selection_fg(), Some(fg));
    assert_eq!(p.selection_bg(), Some(bg));
}

#[test]
fn selection_can_be_cleared() {
    let mut p = Palette::default();
    p.set_selection_fg(Some(Rgb { r: 1, g: 2, b: 3 }));
    p.set_selection_fg(None);
    assert_eq!(p.selection_fg(), None);
}

#[test]
fn selection_colors_returns_pair() {
    let mut p = Palette::default();
    let fg = Rgb {
        r: 0x10,
        g: 0x20,
        b: 0x30,
    };
    p.set_selection_fg(Some(fg));
    let sel = p.selection_colors();
    assert_eq!(sel.fg, Some(fg));
    assert_eq!(sel.bg, None);
}

// --- from_scheme_colors tests ---

#[test]
fn from_scheme_colors_sets_ansi() {
    let mut ansi = [Rgb { r: 0, g: 0, b: 0 }; 16];
    ansi[1] = Rgb {
        r: 0xf3,
        g: 0x8b,
        b: 0xa8,
    };
    ansi[15] = Rgb {
        r: 0xa6,
        g: 0xad,
        b: 0xc8,
    };
    let fg = Rgb {
        r: 0xcd,
        g: 0xd6,
        b: 0xf4,
    };
    let bg = Rgb {
        r: 0x1e,
        g: 0x1e,
        b: 0x2e,
    };
    let cursor = Rgb {
        r: 0xf5,
        g: 0xe0,
        b: 0xdc,
    };
    let p = Palette::from_scheme_colors(&ansi, fg, bg, cursor);
    assert_eq!(p.resolve(Color::Indexed(1)), ansi[1]);
    assert_eq!(p.resolve(Color::Indexed(15)), ansi[15]);
}

#[test]
fn from_scheme_colors_preserves_cube_and_grayscale() {
    let ansi = [Rgb {
        r: 0x42,
        g: 0x42,
        b: 0x42,
    }; 16];
    let p = Palette::from_scheme_colors(
        &ansi,
        Rgb { r: 0, g: 0, b: 0 },
        Rgb { r: 0, g: 0, b: 0 },
        Rgb { r: 0, g: 0, b: 0 },
    );
    // Cube index 231 = (5,5,5) = white.
    assert_eq!(
        p.resolve(Color::Indexed(231)),
        Rgb {
            r: 255,
            g: 255,
            b: 255
        }
    );
    // Grayscale index 232 = 8.
    assert_eq!(p.resolve(Color::Indexed(232)), Rgb { r: 8, g: 8, b: 8 });
}

#[test]
fn from_scheme_colors_sets_semantic_slots() {
    let ansi = [Rgb { r: 0, g: 0, b: 0 }; 16];
    let fg = Rgb {
        r: 0xab,
        g: 0xcd,
        b: 0xef,
    };
    let bg = Rgb {
        r: 0x12,
        g: 0x34,
        b: 0x56,
    };
    let cursor = Rgb {
        r: 0xfe,
        g: 0xdc,
        b: 0xba,
    };
    let p = Palette::from_scheme_colors(&ansi, fg, bg, cursor);
    assert_eq!(p.foreground(), fg);
    assert_eq!(p.background(), bg);
    assert_eq!(p.cursor_color(), cursor);
}

#[test]
fn from_scheme_colors_derives_dim_variants() {
    let mut ansi = [Rgb { r: 0, g: 0, b: 0 }; 16];
    ansi[1] = Rgb {
        r: 0xff,
        g: 0x00,
        b: 0x00,
    };
    let p = Palette::from_scheme_colors(
        &ansi,
        Rgb { r: 0, g: 0, b: 0 },
        Rgb { r: 0, g: 0, b: 0 },
        Rgb { r: 0, g: 0, b: 0 },
    );
    let dim_red = p.resolve(Color::Named(NamedColor::DimRed));
    // dim = 2/3 of 0xff = 170
    assert_eq!(dim_red.r, 170);
    assert_eq!(dim_red.g, 0);
    assert_eq!(dim_red.b, 0);
}

#[test]
fn from_scheme_colors_selection_defaults_none() {
    let ansi = [Rgb { r: 0, g: 0, b: 0 }; 16];
    let p = Palette::from_scheme_colors(
        &ansi,
        Rgb { r: 0, g: 0, b: 0 },
        Rgb { r: 0, g: 0, b: 0 },
        Rgb { r: 0, g: 0, b: 0 },
    );
    assert_eq!(p.selection_fg(), None);
    assert_eq!(p.selection_bg(), None);
}
