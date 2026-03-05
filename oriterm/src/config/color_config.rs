//! Color scheme and theme configuration types.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
        super::clamp_or_default(self.minimum_contrast, 1.0, 21.0, 1.0)
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
