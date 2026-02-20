use crate::draw::{DrawCommand, DrawList};
use crate::geometry::{Point, Rect};
use crate::input::{Key, KeyEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::{Align, Justify, SizeSpec, compute_layout};
use crate::widgets::button::ButtonWidget;
use crate::widgets::label::LabelWidget;
use crate::widgets::panel::PanelWidget;
use crate::widgets::spacer::SpacerWidget;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{DrawCtx, EventCtx, LayoutCtx, Widget, WidgetAction, WidgetResponse};

use super::FlexWidget;

fn label(text: &str) -> Box<dyn Widget> {
    Box::new(LabelWidget::new(text))
}

fn button(text: &str) -> Box<ButtonWidget> {
    Box::new(ButtonWidget::new(text))
}

#[test]
fn row_layout_places_children_horizontally() {
    let row = FlexWidget::row(vec![label("AB"), label("CD")]);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let layout_box = row.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    assert_eq!(node.children.len(), 2);
    // "AB" = 16px, "CD" = 16px. Row hugs: 32px wide, 16px tall.
    assert_eq!(node.rect.width(), 32.0);
    assert_eq!(node.rect.height(), 16.0);
    // First child at x=0, second at x=16.
    assert_eq!(node.children[0].rect.x(), 0.0);
    assert_eq!(node.children[1].rect.x(), 16.0);
}

#[test]
fn column_layout_places_children_vertically() {
    let col = FlexWidget::column(vec![label("AB"), label("CD")]);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let layout_box = col.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    assert_eq!(node.children.len(), 2);
    // Column hugs: 16px wide, 32px tall.
    assert_eq!(node.rect.width(), 16.0);
    assert_eq!(node.rect.height(), 32.0);
    // First child at y=0, second at y=16.
    assert_eq!(node.children[0].rect.y(), 0.0);
    assert_eq!(node.children[1].rect.y(), 16.0);
}

#[test]
fn row_with_gap() {
    let row = FlexWidget::row(vec![label("A"), label("B")]).with_gap(10.0);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let layout_box = row.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    // "A" = 8px, gap = 10px, "B" = 8px → total 26px.
    assert_eq!(node.rect.width(), 26.0);
    assert_eq!(node.children[0].rect.x(), 0.0);
    assert_eq!(node.children[1].rect.x(), 18.0); // 8 + 10
}

#[test]
fn row_with_spacer_pushes_apart() {
    let row = FlexWidget::row(vec![label("L"), Box::new(SpacerWidget::fill()), label("R")]);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let mut layout_box = row.layout(&ctx);
    // Force the row to fill width so the spacer has room to expand.
    layout_box.width = SizeSpec::Fill;
    let viewport = Rect::new(0.0, 0.0, 100.0, 50.0);
    let node = compute_layout(&layout_box, viewport);

    // "L" at x=0 (8px), spacer fills middle, "R" at right edge.
    assert_eq!(node.children[0].rect.x(), 0.0);
    let right_x = node.children[2].rect.x();
    assert_eq!(right_x, 92.0); // 100 - 8
}

#[test]
fn column_with_center_align() {
    let col = FlexWidget::column(vec![label("AB"), label("ABCD")]).with_align(Align::Center);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let layout_box = col.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    // "ABCD" is 32px wide, "AB" is 16px wide. Column is 32px wide.
    // "AB" should be centered: (32 - 16) / 2 = 8px offset.
    assert_eq!(node.children[0].rect.x(), 8.0);
    assert_eq!(node.children[1].rect.x(), 0.0);
}

#[test]
fn row_with_justify_space_between() {
    let row = FlexWidget::row(vec![label("A"), label("B"), label("C")])
        .with_justify(Justify::SpaceBetween);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let mut layout_box = row.layout(&ctx);
    layout_box.width = SizeSpec::Fill;
    let viewport = Rect::new(0.0, 0.0, 100.0, 50.0);
    let node = compute_layout(&layout_box, viewport);

    // 3 children of 8px each = 24px used. 76px free space / 2 gaps = 38px between.
    assert_eq!(node.children[0].rect.x(), 0.0);
    assert_eq!(node.children[1].rect.x(), 46.0); // 8 + 38
    assert_eq!(node.children[2].rect.x(), 92.0); // 46 + 8 + 38
}

#[test]
fn flex_not_focusable() {
    let row = FlexWidget::row(vec![]);
    assert!(!row.is_focusable());
}

#[test]
fn flex_child_count() {
    let row = FlexWidget::row(vec![label("A"), label("B"), label("C")]);
    assert_eq!(row.child_count(), 3);
}

#[test]
fn flex_draws_children() {
    let row = FlexWidget::row(vec![label("A"), label("B")]);
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
    row.draw(&mut ctx);

    // Each label draws one Text command → 2 total.
    let text_cmds = draw_list
        .commands()
        .iter()
        .filter(|c| matches!(c, DrawCommand::Text { .. }))
        .count();
    assert_eq!(text_cmds, 2);
}

#[test]
fn flex_delegates_mouse_to_child() {
    let btn = button("Click");
    let btn_id = btn.id();
    let mut row = FlexWidget::row(vec![label("Label"), btn]);

    let measurer = MockMeasurer::STANDARD;
    // Row layout: "Label" = 40px, "Click" = 40px+padding. Total width ~100px.
    let bounds = Rect::new(0.0, 0.0, 200.0, 50.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
    };

    // Click inside the button area (x > 40px).
    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(50.0, 8.0),
        modifiers: Modifiers::NONE,
    };
    let _ = row.handle_mouse(&down, &ctx);

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(50.0, 8.0),
        modifiers: Modifiers::NONE,
    };
    let resp = row.handle_mouse(&up, &ctx);

    assert!(resp.action.is_some());
    match resp.action {
        Some(WidgetAction::Clicked(id)) => assert_eq!(id, btn_id),
        other => panic!("expected Clicked({btn_id:?}), got {other:?}"),
    }
}

#[test]
fn flex_delegates_key_to_child() {
    let btn = button("OK");
    let btn_id = btn.id();
    let mut col = FlexWidget::column(vec![label("Title"), btn]);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
    };

    let event = KeyEvent {
        key: Key::Enter,
        modifiers: Modifiers::NONE,
    };
    let resp = col.handle_key(event, &ctx);
    assert!(resp.action.is_some());
    match resp.action {
        Some(WidgetAction::Clicked(id)) => assert_eq!(id, btn_id),
        other => panic!("expected Clicked, got {other:?}"),
    }
}

#[test]
fn flex_empty_row() {
    let row = FlexWidget::row(vec![]);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let layout_box = row.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);
    assert_eq!(node.rect.width(), 0.0);
    assert_eq!(node.rect.height(), 0.0);
}

#[test]
fn mouse_outside_children_is_ignored() {
    let mut row = FlexWidget::row(vec![label("X")]);
    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 50.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
    };
    // Click far outside the label (label is 8px wide).
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(150.0, 25.0),
        modifiers: Modifiers::NONE,
    };
    let resp = row.handle_mouse(&event, &ctx);
    assert_eq!(resp, WidgetResponse::ignored());
}

// --- Edge cases from Chromium/Ratatui audit ---

#[test]
fn deeply_nested_layout_correct() {
    // 3 levels: column → row → label. Verify positions propagate correctly.
    let inner_row = FlexWidget::row(vec![label("A"), label("B")]).with_gap(4.0);
    let outer_col = FlexWidget::column(vec![label("Header"), Box::new(inner_row), label("Footer")]);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let layout_box = outer_col.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    assert_eq!(node.children.len(), 3);
    // Header at y=0, row at y=16, footer at y=32.
    assert_eq!(node.children[0].rect.y(), 0.0);
    assert_eq!(node.children[1].rect.y(), 16.0);
    assert_eq!(node.children[2].rect.y(), 32.0);
    // Inner row: "A" (8px) + gap (4px) + "B" (8px) = 20px wide.
    let inner = &node.children[1];
    assert_eq!(inner.rect.width(), 20.0);
    assert_eq!(inner.children.len(), 2);
    assert_eq!(inner.children[0].rect.x(), 0.0);
    assert_eq!(inner.children[1].rect.x(), 12.0); // 8 + 4
}

#[test]
fn deeply_nested_mouse_routing() {
    // column → row → button. Click should reach the button through 2 containers.
    let btn = button("OK");
    let btn_id = btn.id();
    let inner_row = FlexWidget::row(vec![label("Pre"), btn]);
    let mut outer_col = FlexWidget::column(vec![label("Header"), Box::new(inner_row)]);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 300.0, 200.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
    };

    // Button is at row y=16 (after header), x=24 (after "Pre"=24px).
    // Click at approximately (30, 20) to hit the button.
    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(30.0, 20.0),
        modifiers: Modifiers::NONE,
    };
    let _ = outer_col.handle_mouse(&down, &ctx);

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(30.0, 20.0),
        modifiers: Modifiers::NONE,
    };
    let resp = outer_col.handle_mouse(&up, &ctx);

    match resp.action {
        Some(WidgetAction::Clicked(id)) => assert_eq!(id, btn_id),
        other => panic!("expected Clicked through nested containers, got {other:?}"),
    }
}

#[test]
fn mouse_on_gap_between_children_is_ignored() {
    // Row with gap=20. Click in the gap area (between children).
    let mut row = FlexWidget::row(vec![label("A"), label("B")]).with_gap(20.0);
    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 50.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
    };

    // "A" occupies x=[0,8), gap is x=[8,28), "B" at x=[28,36).
    // Click in the gap at x=15.
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(15.0, 8.0),
        modifiers: Modifiers::NONE,
    };
    let resp = row.handle_mouse(&event, &ctx);
    assert_eq!(resp, WidgetResponse::ignored());
}

#[test]
fn empty_flex_mouse_events_ignored() {
    let mut row = FlexWidget::row(vec![]);
    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
    };
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(50.0, 50.0),
        modifiers: Modifiers::NONE,
    };
    assert_eq!(row.handle_mouse(&event, &ctx), WidgetResponse::ignored());
}

#[test]
fn empty_flex_key_events_ignored() {
    let mut row = FlexWidget::row(vec![]);
    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
    };
    let event = KeyEvent {
        key: Key::Enter,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(row.handle_key(event, &ctx), WidgetResponse::ignored());
}

#[test]
fn focused_widget_id_propagates_through_draw() {
    // Verify that focused_widget context passes through flex → child.
    let btn = ButtonWidget::new("Focus Me");
    let btn_id = btn.id();
    let row = FlexWidget::row(vec![Box::new(btn)]);
    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let bounds = Rect::new(0.0, 0.0, 200.0, 50.0);
    let anim_flag = std::cell::Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: Some(btn_id),
        now: std::time::Instant::now(),
        animations_running: &anim_flag,
    };
    row.draw(&mut ctx);

    // Button should draw a focus ring (extra Rect command before the bg).
    let rect_cmds = draw_list
        .commands()
        .iter()
        .filter(|c| matches!(c, DrawCommand::Rect { .. }))
        .count();
    // Focus ring rect + button bg rect = 2 rects.
    assert!(
        rect_cmds >= 2,
        "expected focus ring + bg, got {rect_cmds} rects"
    );
}

#[test]
fn panel_inside_flex_layout() {
    // Flex containing a panel containing a label — verify nested bounds.
    let panel = PanelWidget::new(Box::new(LabelWidget::new("Inner")));
    let row = FlexWidget::row(vec![label("Before"), Box::new(panel)]);
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let layout_box = row.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    assert_eq!(node.children.len(), 2);
    // "Before" = 48px. Panel = "Inner" (40px) + 24px padding = 64px.
    let before = &node.children[0];
    let panel_node = &node.children[1];
    assert_eq!(before.rect.width(), 48.0);
    assert_eq!(panel_node.rect.width(), 64.0);
    assert_eq!(panel_node.rect.x(), 48.0);
}

#[test]
fn child_consumes_event_stops_propagation() {
    // Two buttons in a row. Click the first — second should not receive it.
    let btn1 = button("First");
    let btn1_id = btn1.id();
    let btn2 = button("Second");
    let btn2_id = btn2.id();
    let mut row = FlexWidget::row(vec![btn1, btn2]);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 300.0, 50.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
    };

    // Click at x=5 (inside first button).
    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(5.0, 8.0),
        modifiers: Modifiers::NONE,
    };
    let _ = row.handle_mouse(&down, &ctx);
    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(5.0, 8.0),
        modifiers: Modifiers::NONE,
    };
    let resp = row.handle_mouse(&up, &ctx);

    // First button should handle it, second should not.
    match resp.action {
        Some(WidgetAction::Clicked(id)) => {
            assert_eq!(id, btn1_id);
            assert_ne!(id, btn2_id);
        }
        other => panic!("expected Clicked from first button, got {other:?}"),
    }
}
