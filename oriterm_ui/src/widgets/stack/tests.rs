use crate::draw::{DrawCommand, DrawList};
use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, Key, KeyEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::compute_layout;
use crate::widgets::button::ButtonWidget;
use crate::widgets::flex::FlexWidget;
use crate::widgets::label::LabelWidget;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{DrawCtx, EventCtx, LayoutCtx, Widget, WidgetAction, WidgetResponse};

use super::StackWidget;

fn label(text: &str) -> Box<dyn Widget> {
    Box::new(LabelWidget::new(text))
}

fn button(text: &str) -> Box<ButtonWidget> {
    Box::new(ButtonWidget::new(text))
}

#[test]
fn stack_sizes_to_largest_child() {
    // "AB" = 16px, "ABCD" = 32px. Stack should be 32x16.
    let stack = StackWidget::new(vec![label("AB"), label("ABCD")]);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let layout_box = stack.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    assert_eq!(node.rect.width(), 32.0);
    assert_eq!(node.rect.height(), 16.0);
}

#[test]
fn stack_draws_all_children() {
    let stack = StackWidget::new(vec![label("A"), label("B")]);
    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let bounds = Rect::new(0.0, 0.0, 100.0, 50.0);
    let anim_flag = std::cell::Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: None,
        now: std::time::Instant::now(),
        animations_running: &anim_flag,
    };
    stack.draw(&mut ctx);

    let text_cmds = draw_list
        .commands()
        .iter()
        .filter(|c| matches!(c, DrawCommand::Text { .. }))
        .count();
    assert_eq!(text_cmds, 2, "both children should be drawn");
}

#[test]
fn stack_key_routes_to_frontmost() {
    let btn_back = button("Back");
    let btn_front = button("Front");
    let front_id = btn_front.id();
    let mut stack = StackWidget::new(vec![btn_back, btn_front]);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 100.0, 50.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
        focused_widget: Some(front_id),
    };

    let event = KeyEvent {
        key: Key::Enter,
        modifiers: Modifiers::NONE,
    };
    let resp = stack.handle_key(event, &ctx);

    // The frontmost (last) button should receive the event.
    match resp.action {
        Some(WidgetAction::Clicked(id)) => assert_eq!(id, front_id),
        other => panic!("expected Clicked from front button, got {other:?}"),
    }
}

#[test]
fn stack_mouse_routes_to_frontmost() {
    let btn_back = button("Back");
    let btn_front = button("Front");
    let front_id = btn_front.id();
    let mut stack = StackWidget::new(vec![btn_back, btn_front]);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 100.0, 50.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
        focused_widget: None,
    };

    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(10.0, 10.0),
        modifiers: Modifiers::NONE,
    };
    let _ = stack.handle_mouse(&down, &ctx);

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(10.0, 10.0),
        modifiers: Modifiers::NONE,
    };
    let resp = stack.handle_mouse(&up, &ctx);

    match resp.action {
        Some(WidgetAction::Clicked(id)) => assert_eq!(id, front_id),
        other => panic!("expected Clicked from front button, got {other:?}"),
    }
}

#[test]
fn stack_not_focusable() {
    let stack = StackWidget::new(vec![]);
    assert!(!stack.is_focusable());
}

#[test]
fn stack_child_count() {
    let stack = StackWidget::new(vec![label("A"), label("B"), label("C")]);
    assert_eq!(stack.child_count(), 3);
}

#[test]
fn stack_empty() {
    let stack = StackWidget::new(vec![]);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let layout_box = stack.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);
    assert_eq!(node.rect.width(), 0.0);
    assert_eq!(node.rect.height(), 0.0);
}

// --- Edge cases from Chromium/Ratatui audit ---

#[test]
fn stack_draws_in_painter_order() {
    // Verify the first child is drawn before the last (painter's order).
    let stack = StackWidget::new(vec![label("Back"), label("Front")]);
    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let bounds = Rect::new(0.0, 0.0, 100.0, 50.0);
    let anim_flag = std::cell::Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: None,
        now: std::time::Instant::now(),
        animations_running: &anim_flag,
    };
    stack.draw(&mut ctx);

    // Both are Text commands — first drawn is "Back", second is "Front".
    let texts: Vec<&str> = draw_list
        .commands()
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text { shaped, .. } => {
                // Use glyph count as proxy — "Back" has 4 glyphs, "Front" has 5.
                Some(if shaped.glyph_count() == 4 {
                    "Back"
                } else {
                    "Front"
                })
            }
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["Back", "Front"]);
}

#[test]
fn stack_empty_mouse_ignored() {
    let mut stack = StackWidget::new(vec![]);
    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
        focused_widget: None,
    };
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(50.0, 50.0),
        modifiers: Modifiers::NONE,
    };
    assert_eq!(stack.handle_mouse(&event, &ctx), WidgetResponse::ignored());
}

#[test]
fn stack_empty_key_ignored() {
    let mut stack = StackWidget::new(vec![]);
    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
        focused_widget: None,
    };
    let event = KeyEvent {
        key: Key::Enter,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(stack.handle_key(event, &ctx), WidgetResponse::ignored());
}

#[test]
fn stack_hover_routes_to_frontmost() {
    let btn_back = button("Back");
    let btn_front = button("Front");
    let mut stack = StackWidget::new(vec![btn_back, btn_front]);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 100.0, 50.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
        focused_widget: None,
    };

    // Hover enter should reach the frontmost child.
    let resp = stack.handle_hover(HoverEvent::Enter, &ctx);
    assert!(resp.response.is_handled());
}

#[test]
fn stack_mouse_outside_bounds_ignored() {
    let mut stack = StackWidget::new(vec![button("A")]);
    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(10.0, 10.0, 50.0, 50.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
        focused_widget: None,
    };

    // Click outside the stack's bounds.
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(5.0, 5.0),
        modifiers: Modifiers::NONE,
    };
    assert_eq!(stack.handle_mouse(&event, &ctx), WidgetResponse::ignored());
}

#[test]
fn stack_sizes_to_flex_child() {
    // A Flex (Column) child with two labels should contribute its natural size.
    // 2 labels * 16px = 32px tall, "Hello" = 5*8 = 40px wide.
    let col: Box<dyn Widget> = Box::new(FlexWidget::column(vec![label("Hello"), label("World")]));
    let stack = StackWidget::new(vec![col]);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let layout_box = stack.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    assert_eq!(node.rect.width(), 40.0);
    assert_eq!(node.rect.height(), 32.0);
}

#[test]
fn stack_sizes_to_largest_including_flex() {
    // Mix of Leaf (label) and Flex (column) children. Stack sizes to the largest.
    // Label "Wide label!!" = 12*8 = 96px wide, 16px tall.
    // Column of 3 labels = 3*16 = 48px tall, "AB" = 16px wide.
    let wide_label = label("Wide label!!");
    let tall_col: Box<dyn Widget> = Box::new(FlexWidget::column(vec![
        label("AB"),
        label("AB"),
        label("AB"),
    ]));
    let stack = StackWidget::new(vec![wide_label, tall_col]);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let layout_box = stack.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    // Width from label (96px), height from column (48px).
    assert_eq!(node.rect.width(), 96.0);
    assert_eq!(node.rect.height(), 48.0);
}

#[test]
fn stack_single_child_receives_events() {
    let btn = button("Solo");
    let btn_id = btn.id();
    let mut stack = StackWidget::new(vec![btn]);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 100.0, 50.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
        focused_widget: Some(btn_id),
    };

    let event = KeyEvent {
        key: Key::Enter,
        modifiers: Modifiers::NONE,
    };
    let resp = stack.handle_key(event, &ctx);
    match resp.action {
        Some(WidgetAction::Clicked(id)) => assert_eq!(id, btn_id),
        other => panic!("expected Clicked, got {other:?}"),
    }
}

#[test]
fn stack_mouse_falls_through_to_back_child() {
    // Label (non-interactive) on top, button behind it.
    // Click should fall through the label to reach the button.
    let btn = button("Behind");
    let btn_id = btn.id();
    let mut stack = StackWidget::new(vec![btn, label("Overlay")]);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 100.0, 50.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
        focused_widget: None,
    };

    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(10.0, 10.0),
        modifiers: Modifiers::NONE,
    };
    let _ = stack.handle_mouse(&down, &ctx);

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(10.0, 10.0),
        modifiers: Modifiers::NONE,
    };
    let resp = stack.handle_mouse(&up, &ctx);

    match resp.action {
        Some(WidgetAction::Clicked(id)) => assert_eq!(id, btn_id),
        other => panic!("expected click to fall through to back button, got {other:?}"),
    }
}
