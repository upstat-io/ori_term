//! Configuration structures and loading logic.
//!
//! All structs use `#[serde(default)]` so partial TOML files
//! fill in defaults for missing fields.

mod io;
pub(crate) mod monitor;

pub(crate) use io::config_path;

#[allow(unused_imports, reason = "used in state persistence (Section 15)")]
pub(crate) use io::WindowState;
#[allow(unused_imports, reason = "used in state persistence (Section 15)")]
pub(crate) use io::state_path;

use std::collections::HashMap;

use oriterm_core::Rgb;
use serde::{Deserialize, Serialize};

/// Top-level configuration structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub font: FontConfig,
    pub terminal: TerminalConfig,
    pub colors: ColorConfig,
    pub window: WindowConfig,
    pub behavior: BehaviorConfig,
    pub bell: BellConfig,
    #[serde(default)]
    pub keybind: Vec<KeybindConfig>,
}

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
        }
    }
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

/// Cursor style for config deserialization.
///
/// Maps user-facing TOML strings to `oriterm_core::CursorShape` values.
/// `"beam"` is accepted as an alias for `Bar` (common in other terminals).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorStyle {
    #[default]
    Block,
    #[serde(alias = "beam")]
    Bar,
    Underline,
}

impl CursorStyle {
    /// Convert to the rendering-layer `CursorShape`.
    pub fn to_shape(self) -> oriterm_core::CursorShape {
        match self {
            Self::Block => oriterm_core::CursorShape::Block,
            Self::Bar => oriterm_core::CursorShape::Bar,
            Self::Underline => oriterm_core::CursorShape::Underline,
        }
    }
}

/// Terminal behavior configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    /// Override shell (default: system shell).
    pub shell: Option<String>,
    /// Scrollback lines (default: 10,000).
    pub scrollback: usize,
    /// Cursor style (default: block).
    pub cursor_style: CursorStyle,
    /// Enable cursor blinking (default: true).
    pub cursor_blink: bool,
    /// Blink interval in milliseconds (default: 530).
    pub cursor_blink_interval_ms: u64,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: None,
            scrollback: 10_000,
            cursor_style: CursorStyle::default(),
            cursor_blink: true,
            cursor_blink_interval_ms: 530,
        }
    }
}

/// Alpha blending mode for text rendering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlphaBlending {
    /// Standard sRGB surface format blending.
    Linear,
    /// Ghostty-style luminance-based alpha correction for even text weight.
    #[default]
    LinearCorrected,
}

/// Theme override for dark/light mode.
///
/// When set to `Auto` (or omitted), the system theme is detected at startup.
/// `Dark` and `Light` force the corresponding palette regardless of system
/// preference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeOverride {
    /// Use the system's dark/light mode preference.
    #[default]
    Auto,
    /// Force dark mode (dark background, light text).
    Dark,
    /// Force light mode (light background, dark text).
    Light,
}

/// Color scheme and palette configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorConfig {
    /// Color scheme name (default: "Catppuccin Mocha").
    pub scheme: String,
    /// Dark/light mode override (default: auto-detect from system).
    pub theme: ThemeOverride,
    /// Minimum WCAG 2.0 contrast ratio (1.0 = off, range 1.0-21.0).
    pub minimum_contrast: f32,
    /// Alpha blending mode for text rendering.
    pub alpha_blending: AlphaBlending,
    /// Override foreground color "#RRGGBB".
    pub foreground: Option<String>,
    /// Override background color "#RRGGBB".
    pub background: Option<String>,
    /// Override cursor color "#RRGGBB".
    pub cursor: Option<String>,
    /// Override selection foreground color "#RRGGBB".
    #[allow(
        dead_code,
        reason = "consumed when selection rendering lands (Section 18)"
    )]
    pub selection_foreground: Option<String>,
    /// Override selection background color "#RRGGBB".
    #[allow(
        dead_code,
        reason = "consumed when selection rendering lands (Section 18)"
    )]
    pub selection_background: Option<String>,
    /// Override ANSI colors 0-7 by index. Keys "0"-"7", values "#RRGGBB".
    #[serde(default)]
    pub ansi: HashMap<String, String>,
    /// Override bright ANSI colors 8-15 by index (0-7 maps to colors 8-15).
    #[serde(default)]
    pub bright: HashMap<String, String>,
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            scheme: "Catppuccin Mocha".to_owned(),
            theme: ThemeOverride::default(),
            minimum_contrast: 1.0,
            alpha_blending: AlphaBlending::default(),
            foreground: None,
            background: None,
            cursor: None,
            selection_foreground: None,
            selection_background: None,
            ansi: HashMap::new(),
            bright: HashMap::new(),
        }
    }
}

impl ColorConfig {
    /// Returns `minimum_contrast` clamped to [1.0, 21.0], defaulting to 1.0 for NaN.
    #[allow(dead_code, reason = "used in color config application")]
    pub fn effective_minimum_contrast(&self) -> f32 {
        clamp_or_default(self.minimum_contrast, 1.0, 21.0, 1.0)
    }

    /// Resolve the effective theme given the config override.
    ///
    /// `Dark` / `Light` ignore system detection entirely; `Auto` delegates to
    /// the provided `system_theme` callback.
    pub fn resolve_theme(
        &self,
        detect_system: impl FnOnce() -> oriterm_core::Theme,
    ) -> oriterm_core::Theme {
        match self.theme {
            ThemeOverride::Dark => oriterm_core::Theme::Dark,
            ThemeOverride::Light => oriterm_core::Theme::Light,
            ThemeOverride::Auto => detect_system(),
        }
    }
}

/// Window decoration mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decorations {
    /// OS-native title bar and borders.
    Full,
    /// Frameless window with custom CSD (default).
    #[default]
    None,
    /// macOS: transparent titlebar.
    Transparent,
    /// macOS: hide traffic lights.
    Buttonless,
}

/// Window size and opacity configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    /// Initial terminal columns (default: 120).
    pub columns: usize,
    /// Initial terminal rows (default: 30).
    pub rows: usize,
    /// Window opacity 0.0-1.0 (default: 1.0).
    pub opacity: f32,
    /// Independent tab bar opacity (falls back to `opacity`).
    pub tab_bar_opacity: Option<f32>,
    /// Enable backdrop blur (default: true).
    pub blur: bool,
    /// Window decoration mode (default: `None` for frameless CSD).
    pub decorations: Decorations,
    /// Snap resize to cell boundaries (default: false).
    pub resize_increments: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            columns: 120,
            rows: 30,
            opacity: 1.0,
            tab_bar_opacity: None,
            blur: true,
            decorations: Decorations::default(),
            resize_increments: false,
        }
    }
}

impl WindowConfig {
    /// Returns opacity clamped to [0.0, 1.0], defaulting to 1.0 for NaN.
    pub fn effective_opacity(&self) -> f32 {
        clamp_or_default(self.opacity, 0.0, 1.0, 1.0)
    }

    /// Returns tab bar opacity clamped to [0.0, 1.0].
    /// Falls back to `opacity` when not explicitly set.
    #[allow(dead_code, reason = "used in tab bar rendering (Section 16)")]
    pub fn effective_tab_bar_opacity(&self) -> f32 {
        clamp_or_default(self.tab_bar_opacity.unwrap_or(self.opacity), 0.0, 1.0, 1.0)
    }
}

/// User interaction behavior configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    /// Auto-copy on selection release (default: true).
    pub copy_on_select: bool,
    /// Bold text uses bright colors (default: true).
    pub bold_is_bright: bool,
    /// Enable shell integration injection (default: true).
    pub shell_integration: bool,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            copy_on_select: true,
            bold_is_bright: true,
            shell_integration: true,
        }
    }
}

/// Visual bell animation curve.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BellAnimation {
    #[default]
    EaseOut,
    Linear,
    None,
}

/// Visual bell configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BellConfig {
    /// Visual bell animation curve.
    pub animation: BellAnimation,
    /// Duration in milliseconds (0 = disabled).
    pub duration_ms: u16,
    /// Flash color as "#RRGGBB" hex (default: white).
    pub color: Option<String>,
}

impl Default for BellConfig {
    fn default() -> Self {
        Self {
            animation: BellAnimation::default(),
            duration_ms: 150,
            color: None,
        }
    }
}

impl BellConfig {
    /// Returns true when the visual bell is enabled.
    #[allow(dead_code, reason = "used in bell rendering (Section 24)")]
    pub fn is_enabled(&self) -> bool {
        self.duration_ms > 0 && self.animation != BellAnimation::None
    }
}

/// TOML-serializable keybinding entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindConfig {
    /// Key name (e.g. "c", "Tab", "F1").
    pub key: String,
    /// Pipe-separated modifiers (e.g. "Ctrl|Shift"). Empty for no modifiers.
    #[serde(default)]
    pub mods: String,
    /// Action name (e.g. "Copy", "Paste", "SendText:\x1b[A").
    pub action: String,
}

/// Clamp a float to `[min, max]`, returning `default` for NaN.
///
/// `f32::clamp` passes NaN through unchanged, which can propagate into
/// rendering calculations. This helper treats NaN as "no valid value".
fn clamp_or_default(value: f32, min: f32, max: f32, default: f32) -> f32 {
    if value.is_nan() {
        default
    } else {
        value.clamp(min, max)
    }
}

/// Parse a `#RRGGBB` hex color string to [`Rgb`].
///
/// Accepts with or without the leading `#`. Returns `None` on invalid input.
pub(crate) fn parse_hex_color(hex: &str) -> Option<Rgb> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    let bytes = hex.as_bytes();
    if bytes.len() != 6 || !bytes.iter().all(u8::is_ascii_hexdigit) {
        log::warn!("config: invalid hex color (expected #RRGGBB): {hex}");
        return None;
    }
    // Safe: validated 6 ASCII hex digits above — all single-byte UTF-8.
    let r = (hex_nibble(bytes[0]) << 4) | hex_nibble(bytes[1]);
    let g = (hex_nibble(bytes[2]) << 4) | hex_nibble(bytes[3]);
    let b = (hex_nibble(bytes[4]) << 4) | hex_nibble(bytes[5]);
    Some(Rgb { r, g, b })
}

/// Convert a single ASCII hex digit to its numeric value.
fn hex_nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0, // unreachable after validation
    }
}

#[cfg(test)]
mod tests;
