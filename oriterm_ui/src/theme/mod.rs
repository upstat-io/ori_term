//! Centralized theming for the UI framework.
//!
//! [`UiTheme`] provides a single source of truth for colors and sizing tokens
//! used across all widget styles. Dark and light factories ensure consistency;
//! widget `*Style` structs derive their defaults from the theme via
//! `from_theme()`.

use crate::color::Color;

/// Centralized color and sizing tokens for the UI framework.
///
/// All widget `*Style::from_theme()` constructors read from these fields.
/// `dark()` matches the legacy `DEFAULT_*` constants — zero visual regression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiTheme {
    /// Primary background (widget surfaces).
    pub bg_primary: Color,
    /// Secondary background (disabled surfaces, panels).
    pub bg_secondary: Color,
    /// Background on hover.
    pub bg_hover: Color,
    /// Background on press/active.
    pub bg_active: Color,
    /// Primary foreground (text, icons).
    pub fg_primary: Color,
    /// Secondary foreground (captions, metadata).
    pub fg_secondary: Color,
    /// Disabled foreground.
    pub fg_disabled: Color,
    /// Accent color (toggles, focus rings, checked states).
    pub accent: Color,
    /// Border color.
    pub border: Color,
    /// Shadow color (typically semi-transparent black).
    pub shadow: Color,
    /// Close button hover background (platform standard red).
    pub close_hover_bg: Color,
    /// Close button pressed background (darker red).
    pub close_pressed_bg: Color,
    /// Default corner radius in logical pixels.
    pub corner_radius: f32,
    /// Default spacing/gap in logical pixels.
    pub spacing: f32,
    /// Default font size in points.
    pub font_size: f32,
    /// Small font size in points.
    pub font_size_small: f32,
    /// Large font size in points.
    pub font_size_large: f32,
}

impl UiTheme {
    /// Dark theme matching the legacy `DEFAULT_*` constants.
    pub const fn dark() -> Self {
        Self {
            bg_primary: Color::from_rgb_u8(0x2D, 0x2D, 0x2D),
            bg_secondary: Color::from_rgb_u8(0x25, 0x25, 0x25),
            bg_hover: Color::from_rgb_u8(0x3D, 0x3D, 0x3D),
            bg_active: Color::from_rgb_u8(0x1D, 0x1D, 0x1D),
            fg_primary: Color::from_rgb_u8(0xE0, 0xE0, 0xE0),
            fg_secondary: Color::from_rgb_u8(0xA0, 0xA0, 0xA0),
            fg_disabled: Color::from_rgb_u8(0x80, 0x80, 0x80),
            accent: Color::from_rgb_u8(0x4A, 0x9E, 0xFF),
            border: Color::from_rgb_u8(0x55, 0x55, 0x55),
            shadow: Color::rgba(0.0, 0.0, 0.0, 0.5),
            close_hover_bg: Color::hex(0xC4_2B_1C),
            close_pressed_bg: Color::hex(0xA1_20_12),
            corner_radius: 4.0,
            spacing: 8.0,
            font_size: 13.0,
            font_size_small: 11.0,
            font_size_large: 16.0,
        }
    }

    /// Light theme for bright environments.
    pub const fn light() -> Self {
        Self {
            bg_primary: Color::from_rgb_u8(0xF5, 0xF5, 0xF5),
            bg_secondary: Color::from_rgb_u8(0xFF, 0xFF, 0xFF),
            bg_hover: Color::from_rgb_u8(0xE8, 0xE8, 0xE8),
            bg_active: Color::from_rgb_u8(0xD0, 0xD0, 0xD0),
            fg_primary: Color::from_rgb_u8(0x1A, 0x1A, 0x1A),
            fg_secondary: Color::from_rgb_u8(0x60, 0x60, 0x60),
            fg_disabled: Color::from_rgb_u8(0xA0, 0xA0, 0xA0),
            accent: Color::from_rgb_u8(0x00, 0x78, 0xD4),
            border: Color::from_rgb_u8(0xCC, 0xCC, 0xCC),
            shadow: Color::rgba(0.0, 0.0, 0.0, 0.15),
            close_hover_bg: Color::hex(0xC4_2B_1C),
            close_pressed_bg: Color::hex(0xA1_20_12),
            corner_radius: 4.0,
            spacing: 8.0,
            font_size: 13.0,
            font_size_small: 11.0,
            font_size_large: 16.0,
        }
    }
}

impl Default for UiTheme {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests;
