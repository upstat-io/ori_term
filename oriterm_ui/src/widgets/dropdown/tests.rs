use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, Key, KeyEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::BoxContent;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{EventCtx, LayoutCtx, Widget, WidgetAction, WidgetResponse};

use super::{DropdownStyle, DropdownWidget};

fn items() -> Vec<String> {
    vec!["Alpha".into(), "Beta".into(), "Gamma".into()]
}

fn event_ctx() -> EventCtx {
    EventCtx {
        bounds: Rect::new(0.0, 0.0, 200.0, 28.0),
        is_focused: true,
    }
}

fn key_event(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        modifiers: Modifiers::NONE,
    }
}

fn mouse_down() -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(10.0, 10.0),
        modifiers: Modifiers::NONE,
    }
}

fn mouse_up() -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(10.0, 10.0),
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn default_state() {
    let dd = DropdownWidget::new(items());
    assert_eq!(dd.selected(), 0);
    assert_eq!(dd.selected_text(), "Alpha");
    assert_eq!(dd.items().len(), 3);
    assert!(!dd.is_disabled());
    assert!(!dd.is_hovered());
    assert!(dd.is_focusable());
}

#[test]
fn with_selected_builder() {
    let dd = DropdownWidget::new(items()).with_selected(2);
    assert_eq!(dd.selected(), 2);
    assert_eq!(dd.selected_text(), "Gamma");
}

#[test]
fn selected_clamped() {
    let dd = DropdownWidget::new(items()).with_selected(100);
    assert_eq!(dd.selected(), 2); // Clamped to last index.
}

#[test]
fn layout_accommodates_widest_item() {
    let dd = DropdownWidget::new(items());
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = dd.layout(&ctx);
    let s = DropdownStyle::default();

    if let BoxContent::Leaf {
        intrinsic_width, ..
    } = &layout.content
    {
        // "Gamma" = 5 chars * 8 = 40 (widest) + padding 20 + indicator 20 = 80.
        assert_eq!(
            *intrinsic_width,
            40.0 + s.padding.width() + s.indicator_width
        );
    } else {
        panic!("expected leaf layout");
    }
}

#[test]
fn arrow_down_cycles_forward() {
    let mut dd = DropdownWidget::new(items());
    let ctx = event_ctx();

    let r = dd.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(dd.selected(), 1);
    assert_eq!(
        r.action,
        Some(WidgetAction::Selected {
            id: dd.id(),
            index: 1,
        })
    );

    dd.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(dd.selected(), 2);

    // Wraps to 0.
    dd.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(dd.selected(), 0);
}

#[test]
fn arrow_up_cycles_backward() {
    let mut dd = DropdownWidget::new(items());
    let ctx = event_ctx();

    // At 0, wraps to last.
    let r = dd.handle_key(key_event(Key::ArrowUp), &ctx);
    assert_eq!(dd.selected(), 2);
    assert_eq!(
        r.action,
        Some(WidgetAction::Selected {
            id: dd.id(),
            index: 2,
        })
    );
}

#[test]
fn click_emits_clicked() {
    let mut dd = DropdownWidget::new(items());
    let ctx = event_ctx();

    dd.handle_mouse(&mouse_down(), &ctx);
    let r = dd.handle_mouse(&mouse_up(), &ctx);
    assert_eq!(r.action, Some(WidgetAction::Clicked(dd.id())));
}

#[test]
fn enter_emits_clicked() {
    let mut dd = DropdownWidget::new(items());
    let ctx = event_ctx();

    let r = dd.handle_key(key_event(Key::Enter), &ctx);
    assert_eq!(r.action, Some(WidgetAction::Clicked(dd.id())));
}

#[test]
fn disabled_ignores() {
    let mut dd = DropdownWidget::new(items()).with_disabled(true);
    let ctx = event_ctx();

    assert!(!dd.is_focusable());

    let r = dd.handle_mouse(&mouse_down(), &ctx);
    assert_eq!(r, WidgetResponse::ignored());

    let r = dd.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(r, WidgetResponse::ignored());

    let r = dd.handle_hover(HoverEvent::Enter, &ctx);
    assert_eq!(r, WidgetResponse::ignored());
}

#[test]
fn hover_transitions() {
    let mut dd = DropdownWidget::new(items());
    let ctx = event_ctx();

    dd.handle_hover(HoverEvent::Enter, &ctx);
    assert!(dd.is_hovered());

    dd.handle_hover(HoverEvent::Leave, &ctx);
    assert!(!dd.is_hovered());
}

#[test]
fn set_selected_programmatic() {
    let mut dd = DropdownWidget::new(items());
    dd.set_selected(1);
    assert_eq!(dd.selected(), 1);
    assert_eq!(dd.selected_text(), "Beta");
}

#[test]
fn set_selected_clamped() {
    let mut dd = DropdownWidget::new(items());
    dd.set_selected(99);
    assert_eq!(dd.selected(), 2);
}

#[test]
fn space_emits_clicked() {
    let mut dd = DropdownWidget::new(items());
    let ctx = event_ctx();

    let r = dd.handle_key(key_event(Key::Space), &ctx);
    assert_eq!(r.action, Some(WidgetAction::Clicked(dd.id())));
}

#[test]
fn single_item_arrows_stay() {
    let mut dd = DropdownWidget::new(vec!["Only".into()]);
    let ctx = event_ctx();

    dd.handle_key(key_event(Key::ArrowDown), &ctx);
    assert_eq!(dd.selected(), 0);

    dd.handle_key(key_event(Key::ArrowUp), &ctx);
    assert_eq!(dd.selected(), 0);
}

#[test]
fn leave_clears_pressed() {
    let mut dd = DropdownWidget::new(items());
    let ctx = event_ctx();

    dd.handle_mouse(&mouse_down(), &ctx);
    assert!(dd.pressed);

    dd.handle_hover(HoverEvent::Leave, &ctx);
    assert!(!dd.pressed);
    assert!(!dd.is_hovered());
}

#[test]
fn right_click_ignored() {
    let mut dd = DropdownWidget::new(items());
    let ctx = event_ctx();

    let right_down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        pos: Point::new(10.0, 10.0),
        modifiers: Modifiers::NONE,
    };
    let r = dd.handle_mouse(&right_down, &ctx);
    assert_eq!(r, WidgetResponse::ignored());
}

#[test]
fn release_without_press_no_clicked() {
    let mut dd = DropdownWidget::new(items());
    let ctx = event_ctx();

    // Mouse up without prior press.
    let r = dd.handle_mouse(&mouse_up(), &ctx);
    assert!(r.action.is_none());
}

#[test]
fn set_disabled_clears_visual_state() {
    let mut dd = DropdownWidget::new(items());
    let ctx = event_ctx();

    dd.handle_hover(HoverEvent::Enter, &ctx);
    dd.handle_mouse(&mouse_down(), &ctx);
    assert!(dd.is_hovered());
    assert!(dd.pressed);

    dd.set_disabled(true);
    assert!(!dd.is_hovered());
    assert!(!dd.pressed);
}

#[test]
fn full_cycle_forward() {
    let mut dd = DropdownWidget::new(items());
    let ctx = event_ctx();

    // Cycle through all items and back to start.
    for expected in [1, 2, 0, 1, 2, 0] {
        dd.handle_key(key_event(Key::ArrowDown), &ctx);
        assert_eq!(dd.selected(), expected);
    }
}

#[test]
fn escape_key_ignored() {
    let mut dd = DropdownWidget::new(items());
    let ctx = event_ctx();

    let r = dd.handle_key(key_event(Key::Escape), &ctx);
    assert_eq!(r, WidgetResponse::ignored());
}
