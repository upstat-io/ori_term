//! TOML theme file loading.
//!
//! Loads user-defined color schemes from `<config_dir>/themes/<name>.toml`.
//! Each theme file specifies 16 ANSI colors, foreground, background, cursor,
//! and optional selection colors in `#RRGGBB` hex format.

use oriterm_core::Rgb;
use serde::Deserialize;

use super::ColorScheme;
use crate::config::parse_hex_color;

/// Serde-compatible TOML theme file structure.
#[derive(Debug, Deserialize)]
struct ThemeFile {
    /// Display name (falls back to filename if absent).
    name: Option<String>,
    /// 16 ANSI colors as `#RRGGBB` hex strings.
    ansi: [String; 16],
    /// Foreground color `#RRGGBB`.
    foreground: String,
    /// Background color `#RRGGBB`.
    background: String,
    /// Cursor color `#RRGGBB`.
    cursor: String,
    /// Selection foreground override `#RRGGBB`.
    selection_foreground: Option<String>,
    /// Selection background override `#RRGGBB`.
    selection_background: Option<String>,
}

/// Load a theme file from the user's themes directory.
///
/// Looks for `<config_dir>/themes/<name>.toml` (case-insensitive filename
/// match). Returns `None` if the file doesn't exist or fails to parse.
pub(crate) fn load_from_themes_dir(name: &str) -> Option<ColorScheme> {
    let config_dir = crate::config::config_path();
    let themes_dir = config_dir.parent()?.join("themes");
    if !themes_dir.is_dir() {
        return None;
    }
    load_from_dir(&themes_dir, name)
}

/// Search a directory for a theme file matching `name` (case-insensitive).
fn load_from_dir(dir: &std::path::Path, name: &str) -> Option<ColorScheme> {
    let entries = std::fs::read_dir(dir).ok()?;
    let target = name.to_ascii_lowercase();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if stem.to_ascii_lowercase() == target {
            return load_theme_file(&path);
        }
    }
    None
}

/// Parse a single TOML theme file into a [`ColorScheme`].
fn load_theme_file(path: &std::path::Path) -> Option<ColorScheme> {
    let contents = std::fs::read_to_string(path).ok()?;
    parse_theme_toml(&contents, path)
}

/// Parse TOML content into a [`ColorScheme`].
///
/// Separated from file I/O for testability.
pub(super) fn parse_theme_toml(toml_str: &str, source: &std::path::Path) -> Option<ColorScheme> {
    let theme: ThemeFile = match toml::from_str(toml_str) {
        Ok(t) => t,
        Err(e) => {
            log::warn!("theme: failed to parse {}: {e}", source.display());
            return None;
        }
    };

    let mut ansi = [Rgb { r: 0, g: 0, b: 0 }; 16];
    for (i, hex) in theme.ansi.iter().enumerate() {
        if let Some(rgb) = parse_hex_color(hex) {
            ansi[i] = rgb;
        } else {
            log::warn!(
                "theme: invalid ANSI color {i} in {}: {hex}",
                source.display()
            );
            return None;
        }
    }

    let fg = parse_hex_color(&theme.foreground)?;
    let bg = parse_hex_color(&theme.background)?;
    let cursor = parse_hex_color(&theme.cursor)?;

    let selection_fg = theme
        .selection_foreground
        .as_deref()
        .and_then(parse_hex_color);
    let selection_bg = theme
        .selection_background
        .as_deref()
        .and_then(parse_hex_color);

    let name = theme.name.unwrap_or_else(|| {
        source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_owned()
    });

    Some(ColorScheme {
        name,
        ansi,
        fg,
        bg,
        cursor,
        selection_fg,
        selection_bg,
    })
}

#[cfg(test)]
mod tests;
