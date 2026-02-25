use crate::geometry::{Point, Rect};
use crate::input::{EventResponse, HoverEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{EventCtx, Widget, WidgetAction};

use super::WindowChromeWidget;
use super::constants::{
    CAPTION_HEIGHT, CAPTION_HEIGHT_MAXIMIZED, CONTROL_BUTTON_WIDTH, RESIZE_BORDER_WIDTH,
};
use super::controls::{ControlButtonColors, WindowControlButton};
use super::layout::{ChromeLayout, ControlKind};

// ── Test helpers ──

/// Standard window width for tests.
const TEST_WIDTH: f32 = 800.0;

/// Standard button colors for tests.
fn test_button_colors() -> ControlButtonColors {
    let theme = crate::theme::UiTheme::dark();
    ControlButtonColors {
        fg: crate::color::Color::WHITE,
        bg: crate::color::Color::TRANSPARENT,
        hover_bg: crate::color::Color::WHITE,
        close_hover_bg: theme.close_hover_bg,
        close_pressed_bg: theme.close_pressed_bg,
    }
}

/// Create an `EventCtx` with standard test dimensions.
fn make_ctx<'a>(measurer: &'a MockMeasurer, theme: &'a crate::theme::UiTheme) -> EventCtx<'a> {
    EventCtx {
        measurer,
        bounds: Rect::new(0.0, 0.0, TEST_WIDTH, CAPTION_HEIGHT),
        is_focused: false,
        focused_widget: None,
        theme,
    }
}

/// Left mouse button press at the given position.
fn left_down(x: f32, y: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(x, y),
        modifiers: Modifiers::NONE,
    }
}

/// Left mouse button release at the given position.
fn left_up(x: f32, y: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(x, y),
        modifiers: Modifiers::NONE,
    }
}

/// Center point of the close button in an 800px window.
fn close_center() -> (f32, f32) {
    let x = TEST_WIDTH - CONTROL_BUTTON_WIDTH / 2.0;
    (x, CAPTION_HEIGHT / 2.0)
}

/// Center point of the maximize button in an 800px window.
fn maximize_center() -> (f32, f32) {
    let x = TEST_WIDTH - CONTROL_BUTTON_WIDTH * 1.5;
    (x, CAPTION_HEIGHT / 2.0)
}

/// Center point of the minimize button in an 800px window.
fn minimize_center() -> (f32, f32) {
    let x = TEST_WIDTH - CONTROL_BUTTON_WIDTH * 2.5;
    (x, CAPTION_HEIGHT / 2.0)
}

// ── ChromeLayout tests ──

#[test]
fn layout_restored_caption_height() {
    let layout = ChromeLayout::compute(800.0, false, false);
    assert_eq!(layout.caption_height, CAPTION_HEIGHT);
    assert!(layout.visible);
}

#[test]
fn layout_maximized_caption_height() {
    let layout = ChromeLayout::compute(800.0, true, false);
    assert_eq!(layout.caption_height, CAPTION_HEIGHT_MAXIMIZED);
    assert!(layout.visible);
}

#[test]
fn layout_fullscreen_hidden() {
    let layout = ChromeLayout::compute(800.0, false, true);
    assert_eq!(layout.caption_height, 0.0);
    assert!(!layout.visible);
    assert!(
        layout
            .interactive_rects
            .iter()
            .all(|r| *r == Rect::default())
    );
}

#[test]
fn layout_three_control_buttons() {
    let layout = ChromeLayout::compute(800.0, false, false);
    assert_eq!(layout.controls.len(), 3);
    assert_eq!(layout.controls[0].kind, ControlKind::Minimize);
    assert_eq!(layout.controls[1].kind, ControlKind::MaximizeRestore);
    assert_eq!(layout.controls[2].kind, ControlKind::Close);
}

#[test]
fn layout_close_button_at_right_edge() {
    let width = 1024.0;
    let layout = ChromeLayout::compute(width, false, false);
    let close = layout.controls[2].rect;
    let expected_right = width;
    let epsilon = 0.001;
    assert!((close.right() - expected_right).abs() < epsilon);
    assert_eq!(close.width(), CONTROL_BUTTON_WIDTH);
}

#[test]
fn layout_buttons_ordered_right_to_left() {
    let layout = ChromeLayout::compute(1000.0, false, false);
    let min_x = layout.controls[0].rect.x();
    let max_x = layout.controls[1].rect.x();
    let close_x = layout.controls[2].rect.x();
    assert!(min_x < max_x);
    assert!(max_x < close_x);
}

#[test]
fn layout_buttons_span_full_caption_height() {
    let layout = ChromeLayout::compute(800.0, false, false);
    for ctrl in &layout.controls {
        assert_eq!(ctrl.rect.height(), CAPTION_HEIGHT);
    }
}

#[test]
fn layout_maximized_buttons_span_full_caption_height() {
    let layout = ChromeLayout::compute(800.0, true, false);
    for ctrl in &layout.controls {
        assert_eq!(ctrl.rect.height(), CAPTION_HEIGHT_MAXIMIZED);
    }
}

#[test]
fn layout_title_rect_before_buttons() {
    let layout = ChromeLayout::compute(800.0, false, false);
    let title = layout.title_rect;
    let first_button = layout.controls[0].rect;
    assert_eq!(title.x(), RESIZE_BORDER_WIDTH);
    assert!(title.right() <= first_button.x() + 0.001);
}

#[test]
fn layout_interactive_rects_match_controls() {
    let layout = ChromeLayout::compute(800.0, false, false);
    assert_eq!(layout.interactive_rects.len(), 3);
    for (i, rect) in layout.interactive_rects.iter().enumerate() {
        assert_eq!(*rect, layout.controls[i].rect);
    }
}

#[test]
fn layout_narrow_window_title_rect_zero() {
    // Window too narrow for title (buttons take up most space).
    let width = CONTROL_BUTTON_WIDTH * 3.0 + 1.0;
    let layout = ChromeLayout::compute(width, false, false);
    assert!(layout.title_rect.width() >= 0.0);
}

// ── WindowControlButton tests ──

#[test]
fn control_button_kind() {
    let btn = WindowControlButton::new(ControlKind::Close, test_button_colors());
    assert_eq!(btn.kind(), ControlKind::Close);
}

#[test]
fn control_button_not_focusable() {
    let btn = WindowControlButton::new(ControlKind::Minimize, test_button_colors());
    assert!(!btn.is_focusable());
}

#[test]
fn control_button_hover_sets_pressed() {
    let mut btn = WindowControlButton::new(ControlKind::MaximizeRestore, test_button_colors());
    assert!(!btn.is_pressed());

    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = EventCtx {
        measurer: &measurer,
        bounds: Rect::new(0.0, 0.0, 46.0, 36.0),
        is_focused: false,
        focused_widget: None,
        theme: &theme,
    };

    let event = left_down(23.0, 18.0);
    btn.handle_mouse(&event, &ctx);
    assert!(btn.is_pressed());
}

// ── WindowChromeWidget tests ──

#[test]
fn chrome_widget_caption_height() {
    let chrome = WindowChromeWidget::new("test", 800.0);
    assert_eq!(chrome.caption_height(), CAPTION_HEIGHT);
}

#[test]
fn chrome_widget_fullscreen_invisible() {
    let mut chrome = WindowChromeWidget::new("test", 800.0);
    chrome.set_fullscreen(true);
    assert!(!chrome.is_visible());
    assert_eq!(chrome.caption_height(), 0.0);
}

#[test]
fn chrome_widget_maximized_caption_height() {
    let mut chrome = WindowChromeWidget::new("test", 800.0);
    chrome.set_maximized(true);
    assert_eq!(chrome.caption_height(), CAPTION_HEIGHT_MAXIMIZED);
}

#[test]
fn chrome_widget_interactive_rects_three_buttons() {
    let chrome = WindowChromeWidget::new("test", 800.0);
    assert_eq!(chrome.interactive_rects().len(), 3);
}

#[test]
fn chrome_widget_resize_updates_layout() {
    let mut chrome = WindowChromeWidget::new("test", 800.0);
    let old_close_x = chrome.interactive_rects()[2].x();

    chrome.set_window_width(1200.0);
    let new_close_x = chrome.interactive_rects()[2].x();

    // Close button should move right when window widens.
    assert!(new_close_x > old_close_x);
}

#[test]
fn chrome_widget_set_title() {
    let mut chrome = WindowChromeWidget::new("ori", 800.0);
    chrome.set_title("new title".into());
    // Verify no panic — title is internal, tested through draw path.
}

#[test]
fn chrome_widget_active_inactive() {
    let mut chrome = WindowChromeWidget::new("test", 800.0);
    chrome.set_active(false);
    chrome.set_active(true);
    // Verify no panic — colors tested through draw path.
}

// ── Click → action emission ──

#[test]
fn click_close_emits_window_close() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (cx, cy) = close_center();
    chrome.handle_mouse(&left_down(cx, cy), &ctx);
    let resp = chrome.handle_mouse(&left_up(cx, cy), &ctx);

    assert_eq!(resp.action, Some(WidgetAction::WindowClose));
}

#[test]
fn click_maximize_emits_window_maximize() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (cx, cy) = maximize_center();
    chrome.handle_mouse(&left_down(cx, cy), &ctx);
    let resp = chrome.handle_mouse(&left_up(cx, cy), &ctx);

    assert_eq!(resp.action, Some(WidgetAction::WindowMaximize));
}

#[test]
fn click_minimize_emits_window_minimize() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (cx, cy) = minimize_center();
    chrome.handle_mouse(&left_down(cx, cy), &ctx);
    let resp = chrome.handle_mouse(&left_up(cx, cy), &ctx);

    assert_eq!(resp.action, Some(WidgetAction::WindowMinimize));
}

// ── Drag-off cancels action ──

#[test]
fn press_close_release_outside_no_action() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (cx, cy) = close_center();
    chrome.handle_mouse(&left_down(cx, cy), &ctx);

    // Release in the title area (far from any button).
    let resp = chrome.handle_mouse(&left_up(100.0, cy), &ctx);

    assert!(resp.action.is_none(), "drag-off should cancel action");
}

#[test]
fn press_minimize_release_on_maximize_no_action() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (min_x, min_y) = minimize_center();
    chrome.handle_mouse(&left_down(min_x, min_y), &ctx);

    // Release on the maximize button instead.
    let (max_x, max_y) = maximize_center();
    let resp = chrome.handle_mouse(&left_up(max_x, max_y), &ctx);

    assert!(
        resp.action.is_none(),
        "releasing on a different button should not emit an action",
    );
}

// ── Hover state machine (update_hover) ──

#[test]
fn hover_enter_close_requests_redraw() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (cx, cy) = close_center();
    let resp = chrome.update_hover(Point::new(cx, cy), &ctx);

    assert_eq!(resp.response, EventResponse::RequestRedraw);
}

#[test]
fn hover_move_between_buttons_requests_redraw() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (cx, cy) = close_center();
    chrome.update_hover(Point::new(cx, cy), &ctx);

    // Move to maximize button.
    let (mx, my) = maximize_center();
    let resp = chrome.update_hover(Point::new(mx, my), &ctx);

    assert_eq!(
        resp.response,
        EventResponse::RequestRedraw,
        "transitioning between buttons should redraw",
    );
}

#[test]
fn hover_same_button_twice_is_noop() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (cx, cy) = close_center();
    chrome.update_hover(Point::new(cx, cy), &ctx);

    // Same button again — no state change.
    let resp = chrome.update_hover(Point::new(cx, cy), &ctx);

    assert_eq!(
        resp.response,
        EventResponse::Ignored,
        "re-hovering same button should not trigger redraw",
    );
}

#[test]
fn hover_leave_to_title_area_requests_redraw() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (cx, cy) = close_center();
    chrome.update_hover(Point::new(cx, cy), &ctx);

    // Move to the title area (no button).
    let resp = chrome.update_hover(Point::new(100.0, cy), &ctx);

    assert_eq!(
        resp.response,
        EventResponse::RequestRedraw,
        "leaving all buttons should redraw",
    );
}

// ── Hover leave clears state ──

#[test]
fn handle_hover_leave_clears_hovered_control() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    // Hover over close button.
    let (cx, cy) = close_center();
    chrome.update_hover(Point::new(cx, cy), &ctx);

    // Cursor leaves the chrome area entirely.
    let resp = chrome.handle_hover(HoverEvent::Leave, &ctx);

    assert_eq!(
        resp.response,
        EventResponse::RequestRedraw,
        "hover leave should request redraw to clear highlight",
    );

    // Re-entering the same button should trigger a new redraw
    // (proves the old hover was cleared).
    let resp = chrome.update_hover(Point::new(cx, cy), &ctx);
    assert_eq!(
        resp.response,
        EventResponse::RequestRedraw,
        "re-entering after leave should redraw",
    );
}

#[test]
fn handle_hover_leave_without_prior_hover_is_ignored() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    // Leave without ever hovering — should be a no-op.
    let resp = chrome.handle_hover(HoverEvent::Leave, &ctx);

    assert_eq!(resp.response, EventResponse::Ignored);
}

// ── Fullscreen ignores events ──

#[test]
fn fullscreen_ignores_mouse_down() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    chrome.set_fullscreen(true);

    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (cx, cy) = close_center();
    let resp = chrome.handle_mouse(&left_down(cx, cy), &ctx);

    assert_eq!(resp.response, EventResponse::Ignored);
    assert!(resp.action.is_none());
}

#[test]
fn fullscreen_ignores_hover() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    chrome.set_fullscreen(true);

    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (cx, cy) = close_center();
    let resp = chrome.update_hover(Point::new(cx, cy), &ctx);

    assert_eq!(resp.response, EventResponse::Ignored);
}

// ── Non-left buttons ignored ──

#[test]
fn right_click_on_control_ignored() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (cx, cy) = close_center();
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        pos: Point::new(cx, cy),
        modifiers: Modifiers::NONE,
    };
    let resp = chrome.handle_mouse(&event, &ctx);

    assert_eq!(resp.response, EventResponse::Ignored);
    assert!(resp.action.is_none());
}

#[test]
fn middle_click_on_control_ignored() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    let (cx, cy) = close_center();
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Middle),
        pos: Point::new(cx, cy),
        modifiers: Modifiers::NONE,
    };
    let resp = chrome.handle_mouse(&event, &ctx);

    assert_eq!(resp.response, EventResponse::Ignored);
    assert!(resp.action.is_none());
}

// ── control_at_point boundary cases ──

#[test]
fn click_at_exact_button_left_edge_hits() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    // Close button left edge: x = 800 - 46 = 754.
    let edge_x = TEST_WIDTH - CONTROL_BUTTON_WIDTH;
    chrome.handle_mouse(&left_down(edge_x, CAPTION_HEIGHT / 2.0), &ctx);
    let resp = chrome.handle_mouse(&left_up(edge_x, CAPTION_HEIGHT / 2.0), &ctx);

    assert_eq!(
        resp.action,
        Some(WidgetAction::WindowClose),
        "left edge (inclusive) should hit the close button",
    );
}

#[test]
fn click_at_exact_button_right_edge_misses() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    // Close button right edge: x = 800 (half-open, exclusive).
    let resp = chrome.handle_mouse(&left_down(TEST_WIDTH, CAPTION_HEIGHT / 2.0), &ctx);

    assert_eq!(
        resp.response,
        EventResponse::Ignored,
        "right edge (exclusive) should miss the close button",
    );
}

#[test]
fn click_between_maximize_and_close_hits_close() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    // Boundary between maximize and close: x = 800 - 46 = 754.
    // This is the left edge of close (inclusive) and right edge of
    // maximize (exclusive), so it should hit close.
    let boundary = TEST_WIDTH - CONTROL_BUTTON_WIDTH;
    chrome.handle_mouse(&left_down(boundary, CAPTION_HEIGHT / 2.0), &ctx);
    let resp = chrome.handle_mouse(&left_up(boundary, CAPTION_HEIGHT / 2.0), &ctx);

    assert_eq!(resp.action, Some(WidgetAction::WindowClose));
}

#[test]
fn click_1px_left_of_minimize_hits_title_area() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    // Minimize left edge: x = 800 - 3*46 = 662. Point at 661.9 misses.
    let just_outside = TEST_WIDTH - CONTROL_BUTTON_WIDTH * 3.0 - 0.1;
    let resp = chrome.handle_mouse(&left_down(just_outside, CAPTION_HEIGHT / 2.0), &ctx);

    assert_eq!(
        resp.response,
        EventResponse::Ignored,
        "point just left of minimize should be in the title area",
    );
}

#[test]
fn click_below_caption_height_ignored() {
    let mut chrome = WindowChromeWidget::new("test", TEST_WIDTH);
    let measurer = MockMeasurer::STANDARD;
    let theme = crate::theme::UiTheme::dark();
    let ctx = make_ctx(&measurer, &theme);

    // Point below the caption height (bottom edge is exclusive).
    let (cx, _) = close_center();
    let resp = chrome.handle_mouse(&left_down(cx, CAPTION_HEIGHT), &ctx);

    assert_eq!(
        resp.response,
        EventResponse::Ignored,
        "point at y == caption_height should miss (half-open)",
    );
}
