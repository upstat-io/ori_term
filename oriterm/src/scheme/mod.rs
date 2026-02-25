//! Color scheme resolution and built-in definitions.
//!
//! Resolves `ColorConfig.scheme` to a [`ColorScheme`] via built-in lookup or
//! TOML theme file loading. Supports conditional `"dark:X, light:Y"` syntax
//! for automatic light/dark switching.

mod builtin;
mod loader;

use oriterm_core::{Palette, Rgb, Theme};

pub(crate) use builtin::BUILTIN_SCHEMES;

/// A resolved color scheme with 16 ANSI colors and semantic colors.
#[derive(Debug, Clone)]
pub(crate) struct ColorScheme {
    /// Display name.
    pub name: String,
    /// ANSI colors 0–15.
    pub ansi: [Rgb; 16],
    /// Default foreground.
    pub fg: Rgb,
    /// Default background.
    pub bg: Rgb,
    /// Cursor color.
    pub cursor: Rgb,
    /// Explicit selection foreground (scheme-provided, may be `None`).
    pub selection_fg: Option<Rgb>,
    /// Explicit selection background (scheme-provided, may be `None`).
    pub selection_bg: Option<Rgb>,
}

/// A built-in scheme definition (compile-time constant).
pub(crate) struct BuiltinScheme {
    pub name: &'static str,
    pub ansi: [Rgb; 16],
    pub fg: Rgb,
    pub bg: Rgb,
    pub cursor: Rgb,
}

impl BuiltinScheme {
    /// Convert to a full [`ColorScheme`] (no selection colors).
    fn to_scheme(&self) -> ColorScheme {
        ColorScheme {
            name: self.name.to_owned(),
            ansi: self.ansi,
            fg: self.fg,
            bg: self.bg,
            cursor: self.cursor,
            selection_fg: None,
            selection_bg: None,
        }
    }
}

/// Find a built-in scheme by name (case-insensitive).
pub(crate) fn find_builtin(name: &str) -> Option<ColorScheme> {
    BUILTIN_SCHEMES
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .map(|s| s.to_scheme())
}

/// All built-in scheme names, in definition order.
#[cfg(test)]
fn builtin_names() -> Vec<&'static str> {
    BUILTIN_SCHEMES.iter().map(|s| s.name).collect()
}

/// Resolve a scheme name to a [`ColorScheme`].
///
/// Checks built-in schemes first, then attempts to load from the themes
/// directory. Returns `None` if the scheme cannot be found.
pub(crate) fn resolve_scheme(name: &str) -> Option<ColorScheme> {
    find_builtin(name).or_else(|| loader::load_from_themes_dir(name))
}

/// Parse a conditional scheme string: `"dark:X, light:Y"`.
///
/// Returns `Some((dark_name, light_name))` if the input contains both
/// `dark:` and `light:` prefixed names. Returns `None` for plain names.
pub(crate) fn parse_conditional(spec: &str) -> Option<(&str, &str)> {
    let mut dark = None;
    let mut light = None;

    for part in spec.split(',') {
        let part = part.trim();
        if let Some(name) = part.strip_prefix("dark:") {
            dark = Some(name.trim());
        } else if let Some(name) = part.strip_prefix("light:") {
            light = Some(name.trim());
        } else {
            // Unrecognized prefix — ignored.
        }
    }

    match (dark, light) {
        (Some(d), Some(l)) => Some((d, l)),
        _ => None,
    }
}

/// Resolve a scheme name that may be conditional, given the current theme.
///
/// If the name is `"dark:X, light:Y"`, picks `X` or `Y` based on `theme`.
/// Otherwise returns the name as-is.
pub(crate) fn resolve_scheme_name(spec: &str, theme: Theme) -> &str {
    match parse_conditional(spec) {
        Some((dark, light)) => {
            if theme.is_dark() {
                dark
            } else {
                light
            }
        }
        None => spec.trim(),
    }
}

/// Build a [`Palette`] from a [`ColorScheme`].
pub(crate) fn palette_from_scheme(scheme: &ColorScheme) -> Palette {
    let mut palette =
        Palette::from_scheme_colors(&scheme.ansi, scheme.fg, scheme.bg, scheme.cursor);
    palette.set_selection_fg(scheme.selection_fg);
    palette.set_selection_bg(scheme.selection_bg);
    palette
}

#[cfg(test)]
mod tests;
