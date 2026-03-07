//! Unit tests for draw primitives.

use crate::color::Color;
use crate::geometry::Logical;

type Point = crate::geometry::Point<Logical>;
type Rect = crate::geometry::Rect<Logical>;

use crate::text::{ShapedGlyph, ShapedText};

use super::{Border, DrawCommand, DrawList, RectStyle, Shadow};

// RectStyle

#[test]
fn rect_style_default_is_invisible() {
    let s = RectStyle::default();
    assert!(s.fill.is_none());
    assert!(s.border.is_none());
    assert_eq!(s.corner_radius, [0.0; 4]);
    assert!(s.shadow.is_none());
    assert!(s.gradient.is_none());
}

#[test]
fn rect_style_filled() {
    let s = RectStyle::filled(Color::WHITE);
    assert_eq!(s.fill, Some(Color::WHITE));
}

#[test]
fn rect_style_builder_chain() {
    let s = RectStyle::filled(Color::BLACK)
        .with_border(2.0, Color::WHITE)
        .with_radius(8.0)
        .with_shadow(Shadow {
            offset_x: 0.0,
            offset_y: 4.0,
            blur_radius: 8.0,
            spread: 0.0,
            color: Color::rgba(0.0, 0.0, 0.0, 0.5),
        });

    assert_eq!(s.fill, Some(Color::BLACK));
    assert_eq!(
        s.border,
        Some(Border {
            width: 2.0,
            color: Color::WHITE,
        })
    );
    assert_eq!(s.corner_radius, [8.0; 4]);
    assert!(s.shadow.is_some());
}

#[test]
fn rect_style_per_corner_radius() {
    let s = RectStyle::filled(Color::BLACK).with_per_corner_radius(1.0, 2.0, 3.0, 4.0);
    assert_eq!(s.corner_radius, [1.0, 2.0, 3.0, 4.0]);
}

// DrawList

#[test]
fn draw_list_new_is_empty() {
    let dl = DrawList::new();
    assert!(dl.is_empty());
    assert_eq!(dl.len(), 0);
    assert!(dl.commands().is_empty());
}

#[test]
fn draw_list_default_is_empty() {
    let dl = DrawList::default();
    assert!(dl.is_empty());
}

#[test]
fn push_rect_adds_command() {
    let mut dl = DrawList::new();
    dl.push_rect(
        Rect::new(0.0, 0.0, 100.0, 50.0),
        RectStyle::filled(Color::WHITE),
    );

    assert_eq!(dl.len(), 1);
    assert!(!dl.is_empty());

    match &dl.commands()[0] {
        DrawCommand::Rect { rect, style } => {
            assert_eq!(rect.width(), 100.0);
            assert_eq!(style.fill, Some(Color::WHITE));
        }
        other => panic!("expected Rect, got {other:?}"),
    }
}

#[test]
fn push_line_adds_command() {
    let mut dl = DrawList::new();
    dl.push_line(
        Point::new(0.0, 0.0),
        Point::new(100.0, 100.0),
        2.0,
        Color::BLACK,
    );

    assert_eq!(dl.len(), 1);
    match &dl.commands()[0] {
        DrawCommand::Line {
            from,
            to,
            width,
            color,
        } => {
            assert_eq!(*from, Point::new(0.0, 0.0));
            assert_eq!(*to, Point::new(100.0, 100.0));
            assert_eq!(*width, 2.0);
            assert_eq!(*color, Color::BLACK);
        }
        other => panic!("expected Line, got {other:?}"),
    }
}

#[test]
fn push_image_adds_command() {
    let mut dl = DrawList::new();
    dl.push_image(Rect::new(10.0, 20.0, 64.0, 64.0), 42, [0.0, 0.0, 1.0, 1.0]);

    assert_eq!(dl.len(), 1);
    match &dl.commands()[0] {
        DrawCommand::Image {
            rect,
            texture_id,
            uv,
        } => {
            assert_eq!(rect.x(), 10.0);
            assert_eq!(*texture_id, 42);
            assert_eq!(*uv, [0.0, 0.0, 1.0, 1.0]);
        }
        other => panic!("expected Image, got {other:?}"),
    }
}

#[test]
fn clip_push_pop_balanced() {
    let mut dl = DrawList::new();
    dl.push_clip(Rect::new(0.0, 0.0, 100.0, 100.0));
    dl.push_rect(
        Rect::new(10.0, 10.0, 50.0, 50.0),
        RectStyle::filled(Color::WHITE),
    );
    dl.pop_clip();

    assert_eq!(dl.len(), 3);
    assert!(matches!(dl.commands()[0], DrawCommand::PushClip { .. }));
    assert!(matches!(dl.commands()[1], DrawCommand::Rect { .. }));
    assert!(matches!(dl.commands()[2], DrawCommand::PopClip));
}

#[test]
#[should_panic(expected = "pop_clip called with empty clip stack")]
fn pop_clip_on_empty_panics() {
    let mut dl = DrawList::new();
    dl.pop_clip();
}

#[test]
fn nested_clips() {
    let mut dl = DrawList::new();
    dl.push_clip(Rect::new(0.0, 0.0, 200.0, 200.0));
    dl.push_clip(Rect::new(10.0, 10.0, 100.0, 100.0));
    dl.pop_clip();
    dl.pop_clip();

    assert_eq!(dl.len(), 4);
}

#[test]
fn clear_resets_everything() {
    let mut dl = DrawList::new();
    dl.push_rect(Rect::new(0.0, 0.0, 10.0, 10.0), RectStyle::default());
    dl.push_clip(Rect::new(0.0, 0.0, 100.0, 100.0));
    dl.pop_clip();

    dl.clear();
    assert!(dl.is_empty());
    assert_eq!(dl.len(), 0);
}

#[test]
fn multiple_commands_preserve_order() {
    let mut dl = DrawList::new();
    dl.push_rect(
        Rect::new(0.0, 0.0, 10.0, 10.0),
        RectStyle::filled(Color::BLACK),
    );
    dl.push_line(
        Point::new(0.0, 0.0),
        Point::new(10.0, 10.0),
        1.0,
        Color::WHITE,
    );
    dl.push_rect(
        Rect::new(20.0, 20.0, 30.0, 30.0),
        RectStyle::filled(Color::WHITE),
    );

    assert_eq!(dl.len(), 3);
    assert!(matches!(dl.commands()[0], DrawCommand::Rect { .. }));
    assert!(matches!(dl.commands()[1], DrawCommand::Line { .. }));
    assert!(matches!(dl.commands()[2], DrawCommand::Rect { .. }));
}

// Layer stack

#[test]
fn layer_push_pop_balanced() {
    let mut dl = DrawList::new();
    dl.push_layer(Color::WHITE);
    dl.push_rect(
        Rect::new(0.0, 0.0, 100.0, 50.0),
        RectStyle::filled(Color::WHITE),
    );
    dl.pop_layer();

    assert_eq!(dl.len(), 3);
    assert!(matches!(dl.commands()[0], DrawCommand::PushLayer { .. }));
    assert!(matches!(dl.commands()[1], DrawCommand::Rect { .. }));
    assert!(matches!(dl.commands()[2], DrawCommand::PopLayer));
}

#[test]
#[should_panic(expected = "pop_layer called with empty layer stack")]
fn pop_layer_on_empty_panics() {
    let mut dl = DrawList::new();
    dl.pop_layer();
}

#[test]
fn current_layer_bg_none_when_empty() {
    let dl = DrawList::new();
    assert!(dl.current_layer_bg().is_none());
}

#[test]
fn current_layer_bg_returns_pushed_color() {
    let mut dl = DrawList::new();
    dl.push_layer(Color::rgba(0.2, 0.3, 0.4, 1.0));
    assert_eq!(
        dl.current_layer_bg(),
        Some(&Color::rgba(0.2, 0.3, 0.4, 1.0)),
    );
}

#[test]
fn nested_layers_inner_overrides_outer() {
    let mut dl = DrawList::new();
    dl.push_layer(Color::BLACK);
    assert_eq!(dl.current_layer_bg(), Some(&Color::BLACK));

    dl.push_layer(Color::WHITE);
    assert_eq!(dl.current_layer_bg(), Some(&Color::WHITE));

    dl.pop_layer();
    assert_eq!(dl.current_layer_bg(), Some(&Color::BLACK));

    dl.pop_layer();
    assert!(dl.current_layer_bg().is_none());
}

/// Helper: build a simple shaped text for layer stack tests.
fn test_shaped_text() -> ShapedText {
    ShapedText::new(
        vec![ShapedGlyph {
            glyph_id: 42,
            face_index: 0,
            synthetic: 0,
            x_advance: 7.0,
            x_offset: 0.0,
            y_offset: 0.0,
        }],
        7.0,
        14.0,
        12.0,
    )
}

#[test]
fn push_text_captures_layer_bg() {
    let mut dl = DrawList::new();
    let bg = Color::rgba(0.2, 0.2, 0.2, 1.0);
    dl.push_layer(bg);
    dl.push_text(Point::new(0.0, 0.0), test_shaped_text(), Color::WHITE);
    dl.pop_layer();

    match &dl.commands()[1] {
        DrawCommand::Text { bg_hint, .. } => {
            assert_eq!(*bg_hint, Some(bg));
        }
        other => panic!("expected Text, got {other:?}"),
    }
}

#[test]
fn push_text_without_layer_has_no_bg() {
    let mut dl = DrawList::new();
    dl.push_text(Point::new(0.0, 0.0), test_shaped_text(), Color::WHITE);

    match &dl.commands()[0] {
        DrawCommand::Text { bg_hint, .. } => {
            assert_eq!(*bg_hint, None);
        }
        other => panic!("expected Text, got {other:?}"),
    }
}

#[test]
fn push_text_captures_innermost_layer() {
    let mut dl = DrawList::new();
    dl.push_layer(Color::BLACK);
    dl.push_layer(Color::WHITE);
    dl.push_text(Point::new(0.0, 0.0), test_shaped_text(), Color::BLACK);
    dl.pop_layer();
    dl.pop_layer();

    match &dl.commands()[2] {
        DrawCommand::Text { bg_hint, .. } => {
            assert_eq!(*bg_hint, Some(Color::WHITE));
        }
        other => panic!("expected Text, got {other:?}"),
    }
}

#[test]
fn clear_resets_layer_stack() {
    let mut dl = DrawList::new();
    dl.push_layer(Color::WHITE);
    dl.clear();

    assert!(dl.current_layer_bg().is_none());
    assert!(dl.is_empty());
}
