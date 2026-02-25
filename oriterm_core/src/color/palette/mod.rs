//! 270-entry color palette for terminal emulation.
//!
//! Layout: 0–15 ANSI, 16–231 6×6×6 cube, 232–255 grayscale ramp,
//! 256–269 named semantic slots (foreground, background, cursor, dim
//! variants, bright/dim foreground).

use vte::ansi::{Color, NamedColor};

use crate::theme::Theme;

pub use vte::ansi::Rgb;

/// Total palette entries: 256 indexed + 14 named semantic slots.
pub const NUM_COLORS: usize = 270;

/// Standard xterm ANSI colors (indices 0–15).
const ANSI_COLORS: [Rgb; 16] = [
    Rgb {
        r: 0x00,
        g: 0x00,
        b: 0x00,
    }, // 0  Black
    Rgb {
        r: 0xcc,
        g: 0x00,
        b: 0x00,
    }, // 1  Red
    Rgb {
        r: 0x4e,
        g: 0x9a,
        b: 0x06,
    }, // 2  Green
    Rgb {
        r: 0xc4,
        g: 0xa0,
        b: 0x00,
    }, // 3  Yellow
    Rgb {
        r: 0x34,
        g: 0x65,
        b: 0xa4,
    }, // 4  Blue
    Rgb {
        r: 0x75,
        g: 0x50,
        b: 0x7b,
    }, // 5  Magenta
    Rgb {
        r: 0x06,
        g: 0x98,
        b: 0x9a,
    }, // 6  Cyan
    Rgb {
        r: 0xd3,
        g: 0xd7,
        b: 0xcf,
    }, // 7  White
    Rgb {
        r: 0x55,
        g: 0x57,
        b: 0x53,
    }, // 8  Bright Black
    Rgb {
        r: 0xef,
        g: 0x29,
        b: 0x29,
    }, // 9  Bright Red
    Rgb {
        r: 0x8a,
        g: 0xe2,
        b: 0x34,
    }, // 10 Bright Green
    Rgb {
        r: 0xfc,
        g: 0xe9,
        b: 0x4f,
    }, // 11 Bright Yellow
    Rgb {
        r: 0x72,
        g: 0x9f,
        b: 0xcf,
    }, // 12 Bright Blue
    Rgb {
        r: 0xad,
        g: 0x7f,
        b: 0xa8,
    }, // 13 Bright Magenta
    Rgb {
        r: 0x34,
        g: 0xe2,
        b: 0xe2,
    }, // 14 Bright Cyan
    Rgb {
        r: 0xee,
        g: 0xee,
        b: 0xec,
    }, // 15 Bright White
];

/// Default foreground for dark theme (neutral light gray — matches Windows Terminal).
const DARK_FG: Rgb = Rgb {
    r: 0xcc,
    g: 0xcc,
    b: 0xcc,
};
/// Default background for dark theme (black).
const DARK_BG: Rgb = Rgb {
    r: 0x00,
    g: 0x00,
    b: 0x00,
};
/// Default cursor color for dark theme (white).
const DARK_CURSOR: Rgb = Rgb {
    r: 0xff,
    g: 0xff,
    b: 0xff,
};

/// Default foreground for light theme (dark gray — Tango Aluminium 6).
const LIGHT_FG: Rgb = Rgb {
    r: 0x2e,
    g: 0x34,
    b: 0x36,
};
/// Default background for light theme (white).
const LIGHT_BG: Rgb = Rgb {
    r: 0xff,
    g: 0xff,
    b: 0xff,
};
/// Default cursor color for light theme (black).
const LIGHT_CURSOR: Rgb = Rgb {
    r: 0x00,
    g: 0x00,
    b: 0x00,
};

/// 270-entry color palette with indexed and named color slots.
///
/// Resolves `vte::ansi::Color` variants to concrete `Rgb` values. Supports
/// per-index overrides (OSC 4) and reset-to-default (OSC 104).
#[derive(Debug, Clone)]
pub struct Palette {
    /// Live palette entries.
    colors: [Rgb; NUM_COLORS],
    /// Factory defaults for reset operations.
    defaults: [Rgb; NUM_COLORS],
    /// User-configured selection foreground (overrides swap logic when `Some`).
    selection_fg: Option<Rgb>,
    /// User-configured selection background (overrides swap logic when `Some`).
    selection_bg: Option<Rgb>,
}

impl Default for Palette {
    fn default() -> Self {
        Self::for_theme(Theme::default())
    }
}

/// Intermediate selection color pair.
///
/// Both `fg` and `bg` must be `Some` for the pair to take effect. When
/// either is `None`, the renderer falls back to fg/bg swap logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionColors {
    /// Explicit selection foreground.
    pub fg: Option<Rgb>,
    /// Explicit selection background.
    pub bg: Option<Rgb>,
}

impl Palette {
    /// Build a palette with semantic colors adapted to the given theme.
    ///
    /// The 256 indexed colors (ANSI, cube, grayscale) are theme-independent.
    /// Only semantic slots (foreground, background, cursor, dim/bright
    /// foreground) change between dark and light themes.
    #[must_use]
    pub fn for_theme(theme: Theme) -> Self {
        let colors = build_palette(theme);
        Self {
            colors,
            defaults: colors,
            selection_fg: None,
            selection_bg: None,
        }
    }

    /// Build a palette from explicit scheme colors.
    ///
    /// Uses `ansi[0..16]` instead of hardcoded xterm colors, and sets
    /// foreground/background/cursor from the scheme. Cube, grayscale, and
    /// dim variants are derived as usual.
    #[must_use]
    pub fn from_scheme_colors(ansi: &[Rgb; 16], fg: Rgb, bg: Rgb, cursor: Rgb) -> Self {
        let mut colors = [Rgb { r: 0, g: 0, b: 0 }; NUM_COLORS];

        // 0–15: scheme ANSI colors.
        colors[..16].copy_from_slice(ansi);

        // 16–231: 6×6×6 color cube (theme-independent).
        fill_cube(&mut colors);

        // 232–255: grayscale ramp (theme-independent).
        fill_grayscale(&mut colors);

        // Semantic slots from scheme.
        colors[NamedColor::Foreground as usize] = fg;
        colors[NamedColor::Background as usize] = bg;
        colors[NamedColor::Cursor as usize] = cursor;

        // Dim variants (2/3 brightness of scheme ANSI 0–7).
        for i in 0..8 {
            colors[NamedColor::DimBlack as usize + i] = dim_rgb(colors[i]);
        }

        // Bright/dim foreground.
        colors[NamedColor::BrightForeground as usize] = fg;
        colors[NamedColor::DimForeground as usize] = dim_rgb(fg);

        Self {
            colors,
            defaults: colors,
            selection_fg: None,
            selection_bg: None,
        }
    }

    /// Resolve a `vte::ansi::Color` to an `Rgb` value.
    pub fn resolve(&self, color: Color) -> Rgb {
        match color {
            Color::Spec(rgb) => rgb,
            Color::Indexed(idx) => self.colors[idx as usize],
            Color::Named(name) => self.colors[name as usize],
        }
    }

    /// Set an indexed color (OSC 4).
    pub fn set_indexed(&mut self, index: usize, color: Rgb) {
        if index < NUM_COLORS {
            self.colors[index] = color;
        }
    }

    /// Reset an indexed color to its default (OSC 104).
    pub fn reset_indexed(&mut self, index: usize) {
        if index < NUM_COLORS {
            self.colors[index] = self.defaults[index];
        }
    }

    /// Default foreground color.
    pub fn foreground(&self) -> Rgb {
        self.colors[NamedColor::Foreground as usize]
    }

    /// Default background color.
    pub fn background(&self) -> Rgb {
        self.colors[NamedColor::Background as usize]
    }

    /// Cursor color.
    pub fn cursor_color(&self) -> Rgb {
        self.colors[NamedColor::Cursor as usize]
    }

    /// Look up a color by palette index.
    ///
    /// Returns black for out-of-range indices. Used by OSC 4/10/11/12
    /// color query responses.
    pub fn color(&self, index: usize) -> Rgb {
        if index < NUM_COLORS {
            self.colors[index]
        } else {
            Rgb { r: 0, g: 0, b: 0 }
        }
    }

    /// Set both live and default value for an indexed color.
    ///
    /// Config overrides use this so the config value becomes the baseline
    /// that OSC 104 resets to.
    pub fn set_default(&mut self, index: usize, color: Rgb) {
        if index < NUM_COLORS {
            self.colors[index] = color;
            self.defaults[index] = color;
        }
    }

    /// Override the foreground color (live and default).
    pub fn set_foreground(&mut self, color: Rgb) {
        self.set_default(NamedColor::Foreground as usize, color);
    }

    /// Override the background color (live and default).
    pub fn set_background(&mut self, color: Rgb) {
        self.set_default(NamedColor::Background as usize, color);
    }

    /// Override the cursor color (live and default).
    pub fn set_cursor_color(&mut self, color: Rgb) {
        self.set_default(NamedColor::Cursor as usize, color);
    }

    /// User-configured selection foreground color.
    pub fn selection_fg(&self) -> Option<Rgb> {
        self.selection_fg
    }

    /// User-configured selection background color.
    pub fn selection_bg(&self) -> Option<Rgb> {
        self.selection_bg
    }

    /// Set the selection foreground color override.
    pub fn set_selection_fg(&mut self, color: Option<Rgb>) {
        self.selection_fg = color;
    }

    /// Set the selection background color override.
    pub fn set_selection_bg(&mut self, color: Option<Rgb>) {
        self.selection_bg = color;
    }

    /// Selection colors as a pair for the rendering pipeline.
    pub fn selection_colors(&self) -> SelectionColors {
        SelectionColors {
            fg: self.selection_fg,
            bg: self.selection_bg,
        }
    }
}

/// Build the xterm-256 palette with semantic colors adapted to the theme.
fn build_palette(theme: Theme) -> [Rgb; NUM_COLORS] {
    let mut colors = [Rgb { r: 0, g: 0, b: 0 }; NUM_COLORS];

    // 0–15: ANSI colors (theme-independent).
    colors[..16].copy_from_slice(&ANSI_COLORS);

    // 16–231: 6×6×6 color cube.
    fill_cube(&mut colors);

    // 232–255: grayscale ramp.
    fill_grayscale(&mut colors);

    // Named semantic slots — theme-dependent.
    let (fg, bg, cursor) = if theme.is_dark() {
        (DARK_FG, DARK_BG, DARK_CURSOR)
    } else {
        (LIGHT_FG, LIGHT_BG, LIGHT_CURSOR)
    };
    colors[NamedColor::Foreground as usize] = fg;
    colors[NamedColor::Background as usize] = bg;
    colors[NamedColor::Cursor as usize] = cursor;

    // Dim variants (2/3 brightness of ANSI 0–7).
    for i in 0..8 {
        colors[NamedColor::DimBlack as usize + i] = dim_rgb(colors[i]);
    }

    // Bright/dim foreground.
    colors[NamedColor::BrightForeground as usize] = fg;
    colors[NamedColor::DimForeground as usize] = dim_rgb(fg);

    colors
}

/// Fill the 6×6×6 color cube (palette indices 16–231).
fn fill_cube(colors: &mut [Rgb; NUM_COLORS]) {
    for r in 0..6u8 {
        for g in 0..6u8 {
            for b in 0..6u8 {
                let idx = 16 + (r as usize * 36) + (g as usize * 6) + b as usize;
                colors[idx] = Rgb {
                    r: if r == 0 { 0 } else { 55 + r * 40 },
                    g: if g == 0 { 0 } else { 55 + g * 40 },
                    b: if b == 0 { 0 } else { 55 + b * 40 },
                };
            }
        }
    }
}

/// Fill the 24-step grayscale ramp (palette indices 232–255).
fn fill_grayscale(colors: &mut [Rgb; NUM_COLORS]) {
    for i in 0..24u8 {
        let v = 8 + i * 10;
        colors[232 + i as usize] = Rgb { r: v, g: v, b: v };
    }
}

/// Reduce a color to 2/3 brightness for dim variants.
pub(crate) fn dim_rgb(c: Rgb) -> Rgb {
    Rgb {
        r: (c.r as u16 * 2 / 3) as u8,
        g: (c.g as u16 * 2 / 3) as u8,
        b: (c.b as u16 * 2 / 3) as u8,
    }
}

#[cfg(test)]
mod tests;
