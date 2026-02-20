use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, Key, KeyEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::BoxContent;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{EventCtx, LayoutCtx, Widget, WidgetAction, WidgetResponse};

use super::{ToggleStyle, ToggleWidget};

static MEASURER: MockMeasurer = MockMeasurer::STANDARD;

fn event_ctx() -> EventCtx<'static> {
    EventCtx {
        measurer: &MEASURER,
        bounds: Rect::new(0.0, 0.0, 40.0, 22.0),
        is_focused: true,
    }
}

fn left_click() -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(10.0, 10.0),
        modifiers: Modifiers::NONE,
    }
}

fn space_key() -> KeyEvent {
    KeyEvent {
        key: Key::Space,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn default_state() {
    let t = ToggleWidget::new();
    assert!(!t.is_on());
    assert!(!t.is_disabled());
    assert!(!t.is_hovered());
    assert!(t.is_focusable());
    assert_eq!(t.toggle_progress(), 0.0);
}

#[test]
fn with_on_builder() {
    let t = ToggleWidget::new().with_on(true);
    assert!(t.is_on());
    assert_eq!(t.toggle_progress(), 1.0);
}

#[test]
fn layout_fixed_size() {
    let t = ToggleWidget::new();
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = t.layout(&ctx);
    let s = ToggleStyle::default();

    if let BoxContent::Leaf {
        intrinsic_width,
        intrinsic_height,
    } = &layout.content
    {
        assert_eq!(*intrinsic_width, s.width);
        assert_eq!(*intrinsic_height, s.height);
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn click_toggles() {
    let mut t = ToggleWidget::new();
    let ctx = event_ctx();

    let r = t.handle_mouse(&left_click(), &ctx);
    assert!(t.is_on());
    assert_eq!(t.toggle_progress(), 1.0);
    assert_eq!(
        r.action,
        Some(WidgetAction::Toggled {
            id: t.id(),
            value: true,
        })
    );

    let r = t.handle_mouse(&left_click(), &ctx);
    assert!(!t.is_on());
    assert_eq!(t.toggle_progress(), 0.0);
    assert_eq!(
        r.action,
        Some(WidgetAction::Toggled {
            id: t.id(),
            value: false,
        })
    );
}

#[test]
fn space_toggles() {
    let mut t = ToggleWidget::new();
    let ctx = event_ctx();

    let r = t.handle_key(space_key(), &ctx);
    assert!(t.is_on());
    assert_eq!(
        r.action,
        Some(WidgetAction::Toggled {
            id: t.id(),
            value: true,
        })
    );
}

#[test]
fn disabled_ignores() {
    let mut t = ToggleWidget::new().with_disabled(true);
    let ctx = event_ctx();

    assert!(!t.is_focusable());

    let r = t.handle_mouse(&left_click(), &ctx);
    assert_eq!(r, WidgetResponse::ignored());

    let r = t.handle_key(space_key(), &ctx);
    assert_eq!(r, WidgetResponse::ignored());

    let r = t.handle_hover(HoverEvent::Enter, &ctx);
    assert_eq!(r, WidgetResponse::ignored());
}

#[test]
fn hover_transitions() {
    let mut t = ToggleWidget::new();
    let ctx = event_ctx();

    t.handle_hover(HoverEvent::Enter, &ctx);
    assert!(t.is_hovered());

    t.handle_hover(HoverEvent::Leave, &ctx);
    assert!(!t.is_hovered());
}

#[test]
fn set_on_programmatic() {
    let mut t = ToggleWidget::new();
    t.set_on(true);
    assert!(t.is_on());
    assert_eq!(t.toggle_progress(), 1.0);
    t.set_on(false);
    assert!(!t.is_on());
    assert_eq!(t.toggle_progress(), 0.0);
}

#[test]
fn set_toggle_progress_clamps() {
    let mut t = ToggleWidget::new();
    t.set_toggle_progress(1.5);
    assert_eq!(t.toggle_progress(), 1.0);
    t.set_toggle_progress(-0.5);
    assert_eq!(t.toggle_progress(), 0.0);
}

#[test]
fn enter_key_does_not_toggle() {
    let mut t = ToggleWidget::new();
    let ctx = event_ctx();

    // Only Space toggles, not Enter.
    let r = t.handle_key(
        KeyEvent {
            key: Key::Enter,
            modifiers: Modifiers::NONE,
        },
        &ctx,
    );
    assert_eq!(r, WidgetResponse::ignored());
    assert!(!t.is_on());
}

#[test]
fn right_click_ignored() {
    let mut t = ToggleWidget::new();
    let ctx = event_ctx();

    let right_click = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Right),
        pos: Point::new(10.0, 10.0),
        modifiers: Modifiers::NONE,
    };
    let r = t.handle_mouse(&right_click, &ctx);
    assert_eq!(r, WidgetResponse::ignored());
    assert!(!t.is_on());
}

#[test]
fn release_outside_bounds_no_toggle() {
    let mut t = ToggleWidget::new();
    let ctx = event_ctx();

    // MouseUp outside the widget bounds should not toggle.
    let outside_click = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(300.0, 300.0),
        modifiers: Modifiers::NONE,
    };
    let r = t.handle_mouse(&outside_click, &ctx);
    assert!(!t.is_on());
    assert_eq!(r, WidgetResponse::ignored());
}

#[test]
fn disable_while_hovered_clears_state() {
    let mut t = ToggleWidget::new();
    let ctx = event_ctx();

    t.handle_hover(HoverEvent::Enter, &ctx);
    assert!(t.is_hovered());

    t.set_disabled(true);
    assert!(!t.is_hovered());
}

#[test]
fn rapid_toggle_maintains_consistency() {
    let mut t = ToggleWidget::new();
    let ctx = event_ctx();

    for i in 0..6 {
        t.handle_key(space_key(), &ctx);
        assert_eq!(t.is_on(), i % 2 == 0);
        let expected_progress = if t.is_on() { 1.0 } else { 0.0 };
        assert_eq!(t.toggle_progress(), expected_progress);
    }
}
