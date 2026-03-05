//! Font configuration types: primary font, fallbacks, and codepoint mappings.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Per-fallback font configuration.
///
/// Allows overriding OpenType features and size for individual fallback fonts.
/// Users specify these via `[[font.fallback]]` TOML array-of-tables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FallbackFontConfig {
    /// Font family name (resolved via platform font discovery) or absolute path.
    pub family: String,
    /// Override OpenType features for this fallback (uses primary features if `None`).
    #[serde(default)]
    pub features: Option<Vec<String>>,
    /// Point size adjustment relative to primary font (e.g. `-1.0` for smaller).
    #[serde(default)]
    pub size_offset: Option<f32>,
}

/// Codepoint-to-font mapping entry.
///
/// Forces a Unicode range to render with a specific font, overriding the
/// normal fallback chain. Common use: Nerd Font symbols, CJK to a specific
/// variant, or emoji to a dedicated color font.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodepointMapConfig {
    /// Hex range: `"E000-F8FF"` for a range, or `"E0B0"` for a single codepoint.
    pub range: String,
    /// Font family name to use for this codepoint range.
    pub family: String,
}

/// Font configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    /// Font size in points.
    pub size: f32,
    /// Primary font family name.
    pub family: Option<String>,
    /// CSS-style font weight (100-900). Default: 400 (Regular).
    ///
    /// Bold is derived as `min(900, weight + 300)`, matching CSS "bolder".
    pub weight: u16,
    /// CSS-style font weight for tab bar text (100-900).
    /// When `None`, defaults to 600 (`SemiBold`).
    pub tab_bar_font_weight: Option<u16>,
    /// Font family for tab bar text. When `None`, uses `family`.
    pub tab_bar_font_family: Option<String>,
    /// OpenType features to enable/disable during text shaping.
    ///
    /// Each string is a 4-character feature tag, optionally prefixed with `-`
    /// to disable. Defaults to `["calt", "liga"]`.
    pub features: Vec<String>,
    /// User-configured fallback fonts with per-font feature and size overrides.
    #[serde(default)]
    pub fallback: Vec<FallbackFontConfig>,
    /// Hinting mode override: `"full"` or `"none"`.
    ///
    /// When `None` (default), auto-detected from display scale factor:
    /// non-HiDPI → Full, `HiDPI` (2x+) → None.
    #[serde(default)]
    pub hinting: Option<String>,
    /// Subpixel rendering mode override: `"rgb"`, `"bgr"`, or `"none"`.
    ///
    /// When `None` (default), auto-detected from display scale factor:
    /// non-HiDPI → RGB, `HiDPI` (2x+) → None.
    #[serde(default)]
    pub subpixel_mode: Option<String>,
    /// Subpixel glyph positioning. Default: `true`.
    ///
    /// When `false`, snaps all glyph positions to integer pixel boundaries.
    /// When `true`, uses fractional offsets for UI text and combining marks.
    #[serde(default = "default_true")]
    pub subpixel_positioning: bool,
    /// Variable font axis overrides.
    ///
    /// Keys are 4-character axis tags (e.g. `"wght"`, `"wdth"`), values are
    /// axis positions. Values are clamped to the font's axis min/max range.
    #[serde(default)]
    pub variations: HashMap<String, f32>,
    /// Codepoint-to-font mappings for overriding the fallback chain.
    #[serde(default)]
    pub codepoint_map: Vec<CodepointMapConfig>,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            size: 11.0,
            family: None,
            weight: 400,
            tab_bar_font_weight: None,
            tab_bar_font_family: None,
            features: vec!["calt".into(), "liga".into()],
            fallback: Vec::new(),
            hinting: None,
            subpixel_mode: None,
            subpixel_positioning: true,
            variations: HashMap::new(),
            codepoint_map: Vec::new(),
        }
    }
}

/// Serde default for `true` booleans.
fn default_true() -> bool {
    true
}

impl FontConfig {
    /// Returns `weight` clamped to the CSS font-weight range [100, 900].
    pub fn effective_weight(&self) -> u16 {
        self.weight.clamp(100, 900)
    }

    /// Returns the bold weight derived from the user weight: `min(900, weight + 300)`.
    #[allow(dead_code, reason = "used in config hot reload (Section 13.4)")]
    pub fn effective_bold_weight(&self) -> u16 {
        (self.effective_weight() + 300).min(900)
    }

    /// Returns `tab_bar_font_weight` clamped to [100, 900], defaulting to 600 (`SemiBold`).
    #[allow(dead_code, reason = "used in config hot reload (Section 13.4)")]
    pub fn effective_tab_bar_weight(&self) -> u16 {
        self.tab_bar_font_weight.unwrap_or(600).clamp(100, 900)
    }
}
