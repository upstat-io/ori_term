use crate::draw::DrawList;
use crate::geometry::{Insets, Rect};
use crate::layout::compute_layout;
use crate::widgets::button::ButtonWidget;
use crate::widgets::label::LabelWidget;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{DrawCtx, LayoutCtx, Widget};

use super::{PanelStyle, PanelWidget};

fn make_panel_with_label(label: &str) -> PanelWidget {
    let child = Box::new(LabelWidget::new(label));
    PanelWidget::new(child)
}

#[test]
fn panel_layout_includes_padding() {
    let panel = make_panel_with_label("Hello");
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
        theme: &super::super::tests::TEST_THEME,
    };
    let layout_box = panel.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    // Label: 5 chars * 8px = 40px wide, 16px tall.
    // Default padding: 12px all sides.
    // Panel size: 40 + 24 = 64 wide, 16 + 24 = 40 tall (Hug mode).
    assert_eq!(node.rect.width(), 64.0);
    assert_eq!(node.rect.height(), 40.0);
}

#[test]
fn panel_child_gets_content_rect() {
    let panel = make_panel_with_label("Hi");
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
        theme: &super::super::tests::TEST_THEME,
    };
    let layout_box = panel.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    // Child is the first child of the panel layout node.
    assert_eq!(node.children.len(), 1);
    let child = &node.children[0];
    // Child rect should be inset by padding (12px each side).
    assert_eq!(child.rect.x(), 12.0);
    assert_eq!(child.rect.y(), 12.0);
}

#[test]
fn panel_draws_background_rect() {
    let panel = make_panel_with_label("Test");
    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let bounds = Rect::new(10.0, 20.0, 100.0, 50.0);
    let anim_flag = std::cell::Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: None,
        now: std::time::Instant::now(),
        animations_running: &anim_flag,
        theme: &super::super::tests::TEST_THEME,
    };
    panel.draw(&mut ctx);

    // First command is PushLayer, second is the background rect.
    let cmds = draw_list.commands();
    assert!(!cmds.is_empty(), "panel should produce draw commands");
    assert!(matches!(
        cmds[0],
        crate::draw::DrawCommand::PushLayer { .. }
    ));
    match &cmds[1] {
        crate::draw::DrawCommand::Rect { rect, .. } => {
            assert_eq!(*rect, bounds);
        }
        other => panic!("expected Rect command, got {other:?}"),
    }
}

#[test]
fn panel_not_focusable() {
    let panel = make_panel_with_label("X");
    assert!(!panel.is_focusable());
}

#[test]
fn panel_custom_style() {
    use crate::color::Color;
    let style = PanelStyle {
        bg: Color::WHITE,
        border_width: 2.0,
        corner_radius: 16.0,
        padding: Insets::all(20.0),
        ..PanelStyle::default()
    };
    let panel = make_panel_with_label("Styled").with_style(style.clone());
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
        theme: &super::super::tests::TEST_THEME,
    };
    let layout_box = panel.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    // "Styled" = 6 chars * 8 = 48px, + 40px padding = 88px wide.
    assert_eq!(node.rect.width(), 88.0);
    assert_eq!(node.rect.height(), 56.0); // 16 + 40
}

#[test]
fn panel_delegates_key_to_child() {
    use crate::input::{Key, KeyEvent, Modifiers};

    let child = Box::new(ButtonWidget::new("Click me"));
    let child_id = child.id();
    let mut panel = PanelWidget::new(child);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = super::super::EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
        focused_widget: Some(child_id),
        theme: &super::super::tests::TEST_THEME,
    };
    let event = KeyEvent {
        key: Key::Enter,
        modifiers: Modifiers::NONE,
    };
    let response = panel.handle_key(event, &ctx);

    // Button should have emitted Clicked action.
    assert!(response.action.is_some());
    match response.action {
        Some(super::super::WidgetAction::Clicked(id)) => assert_eq!(id, child_id),
        other => panic!("expected Clicked, got {other:?}"),
    }
}

// Mouse and hover delegation (Chromium event dispatch patterns)

#[test]
fn panel_delegates_mouse_to_child() {
    use crate::geometry::Point;
    use crate::input::{Modifiers, MouseButton, MouseEvent, MouseEventKind};

    let child = Box::new(ButtonWidget::new("Click"));
    let child_id = child.id();
    let mut panel = PanelWidget::new(child);

    let measurer = MockMeasurer::STANDARD;
    // Panel with default padding (12px). Child is inset at (12, 12).
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = super::super::EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
        focused_widget: None,
        theme: &super::super::tests::TEST_THEME,
    };

    // Click inside the child area (accounting for 12px padding).
    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(20.0, 20.0),
        modifiers: Modifiers::NONE,
    };
    let _ = panel.handle_mouse(&down, &ctx);

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(20.0, 20.0),
        modifiers: Modifiers::NONE,
    };
    let resp = panel.handle_mouse(&up, &ctx);

    match resp.action {
        Some(super::super::WidgetAction::Clicked(id)) => assert_eq!(id, child_id),
        other => panic!("expected Clicked through panel, got {other:?}"),
    }
}

#[test]
fn panel_mouse_outside_child_ignored() {
    use crate::geometry::Point;
    use crate::input::{Modifiers, MouseButton, MouseEvent, MouseEventKind};

    let child = Box::new(ButtonWidget::new("X"));
    let mut panel = PanelWidget::new(child);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = super::super::EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
        focused_widget: None,
        theme: &super::super::tests::TEST_THEME,
    };

    // Click in the panel's padding area (outside child bounds).
    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(2.0, 2.0),
        modifiers: Modifiers::NONE,
    };
    let resp = panel.handle_mouse(&down, &ctx);
    assert_eq!(resp, super::super::WidgetResponse::ignored());
}

#[test]
fn panel_delegates_hover_to_child() {
    use crate::input::HoverEvent;

    let child = Box::new(ButtonWidget::new("Hover"));
    let mut panel = PanelWidget::new(child);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = super::super::EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
        focused_widget: None,
        theme: &super::super::tests::TEST_THEME,
    };

    // Hover should delegate to child.
    let resp = panel.handle_hover(HoverEvent::Enter, &ctx);
    // ButtonWidget returns redraw on hover enter.
    assert!(resp.response.is_handled());
}

// Builder method tests

#[test]
fn panel_with_bg() {
    use crate::color::Color;

    let panel = make_panel_with_label("Test").with_bg(Color::WHITE);
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
        theme: &super::super::tests::TEST_THEME,
    };
    panel.draw(&mut ctx);

    // First command is PushLayer, second is the background rect.
    assert!(matches!(
        draw_list.commands()[0],
        crate::draw::DrawCommand::PushLayer { .. },
    ));
    match &draw_list.commands()[1] {
        crate::draw::DrawCommand::Rect { style, .. } => {
            assert_eq!(style.fill, Some(Color::WHITE));
        }
        other => panic!("expected Rect command, got {other:?}"),
    }
}

#[test]
fn panel_with_corner_radius() {
    let panel = make_panel_with_label("R").with_corner_radius(20.0);
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
        theme: &super::super::tests::TEST_THEME,
    };
    panel.draw(&mut ctx);

    // First command is PushLayer, second is the background rect.
    match &draw_list.commands()[1] {
        crate::draw::DrawCommand::Rect { style, .. } => {
            assert_eq!(style.corner_radius, [20.0; 4]);
        }
        other => panic!("expected Rect command, got {other:?}"),
    }
}

#[test]
fn panel_with_padding_affects_layout() {
    let panel = make_panel_with_label("Pad").with_padding(Insets::all(30.0));
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
        theme: &super::super::tests::TEST_THEME,
    };
    let layout_box = panel.layout(&ctx);
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let node = compute_layout(&layout_box, viewport);

    // "Pad" = 3 chars * 8px = 24px wide, 16px tall.
    // Padding: 30px all sides → 24 + 60 = 84 wide, 16 + 60 = 76 tall.
    assert_eq!(node.rect.width(), 84.0);
    assert_eq!(node.rect.height(), 76.0);
}

#[test]
fn panel_with_shadow() {
    use crate::color::Color;
    use crate::draw::Shadow;

    let shadow = Shadow {
        offset_x: 0.0,
        offset_y: 4.0,
        blur_radius: 8.0,
        spread: 0.0,
        color: Color::BLACK.with_alpha(0.3),
    };
    let panel = make_panel_with_label("S").with_shadow(shadow);
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
        theme: &super::super::tests::TEST_THEME,
    };
    panel.draw(&mut ctx);

    // First command is PushLayer, second is the background rect.
    match &draw_list.commands()[1] {
        crate::draw::DrawCommand::Rect { style, .. } => {
            assert!(style.shadow.is_some(), "shadow should be set on panel rect");
            let s = style.shadow.unwrap();
            assert_eq!(s.offset_y, 4.0);
            assert_eq!(s.blur_radius, 8.0);
        }
        other => panic!("expected Rect command, got {other:?}"),
    }
}
