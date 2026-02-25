//! Tests for TOML theme file parsing.

use std::path::Path;

use super::parse_theme_toml;

const VALID_TOML: &str = r##"
name = "Test Theme"
ansi = [
    "#282c34", "#e06c75", "#98c379", "#e5c07b",
    "#61afef", "#c678dd", "#56b6c2", "#abb2bf",
    "#545862", "#e06c75", "#98c379", "#e5c07b",
    "#61afef", "#c678dd", "#56b6c2", "#bec5d4",
]
foreground = "#abb2bf"
background = "#282c34"
cursor = "#528bff"
"##;

#[test]
fn parse_valid_theme() {
    let path = Path::new("test.toml");
    let scheme = parse_theme_toml(VALID_TOML, path).expect("should parse");
    assert_eq!(scheme.name, "Test Theme");
    assert_eq!(scheme.fg.r, 0xab);
    assert_eq!(scheme.bg.r, 0x28);
    assert_eq!(scheme.cursor.b, 0xff);
    assert_eq!(scheme.ansi[1].r, 0xe0); // Red
    assert_eq!(scheme.selection_fg, None);
    assert_eq!(scheme.selection_bg, None);
}

#[test]
fn parse_theme_with_selection_colors() {
    let toml = format!(
        "{VALID_TOML}\nselection_foreground = \"#112233\"\nselection_background = \"#aabbcc\"\n"
    );
    let path = Path::new("test.toml");
    let scheme = parse_theme_toml(&toml, path).expect("should parse");
    assert_eq!(
        scheme.selection_fg,
        Some(oriterm_core::Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33
        })
    );
    assert_eq!(
        scheme.selection_bg,
        Some(oriterm_core::Rgb {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc
        })
    );
}

#[test]
fn parse_theme_name_fallback_to_filename() {
    let toml = r##"
ansi = [
    "#000000", "#000000", "#000000", "#000000",
    "#000000", "#000000", "#000000", "#000000",
    "#000000", "#000000", "#000000", "#000000",
    "#000000", "#000000", "#000000", "#000000",
]
foreground = "#ffffff"
background = "#000000"
cursor = "#ffffff"
"##;
    let path = Path::new("my-cool-theme.toml");
    let scheme = parse_theme_toml(toml, path).expect("should parse");
    assert_eq!(scheme.name, "my-cool-theme");
}

#[test]
fn malformed_hex_rejected() {
    let toml = r##"
ansi = [
    "#ZZZZZZ", "#000000", "#000000", "#000000",
    "#000000", "#000000", "#000000", "#000000",
    "#000000", "#000000", "#000000", "#000000",
    "#000000", "#000000", "#000000", "#000000",
]
foreground = "#ffffff"
background = "#000000"
cursor = "#ffffff"
"##;
    let path = Path::new("bad.toml");
    assert!(parse_theme_toml(toml, path).is_none());
}

#[test]
fn invalid_toml_returns_none() {
    let path = Path::new("broken.toml");
    assert!(parse_theme_toml("not valid toml {{{{", path).is_none());
}

#[test]
fn missing_required_field_returns_none() {
    let toml = r##"
ansi = [
    "#000000", "#000000", "#000000", "#000000",
    "#000000", "#000000", "#000000", "#000000",
    "#000000", "#000000", "#000000", "#000000",
    "#000000", "#000000", "#000000", "#000000",
]
foreground = "#ffffff"
background = "#000000"
"##;
    let path = Path::new("missing-cursor.toml");
    assert!(parse_theme_toml(toml, path).is_none());
}
