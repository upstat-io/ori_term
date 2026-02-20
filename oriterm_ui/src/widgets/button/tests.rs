use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, Key, KeyEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::BoxContent;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{EventCtx, LayoutCtx, Widget, WidgetAction, WidgetResponse};

use super::{ButtonStyle, ButtonWidget};

fn event_ctx(bounds: Rect) -> EventCtx {
    EventCtx {
        bounds,
        is_focused: true,
    }
}

fn mouse_down(x: f32, y: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(x, y),
        modifiers: Modifiers::NONE,
    }
}

fn mouse_up(x: f32, y: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(x, y),
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
    let btn = ButtonWidget::new("OK");
    assert_eq!(btn.label(), "OK");
    assert!(!btn.is_disabled());
    assert!(!btn.is_hovered());
    assert!(!btn.is_pressed());
    assert!(btn.is_focusable());
}

#[test]
fn disabled_not_focusable() {
    let btn = ButtonWidget::new("OK").with_disabled(true);
    assert!(!btn.is_focusable());
}

#[test]
fn layout_includes_padding() {
    let btn = ButtonWidget::new("OK");
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = btn.layout(&ctx);
    let style = ButtonStyle::default();

    if let BoxContent::Leaf {
        intrinsic_width,
        intrinsic_height,
    } = &layout.content
    {
        // "OK" = 2 chars * 8px = 16px + padding (12 + 12 = 24) = 40px.
        assert_eq!(*intrinsic_width, 16.0 + style.padding.width());
        // 16px line + padding (6 + 6 = 12) = 28px.
        assert_eq!(*intrinsic_height, 16.0 + style.padding.height());
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn click_emits_action() {
    let mut btn = ButtonWidget::new("OK");
    let bounds = Rect::new(0.0, 0.0, 100.0, 30.0);
    let ctx = event_ctx(bounds);

    // Press.
    let r = btn.handle_mouse(&mouse_down(10.0, 10.0), &ctx);
    assert!(r.response.is_handled());
    assert!(btn.is_pressed());

    // Release inside bounds.
    let r = btn.handle_mouse(&mouse_up(10.0, 10.0), &ctx);
    assert_eq!(r.action, Some(WidgetAction::Clicked(btn.id())));
    assert!(!btn.is_pressed());
}

#[test]
fn release_outside_no_action() {
    let mut btn = ButtonWidget::new("OK");
    let bounds = Rect::new(0.0, 0.0, 100.0, 30.0);
    let ctx = event_ctx(bounds);

    btn.handle_mouse(&mouse_down(10.0, 10.0), &ctx);
    // Release outside bounds.
    let r = btn.handle_mouse(&mouse_up(200.0, 200.0), &ctx);
    assert_eq!(r.action, None);
}

#[test]
fn hover_state_transitions() {
    let mut btn = ButtonWidget::new("OK");
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));

    btn.handle_hover(HoverEvent::Enter, &ctx);
    assert!(btn.is_hovered());

    btn.handle_hover(HoverEvent::Leave, &ctx);
    assert!(!btn.is_hovered());
}

#[test]
fn disabled_ignores_events() {
    let mut btn = ButtonWidget::new("OK").with_disabled(true);
    let bounds = Rect::new(0.0, 0.0, 100.0, 30.0);
    let ctx = event_ctx(bounds);

    let r = btn.handle_mouse(&mouse_down(10.0, 10.0), &ctx);
    assert_eq!(r, WidgetResponse::ignored());

    let r = btn.handle_hover(HoverEvent::Enter, &ctx);
    assert_eq!(r, WidgetResponse::ignored());

    let r = btn.handle_key(key_event(Key::Enter), &ctx);
    assert_eq!(r, WidgetResponse::ignored());
}

#[test]
fn keyboard_activation_enter() {
    let mut btn = ButtonWidget::new("OK");
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));
    let r = btn.handle_key(key_event(Key::Enter), &ctx);
    assert_eq!(r.action, Some(WidgetAction::Clicked(btn.id())));
}

#[test]
fn keyboard_activation_space() {
    let mut btn = ButtonWidget::new("OK");
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));
    let r = btn.handle_key(key_event(Key::Space), &ctx);
    assert_eq!(r.action, Some(WidgetAction::Clicked(btn.id())));
}

#[test]
fn keyboard_other_ignored() {
    let mut btn = ButtonWidget::new("OK");
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));
    let r = btn.handle_key(key_event(Key::Escape), &ctx);
    assert_eq!(r, WidgetResponse::ignored());
}

#[test]
fn leave_clears_pressed() {
    let mut btn = ButtonWidget::new("OK");
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));

    btn.handle_mouse(&mouse_down(10.0, 10.0), &ctx);
    assert!(btn.is_pressed());

    btn.handle_hover(HoverEvent::Leave, &ctx);
    assert!(!btn.is_pressed());
}

#[test]
fn disable_while_pressed_clears_state() {
    let mut btn = ButtonWidget::new("OK");
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));

    // Enter hover + press state.
    btn.handle_hover(HoverEvent::Enter, &ctx);
    btn.handle_mouse(&mouse_down(10.0, 10.0), &ctx);
    assert!(btn.is_pressed());
    assert!(btn.is_hovered());

    // Disable mid-press — both flags must clear.
    btn.set_disabled(true);
    assert!(!btn.is_pressed());
    assert!(!btn.is_hovered());
}

#[test]
fn right_click_ignored() {
    let mut btn = ButtonWidget::new("OK");
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));

    let right_down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        pos: Point::new(10.0, 10.0),
        modifiers: Modifiers::NONE,
    };
    let r = btn.handle_mouse(&right_down, &ctx);
    assert_eq!(r, WidgetResponse::ignored());
    assert!(!btn.is_pressed());
}

#[test]
fn release_without_press_no_action() {
    let mut btn = ButtonWidget::new("OK");
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));

    // Mouse up without a prior mouse down.
    let r = btn.handle_mouse(&mouse_up(10.0, 10.0), &ctx);
    assert!(r.action.is_none());
}

#[test]
fn set_label_updates() {
    let mut btn = ButtonWidget::new("OK");
    btn.label = "Cancel".into();
    assert_eq!(btn.label(), "Cancel");
}

#[test]
fn empty_label_layout() {
    let btn = ButtonWidget::new("");
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = btn.layout(&ctx);
    let style = ButtonStyle::default();

    if let BoxContent::Leaf {
        intrinsic_width, ..
    } = &layout.content
    {
        // Empty text = 0px + padding.
        assert_eq!(*intrinsic_width, style.padding.width());
    } else {
        panic!("expected leaf layout");
    }
}
