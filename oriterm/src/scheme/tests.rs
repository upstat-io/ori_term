//! Tests for scheme resolution, conditional parsing, and palette integration.

use oriterm_core::{Rgb, Theme};

use super::{
    ColorScheme, builtin_names, find_builtin, palette_from_scheme, parse_conditional,
    resolve_scheme_name,
};

#[test]
fn find_builtin_case_insensitive() {
    let scheme = find_builtin("catppuccin mocha").expect("should find");
    assert_eq!(scheme.name, "Catppuccin Mocha");

    let scheme = find_builtin("CATPPUCCIN MOCHA").expect("should find uppercase");
    assert_eq!(scheme.name, "Catppuccin Mocha");
}

#[test]
fn find_builtin_exact_match() {
    let scheme = find_builtin("Nord").expect("should find");
    assert_eq!(scheme.name, "Nord");
}

#[test]
fn find_builtin_missing_returns_none() {
    assert!(find_builtin("Nonexistent Theme").is_none());
}

#[test]
fn builtin_names_not_empty() {
    let names = builtin_names();
    assert!(
        names.len() >= 50,
        "expected 50+ builtins, got {}",
        names.len()
    );
}

#[test]
fn builtin_names_unique() {
    let names = builtin_names();
    let mut seen = std::collections::HashSet::new();
    for name in &names {
        let lower = name.to_ascii_lowercase();
        assert!(seen.insert(lower.clone()), "duplicate scheme name: {name}");
    }
}

#[test]
fn builtin_schemes_have_valid_rgb() {
    // All 16 ANSI + fg/bg/cursor should be in 0x00-0xFF range (inherently true
    // for u8 fields, but verify the schemes loaded correctly by checking they
    // are not all zero — a common copy-paste error).
    for name in builtin_names() {
        let scheme = find_builtin(name).unwrap();
        // At minimum, fg or bg should be non-black for most schemes.
        let non_zero =
            scheme.fg != Rgb { r: 0, g: 0, b: 0 } || scheme.bg != Rgb { r: 0, g: 0, b: 0 };
        assert!(non_zero, "scheme {name} has all-zero fg and bg");
    }
}

#[test]
fn parse_conditional_dark_light() {
    let result = parse_conditional("dark:Tokyo Night, light:Tokyo Night Light");
    assert_eq!(result, Some(("Tokyo Night", "Tokyo Night Light")));
}

#[test]
fn parse_conditional_reversed_order() {
    let result = parse_conditional("light:Catppuccin Latte, dark:Catppuccin Mocha");
    assert_eq!(result, Some(("Catppuccin Mocha", "Catppuccin Latte")));
}

#[test]
fn parse_conditional_plain_name() {
    assert!(parse_conditional("Nord").is_none());
}

#[test]
fn parse_conditional_only_dark() {
    assert!(parse_conditional("dark:Tokyo Night").is_none());
}

#[test]
fn parse_conditional_extra_whitespace() {
    let result = parse_conditional("  dark: One Dark ,  light: One Light  ");
    assert_eq!(result, Some(("One Dark", "One Light")));
}

#[test]
fn resolve_scheme_name_plain() {
    let name = resolve_scheme_name("Nord", Theme::Dark);
    assert_eq!(name, "Nord");
}

#[test]
fn resolve_scheme_name_conditional_dark() {
    let name = resolve_scheme_name("dark:Tokyo Night, light:Tokyo Night Light", Theme::Dark);
    assert_eq!(name, "Tokyo Night");
}

#[test]
fn resolve_scheme_name_conditional_light() {
    let name = resolve_scheme_name("dark:Tokyo Night, light:Tokyo Night Light", Theme::Light);
    assert_eq!(name, "Tokyo Night Light");
}

// --- palette_from_scheme integration ---

/// Verify `palette_from_scheme` bridges scheme colors into the palette correctly.
#[test]
fn palette_from_scheme_roundtrip() {
    let scheme = find_builtin("Catppuccin Mocha").expect("builtin exists");
    let palette = palette_from_scheme(&scheme);

    assert_eq!(palette.foreground(), scheme.fg);
    assert_eq!(palette.background(), scheme.bg);
    assert_eq!(palette.cursor_color(), scheme.cursor);

    // ANSI colors from the scheme are preserved.
    for i in 0..16u8 {
        assert_eq!(
            palette.resolve(vte::ansi::Color::Indexed(i)),
            scheme.ansi[i as usize],
            "ANSI color {i} mismatch",
        );
    }

    // Cube and grayscale are unaffected by the scheme.
    assert_eq!(
        palette.resolve(vte::ansi::Color::Indexed(231)),
        Rgb {
            r: 255,
            g: 255,
            b: 255
        },
    );
}

/// Scheme with selection colors → palette propagation.
#[test]
fn palette_from_scheme_with_selection_colors() {
    let scheme = ColorScheme {
        name: "Test".into(),
        ansi: [Rgb { r: 0, g: 0, b: 0 }; 16],
        fg: Rgb {
            r: 0xcc,
            g: 0xcc,
            b: 0xcc,
        },
        bg: Rgb { r: 0, g: 0, b: 0 },
        cursor: Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff,
        },
        selection_fg: Some(Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33,
        }),
        selection_bg: Some(Rgb {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
        }),
    };
    let palette = palette_from_scheme(&scheme);

    assert_eq!(
        palette.selection_fg(),
        Some(Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33
        })
    );
    assert_eq!(
        palette.selection_bg(),
        Some(Rgb {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc
        })
    );
}

/// Builtins have no selection colors — verify None propagates.
#[test]
fn palette_from_builtin_has_no_selection() {
    let scheme = find_builtin("Nord").expect("builtin exists");
    let palette = palette_from_scheme(&scheme);
    assert_eq!(palette.selection_fg(), None);
    assert_eq!(palette.selection_bg(), None);
}

// --- resolve_scheme builtins-first semantics ---

/// `resolve_scheme` finds builtins without touching the filesystem.
#[test]
fn resolve_scheme_finds_builtin() {
    let scheme = super::resolve_scheme("Nord").expect("should resolve");
    assert_eq!(scheme.name, "Nord");
}

/// `resolve_scheme` returns None for nonexistent name (no file match either).
#[test]
fn resolve_scheme_returns_none_for_unknown() {
    assert!(super::resolve_scheme("__nonexistent_scheme_xyz__").is_none());
}

// --- conditional parsing edge cases ---

/// `resolve_scheme_name` with `Theme::Unknown` picks the dark variant.
#[test]
fn resolve_scheme_name_unknown_is_dark() {
    let name = resolve_scheme_name("dark:Tokyo Night, light:Tokyo Night Light", Theme::Unknown);
    assert_eq!(name, "Tokyo Night");
}

/// Duplicate dark keys: last one wins (current implementation overwrites).
#[test]
fn parse_conditional_duplicate_dark() {
    let result = parse_conditional("dark:First, dark:Second, light:Light");
    assert_eq!(result, Some(("Second", "Light")));
}

/// Empty scheme name returns None from `find_builtin`.
#[test]
fn find_builtin_empty_string() {
    assert!(find_builtin("").is_none());
}

// --- all builtins produce valid palettes ---

/// Every builtin scheme produces a palette without panicking and has
/// distinguishable fg/bg (no all-black copy-paste errors).
#[test]
fn all_builtins_produce_valid_palettes() {
    for name in builtin_names() {
        let scheme = find_builtin(name).unwrap();
        let palette = palette_from_scheme(&scheme);

        // At least fg or bg should be non-black.
        let fg = palette.foreground();
        let bg = palette.background();
        let non_zero = fg != (Rgb { r: 0, g: 0, b: 0 }) || bg != (Rgb { r: 0, g: 0, b: 0 });
        assert!(non_zero, "scheme {name}: palette fg and bg are both black");
    }
}
