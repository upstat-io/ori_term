//! Text measurement abstraction for widget layout.
//!
//! Widgets need to measure text during layout but must not depend on the
//! concrete font/shaping implementation. The `TextMeasurer` trait provides
//! that indirection — the `oriterm` crate supplies the real implementation,
//! while tests use a `MockMeasurer`.

use crate::text::{ShapedText, TextMetrics, TextStyle};

/// Measures and shapes text for widget layout and rendering.
///
/// Passed to widgets via [`super::LayoutCtx`] and [`super::DrawCtx`] so
/// they can compute text dimensions without depending on the font stack.
pub trait TextMeasurer {
    /// Measures text dimensions without producing glyph data.
    ///
    /// Returns layout metrics for the given `text` rendered in `style`,
    /// constrained to `max_width` pixels. If `max_width` is `f32::INFINITY`,
    /// no width constraint is applied.
    fn measure(&self, text: &str, style: &TextStyle, max_width: f32) -> TextMetrics;

    /// Shapes text into positioned glyphs for rendering.
    ///
    /// Returns a [`ShapedText`] block suitable for passing to
    /// [`DrawList::push_text`](crate::draw::DrawList::push_text).
    fn shape(&self, text: &str, style: &TextStyle, max_width: f32) -> ShapedText;
}
