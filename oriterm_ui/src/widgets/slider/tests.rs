use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, Key, KeyEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::BoxContent;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{EventCtx, LayoutCtx, Widget, WidgetResponse};

use super::{SliderStyle, SliderWidget};

static MEASURER: MockMeasurer = MockMeasurer::STANDARD;

fn slider_ctx() -> EventCtx<'static> {
    EventCtx {
        measurer: &MEASURER,
        bounds: Rect::new(0.0, 0.0, 200.0, 16.0),
        is_focused: true,
        focused_widget: None,
    }
}

fn mouse_down(x: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(x, 8.0),
        modifiers: Modifiers::NONE,
    }
}

fn mouse_move(x: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Move,
        pos: Point::new(x, 8.0),
        modifiers: Modifiers::NONE,
    }
}

fn mouse_up(x: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(x, 8.0),
        modifiers: Modifiers::NONE,
    }
}

fn key_event(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn default_state() {
    let s = SliderWidget::new();
    assert_eq!(s.value(), 0.0);
    assert_eq!(s.min(), 0.0);
    assert_eq!(s.max(), 1.0);
    assert!(!s.is_disabled());
    assert!(!s.is_hovered());
    assert!(!s.is_dragging());
    assert!(s.is_focusable());
}

#[test]
fn with_range_and_value() {
    let s = SliderWidget::new().with_range(10.0, 100.0).with_value(50.0);
    assert_eq!(s.value(), 50.0);
    assert_eq!(s.min(), 10.0);
    assert_eq!(s.max(), 100.0);
}

#[test]
fn value_clamped_to_range() {
    let s = SliderWidget::new().with_range(0.0, 10.0).with_value(20.0);
    assert_eq!(s.value(), 10.0);
}

#[test]
fn layout_dimensions() {
    let s = SliderWidget::new();
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = s.layout(&ctx);
    let style = SliderStyle::default();

    if let BoxContent::Leaf {
        intrinsic_width,
        intrinsic_height,
    } = &layout.content
    {
        assert_eq!(*intrinsic_width, style.width);
        assert_eq!(*intrinsic_height, style.thumb_size);
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn drag_changes_value() {
    let mut s = SliderWidget::new().with_range(0.0, 100.0).with_step(1.0);
    let ctx = slider_ctx();

    // Click at center of track (x=100 of 200px bounds).
    s.handle_mouse(&mouse_down(100.0), &ctx);
    assert!(s.is_dragging());
    // Value should be near 50 (center of 0..100 range).
    assert!((s.value() - 50.0).abs() < 2.0);

    // Drag to right edge.
    s.handle_mouse(&mouse_move(192.0), &ctx);
    assert!((s.value() - 100.0).abs() < 2.0);

    // Release.
    s.handle_mouse(&mouse_up(192.0), &ctx);
    assert!(!s.is_dragging());
}

#[test]
fn arrow_keys_step() {
    let mut s = SliderWidget::new()
        .with_range(0.0, 10.0)
        .with_step(1.0)
        .with_value(5.0);
    let ctx = slider_ctx();

    let r = s.handle_key(key_event(Key::ArrowRight), &ctx);
    assert_eq!(s.value(), 6.0);
    assert!(r.action.is_some());

    let r = s.handle_key(key_event(Key::ArrowLeft), &ctx);
    assert_eq!(s.value(), 5.0);
    assert!(r.action.is_some());
}

#[test]
fn home_end_keys() {
    let mut s = SliderWidget::new().with_range(0.0, 10.0).with_value(5.0);
    let ctx = slider_ctx();

    s.handle_key(key_event(Key::Home), &ctx);
    assert_eq!(s.value(), 0.0);

    s.handle_key(key_event(Key::End), &ctx);
    assert_eq!(s.value(), 10.0);
}

#[test]
fn at_min_left_arrow_no_change() {
    let mut s = SliderWidget::new()
        .with_range(0.0, 10.0)
        .with_step(1.0)
        .with_value(0.0);
    let ctx = slider_ctx();

    let r = s.handle_key(key_event(Key::ArrowLeft), &ctx);
    assert_eq!(s.value(), 0.0);
    // No action when value doesn't change.
    assert!(r.action.is_none());
}

#[test]
fn at_max_right_arrow_no_change() {
    let mut s = SliderWidget::new()
        .with_range(0.0, 10.0)
        .with_step(1.0)
        .with_value(10.0);
    let ctx = slider_ctx();

    let r = s.handle_key(key_event(Key::ArrowRight), &ctx);
    assert_eq!(s.value(), 10.0);
    assert!(r.action.is_none());
}

#[test]
fn disabled_ignores() {
    let mut s = SliderWidget::new().with_disabled(true);
    let ctx = slider_ctx();

    assert!(!s.is_focusable());

    let r = s.handle_mouse(&mouse_down(100.0), &ctx);
    assert_eq!(r, WidgetResponse::ignored());

    let r = s.handle_key(key_event(Key::ArrowRight), &ctx);
    assert_eq!(r, WidgetResponse::ignored());

    let r = s.handle_hover(HoverEvent::Enter, &ctx);
    assert_eq!(r, WidgetResponse::ignored());
}

#[test]
fn hover_transitions() {
    let mut s = SliderWidget::new();
    let ctx = slider_ctx();

    s.handle_hover(HoverEvent::Enter, &ctx);
    assert!(s.is_hovered());

    s.handle_hover(HoverEvent::Leave, &ctx);
    assert!(!s.is_hovered());
}

#[test]
fn drag_beyond_left_clamps_to_min() {
    let mut s = SliderWidget::new().with_range(0.0, 100.0).with_step(1.0);
    let ctx = slider_ctx();

    s.handle_mouse(&mouse_down(100.0), &ctx);
    // Drag far to the left (negative X).
    s.handle_mouse(&mouse_move(-50.0), &ctx);
    assert_eq!(s.value(), 0.0);
}

#[test]
fn drag_beyond_right_clamps_to_max() {
    let mut s = SliderWidget::new().with_range(0.0, 100.0).with_step(1.0);
    let ctx = slider_ctx();

    s.handle_mouse(&mouse_down(100.0), &ctx);
    // Drag far to the right.
    s.handle_mouse(&mouse_move(500.0), &ctx);
    assert_eq!(s.value(), 100.0);
}

#[test]
fn leave_during_drag_clears_hover_not_dragging() {
    let mut s = SliderWidget::new().with_range(0.0, 100.0);
    let ctx = slider_ctx();

    s.handle_hover(HoverEvent::Enter, &ctx);
    s.handle_mouse(&mouse_down(100.0), &ctx);
    assert!(s.is_dragging());
    assert!(s.is_hovered());

    // Mouse leaves — hover clears but drag continues (capture semantics).
    s.handle_hover(HoverEvent::Leave, &ctx);
    assert!(!s.is_hovered());
    // Drag should still be active (would be released on mouse up).
    // Note: current impl doesn't clear dragging on leave, which is correct.
}

#[test]
fn min_equals_max_returns_min() {
    let s = SliderWidget::new().with_range(5.0, 5.0).with_value(5.0);
    assert_eq!(s.value(), 5.0);
}

#[test]
fn arrow_up_down_also_step() {
    let mut s = SliderWidget::new()
        .with_range(0.0, 10.0)
        .with_step(1.0)
        .with_value(5.0);
    let ctx = slider_ctx();

    s.handle_key(key_event(Key::ArrowUp), &ctx);
    assert_eq!(s.value(), 6.0);

    s.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(s.value(), 5.0);
}

#[test]
fn right_click_ignored() {
    let mut s = SliderWidget::new();
    let ctx = slider_ctx();

    let right_down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        pos: Point::new(100.0, 8.0),
        modifiers: Modifiers::NONE,
    };
    let r = s.handle_mouse(&right_down, &ctx);
    assert_eq!(r, WidgetResponse::ignored());
    assert!(!s.is_dragging());
}

#[test]
fn move_without_drag_ignored() {
    let mut s = SliderWidget::new().with_range(0.0, 100.0).with_value(50.0);
    let ctx = slider_ctx();

    // Mouse move without prior mouse down — no drag started.
    let r = s.handle_mouse(&mouse_move(150.0), &ctx);
    assert_eq!(r, WidgetResponse::ignored());
    assert_eq!(s.value(), 50.0);
}

#[test]
fn set_value_clamps() {
    let mut s = SliderWidget::new().with_range(0.0, 10.0);
    s.set_value(20.0);
    assert_eq!(s.value(), 10.0);
    s.set_value(-5.0);
    assert_eq!(s.value(), 0.0);
}
