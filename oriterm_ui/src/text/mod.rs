//! Text types for UI rendering.
//!
//! Provides style descriptors, shaped glyph output, and measurement results
//! for non-grid UI text (labels, tab titles, overlays). These types are
//! GPU-agnostic — shaping and rasterization live in the `oriterm` crate.

use crate::color::Color;

/// Font weight for UI text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontWeight {
    /// Normal weight (CSS 400).
    #[default]
    Regular,
    /// Bold weight (CSS 700).
    Bold,
}

/// Horizontal text alignment within a bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextAlign {
    /// Left-aligned (default for LTR text).
    #[default]
    Left,
    /// Horizontally centered.
    Center,
    /// Right-aligned.
    Right,
}

/// How text that exceeds its container width is handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextOverflow {
    /// Clip at the container edge (no visual indicator).
    #[default]
    Clip,
    /// Truncate with ellipsis (U+2026 `...`).
    Ellipsis,
    /// Wrap at word boundaries to the next line.
    Wrap,
}

/// Style descriptor for UI text rendering.
///
/// Input to the shaping pipeline. The shaper uses these parameters to select
/// the correct font face and size, then produces a [`ShapedText`] block.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// Font family name. `None` uses the default UI font.
    pub font_family: Option<String>,
    /// Font size in points.
    pub size: f32,
    /// Font weight.
    pub weight: FontWeight,
    /// Text color.
    pub color: Color,
    /// Horizontal alignment within the layout box.
    pub align: TextAlign,
    /// Overflow handling when text exceeds available width.
    pub overflow: TextOverflow,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: None,
            size: 12.0,
            weight: FontWeight::Regular,
            color: Color::WHITE,
            align: TextAlign::Left,
            overflow: TextOverflow::Clip,
        }
    }
}

impl TextStyle {
    /// Create a text style with the given size and color, using defaults for
    /// all other fields.
    pub fn new(size: f32, color: Color) -> Self {
        Self {
            size,
            color,
            ..Self::default()
        }
    }

    /// Set the font weight.
    #[must_use]
    pub fn with_weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    /// Set the text alignment.
    #[must_use]
    pub fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Set the overflow behavior.
    #[must_use]
    pub fn with_overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }
}

/// A shaped glyph for UI text rendering.
///
/// Output of the shaping pipeline, input to the GPU renderer. Uses pixel-based
/// `x_advance` positioning instead of grid column mapping — suitable for
/// proportional and variable-width fonts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapedGlyph {
    /// Glyph ID within the font face (0 for advance-only entries like spaces).
    pub glyph_id: u16,
    /// Font face index as a raw `u16`. Avoids dependency on `oriterm`'s
    /// `FaceIdx` type — the renderer maps this back.
    pub face_index: u16,
    /// Horizontal advance in pixels (cursor moves right by this amount).
    pub x_advance: f32,
    /// Shaper X offset from the glyph origin in pixels.
    pub x_offset: f32,
    /// Shaper Y offset from the baseline in pixels.
    pub y_offset: f32,
}

/// Pre-shaped text block ready for rendering.
///
/// Contains the shaped glyph sequence and layout metrics. Produced by the
/// shaper in the `oriterm` crate, consumed by the draw list converter to
/// emit GPU glyph instances.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedText {
    /// Shaped glyphs in visual order.
    pub glyphs: Vec<ShapedGlyph>,
    /// Total advance width in pixels.
    pub width: f32,
    /// Line height in pixels.
    pub height: f32,
    /// Baseline offset from the top of the text block in pixels.
    pub baseline: f32,
}

impl ShapedText {
    /// Create a shaped text block from pre-computed data.
    pub fn new(glyphs: Vec<ShapedGlyph>, width: f32, height: f32, baseline: f32) -> Self {
        Self {
            glyphs,
            width,
            height,
            baseline,
        }
    }

    /// Whether this text block contains no glyphs.
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// Number of shaped glyphs.
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }
}

/// Text measurement result — dimensions without glyph data.
///
/// Lighter than [`ShapedText`] when only layout dimensions are needed
/// (e.g. for hit testing or container sizing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    /// Total text width in pixels.
    pub width: f32,
    /// Total text height in pixels.
    pub height: f32,
    /// Number of lines (1 for single-line text, more with wrapping).
    pub line_count: u32,
}

#[cfg(test)]
mod tests;
