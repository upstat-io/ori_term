//! Unit tests for text types.

use crate::color::Color;

use super::{FontWeight, ShapedGlyph, ShapedText, TextAlign, TextMetrics, TextOverflow, TextStyle};

// --- TextStyle ---

#[test]
fn text_style_default() {
    let s = TextStyle::default();
    assert!(s.font_family.is_none());
    assert_eq!(s.size, 12.0);
    assert_eq!(s.weight, FontWeight::Regular);
    assert_eq!(s.color, Color::WHITE);
    assert_eq!(s.align, TextAlign::Left);
    assert_eq!(s.overflow, TextOverflow::Clip);
}

#[test]
fn text_style_new() {
    let s = TextStyle::new(16.0, Color::BLACK);
    assert_eq!(s.size, 16.0);
    assert_eq!(s.color, Color::BLACK);
    assert_eq!(s.weight, FontWeight::Regular);
}

#[test]
fn text_style_builder_chain() {
    let s = TextStyle::new(14.0, Color::WHITE)
        .with_weight(FontWeight::Bold)
        .with_align(TextAlign::Center)
        .with_overflow(TextOverflow::Ellipsis);

    assert_eq!(s.size, 14.0);
    assert_eq!(s.weight, FontWeight::Bold);
    assert_eq!(s.align, TextAlign::Center);
    assert_eq!(s.overflow, TextOverflow::Ellipsis);
}

// --- ShapedGlyph ---

#[test]
fn shaped_glyph_construction() {
    let g = ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        x_advance: 7.5,
        x_offset: 0.0,
        y_offset: 0.0,
    };
    assert_eq!(g.glyph_id, 42);
    assert_eq!(g.face_index, 0);
    assert_eq!(g.x_advance, 7.5);
}

// --- ShapedText ---

#[test]
fn shaped_text_empty() {
    let t = ShapedText::new(Vec::new(), 0.0, 0.0, 0.0);
    assert!(t.is_empty());
    assert_eq!(t.glyph_count(), 0);
    assert_eq!(t.width, 0.0);
}

#[test]
fn shaped_text_with_glyphs() {
    let glyphs = vec![
        ShapedGlyph {
            glyph_id: 10,
            face_index: 0,
            x_advance: 8.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 20,
            face_index: 0,
            x_advance: 8.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];
    let t = ShapedText::new(glyphs, 16.0, 20.0, 14.0);
    assert!(!t.is_empty());
    assert_eq!(t.glyph_count(), 2);
    assert_eq!(t.width, 16.0);
    assert_eq!(t.height, 20.0);
    assert_eq!(t.baseline, 14.0);
}

// --- TextMetrics ---

#[test]
fn text_metrics_single_line() {
    let m = TextMetrics {
        width: 50.0,
        height: 16.0,
        line_count: 1,
    };
    assert_eq!(m.width, 50.0);
    assert_eq!(m.height, 16.0);
    assert_eq!(m.line_count, 1);
}

#[test]
fn text_metrics_multi_line() {
    let m = TextMetrics {
        width: 100.0,
        height: 48.0,
        line_count: 3,
    };
    assert_eq!(m.line_count, 3);
    assert_eq!(m.height, 48.0);
}

// --- Enum defaults ---

#[test]
fn font_weight_default_is_regular() {
    assert_eq!(FontWeight::default(), FontWeight::Regular);
}

#[test]
fn text_align_default_is_left() {
    assert_eq!(TextAlign::default(), TextAlign::Left);
}

#[test]
fn text_overflow_default_is_clip() {
    assert_eq!(TextOverflow::default(), TextOverflow::Clip);
}

// --- Boundary value tests ---

#[test]
fn shaped_text_negative_baseline() {
    // Negative baseline should be stored as-is (no clamping).
    let t = ShapedText::new(Vec::new(), 0.0, 14.0, -5.0);
    assert_eq!(t.baseline, -5.0);
    assert!(t.is_empty());
}

#[test]
fn shaped_glyph_zero_advance() {
    // Zero-advance glyphs (e.g., combining marks) are valid.
    let g = ShapedGlyph {
        glyph_id: 100,
        face_index: 1,
        x_advance: 0.0,
        x_offset: 2.0,
        y_offset: -3.0,
    };
    assert_eq!(g.x_advance, 0.0);
    assert_eq!(g.x_offset, 2.0);
    assert_eq!(g.y_offset, -3.0);
}
