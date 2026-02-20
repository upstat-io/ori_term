//! RGBA color type for UI rendering.
//!
//! All channels are `f32` in `[0.0, 1.0]`. No clamping is applied — callers
//! are trusted to provide valid values (GPU shaders handle out-of-range
//! gracefully via saturation).

/// An RGBA color with `f32` channels in `[0.0, 1.0]`.
///
/// Designed for direct upload to GPU uniform/instance buffers. No implicit
/// premultiplication — the shader handles that at output time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red channel.
    pub r: f32,
    /// Green channel.
    pub g: f32,
    /// Blue channel.
    pub b: f32,
    /// Alpha channel (0 = transparent, 1 = opaque).
    pub a: f32,
}

impl Color {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Opaque white.
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    /// Opaque black.
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// Creates a color from RGBA float channels.
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Creates an opaque color from RGB float channels.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Creates an opaque color from a `0xRRGGBB` hex literal.
    pub const fn hex(rgb: u32) -> Self {
        Self {
            r: ((rgb >> 16) & 0xFF) as f32 / 255.0,
            g: ((rgb >> 8) & 0xFF) as f32 / 255.0,
            b: (rgb & 0xFF) as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Creates a color from a `0xRRGGBBAA` hex literal.
    pub const fn hex_alpha(rgba: u32) -> Self {
        Self {
            r: ((rgba >> 24) & 0xFF) as f32 / 255.0,
            g: ((rgba >> 16) & 0xFF) as f32 / 255.0,
            b: ((rgba >> 8) & 0xFF) as f32 / 255.0,
            a: (rgba & 0xFF) as f32 / 255.0,
        }
    }

    /// Creates an opaque color from `u8` RGB components.
    pub const fn from_rgb_u8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Returns a copy with the given alpha value.
    #[must_use]
    pub const fn with_alpha(self, a: f32) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a,
        }
    }

    /// Converts to a 4-element array `[r, g, b, a]` for GPU upload.
    pub const fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

impl crate::animation::Lerp for Color {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        Self {
            r: a.r + (b.r - a.r) * t,
            g: a.g + (b.g - a.g) * t,
            b: a.b + (b.b - a.b) * t,
            a: a.a + (b.a - a.a) * t,
        }
    }
}

#[cfg(test)]
mod tests;
