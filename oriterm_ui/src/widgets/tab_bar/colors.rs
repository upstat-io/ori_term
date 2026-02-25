//! Color palette for tab bar rendering.
//!
//! [`TabBarColors`] holds every color needed to draw the tab bar, derived
//! from a [`UiTheme`]. The app layer constructs one per theme and passes it
//! to the rendering phase.

use crate::animation::Lerp;
use crate::color::Color;
use crate::theme::UiTheme;

/// All colors needed to render the tab bar.
///
/// Constructed from a [`UiTheme`] via [`TabBarColors::from_theme`].
/// Window control button colors are shared with the existing
/// [`WindowChromeWidget`](crate::widgets::window_chrome::WindowChromeWidget).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabBarColors {
    /// Tab bar background (full-width strip behind all tabs).
    pub bar_bg: Color,
    /// Active tab background (rendered with rounded top corners).
    pub active_bg: Color,
    /// Inactive tab background.
    pub inactive_bg: Color,
    /// Inactive tab background on hover.
    pub tab_hover_bg: Color,
    /// Active tab title text.
    pub text_fg: Color,
    /// Inactive tab title text (dimmer than active).
    pub inactive_text: Color,
    /// 1px vertical separator between tabs.
    pub separator: Color,
    /// Close button icon color (unhovered).
    pub close_fg: Color,
    /// New-tab (+) and dropdown button hover background.
    pub button_hover_bg: Color,
    /// Window control button hover background.
    pub control_hover_bg: Color,
    /// Window control icon color.
    pub control_fg: Color,
    /// Dimmed window control icon color.
    pub control_fg_dim: Color,
    /// Close button red hover background (platform standard).
    pub control_close_hover_bg: Color,
    /// Close button text on red hover (white).
    pub control_close_hover_fg: Color,
}

impl TabBarColors {
    /// Construct tab bar colors from a UI theme.
    pub fn from_theme(theme: &UiTheme) -> Self {
        Self {
            bar_bg: theme.bg_secondary,
            active_bg: theme.bg_primary,
            inactive_bg: theme.bg_secondary,
            tab_hover_bg: theme.bg_hover,
            text_fg: theme.fg_primary,
            inactive_text: theme.fg_secondary,
            separator: theme.border.with_alpha(0.5),
            close_fg: theme.fg_secondary,
            button_hover_bg: theme.bg_hover,
            control_hover_bg: theme.bg_hover,
            control_fg: theme.fg_primary,
            control_fg_dim: theme.fg_disabled,
            control_close_hover_bg: theme.close_hover_bg,
            control_close_hover_fg: Color::WHITE,
        }
    }

    /// Compute the bell pulse color for an inactive tab.
    ///
    /// `phase` ranges from 0.0 to 1.0 (sine wave). Returns a color that
    /// smoothly oscillates between `inactive_bg` and `tab_hover_bg`.
    pub fn bell_pulse(&self, phase: f32) -> Color {
        Color::lerp(self.inactive_bg, self.tab_hover_bg, phase)
    }
}
