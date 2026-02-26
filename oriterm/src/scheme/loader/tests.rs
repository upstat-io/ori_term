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

/// Wrong ANSI array length (15 instead of 16) — serde rejects fixed-size array mismatch.
#[test]
fn wrong_ansi_array_length_rejected() {
    let toml = r##"
ansi = [
    "#000000", "#000000", "#000000", "#000000",
    "#000000", "#000000", "#000000", "#000000",
    "#000000", "#000000", "#000000", "#000000",
    "#000000", "#000000", "#000000",
]
foreground = "#ffffff"
background = "#000000"
cursor = "#ffffff"
"##;
    let path = Path::new("short-ansi.toml");
    assert!(parse_theme_toml(toml, path).is_none());
}

/// Extra ANSI colors (17) — serde silently takes the first 16 for `[String; 16]`,
/// so parsing succeeds but only 16 are used.
#[test]
fn extra_ansi_array_length_takes_first_16() {
    let toml = r##"
ansi = [
    "#000000", "#111111", "#222222", "#333333",
    "#444444", "#555555", "#666666", "#777777",
    "#888888", "#999999", "#AAAAAA", "#BBBBBB",
    "#CCCCCC", "#DDDDDD", "#EEEEEE", "#FFFFFF",
    "#FF0000",
]
foreground = "#ffffff"
background = "#000000"
cursor = "#ffffff"
"##;
    let path = Path::new("long-ansi.toml");
    let scheme = parse_theme_toml(toml, path).expect("serde accepts 17 as [String;16]");
    // 17th entry is silently dropped.
    assert_eq!(
        scheme.ansi[15],
        oriterm_core::Rgb {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF
        }
    );
}

/// Unknown fields in TOML are silently ignored (forward compatibility).
#[test]
fn unknown_fields_ignored() {
    let toml = format!("{VALID_TOML}\nfuture_field = true\nanother = 42\n");
    let path = Path::new("future.toml");
    let scheme = parse_theme_toml(&toml, path).expect("should parse with unknown fields");
    assert_eq!(scheme.name, "Test Theme");
}

// --- discover_themes ---

/// `discover_themes` finds valid `.toml` files in a directory.
#[test]
fn discover_themes_finds_valid_files() {
    let dir = std::env::temp_dir().join("oriterm_test_discover");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create dir");

    // Write two valid themes.
    std::fs::write(dir.join("alpha.toml"), VALID_TOML).expect("write alpha");
    std::fs::write(
        dir.join("beta.toml"),
        VALID_TOML.replace("Test Theme", "Beta Theme"),
    )
    .expect("write beta");

    // Write an invalid file and a non-TOML file (should be skipped).
    std::fs::write(dir.join("bad.toml"), "not valid toml {{{{").expect("write bad");
    std::fs::write(dir.join("readme.txt"), "not a theme").expect("write txt");

    let themes = super::discover_themes(&dir);
    assert_eq!(themes.len(), 2, "should find 2 valid themes");

    let names: Vec<&str> = themes.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"Test Theme") || names.contains(&"Beta Theme"));

    let _ = std::fs::remove_dir_all(&dir);
}

/// `discover_themes` returns empty vec for nonexistent directory.
#[test]
fn discover_themes_nonexistent_dir() {
    let themes = super::discover_themes(Path::new("/tmp/__nonexistent_themes_dir_oriterm_test__"));
    assert!(themes.is_empty());
}
