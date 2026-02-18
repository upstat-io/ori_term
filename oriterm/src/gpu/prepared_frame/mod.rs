//! Prepared frame output from the Prepare phase of the render pipeline.
//!
//! [`PreparedFrame`] holds four [`InstanceWriter`] buffers (backgrounds,
//! glyphs, color glyphs, cursors) plus metadata the Render phase needs to
//! upload and draw. The four buffers map to four draw calls in painter's
//! order: backgrounds → monochrome glyphs → color glyphs → cursors.

use oriterm_core::Rgb;

use super::frame_input::ViewportSize;
use super::instance_writer::InstanceWriter;

/// GPU-ready frame data produced by the Prepare phase.
///
/// Contains four instance buffers for the four rendering layers
/// (drawn in order: backgrounds → glyphs → color glyphs → cursors)
/// plus the clear color and total instance count for the Render phase.
pub struct PreparedFrame {
    /// Background rectangle instances (solid-color cell fills).
    pub backgrounds: InstanceWriter,
    /// Monochrome glyph instances (`R8Unorm` atlas, tinted by `fg_color`).
    pub glyphs: InstanceWriter,
    /// Color glyph instances (`Rgba8Unorm` atlas, rendered as-is).
    pub color_glyphs: InstanceWriter,
    /// Cursor instances (block, bar, underline shapes).
    pub cursors: InstanceWriter,
    /// Viewport pixel dimensions for uniform buffer update.
    pub viewport: ViewportSize,
    /// Window clear color (alpha-premultiplied).
    pub clear_color: [f64; 4],
}

impl PreparedFrame {
    /// Create an empty frame with the given clear color.
    pub fn new(viewport: ViewportSize, background: Rgb, opacity: f64) -> Self {
        Self {
            backgrounds: InstanceWriter::new(),
            glyphs: InstanceWriter::new(),
            color_glyphs: InstanceWriter::new(),
            cursors: InstanceWriter::new(),
            viewport,
            clear_color: rgb_to_clear(background, opacity),
        }
    }

    /// Create an empty frame pre-allocated for the given grid dimensions.
    ///
    /// `cols * rows` instances are reserved for backgrounds (one per cell),
    /// and the same for glyphs. Cursors are always small (typically 1–2).
    #[cfg(test)]
    pub fn with_capacity(
        viewport: ViewportSize,
        cols: usize,
        rows: usize,
        background: Rgb,
        opacity: f64,
    ) -> Self {
        let cells = cols * rows;
        Self {
            backgrounds: InstanceWriter::with_capacity(cells),
            glyphs: InstanceWriter::with_capacity(cells),
            color_glyphs: InstanceWriter::new(),
            cursors: InstanceWriter::with_capacity(4),
            viewport,
            clear_color: rgb_to_clear(background, opacity),
        }
    }

    /// Total instance count across all four buffers.
    #[allow(dead_code, reason = "frame management methods for later sections")]
    pub fn total_instances(&self) -> usize {
        self.backgrounds.len()
            + self.glyphs.len()
            + self.color_glyphs.len()
            + self.cursors.len()
    }

    /// Whether all four buffers are empty.
    #[allow(dead_code, reason = "frame management methods for later sections")]
    pub fn is_empty(&self) -> bool {
        self.backgrounds.is_empty()
            && self.glyphs.is_empty()
            && self.color_glyphs.is_empty()
            && self.cursors.is_empty()
    }

    /// Reset all buffers for the next frame, retaining allocated memory.
    pub fn clear(&mut self) {
        self.backgrounds.clear();
        self.glyphs.clear();
        self.color_glyphs.clear();
        self.cursors.clear();
    }

    /// Update the clear color (e.g. after a palette change).
    pub fn set_clear_color(&mut self, background: Rgb, opacity: f64) {
        self.clear_color = rgb_to_clear(background, opacity);
    }
}

/// Convert an `Rgb` + opacity to the `[f64; 4]` wgpu expects for clear color.
///
/// The color is premultiplied: each channel is scaled by opacity so the
/// compositor blends correctly with `PreMultiplied` alpha mode.
fn rgb_to_clear(c: Rgb, opacity: f64) -> [f64; 4] {
    [
        f64::from(c.r) / 255.0 * opacity,
        f64::from(c.g) / 255.0 * opacity,
        f64::from(c.b) / 255.0 * opacity,
        opacity,
    ]
}

#[cfg(test)]
mod tests;
