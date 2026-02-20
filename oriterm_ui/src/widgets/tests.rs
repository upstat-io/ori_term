use crate::text::{ShapedGlyph, ShapedText, TextMetrics, TextStyle};

use super::text_measurer::TextMeasurer;

/// Mock text measurer for widget tests.
///
/// Uses fixed metrics: each character is `char_width` pixels wide,
/// line height is `line_height` pixels, baseline at 80% of line height.
pub struct MockMeasurer {
    pub char_width: f32,
    pub line_height: f32,
}

impl MockMeasurer {
    /// Standard mock: 8px per char, 16px line height (const for static usage).
    pub const STANDARD: Self = Self {
        char_width: 8.0,
        line_height: 16.0,
    };

    /// Standard mock: 8px per char, 16px line height.
    pub fn new() -> Self {
        Self::STANDARD
    }
}

impl TextMeasurer for MockMeasurer {
    fn measure(&self, text: &str, _style: &TextStyle, max_width: f32) -> TextMetrics {
        let full_width = self.char_width * text.len() as f32;
        if max_width.is_finite() && full_width > max_width {
            // Simple wrapping: number of lines = ceil(full_width / max_width).
            let line_count = (full_width / max_width).ceil() as u32;
            TextMetrics {
                width: max_width,
                height: self.line_height * line_count as f32,
                line_count,
            }
        } else {
            TextMetrics {
                width: full_width,
                height: self.line_height,
                line_count: 1,
            }
        }
    }

    fn shape(&self, text: &str, _style: &TextStyle, _max_width: f32) -> ShapedText {
        let glyphs: Vec<ShapedGlyph> = text
            .chars()
            .enumerate()
            .map(|(i, _)| ShapedGlyph {
                glyph_id: (i as u16) + 1,
                face_index: 0,
                x_advance: self.char_width,
                x_offset: 0.0,
                y_offset: 0.0,
            })
            .collect();
        let width = self.char_width * text.len() as f32;
        let baseline = self.line_height * 0.8;
        ShapedText::new(glyphs, width, self.line_height, baseline)
    }
}

#[test]
fn mock_measurer_basic() {
    let m = MockMeasurer::new();
    let style = TextStyle::default();
    let metrics = m.measure("hello", &style, f32::INFINITY);
    assert_eq!(metrics.width, 40.0); // 5 chars * 8px
    assert_eq!(metrics.height, 16.0);
    assert_eq!(metrics.line_count, 1);
}

#[test]
fn mock_measurer_wrapping() {
    let m = MockMeasurer::new();
    let style = TextStyle::default();
    // "hello world" = 11 chars * 8px = 88px, max_width = 50px → 2 lines.
    let metrics = m.measure("hello world", &style, 50.0);
    assert_eq!(metrics.width, 50.0);
    assert_eq!(metrics.line_count, 2);
    assert_eq!(metrics.height, 32.0);
}

#[test]
fn mock_measurer_shape() {
    let m = MockMeasurer::new();
    let style = TextStyle::default();
    let shaped = m.shape("abc", &style, f32::INFINITY);
    assert_eq!(shaped.glyph_count(), 3);
    assert_eq!(shaped.width, 24.0);
    assert_eq!(shaped.height, 16.0);
}

#[test]
fn widget_ids_are_unique() {
    use super::Widget;
    use super::button::ButtonWidget;
    use super::checkbox::CheckboxWidget;
    use super::label::LabelWidget;

    let a = ButtonWidget::new("A");
    let b = ButtonWidget::new("B");
    let c = LabelWidget::new("C");
    let d = CheckboxWidget::new("D");

    // All IDs must be distinct.
    let ids = [a.id(), b.id(), c.id(), d.id()];
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert_ne!(ids[i], ids[j], "widget IDs must be unique");
        }
    }
}

#[test]
fn widget_response_equality() {
    use super::{WidgetAction, WidgetResponse};
    use crate::widget_id::WidgetId;

    let id = WidgetId::next();
    let r1 = WidgetResponse::redraw().with_action(WidgetAction::Clicked(id));
    let r2 = WidgetResponse::redraw().with_action(WidgetAction::Clicked(id));
    assert_eq!(r1, r2);

    let r3 = WidgetResponse::handled();
    assert_ne!(r1, r3);

    let r4 = WidgetResponse::ignored();
    let r5 = WidgetResponse::ignored();
    assert_eq!(r4, r5);
}

#[test]
fn mock_measurer_empty_text() {
    let m = MockMeasurer::new();
    let style = TextStyle::default();
    let metrics = m.measure("", &style, f32::INFINITY);
    assert_eq!(metrics.width, 0.0);
    assert_eq!(metrics.height, 16.0);
    assert_eq!(metrics.line_count, 1);
}
