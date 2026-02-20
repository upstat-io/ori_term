use crate::draw::{DrawCommand, DrawList};
use crate::geometry::{Point, Rect};
use crate::input::{
    Key, KeyEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind, ScrollDelta,
};
use crate::layout::compute_layout;
use crate::widgets::button::ButtonWidget;
use crate::widgets::flex::FlexWidget;
use crate::widgets::label::LabelWidget;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{DrawCtx, EventCtx, LayoutCtx, Widget};

use super::ScrollWidget;

/// Creates content that is 16px tall (single label).
fn short_content() -> Box<dyn Widget> {
    Box::new(LabelWidget::new("A".repeat(100)))
}

/// Creates a tall column of labels that overflows a small viewport.
/// 20 labels * 16px = 320px tall.
fn tall_content() -> Box<dyn Widget> {
    let labels: Vec<Box<dyn Widget>> = (0..20)
        .map(|i| Box::new(LabelWidget::new(format!("Line {i}"))) as Box<dyn Widget>)
        .collect();
    Box::new(FlexWidget::column(labels))
}

fn make_scroll(child: Box<dyn Widget>) -> ScrollWidget {
    ScrollWidget::vertical(child)
}

#[test]
fn scroll_layout_reports_child_size() {
    let scroll = make_scroll(short_content());
    let ctx = LayoutCtx {
        measurer: &MockMeasurer::STANDARD,
    };
    let layout_box = scroll.layout(&ctx);
    // Use a viewport wide enough that no clamping occurs.
    let viewport = Rect::new(0.0, 0.0, 1000.0, 1000.0);
    let node = compute_layout(&layout_box, viewport);

    // Label: 100 chars * 8px = 800px wide, 16px tall.
    assert_eq!(node.rect.width(), 800.0);
    assert_eq!(node.rect.height(), 16.0);
}

#[test]
fn scroll_offset_starts_at_zero() {
    let scroll = make_scroll(tall_content());
    assert_eq!(scroll.scroll_offset(), 0.0);
}

#[test]
fn scroll_offset_clamps_to_range() {
    let mut scroll = make_scroll(tall_content());
    // Content 500px tall, viewport 200px → max offset = 300.
    scroll.set_scroll_offset(999.0, 500.0, 200.0);
    assert_eq!(scroll.scroll_offset(), 300.0);

    scroll.set_scroll_offset(-10.0, 500.0, 200.0);
    assert_eq!(scroll.scroll_offset(), 0.0);
}

#[test]
fn scroll_offset_zero_when_content_fits() {
    let mut scroll = make_scroll(tall_content());
    // Content 100px, viewport 200px → max offset = 0.
    scroll.set_scroll_offset(50.0, 100.0, 200.0);
    assert_eq!(scroll.scroll_offset(), 0.0);
}

#[test]
fn scroll_is_focusable() {
    let scroll = make_scroll(tall_content());
    assert!(scroll.is_focusable());
}

#[test]
fn scroll_draws_with_clip() {
    let scroll = make_scroll(tall_content());
    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let anim_flag = std::cell::Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: None,
        now: std::time::Instant::now(),
        animations_running: &anim_flag,
    };
    scroll.draw(&mut ctx);

    // Should have PushClip and PopClip commands (balanced).
    let push_count = draw_list
        .commands()
        .iter()
        .filter(|c| matches!(c, DrawCommand::PushClip { .. }))
        .count();
    let pop_count = draw_list
        .commands()
        .iter()
        .filter(|c| matches!(c, DrawCommand::PopClip))
        .count();
    assert_eq!(push_count, 1);
    assert_eq!(pop_count, 1);
}

#[test]
fn scroll_wheel_changes_offset() {
    // tall_content = 20 labels * 16px = 320px tall.
    let mut scroll = ScrollWidget::vertical(tall_content());

    let measurer = MockMeasurer::STANDARD;
    // Viewport 100px tall — content (320px) overflows by 220px.
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
        focused_widget: None,
    };

    // Scroll down (negative delta_y means scroll down in our convention).
    let event = MouseEvent {
        kind: MouseEventKind::Scroll(ScrollDelta::Lines { x: 0.0, y: -3.0 }),
        pos: Point::new(25.0, 25.0),
        modifiers: Modifiers::NONE,
    };
    let resp = scroll.handle_mouse(&event, &ctx);

    // Should have scrolled (redraw).
    assert!(resp.response.is_handled());
    // Offset should have increased (scrolled down).
    assert!(scroll.scroll_offset() > 0.0);
}

#[test]
fn key_home_resets_to_top() {
    // tall_content = 320px tall.
    let mut scroll = ScrollWidget::vertical(tall_content());
    // Manually set offset.
    scroll.set_scroll_offset(100.0, 320.0, 100.0);
    assert!(scroll.scroll_offset() > 0.0);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
        focused_widget: None,
    };

    let event = KeyEvent {
        key: Key::Home,
        modifiers: Modifiers::NONE,
    };
    let resp = scroll.handle_key(event, &ctx);
    assert!(resp.response.is_handled());
    assert_eq!(scroll.scroll_offset(), 0.0);
}

#[test]
fn key_end_scrolls_to_bottom() {
    // tall_content = 320px tall, viewport 100px → max offset 220.
    let mut scroll = ScrollWidget::vertical(tall_content());

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
        focused_widget: None,
    };

    let event = KeyEvent {
        key: Key::End,
        modifiers: Modifiers::NONE,
    };
    let resp = scroll.handle_key(event, &ctx);
    assert!(resp.response.is_handled());
    // Content 320px, view 100px → max offset = 220.
    assert_eq!(scroll.scroll_offset(), 220.0);
}

#[test]
fn key_arrow_down_scrolls() {
    // tall_content = 320px tall, viewport 100px.
    let mut scroll = ScrollWidget::vertical(tall_content());

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
        focused_widget: None,
    };

    let event = KeyEvent {
        key: Key::ArrowDown,
        modifiers: Modifiers::NONE,
    };
    let resp = scroll.handle_key(event, &ctx);
    assert!(resp.response.is_handled());
    // Should have scrolled down by line_height (20px).
    assert_eq!(scroll.scroll_offset(), 20.0);
}

// --- Edge cases from Chromium/Ratatui audit ---

#[test]
fn scroll_clip_rect_matches_viewport() {
    let scroll = make_scroll(tall_content());
    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let bounds = Rect::new(10.0, 20.0, 150.0, 80.0);
    let anim_flag = std::cell::Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: None,
        now: std::time::Instant::now(),
        animations_running: &anim_flag,
    };
    scroll.draw(&mut ctx);

    // The PushClip should use the scroll widget's bounds exactly.
    let clip = draw_list.commands().iter().find_map(|c| match c {
        DrawCommand::PushClip { rect } => Some(*rect),
        _ => None,
    });
    assert_eq!(clip, Some(bounds), "clip rect must match scroll viewport");
}

#[test]
fn scroll_child_drawn_offset_by_scroll() {
    let mut scroll = make_scroll(tall_content());
    scroll.set_scroll_offset(40.0, 320.0, 100.0);

    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let anim_flag = std::cell::Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: None,
        now: std::time::Instant::now(),
        animations_running: &anim_flag,
    };
    scroll.draw(&mut ctx);

    // Find the first Text command — it should be offset by -40px vertically.
    let first_text = draw_list.commands().iter().find_map(|c| match c {
        DrawCommand::Text { position, .. } => Some(*position),
        _ => None,
    });
    assert!(first_text.is_some(), "should have text commands");
    let pos = first_text.unwrap();
    assert_eq!(pos.y, -40.0, "text y should be offset by scroll amount");
}

#[test]
fn scroll_draws_scrollbar_when_overflowing() {
    let scroll = make_scroll(tall_content());
    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    // Viewport 100px < content 320px → scrollbar should appear.
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let anim_flag = std::cell::Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: None,
        now: std::time::Instant::now(),
        animations_running: &anim_flag,
    };
    scroll.draw(&mut ctx);

    // Should have a Rect command after PopClip (the scrollbar thumb).
    let after_pop = draw_list
        .commands()
        .iter()
        .skip_while(|c| !matches!(c, DrawCommand::PopClip))
        .filter(|c| matches!(c, DrawCommand::Rect { .. }))
        .count();
    assert!(
        after_pop >= 1,
        "scrollbar thumb rect should be drawn after clip"
    );
}

#[test]
fn scroll_no_scrollbar_when_content_fits() {
    let scroll = make_scroll(short_content());
    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    // Viewport 100px > content 16px → no scrollbar.
    let bounds = Rect::new(0.0, 0.0, 1000.0, 100.0);
    let anim_flag = std::cell::Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: None,
        now: std::time::Instant::now(),
        animations_running: &anim_flag,
    };
    scroll.draw(&mut ctx);

    // No Rect commands after PopClip (no scrollbar).
    let after_pop = draw_list
        .commands()
        .iter()
        .skip_while(|c| !matches!(c, DrawCommand::PopClip))
        .filter(|c| matches!(c, DrawCommand::Rect { .. }))
        .count();
    assert_eq!(after_pop, 0, "no scrollbar when content fits");
}

#[test]
fn scroll_multiple_wheel_events_accumulate() {
    let mut scroll = ScrollWidget::vertical(tall_content());
    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
        focused_widget: None,
    };

    // Scroll down 3 times.
    for _ in 0..3 {
        let event = MouseEvent {
            kind: MouseEventKind::Scroll(ScrollDelta::Lines { x: 0.0, y: -1.0 }),
            pos: Point::new(25.0, 25.0),
            modifiers: Modifiers::NONE,
        };
        scroll.handle_mouse(&event, &ctx);
    }

    // 3 lines * 20px line_height = 60px offset.
    assert_eq!(scroll.scroll_offset(), 60.0);
}

#[test]
fn scroll_wheel_clamps_at_bottom() {
    let mut scroll = ScrollWidget::vertical(tall_content());
    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
        focused_widget: None,
    };

    // Scroll way past the bottom.
    let event = MouseEvent {
        kind: MouseEventKind::Scroll(ScrollDelta::Lines { x: 0.0, y: -999.0 }),
        pos: Point::new(25.0, 25.0),
        modifiers: Modifiers::NONE,
    };
    scroll.handle_mouse(&event, &ctx);

    // Content 320px, viewport 100px → max offset 220.
    assert_eq!(scroll.scroll_offset(), 220.0);
}

#[test]
fn scroll_wheel_clamps_at_top() {
    let mut scroll = ScrollWidget::vertical(tall_content());
    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
        focused_widget: None,
    };

    // Scroll up from top (should stay at 0).
    let event = MouseEvent {
        kind: MouseEventKind::Scroll(ScrollDelta::Lines { x: 0.0, y: 5.0 }),
        pos: Point::new(25.0, 25.0),
        modifiers: Modifiers::NONE,
    };
    scroll.handle_mouse(&event, &ctx);
    assert_eq!(scroll.scroll_offset(), 0.0);
}

#[test]
fn scroll_pixel_delta_works() {
    let mut scroll = ScrollWidget::vertical(tall_content());
    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
        focused_widget: None,
    };

    // Trackpad-style pixel delta.
    let event = MouseEvent {
        kind: MouseEventKind::Scroll(ScrollDelta::Pixels { x: 0.0, y: -35.0 }),
        pos: Point::new(25.0, 25.0),
        modifiers: Modifiers::NONE,
    };
    scroll.handle_mouse(&event, &ctx);
    assert_eq!(scroll.scroll_offset(), 35.0);
}

#[test]
fn scroll_delegates_non_scroll_mouse_to_child() {
    // Button inside scroll container — click should reach it.
    let btn = ButtonWidget::new("Click");
    let btn_id = btn.id();
    let mut scroll = ScrollWidget::vertical(Box::new(btn));

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
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
    let _ = scroll.handle_mouse(&down, &ctx);

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(10.0, 10.0),
        modifiers: Modifiers::NONE,
    };
    let resp = scroll.handle_mouse(&up, &ctx);

    match resp.action {
        Some(crate::widgets::WidgetAction::Clicked(id)) => assert_eq!(id, btn_id),
        other => panic!("expected Clicked through scroll, got {other:?}"),
    }
}

#[test]
fn arrow_up_scrolls_upward() {
    let mut scroll = ScrollWidget::vertical(tall_content());
    // Start scrolled down.
    scroll.set_scroll_offset(100.0, 320.0, 100.0);

    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: true,
        focused_widget: None,
    };

    let event = KeyEvent {
        key: Key::ArrowUp,
        modifiers: Modifiers::NONE,
    };
    scroll.handle_key(event, &ctx);
    assert_eq!(scroll.scroll_offset(), 80.0); // 100 - 20
}

// --- Horizontal and both-direction tests (Chromium scroll view patterns) ---

/// Creates a wide row of labels that overflows a narrow viewport.
/// 20 labels * 8px * 10 chars = 1600px wide.
fn wide_content() -> Box<dyn Widget> {
    let labels: Vec<Box<dyn Widget>> = (0..20)
        .map(|i| Box::new(LabelWidget::new(format!("HorizLbl{i}"))) as Box<dyn Widget>)
        .collect();
    Box::new(FlexWidget::row(labels))
}

#[test]
fn horizontal_scroll_new_constructor() {
    let scroll = ScrollWidget::new(wide_content(), super::ScrollDirection::Horizontal);
    assert_eq!(scroll.scroll_offset(), 0.0);
    assert!(scroll.is_focusable());
}

#[test]
fn both_direction_new_constructor() {
    let scroll = ScrollWidget::new(tall_content(), super::ScrollDirection::Both);
    assert_eq!(scroll.scroll_offset(), 0.0);
    assert!(scroll.is_focusable());
}

#[test]
fn horizontal_scroll_draws_with_clip() {
    let scroll = ScrollWidget::new(wide_content(), super::ScrollDirection::Horizontal);
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
    scroll.draw(&mut ctx);

    // Clip should be balanced.
    let push_count = draw_list
        .commands()
        .iter()
        .filter(|c| matches!(c, DrawCommand::PushClip { .. }))
        .count();
    let pop_count = draw_list
        .commands()
        .iter()
        .filter(|c| matches!(c, DrawCommand::PopClip))
        .count();
    assert_eq!(push_count, 1);
    assert_eq!(pop_count, 1);
}

#[test]
fn scroll_content_exactly_fits_viewport() {
    // When content height == viewport height, max offset should be 0.
    let mut scroll = ScrollWidget::vertical(tall_content());
    // tall_content = 320px. Set viewport to 320px.
    scroll.set_scroll_offset(50.0, 320.0, 320.0);
    assert_eq!(scroll.scroll_offset(), 0.0, "no scroll when content fits");
}

#[test]
fn scroll_content_exactly_fits_no_scrollbar() {
    // Content exactly fitting the viewport should not draw a scrollbar.
    let label = LabelWidget::new("A".repeat(10)); // 80px wide, 16px tall
    let scroll = ScrollWidget::vertical(Box::new(label));
    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    // Viewport exactly matches content height (16px).
    let bounds = Rect::new(0.0, 0.0, 200.0, 16.0);
    let anim_flag = std::cell::Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: None,
        now: std::time::Instant::now(),
        animations_running: &anim_flag,
    };
    scroll.draw(&mut ctx);

    // No scrollbar rects after PopClip.
    let after_pop = draw_list
        .commands()
        .iter()
        .skip_while(|c| !matches!(c, DrawCommand::PopClip))
        .filter(|c| matches!(c, DrawCommand::Rect { .. }))
        .count();
    assert_eq!(after_pop, 0, "no scrollbar when content exactly fits");
}

#[test]
fn scroll_hover_delegates_to_child() {
    use crate::input::HoverEvent;

    let btn = ButtonWidget::new("HoverMe");
    let mut scroll = ScrollWidget::vertical(Box::new(btn));
    let measurer = MockMeasurer::STANDARD;
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let ctx = EventCtx {
        measurer: &measurer,
        bounds,
        is_focused: false,
        focused_widget: None,
    };

    // Hover should delegate to the child.
    let resp = scroll.handle_hover(HoverEvent::Enter, &ctx);
    // ButtonWidget returns redraw on hover enter.
    assert!(resp.response.is_handled());
}

#[test]
fn scroll_with_scrollbar_style() {
    use super::ScrollbarStyle;
    use crate::color::Color;

    let custom_style = ScrollbarStyle {
        width: 10.0,
        thumb_color: Color::WHITE,
        track_color: Color::BLACK,
        thumb_radius: 5.0,
        min_thumb_height: 30.0,
    };
    let scroll = ScrollWidget::vertical(tall_content()).with_scrollbar_style(custom_style);
    // Just verify it doesn't panic and produces valid output.
    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
    let anim_flag = std::cell::Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: None,
        now: std::time::Instant::now(),
        animations_running: &anim_flag,
    };
    scroll.draw(&mut ctx);
    assert!(!draw_list.is_empty());
}
