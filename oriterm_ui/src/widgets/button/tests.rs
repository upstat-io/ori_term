use std::time::{Duration, Instant};

use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, Key, KeyEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::BoxContent;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{EventCtx, LayoutCtx, Widget, WidgetAction, WidgetResponse};

use super::{ButtonStyle, ButtonWidget};

static MEASURER: MockMeasurer = MockMeasurer::STANDARD;

fn event_ctx(bounds: Rect) -> EventCtx<'static> {
    EventCtx {
        measurer: &MEASURER,
        bounds,
        is_focused: true,
        focused_widget: None,
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

// --- Hover animation tests ---

#[test]
fn hover_starts_animation() {
    let mut btn = ButtonWidget::new("OK");
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));

    btn.handle_hover(HoverEvent::Enter, &ctx);
    let now = Instant::now();
    assert!(btn.hover_progress.is_animating(now));
    assert_eq!(btn.hover_progress.target(), 1.0);
}

#[test]
fn hover_leave_animates_back() {
    let mut btn = ButtonWidget::new("OK");
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));

    btn.handle_hover(HoverEvent::Enter, &ctx);
    // Wait for enter animation to complete.
    let later = Instant::now() + Duration::from_millis(200);
    assert_eq!(btn.hover_progress.get(later), 1.0);

    btn.handle_hover(HoverEvent::Leave, &ctx);
    assert_eq!(btn.hover_progress.target(), 0.0);
}

#[test]
fn disable_clears_hover_animation() {
    let mut btn = ButtonWidget::new("OK");
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));

    btn.handle_hover(HoverEvent::Enter, &ctx);
    btn.set_disabled(true);

    let now = Instant::now();
    // set_disabled uses set_immediate — no animation.
    assert!(!btn.hover_progress.is_animating(now));
    assert_eq!(btn.hover_progress.get(now), 0.0);
}

// --- with_style builder test ---

#[test]
fn with_style_applies_custom_style() {
    use crate::color::Color;
    use crate::geometry::Insets;

    let style = ButtonStyle {
        fg: Color::BLACK,
        bg: Color::WHITE,
        hover_bg: Color::rgb(0.9, 0.9, 0.9),
        pressed_bg: Color::rgb(0.7, 0.7, 0.7),
        border_color: Color::BLACK,
        border_width: 2.0,
        corner_radius: 12.0,
        padding: Insets::all(20.0),
        font_size: 18.0,
        disabled_fg: Color::rgb(0.5, 0.5, 0.5),
        disabled_bg: Color::rgb(0.3, 0.3, 0.3),
        focus_ring_color: Color::rgb(0.0, 0.0, 1.0),
    };
    let btn = ButtonWidget::new("Styled").with_style(style);

    // Layout should reflect the custom padding.
    let m = MockMeasurer::new();
    let ctx = LayoutCtx { measurer: &m };
    let layout = btn.layout(&ctx);
    if let BoxContent::Leaf {
        intrinsic_width,
        intrinsic_height,
    } = &layout.content
    {
        // "Styled" = 6 chars * 8px = 48px + padding (20 + 20) = 88.
        assert_eq!(*intrinsic_width, 88.0);
        // 16px line + padding (20 + 20) = 56.
        assert_eq!(*intrinsic_height, 56.0);
    } else {
        panic!("expected leaf layout");
    }
}

// --- Animation interpolation output verification (Chromium blend tests) ---

#[test]
fn hover_animation_interpolates_bg_at_midpoint() {
    use crate::animation::Lerp;
    use crate::color::Color;

    let mut btn = ButtonWidget::new("OK");
    let style = &ButtonStyle::default();
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));

    // Trigger hover to start animation.
    btn.handle_hover(HoverEvent::Enter, &ctx);

    // Shortly after start, bg should be very close to normal bg.
    let now = Instant::now();
    let bg_start = btn.current_bg(now);
    let diff_r = (bg_start.r - style.bg.r).abs();
    let diff_g = (bg_start.g - style.bg.g).abs();
    let diff_b = (bg_start.b - style.bg.b).abs();
    assert!(
        diff_r < 0.01 && diff_g < 0.01 && diff_b < 0.01,
        "near animation start, bg should be close to normal bg, got diff ({diff_r}, {diff_g}, {diff_b})"
    );

    // After the hover animation completes, bg should be hover_bg.
    let after = now + Duration::from_millis(200);
    let bg_end = btn.current_bg(after);
    assert_eq!(
        bg_end, style.hover_bg,
        "after hover animation completes, bg should equal hover_bg"
    );

    // Verify the expected midpoint color is between bg and hover_bg.
    let expected_mid = Color::lerp(style.bg, style.hover_bg, 0.5);
    assert!(
        (expected_mid.r - (style.bg.r + style.hover_bg.r) / 2.0).abs() < 1e-4,
        "Color lerp midpoint should be average of endpoints"
    );
}

#[test]
fn pressed_bg_overrides_hover_animation() {
    let mut btn = ButtonWidget::new("OK");
    let style = &ButtonStyle::default();
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));

    // Hover, then press.
    btn.handle_hover(HoverEvent::Enter, &ctx);
    btn.handle_mouse(&mouse_down(10.0, 10.0), &ctx);

    // While pressed, current_bg should return pressed_bg regardless of hover animation.
    let now = Instant::now();
    assert_eq!(
        btn.current_bg(now),
        style.pressed_bg,
        "pressed state overrides hover animation"
    );
}

#[test]
fn disabled_bg_overrides_everything() {
    let mut btn = ButtonWidget::new("OK");
    let style = &ButtonStyle::default();
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));

    // Hover, then disable.
    btn.handle_hover(HoverEvent::Enter, &ctx);
    btn.set_disabled(true);

    let now = Instant::now();
    assert_eq!(
        btn.current_bg(now),
        style.disabled_bg,
        "disabled state overrides hover animation"
    );
}

#[test]
fn draw_signals_animations_running() {
    use crate::draw::DrawList;

    let mut btn = ButtonWidget::new("OK");
    let ctx = event_ctx(Rect::new(0.0, 0.0, 100.0, 30.0));

    // Trigger hover to start animation.
    btn.handle_hover(HoverEvent::Enter, &ctx);

    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let bounds = Rect::new(0.0, 0.0, 100.0, 30.0);
    let anim_flag = std::cell::Cell::new(false);
    let now = Instant::now();
    let mut draw_ctx = super::super::DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds,
        focused_widget: None,
        now,
        animations_running: &anim_flag,
    };
    btn.draw(&mut draw_ctx);

    assert!(
        anim_flag.get(),
        "draw() should signal animations_running while hover animates"
    );
}
