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

#[allow(unused_imports, reason = "wired by config system in Section 13")]
pub(crate) use collection::parse_hex_range;
pub use collection::{FontCollection, FontSet, RasterizedGlyph, size_key};
pub use shaper::{ShapedGlyph, ShapingRun, build_col_glyph_map, prepare_line, shape_prepared_runs};

/// Cell dimensions in pixels, derived from the font metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellMetrics {
    /// Cell width in pixels (fractional for subpixel accuracy).
    pub width: f32,
    /// Cell height in pixels (fractional for subpixel accuracy).
    pub height: f32,
    /// Distance from cell top to text baseline, in pixels.
    pub baseline: f32,
    /// Distance from baseline to underline stroke, in pixels.
    ///
    /// Positive values are below the baseline (typical). Extracted from the
    /// font's `post` table via swash `underline_offset`, negated so that a
    /// larger value means further below baseline.
    pub underline_offset: f32,
    /// Thickness of underline and strikethrough strokes, in pixels.
    ///
    /// Extracted from the font's OS/2 `stroke_size` via swash. Clamped to
    /// a minimum of 1.0 to ensure visibility at small sizes.
    pub stroke_size: f32,
    /// Distance from baseline to strikeout stroke, in pixels.
    ///
    /// Positive values are above the baseline (typical). Extracted from the
    /// font's OS/2 table via swash `strikeout_offset`.
    pub strikeout_offset: f32,
}

impl CellMetrics {
    /// Create cell metrics from font-derived dimensions.
    ///
    /// # Panics
    ///
    /// Panics in debug mode if any dimension is non-positive or non-finite.
    #[expect(
        clippy::too_many_arguments,
        reason = "struct field initialization from independent font metrics"
    )]
    pub fn new(
        width: f32,
        height: f32,
        baseline: f32,
        underline_offset: f32,
        stroke_size: f32,
        strikeout_offset: f32,
    ) -> Self {
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
            underline_offset,
            stroke_size: stroke_size.max(1.0),
            strikeout_offset,
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

/// Glyph hinting mode — controls grid-fitting of outlines to pixel boundaries.
///
/// Hinting snaps glyph outlines to the pixel grid for sharper rendering at
/// small sizes. On high-DPI displays (2x+) the extra pixels make hinting
/// unnecessary, so disabling it preserves outline shape fidelity.
///
/// swash only supports a boolean hint flag — no "light" mode — so two
/// variants is the honest representation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum HintingMode {
    /// Full hinting (snaps to pixel grid). Crispest text on non-high-DPI.
    #[default]
    Full,
    /// No hinting (preserves outline shape). Best on high-DPI (2x+) where
    /// subpixel precision isn't needed for sharpness.
    None,
}

impl HintingMode {
    /// Convert to the boolean flag expected by swash's `ScalerBuilder::hint()`.
    pub fn hint_flag(self) -> bool {
        matches!(self, Self::Full)
    }

    /// Auto-detect hinting mode from display scale factor.
    ///
    /// `scale_factor < 2.0` → `Full` (non-high-DPI needs grid-fitting).
    /// `scale_factor >= 2.0` → `None` (Retina/4K has enough pixels).
    pub fn from_scale_factor(scale_factor: f64) -> Self {
        if scale_factor < 2.0 {
            Self::Full
        } else {
            Self::None
        }
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
    SubpixelRgb,
    /// 4 bytes/pixel RGBA per-channel subpixel coverage (B-G-R order).
    #[allow(dead_code, reason = "BGR display support — config in Section 13")]
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

    /// Whether this format is a subpixel variant.
    pub fn is_subpixel(self) -> bool {
        matches!(self, Self::SubpixelRgb | Self::SubpixelBgr)
    }
}

/// LCD subpixel rendering mode.
///
/// Controls whether glyphs are rasterized with per-channel coverage for
/// ~3x effective horizontal resolution on LCD displays. Automatically
/// disabled on high-DPI (scale >= 2.0) where subpixels are invisible.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SubpixelMode {
    /// RGB subpixel order (vast majority of displays).
    #[default]
    Rgb,
    /// BGR subpixel order (rare panels).
    #[allow(dead_code, reason = "BGR display support — config in Section 13")]
    Bgr,
    /// Disabled — grayscale alpha rendering only.
    None,
}

impl SubpixelMode {
    /// Auto-detect subpixel mode from display scale factor.
    ///
    /// `scale_factor < 2.0` → `Rgb` (subpixels visible on non-HiDPI).
    /// `scale_factor >= 2.0` → `None` (Retina/4K — subpixels invisible).
    pub fn from_scale_factor(scale_factor: f64) -> Self {
        if scale_factor < 2.0 {
            Self::Rgb
        } else {
            Self::None
        }
    }

    /// Auto-detect subpixel mode considering both scale and background opacity.
    ///
    /// Subpixel rendering over transparent backgrounds produces visible color
    /// fringing because the per-channel blending assumes an opaque background.
    /// When `opacity < 1.0`, forces grayscale regardless of scale factor.
    #[allow(
        dead_code,
        reason = "wired when opacity becomes configurable — Section 13"
    )]
    pub fn for_display(scale_factor: f64, opacity: f64) -> Self {
        if opacity < 1.0 {
            Self::None
        } else {
            Self::from_scale_factor(scale_factor)
        }
    }

    /// Convert to the [`GlyphFormat`] used for rasterization.
    ///
    /// Returns `Alpha` when subpixel is disabled, otherwise the matching
    /// subpixel format.
    pub fn glyph_format(self) -> GlyphFormat {
        match self {
            Self::Rgb => GlyphFormat::SubpixelRgb,
            Self::Bgr => GlyphFormat::SubpixelBgr,
            Self::None => GlyphFormat::Alpha,
        }
    }

    /// Whether subpixel rendering is enabled.
    #[allow(dead_code, reason = "convenience predicate — config in Section 13")]
    pub fn is_enabled(self) -> bool {
        !matches!(self, Self::None)
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

/// Distinguishes terminal grid fonts from UI fonts in atlas cache keys.
///
/// Terminal and UI text may use different font collections at different sizes.
/// Including the realm in [`RasterKey`] ensures glyphs from different
/// collections never collide in the atlas cache, even if they share the
/// same glyph ID and face index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FontRealm {
    /// Terminal grid text (monospace).
    #[default]
    Terminal = 0,
    /// UI overlay text (tab bar, labels, dialogs).
    #[allow(
        dead_code,
        reason = "used by draw_list_convert for UI text in Section 07.2"
    )]
    Ui = 1,
}

/// Cache key for rasterized glyphs — glyph-ID-based, not character-based.
///
/// The `size_q6` field encodes size in 26.6 fixed-point: `(size_px * 64.0).round() as u32`.
/// This avoids floating-point hashing while preserving sub-pixel size changes.
///
/// Includes [`SyntheticFlags`] so that emboldened/skewed glyphs are cached
/// separately from their unsynthesized counterparts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RasterKey {
    /// Glyph ID within the font face.
    pub glyph_id: u16,
    /// Which font face this glyph belongs to.
    pub face_idx: FaceIdx,
    /// Size in 26.6 fixed-point: `(size_px * 64.0).round() as u32`.
    pub size_q6: u32,
    /// Synthetic transformations applied at rasterization time.
    pub synthetic: SyntheticFlags,
    /// Whether this glyph was rasterized with hinting enabled.
    pub hinted: bool,
    /// Horizontal subpixel phase (0–3). See [`subpx_bin`].
    pub subpx_x: u8,
    /// Which font realm this glyph belongs to (terminal vs UI).
    pub font_realm: FontRealm,
}

impl RasterKey {
    /// Construct a raster key from a resolved glyph, size, hinting, and subpixel phase.
    ///
    /// Defaults to [`FontRealm::Terminal`]. Use [`with_realm`](Self::with_realm)
    /// for UI text glyphs.
    pub fn from_resolved(resolved: ResolvedGlyph, size_q6: u32, hinted: bool, subpx_x: u8) -> Self {
        Self {
            glyph_id: resolved.glyph_id,
            face_idx: resolved.face_idx,
            size_q6,
            synthetic: resolved.synthetic,
            hinted,
            subpx_x,
            font_realm: FontRealm::Terminal,
        }
    }

    /// Return a copy with the given font realm.
    #[must_use]
    #[allow(
        dead_code,
        reason = "used by draw_list_convert for UI text in Section 07.2"
    )]
    pub fn with_realm(mut self, realm: FontRealm) -> Self {
        self.font_realm = realm;
        self
    }
}

/// Quantize a fractional pixel offset to one of 4 horizontal phases.
///
/// Phases: 0 → 0.00, 1 → 0.25, 2 → 0.50, 3 → 0.75.
/// Grid text at integer boundaries always returns 0.
pub fn subpx_bin(offset: f32) -> u8 {
    let fract = offset.fract().abs();
    // 4 bins centered at 0.00, 0.25, 0.50, 0.75 with boundaries at
    // 0.125, 0.375, 0.625, 0.875.
    match (fract * 4.0 + 0.5) as u8 {
        1 => 1,
        2 => 2,
        3 => 3,
        _ => 0, // 0, 4+ (0.875+ wraps to next integer) → phase 0
    }
}

/// Convert a subpixel bin (0–3) back to a fractional offset for rasterization.
pub fn subpx_offset(bin: u8) -> f32 {
    match bin {
        1 => 0.25,
        2 => 0.50,
        3 => 0.75,
        _ => 0.0, // 0 and out-of-range
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

#[cfg(test)]
mod tests;
