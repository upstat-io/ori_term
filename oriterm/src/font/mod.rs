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
pub mod shaper;

use std::fmt;

use bitflags::bitflags;

pub use collection::{FontCollection, FontSet};

/// Cell dimensions in pixels, derived from the font metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellMetrics {
    /// Cell width in pixels (fractional for subpixel accuracy).
    pub width: f32,
    /// Cell height in pixels (fractional for subpixel accuracy).
    pub height: f32,
    /// Distance from cell top to text baseline, in pixels.
    pub baseline: f32,
}

impl CellMetrics {
    /// Create cell metrics from font-derived dimensions.
    ///
    /// # Panics
    ///
    /// Panics in debug mode if any dimension is non-positive or non-finite.
    pub fn new(width: f32, height: f32, baseline: f32) -> Self {
        debug_assert!(
            width > 0.0 && width.is_finite(),
            "cell width must be positive"
        );
        debug_assert!(
            height > 0.0 && height.is_finite(),
            "cell height must be positive"
        );
        debug_assert!(baseline.is_finite(), "baseline must be finite");
        Self {
            width,
            height,
            baseline,
        }
    }

    /// Number of columns that fit in the viewport width.
    pub fn columns(&self, viewport_width: u32) -> usize {
        (f64::from(viewport_width) / f64::from(self.width)).floor() as usize
    }

    /// Number of rows that fit in the viewport height.
    pub fn rows(&self, viewport_height: u32) -> usize {
        (f64::from(viewport_height) / f64::from(self.height)).floor() as usize
    }
}

/// Rasterization output format.
///
/// Determines pixel layout in [`RasterizedGlyph::bitmap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlyphFormat {
    /// 1 byte/pixel grayscale alpha coverage.
    Alpha,
    /// 4 bytes/pixel RGBA per-channel subpixel coverage (R-G-B order).
    #[allow(dead_code, reason = "subpixel rendering in Section 6")]
    SubpixelRgb,
    /// 4 bytes/pixel RGBA per-channel subpixel coverage (B-G-R order).
    #[allow(dead_code, reason = "subpixel rendering in Section 6")]
    SubpixelBgr,
    /// 4 bytes/pixel RGBA premultiplied color (for color emoji).
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

impl GlyphStyle {
    /// Derive the glyph style from cell attribute flags.
    pub fn from_cell_flags(flags: oriterm_core::CellFlags) -> Self {
        let bold = flags.contains(oriterm_core::CellFlags::BOLD);
        let italic = flags.contains(oriterm_core::CellFlags::ITALIC);
        match (bold, italic) {
            (true, true) => Self::BoldItalic,
            (true, false) => Self::Bold,
            (false, true) => Self::Italic,
            (false, false) => Self::Regular,
        }
    }
}

/// Compact face index into the font collection.
///
/// Indices 0–3 map to primary style variants (Regular, Bold, Italic, `BoldItalic`).
/// Indices 4+ map to fallback fonts in priority order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FaceIdx(pub u16);

impl FaceIdx {
    /// Regular primary face.
    pub const REGULAR: Self = Self(0);

    /// Sentinel for built-in geometric glyphs (box drawing, blocks, braille, powerline).
    ///
    /// These glyphs are rasterized from cell dimensions, not from any font face.
    pub const BUILTIN: Self = Self(u16::MAX);

    /// Whether this index refers to a fallback font (index >= 4).
    pub fn is_fallback(self) -> bool {
        self.0 >= 4 && self != Self::BUILTIN
    }

    /// Convert to `usize` for array indexing.
    pub fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// Fallback index (offset from 4) for indexing into the fallback array.
    ///
    /// Returns `None` if this is a primary face.
    pub fn fallback_index(self) -> Option<usize> {
        if self.is_fallback() {
            Some(self.0 as usize - 4)
        } else {
            None
        }
    }
}

/// Cache key for rasterized glyphs — glyph-ID-based, not character-based.
///
/// The `size_q6` field encodes size in 26.6 fixed-point: `(size_px * 64.0).round() as u32`.
/// This avoids floating-point hashing while preserving sub-pixel size changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RasterKey {
    /// Glyph ID within the font face.
    pub glyph_id: u16,
    /// Which font face this glyph belongs to.
    pub face_idx: FaceIdx,
    /// Size in 26.6 fixed-point: `(size_px * 64.0).round() as u32`.
    pub size_q6: u32,
}

impl RasterKey {
    /// Construct a raster key from a resolved glyph and a 26.6 fixed-point size.
    pub fn from_resolved(resolved: ResolvedGlyph, size_q6: u32) -> Self {
        Self {
            glyph_id: resolved.glyph_id,
            face_idx: resolved.face_idx,
            size_q6,
        }
    }
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
    /// Which font face resolved this character.
    pub face_idx: FaceIdx,
    /// Whether synthetic style transformations are needed.
    #[allow(dead_code, reason = "synthetic bold/italic rendering in Section 6")]
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

/// Whether a character should be rendered as a built-in geometric glyph.
///
/// O(1) range match covering box drawing, block elements, braille patterns,
/// and powerline symbols. Lives here (not in `gpu::builtin_glyphs`) because
/// the font shaper needs it to skip built-in chars during run segmentation,
/// and the font module must not depend on the GPU module.
pub(crate) fn is_builtin(ch: char) -> bool {
    matches!(
        ch,
        '\u{2500}'..='\u{257F}'   // Box Drawing
        | '\u{2580}'..='\u{259F}' // Block Elements
        | '\u{2800}'..='\u{28FF}' // Braille Patterns
        | '\u{E0B0}'..='\u{E0B4}' // Powerline separators (solid + outline triangles)
        | '\u{E0B6}'              // Powerline left rounded separator
    )
}
