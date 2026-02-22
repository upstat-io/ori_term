//! Configuration structures and loading logic.
//!
//! All structs use `#[serde(default)]` so partial TOML files
//! fill in defaults for missing fields.

// Config is wired into the application startup path in 13.4 (hot reload).
// Until then, the module is only exercised by tests.
#![expect(dead_code, reason = "config wired into App in section 13.4")]

mod io;
pub(crate) mod monitor;

#[expect(
    unused_imports,
    reason = "WindowState and state_path used in section 13.4"
)]
pub(crate) use io::{WindowState, config_path, parse_cursor_style, state_path};

use std::collections::HashMap;

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
    pub fn effective_bold_weight(&self) -> u16 {
        (self.effective_weight() + 300).min(900)
    }

    /// Returns `tab_bar_font_weight` clamped to [100, 900], defaulting to 600 (`SemiBold`).
    pub fn effective_tab_bar_weight(&self) -> u16 {
        self.tab_bar_font_weight.unwrap_or(600).clamp(100, 900)
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
    /// Cursor style: "block", "bar"/"beam", "underline" (default: "block").
    pub cursor_style: String,
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
            cursor_style: "block".to_owned(),
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

/// Color scheme and palette configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorConfig {
    /// Color scheme name (default: "Catppuccin Mocha").
    pub scheme: String,
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
    pub selection_foreground: Option<String>,
    /// Override selection background color "#RRGGBB".
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
    /// Returns `minimum_contrast` clamped to [1.0, 21.0].
    pub fn effective_minimum_contrast(&self) -> f32 {
        self.minimum_contrast.clamp(1.0, 21.0)
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
    /// Returns opacity clamped to [0.0, 1.0].
    pub fn effective_opacity(&self) -> f32 {
        self.opacity.clamp(0.0, 1.0)
    }

    /// Returns tab bar opacity clamped to [0.0, 1.0].
    /// Falls back to `opacity` when not explicitly set.
    pub fn effective_tab_bar_opacity(&self) -> f32 {
        self.tab_bar_opacity.unwrap_or(self.opacity).clamp(0.0, 1.0)
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

/// Visual bell configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BellConfig {
    /// Visual bell animation: `ease_out`, `linear`, `none`.
    pub animation: String,
    /// Duration in milliseconds (0 = disabled).
    pub duration_ms: u16,
    /// Flash color as "#RRGGBB" hex (default: white).
    pub color: Option<String>,
}

impl Default for BellConfig {
    fn default() -> Self {
        Self {
            animation: "ease_out".into(),
            duration_ms: 150,
            color: None,
        }
    }
}

impl BellConfig {
    /// Returns true when the visual bell is enabled.
    pub fn is_enabled(&self) -> bool {
        self.duration_ms > 0 && self.animation != "none"
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

#[cfg(test)]
mod tests;
