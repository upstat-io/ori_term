// Font types consumed starting in Section 5.7; suppress until then.
#![expect(dead_code, reason = "GPU infrastructure used starting in Section 5.7")]

//! Font management: discovery, loading, and rasterization.
//!
//! This module handles finding font files on disk across platforms, loading
//! them into memory, and rasterizing glyphs for the GPU renderer.
//!
//! # Architecture
//!
//! - [`discovery`] resolves family names and style variants to file paths.
//! - [`collection`] loads font bytes, computes cell metrics, and rasterizes
//!   glyphs into bitmaps for atlas upload.

pub mod collection;
pub mod discovery;

use std::fmt;

use bitflags::bitflags;

#[expect(unused_imports, reason = "re-exports consumed starting in Section 5.7")]
pub use collection::{FontCollection, FontData, FontSet, RasterizedGlyph};

/// Rasterization output format.
///
/// Determines pixel layout in [`RasterizedGlyph::bitmap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlyphFormat {
    /// 1 byte/pixel grayscale alpha coverage.
    Alpha,
    /// 4 bytes/pixel RGBA per-channel subpixel coverage (R-G-B order).
    SubpixelRgb,
    /// 4 bytes/pixel RGBA per-channel subpixel coverage (B-G-R order).
    SubpixelBgr,
    /// 4 bytes/pixel RGBA premultiplied color (for color emoji via skrifa in Section 6).
    Color,
}

impl GlyphFormat {
    /// Bytes per pixel for this format.
    pub fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::Alpha => 1,
            Self::SubpixelRgb | Self::SubpixelBgr | Self::Color => 4,
        }
    }
}

/// Font style for face selection.
///
/// Discriminant values match the primary face array indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlyphStyle {
    /// Normal weight, upright.
    Regular = 0,
    /// Bold weight, upright.
    Bold = 1,
    /// Normal weight, italic/oblique.
    Italic = 2,
    /// Bold weight, italic/oblique.
    BoldItalic = 3,
}

/// Cache key for rasterized glyphs — glyph-ID-based, not character-based.
///
/// The `size_q6` field encodes size in 26.6 fixed-point: `(size_px * 64.0).round() as u32`.
/// This avoids floating-point hashing while preserving sub-pixel size changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RasterKey {
    /// Glyph ID within the font face.
    pub glyph_id: u16,
    /// Face index: 0–3 = primary (Regular/Bold/Italic/BoldItalic), 4+ = fallback.
    pub face_idx: u16,
    /// Size in 26.6 fixed-point: `(size_px * 64.0).round() as u32`.
    pub size_q6: u32,
}

bitflags! {
    /// Flags indicating synthetic style transformations needed at render time.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SyntheticFlags: u8 {
        /// No synthetic transformations.
        const NONE   = 0;
        /// Synthetic emboldening needed (no real bold variant).
        const BOLD   = 0b01;
        /// Synthetic slant needed (no real italic variant).
        const ITALIC = 0b10;
    }
}

/// Result of resolving a character to a font face and glyph ID.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedGlyph {
    /// Glyph ID within the font face.
    pub glyph_id: u16,
    /// Face index: 0–3 = primary, 4+ = fallback.
    pub face_idx: u16,
    /// Whether synthetic style transformations are needed.
    pub synthetic: SyntheticFlags,
}

/// Font loading and validation errors.
#[derive(Debug)]
pub enum FontError {
    /// Font data is invalid or could not be parsed.
    InvalidFont(String),
    /// I/O error reading a font file.
    Io(std::io::Error),
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFont(msg) => write!(f, "invalid font: {msg}"),
            Self::Io(err) => write!(f, "font I/O error: {err}"),
        }
    }
}

impl std::error::Error for FontError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::InvalidFont(_) => None,
        }
    }
}

impl From<std::io::Error> for FontError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
