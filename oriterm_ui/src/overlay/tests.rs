use std::cell::Cell;
use std::time::Instant;

use crate::draw::{DrawCommand, DrawList};
use crate::geometry::{Point, Rect, Size};
use crate::input::{HoverEvent, Key, KeyEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::widgets::button::ButtonWidget;
use crate::widgets::label::LabelWidget;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{DrawCtx, Widget, WidgetAction};

use super::OverlayManager;
use super::manager::OverlayEventResult;
use super::overlay_id::OverlayId;
use super::placement::{Placement, compute_overlay_rect};

fn viewport() -> Rect {
    Rect::new(0.0, 0.0, 800.0, 600.0)
}

fn anchor() -> Rect {
    Rect::new(100.0, 100.0, 80.0, 30.0)
}

fn content_size() -> Size {
    Size::new(120.0, 40.0)
}

fn label_widget(text: &str) -> Box<dyn Widget> {
    Box::new(LabelWidget::new(text))
}

fn button_widget(text: &str) -> Box<ButtonWidget> {
    Box::new(ButtonWidget::new(text))
}

fn mouse_down(x: f32, y: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        pos: Point::new(x, y),
        modifiers: Modifiers::NONE,
    }
}

fn mouse_move(x: f32, y: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Move,
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

// --- Placement tests (pure function) ---

#[test]
fn placement_below_fits() {
    let rect = compute_overlay_rect(anchor(), content_size(), viewport(), Placement::Below);
    // Below anchor: y = 100 + 30 + 4 = 134.
    assert_eq!(rect.y(), 134.0);
    // Left-aligned with anchor.
    assert_eq!(rect.x(), 100.0);
    assert_eq!(rect.width(), 120.0);
    assert_eq!(rect.height(), 40.0);
}

#[test]
fn placement_below_flips_to_above() {
    // Anchor near bottom — not enough room below.
    let anchor = Rect::new(100.0, 570.0, 80.0, 20.0);
    let rect = compute_overlay_rect(anchor, content_size(), viewport(), Placement::Below);
    // Should flip above: y = 570 - 40 - 4 = 526.
    assert_eq!(rect.y(), 526.0);
}

#[test]
fn placement_above_fits() {
    let rect = compute_overlay_rect(anchor(), content_size(), viewport(), Placement::Above);
    // Above anchor: y = 100 - 40 - 4 = 56.
    assert_eq!(rect.y(), 56.0);
    assert_eq!(rect.x(), 100.0);
}

#[test]
fn placement_above_flips_to_below() {
    // Anchor near top — not enough room above.
    let anchor = Rect::new(100.0, 10.0, 80.0, 20.0);
    let rect = compute_overlay_rect(anchor, content_size(), viewport(), Placement::Above);
    // Should flip below: y = 10 + 20 + 4 = 34.
    assert_eq!(rect.y(), 34.0);
}

#[test]
fn placement_right_fits() {
    let rect = compute_overlay_rect(anchor(), content_size(), viewport(), Placement::Right);
    // Right of anchor: x = 100 + 80 + 4 = 184.
    assert_eq!(rect.x(), 184.0);
    // Top-aligned with anchor.
    assert_eq!(rect.y(), 100.0);
}

#[test]
fn placement_right_flips_to_left() {
    // Anchor near right edge.
    let anchor = Rect::new(700.0, 100.0, 80.0, 30.0);
    let rect = compute_overlay_rect(anchor, content_size(), viewport(), Placement::Right);
    // Should flip left: x = 700 - 120 - 4 = 576.
    assert_eq!(rect.x(), 576.0);
}

#[test]
fn placement_left_fits() {
    // Anchor with enough room to the left.
    let anchor = Rect::new(300.0, 100.0, 80.0, 30.0);
    let rect = compute_overlay_rect(anchor, content_size(), viewport(), Placement::Left);
    // Left of anchor: x = 300 - 120 - 4 = 176.
    assert_eq!(rect.x(), 176.0);
    assert_eq!(rect.y(), 100.0);
}

#[test]
fn placement_left_flips_to_right() {
    // Anchor near left edge — not enough room.
    let anchor = Rect::new(10.0, 100.0, 80.0, 30.0);
    let rect = compute_overlay_rect(anchor, content_size(), viewport(), Placement::Left);
    // Should flip right: x = 10 + 80 + 4 = 94.
    assert_eq!(rect.x(), 94.0);
}

#[test]
fn placement_center() {
    let rect = compute_overlay_rect(anchor(), content_size(), viewport(), Placement::Center);
    // Centered: x = (800 - 120) / 2 = 340, y = (600 - 40) / 2 = 280.
    assert_eq!(rect.x(), 340.0);
    assert_eq!(rect.y(), 280.0);
}

#[test]
fn placement_at_point_fits() {
    let pt = Point::new(200.0, 300.0);
    let rect = compute_overlay_rect(anchor(), content_size(), viewport(), Placement::AtPoint(pt));
    assert_eq!(rect.x(), 200.0);
    assert_eq!(rect.y(), 300.0);
}

#[test]
fn placement_at_point_clamps() {
    // Point near bottom-right corner — overlay should clamp.
    let pt = Point::new(750.0, 580.0);
    let rect = compute_overlay_rect(anchor(), content_size(), viewport(), Placement::AtPoint(pt));
    // Clamped: x = 800 - 120 = 680, y = 600 - 40 = 560.
    assert_eq!(rect.x(), 680.0);
    assert_eq!(rect.y(), 560.0);
}

#[test]
fn placement_clamp_x_alignment() {
    // Anchor at right edge — left-aligned x would overflow.
    let anchor = Rect::new(750.0, 100.0, 80.0, 30.0);
    let rect = compute_overlay_rect(anchor, content_size(), viewport(), Placement::Below);
    // x starts at 750, but 750 + 120 > 800 → clamped to 680.
    assert_eq!(rect.x(), 680.0);
}

#[test]
fn placement_tiny_viewport() {
    // Viewport smaller than content — pinned to top-left of viewport.
    let small_vp = Rect::new(0.0, 0.0, 50.0, 20.0);
    let anchor = Rect::new(0.0, 0.0, 10.0, 10.0);
    let rect = compute_overlay_rect(anchor, content_size(), small_vp, Placement::Below);
    assert_eq!(rect.x(), 0.0);
    assert_eq!(rect.y(), 0.0);
}

#[test]
fn placement_zero_size_content() {
    let zero = Size::new(0.0, 0.0);
    let rect = compute_overlay_rect(anchor(), zero, viewport(), Placement::Below);
    assert_eq!(rect.width(), 0.0);
    assert_eq!(rect.height(), 0.0);
}

#[test]
fn placement_anchor_at_viewport_edge() {
    // Anchor exactly at bottom-right corner of viewport.
    let anchor = Rect::new(800.0, 600.0, 0.0, 0.0);
    let rect = compute_overlay_rect(anchor, content_size(), viewport(), Placement::Below);
    // Should clamp to viewport.
    assert!(rect.x() + rect.width() <= viewport().right());
    assert!(rect.y() + rect.height() <= viewport().bottom());
}

// --- OverlayId tests ---

#[test]
fn overlay_ids_are_unique() {
    let a = OverlayId::next();
    let b = OverlayId::next();
    let c = OverlayId::next();
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
}

#[test]
fn overlay_id_display() {
    let id = OverlayId::next();
    let s = format!("{id}");
    assert!(s.parse::<u64>().is_ok());
}

#[test]
fn overlay_id_debug() {
    let id = OverlayId::next();
    let s = format!("{id:?}");
    assert!(s.starts_with("OverlayId("));
}

// --- Manager lifecycle tests ---

#[test]
fn manager_starts_empty() {
    let mgr = OverlayManager::new(viewport());
    assert!(mgr.is_empty());
    assert_eq!(mgr.count(), 0);
    assert!(!mgr.has_modal());
}

#[test]
fn push_overlay_increments_count() {
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_overlay(label_widget("A"), anchor(), Placement::Below);
    assert_eq!(mgr.count(), 1);
    assert!(!mgr.is_empty());
}

#[test]
fn push_modal_sets_has_modal() {
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_modal(label_widget("Modal"), anchor(), Placement::Center);
    assert!(mgr.has_modal());
}

#[test]
fn pop_overlay_by_id() {
    let mut mgr = OverlayManager::new(viewport());
    let id1 = mgr.push_overlay(label_widget("A"), anchor(), Placement::Below);
    let _id2 = mgr.push_overlay(label_widget("B"), anchor(), Placement::Below);

    assert!(mgr.pop_overlay(id1));
    assert_eq!(mgr.count(), 1);
    // Can't pop again.
    assert!(!mgr.pop_overlay(id1));
}

#[test]
fn pop_topmost() {
    let mut mgr = OverlayManager::new(viewport());
    let _id1 = mgr.push_overlay(label_widget("A"), anchor(), Placement::Below);
    let id2 = mgr.push_overlay(label_widget("B"), anchor(), Placement::Below);

    assert_eq!(mgr.pop_topmost(), Some(id2));
    assert_eq!(mgr.count(), 1);
}

#[test]
fn pop_topmost_empty() {
    let mut mgr = OverlayManager::new(viewport());
    assert_eq!(mgr.pop_topmost(), None);
}

#[test]
fn clear_all() {
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_overlay(label_widget("A"), anchor(), Placement::Below);
    mgr.push_overlay(label_widget("B"), anchor(), Placement::Below);
    mgr.push_modal(label_widget("C"), anchor(), Placement::Center);

    mgr.clear_all();
    assert!(mgr.is_empty());
    assert!(!mgr.has_modal());
}

#[test]
fn pop_nonexistent_id() {
    let mut mgr = OverlayManager::new(viewport());
    let fake_id = OverlayId::next();
    assert!(!mgr.pop_overlay(fake_id));
}

#[test]
fn multiple_overlays_ordering() {
    let mut mgr = OverlayManager::new(viewport());
    let _id1 = mgr.push_overlay(label_widget("A"), anchor(), Placement::Below);
    let _id2 = mgr.push_overlay(label_widget("B"), anchor(), Placement::Below);
    let id3 = mgr.push_overlay(label_widget("C"), anchor(), Placement::Below);

    // Topmost is the last pushed.
    assert_eq!(mgr.pop_topmost(), Some(id3));
    assert_eq!(mgr.count(), 2);
}

#[test]
fn overlay_rect_accessor() {
    let mut mgr = OverlayManager::new(viewport());
    let id = mgr.push_overlay(label_widget("Test"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let rect = mgr.overlay_rect(id);
    assert!(rect.is_some());
    let rect = rect.unwrap();
    assert!(rect.width() > 0.0);
    assert!(rect.height() > 0.0);
}

#[test]
fn overlay_rect_unknown_id() {
    let mgr = OverlayManager::new(viewport());
    let fake_id = OverlayId::next();
    assert!(mgr.overlay_rect(fake_id).is_none());
}

// --- Mouse routing tests ---

#[test]
fn mouse_pass_through_when_empty() {
    let mut mgr = OverlayManager::new(viewport());
    let event = mouse_down(50.0, 50.0);
    let result = mgr.process_mouse_event(&event, &MockMeasurer::STANDARD);
    assert!(matches!(result, OverlayEventResult::PassThrough));
}

#[test]
fn mouse_click_inside_overlay_delivers() {
    let mut mgr = OverlayManager::new(viewport());
    let id = mgr.push_overlay(button_widget("Click"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let rect = mgr.overlay_rect(id).unwrap();
    let event = mouse_down(rect.x() + 5.0, rect.y() + 5.0);
    let result = mgr.process_mouse_event(&event, &MockMeasurer::STANDARD);
    assert!(matches!(result, OverlayEventResult::Delivered { .. }));
}

#[test]
fn mouse_click_outside_dismisses() {
    let mut mgr = OverlayManager::new(viewport());
    let id = mgr.push_overlay(label_widget("Popup"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    // Click far from the overlay.
    let event = mouse_down(1.0, 1.0);
    let result = mgr.process_mouse_event(&event, &MockMeasurer::STANDARD);

    match result {
        OverlayEventResult::Dismissed(dismissed_id) => assert_eq!(dismissed_id, id),
        other => panic!("expected Dismissed, got {other:?}"),
    }
    assert!(mgr.is_empty());
}

#[test]
fn mouse_move_outside_does_not_dismiss() {
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_overlay(label_widget("Popup"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    // Mouse move (not click) outside should pass through, not dismiss.
    let event = mouse_move(1.0, 1.0);
    let result = mgr.process_mouse_event(&event, &MockMeasurer::STANDARD);
    assert!(matches!(result, OverlayEventResult::PassThrough));
    assert_eq!(mgr.count(), 1);
}

#[test]
fn mouse_click_outside_modal_blocks() {
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_modal(label_widget("Modal"), anchor(), Placement::Center);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let event = mouse_down(1.0, 1.0);
    let result = mgr.process_mouse_event(&event, &MockMeasurer::STANDARD);
    assert!(matches!(result, OverlayEventResult::Blocked));
    // Modal should still be there.
    assert_eq!(mgr.count(), 1);
}

#[test]
fn mouse_topmost_overlay_wins() {
    let mut mgr = OverlayManager::new(viewport());
    // Two overlays at the same position.
    let _id1 = mgr.push_overlay(button_widget("Back"), anchor(), Placement::Below);
    let id2 = mgr.push_overlay(button_widget("Front"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let rect = mgr.overlay_rect(id2).unwrap();
    let event = mouse_down(rect.x() + 5.0, rect.y() + 5.0);
    let result = mgr.process_mouse_event(&event, &MockMeasurer::STANDARD);

    match result {
        OverlayEventResult::Delivered { overlay_id, .. } => assert_eq!(overlay_id, id2),
        other => panic!("expected Delivered to topmost, got {other:?}"),
    }
}

// --- Key routing tests ---

#[test]
fn key_pass_through_when_empty() {
    let mut mgr = OverlayManager::new(viewport());
    let result = mgr.process_key_event(key_event(Key::Enter), &MockMeasurer::STANDARD);
    assert!(matches!(result, OverlayEventResult::PassThrough));
}

#[test]
fn escape_dismisses_topmost() {
    let mut mgr = OverlayManager::new(viewport());
    let _id1 = mgr.push_overlay(label_widget("A"), anchor(), Placement::Below);
    let id2 = mgr.push_overlay(label_widget("B"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let result = mgr.process_key_event(key_event(Key::Escape), &MockMeasurer::STANDARD);
    match result {
        OverlayEventResult::Dismissed(id) => assert_eq!(id, id2),
        other => panic!("expected Dismissed, got {other:?}"),
    }
    assert_eq!(mgr.count(), 1);
}

#[test]
fn escape_dismisses_modal() {
    let mut mgr = OverlayManager::new(viewport());
    let id = mgr.push_modal(label_widget("Modal"), anchor(), Placement::Center);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let result = mgr.process_key_event(key_event(Key::Escape), &MockMeasurer::STANDARD);
    match result {
        OverlayEventResult::Dismissed(dismissed_id) => assert_eq!(dismissed_id, id),
        other => panic!("expected Dismissed, got {other:?}"),
    }
}

#[test]
fn modal_never_passes_key_through() {
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_modal(label_widget("Modal"), anchor(), Placement::Center);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    // A random key that the label won't handle.
    let result = mgr.process_key_event(key_event(Key::ArrowDown), &MockMeasurer::STANDARD);
    // Modal should deliver (even if Ignored by widget), never PassThrough.
    assert!(matches!(result, OverlayEventResult::Delivered { .. }));
}

#[test]
fn non_modal_key_can_pass_through() {
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_overlay(label_widget("Popup"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    // Labels don't handle key events → should pass through.
    let result = mgr.process_key_event(key_event(Key::ArrowDown), &MockMeasurer::STANDARD);
    assert!(matches!(result, OverlayEventResult::PassThrough));
}

// --- Hover routing tests ---

#[test]
fn hover_pass_through_when_empty() {
    let mut mgr = OverlayManager::new(viewport());
    let result = mgr.process_hover_event(
        Point::new(50.0, 50.0),
        HoverEvent::Enter,
        &MockMeasurer::STANDARD,
    );
    assert!(matches!(result, OverlayEventResult::PassThrough));
}

#[test]
fn hover_inside_overlay_delivers() {
    let mut mgr = OverlayManager::new(viewport());
    let id = mgr.push_overlay(button_widget("Btn"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let rect = mgr.overlay_rect(id).unwrap();
    let result = mgr.process_hover_event(
        Point::new(rect.x() + 5.0, rect.y() + 5.0),
        HoverEvent::Enter,
        &MockMeasurer::STANDARD,
    );
    assert!(matches!(result, OverlayEventResult::Delivered { .. }));
}

#[test]
fn hover_outside_modal_blocks() {
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_modal(label_widget("Modal"), anchor(), Placement::Center);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let result = mgr.process_hover_event(
        Point::new(1.0, 1.0),
        HoverEvent::Enter,
        &MockMeasurer::STANDARD,
    );
    assert!(matches!(result, OverlayEventResult::Blocked));
}

#[test]
fn hover_outside_non_modal_passes_through() {
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_overlay(label_widget("Popup"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let result = mgr.process_hover_event(
        Point::new(1.0, 1.0),
        HoverEvent::Enter,
        &MockMeasurer::STANDARD,
    );
    assert!(matches!(result, OverlayEventResult::PassThrough));
}

// --- Drawing tests ---

#[test]
fn draw_empty_is_noop() {
    let mgr = OverlayManager::new(viewport());
    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let anim_flag = Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds: Rect::default(),
        focused_widget: None,
        now: Instant::now(),
        animations_running: &anim_flag,
    };
    mgr.draw_overlays(&mut ctx);
    assert!(draw_list.is_empty());
}

#[test]
fn draw_non_modal_no_dimming() {
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_overlay(label_widget("A"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let anim_flag = Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds: Rect::default(),
        focused_widget: None,
        now: Instant::now(),
        animations_running: &anim_flag,
    };
    mgr.draw_overlays(&mut ctx);

    // Should have text command but no dim rect.
    let has_rect = draw_list
        .commands()
        .iter()
        .any(|c| matches!(c, DrawCommand::Rect { .. }));
    assert!(!has_rect, "non-modal should not emit dim rect");
}

#[test]
fn draw_modal_emits_dimming_rect() {
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_modal(label_widget("Modal"), anchor(), Placement::Center);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let anim_flag = Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds: Rect::default(),
        focused_widget: None,
        now: Instant::now(),
        animations_running: &anim_flag,
    };
    mgr.draw_overlays(&mut ctx);

    // First command should be the dim rect covering the viewport.
    let first = &draw_list.commands()[0];
    match first {
        DrawCommand::Rect { rect, style } => {
            assert_eq!(*rect, viewport());
            assert!(style.fill.is_some());
            let fill = style.fill.unwrap();
            assert!(fill.a < 1.0, "dim rect should be semi-transparent");
            assert!(fill.a > 0.0, "dim rect should be visible");
        }
        other => panic!("expected Rect command for dimming, got {other:?}"),
    }
}

#[test]
fn draw_overlays_in_painter_order() {
    let mut mgr = OverlayManager::new(viewport());
    // Use different labels so we can distinguish them by glyph count.
    mgr.push_overlay(label_widget("AB"), anchor(), Placement::Below);
    mgr.push_overlay(label_widget("ABCDE"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let anim_flag = Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds: Rect::default(),
        focused_widget: None,
        now: Instant::now(),
        animations_running: &anim_flag,
    };
    mgr.draw_overlays(&mut ctx);

    let glyph_counts: Vec<usize> = draw_list
        .commands()
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text { shaped, .. } => Some(shaped.glyph_count()),
            _ => None,
        })
        .collect();
    // First overlay drawn first (back), second drawn last (front).
    assert_eq!(glyph_counts, vec![2, 5]);
}

// --- Focus tests ---

#[test]
fn modal_focus_order_returns_focusable_ids() {
    let mut mgr = OverlayManager::new(viewport());
    let btn = button_widget("Focus Me");
    let btn_id = btn.id();
    mgr.push_modal(btn, anchor(), Placement::Center);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let order = mgr.modal_focus_order();
    assert!(order.is_some());
    let ids = order.unwrap();
    assert!(ids.contains(&btn_id));
}

#[test]
fn no_modal_returns_none_focus_order() {
    let mgr = OverlayManager::new(viewport());
    assert!(mgr.modal_focus_order().is_none());
}

#[test]
fn non_modal_overlay_returns_none_focus_order() {
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_overlay(button_widget("Btn"), anchor(), Placement::Below);
    assert!(mgr.modal_focus_order().is_none());
}

// --- Viewport tests ---

#[test]
fn set_viewport_updates() {
    let mut mgr = OverlayManager::new(viewport());
    let new_vp = Rect::new(0.0, 0.0, 1024.0, 768.0);
    mgr.set_viewport(new_vp);
    assert_eq!(mgr.viewport(), new_vp);
}

// --- Integration: button click through overlay ---

#[test]
fn button_in_overlay_receives_click_action() {
    let mut mgr = OverlayManager::new(viewport());
    let btn = button_widget("Go");
    let btn_id = btn.id();
    let id = mgr.push_overlay(btn, anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let rect = mgr.overlay_rect(id).unwrap();

    // Down then up = click.
    let down = mouse_down(rect.x() + 5.0, rect.y() + 5.0);
    mgr.process_mouse_event(&down, &MockMeasurer::STANDARD);

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(rect.x() + 5.0, rect.y() + 5.0),
        modifiers: Modifiers::NONE,
    };
    let result = mgr.process_mouse_event(&up, &MockMeasurer::STANDARD);

    match result {
        OverlayEventResult::Delivered { response, .. } => {
            assert_eq!(response.action, Some(WidgetAction::Clicked(btn_id)));
        }
        other => panic!("expected Delivered with Clicked, got {other:?}"),
    }
}

// --- Edge cases from Chromium/WezTerm audit ---

#[test]
fn stacked_modals_inner_dismiss_restores_outer() {
    // Chromium pattern: modal on top of modal. Dismiss inner → outer active.
    let mut mgr = OverlayManager::new(viewport());
    let id1 = mgr.push_modal(label_widget("Outer"), anchor(), Placement::Center);
    let id2 = mgr.push_modal(label_widget("Inner"), anchor(), Placement::Center);
    assert_eq!(mgr.count(), 2);
    assert!(mgr.has_modal());

    // Escape dismisses inner modal.
    let result = mgr.process_key_event(key_event(Key::Escape), &MockMeasurer::STANDARD);
    match result {
        OverlayEventResult::Dismissed(id) => assert_eq!(id, id2),
        other => panic!("expected inner dismissed, got {other:?}"),
    }

    // Outer modal is still active.
    assert_eq!(mgr.count(), 1);
    assert!(mgr.has_modal());

    // Click outside is still blocked by outer modal.
    let event = mouse_down(1.0, 1.0);
    let result = mgr.process_mouse_event(&event, &MockMeasurer::STANDARD);
    assert!(matches!(result, OverlayEventResult::Blocked));

    // Second escape dismisses outer.
    let result = mgr.process_key_event(key_event(Key::Escape), &MockMeasurer::STANDARD);
    match result {
        OverlayEventResult::Dismissed(id) => assert_eq!(id, id1),
        other => panic!("expected outer dismissed, got {other:?}"),
    }
    assert!(mgr.is_empty());
}

#[test]
fn multiple_escapes_dismiss_stack_one_at_a_time() {
    let mut mgr = OverlayManager::new(viewport());
    let id1 = mgr.push_overlay(label_widget("A"), anchor(), Placement::Below);
    let id2 = mgr.push_overlay(label_widget("B"), anchor(), Placement::Below);
    let id3 = mgr.push_overlay(label_widget("C"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    // Three escapes should dismiss C, B, A in order.
    let r = mgr.process_key_event(key_event(Key::Escape), &MockMeasurer::STANDARD);
    assert!(matches!(r, OverlayEventResult::Dismissed(id) if id == id3));
    assert_eq!(mgr.count(), 2);

    let r = mgr.process_key_event(key_event(Key::Escape), &MockMeasurer::STANDARD);
    assert!(matches!(r, OverlayEventResult::Dismissed(id) if id == id2));
    assert_eq!(mgr.count(), 1);

    let r = mgr.process_key_event(key_event(Key::Escape), &MockMeasurer::STANDARD);
    assert!(matches!(r, OverlayEventResult::Dismissed(id) if id == id1));
    assert!(mgr.is_empty());

    // Fourth escape passes through (stack empty).
    let r = mgr.process_key_event(key_event(Key::Escape), &MockMeasurer::STANDARD);
    assert!(matches!(r, OverlayEventResult::PassThrough));
}

#[test]
fn scroll_outside_overlay_does_not_dismiss() {
    // Scroll events are not clicks — should not dismiss.
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_overlay(label_widget("Menu"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let scroll = MouseEvent {
        kind: MouseEventKind::Scroll(crate::input::ScrollDelta::Lines { x: 0.0, y: -3.0 }),
        pos: Point::new(1.0, 1.0),
        modifiers: Modifiers::NONE,
    };
    let result = mgr.process_mouse_event(&scroll, &MockMeasurer::STANDARD);
    assert!(matches!(result, OverlayEventResult::PassThrough));
    assert_eq!(mgr.count(), 1, "scroll should not dismiss overlay");
}

#[test]
fn right_click_outside_also_dismisses() {
    // Right-click is also a Down event — should dismiss non-modal overlay.
    let mut mgr = OverlayManager::new(viewport());
    let id = mgr.push_overlay(label_widget("Context"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let right_click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        pos: Point::new(1.0, 1.0),
        modifiers: Modifiers::NONE,
    };
    let result = mgr.process_mouse_event(&right_click, &MockMeasurer::STANDARD);
    match result {
        OverlayEventResult::Dismissed(dismissed_id) => assert_eq!(dismissed_id, id),
        other => panic!("expected Dismissed on right-click, got {other:?}"),
    }
}

#[test]
fn pop_middle_overlay_preserves_stack() {
    // Remove by ID when not topmost. Chromium tests stacking integrity.
    let mut mgr = OverlayManager::new(viewport());
    let id1 = mgr.push_overlay(label_widget("A"), anchor(), Placement::Below);
    let _id2 = mgr.push_overlay(label_widget("B"), anchor(), Placement::Below);
    let id3 = mgr.push_overlay(label_widget("C"), anchor(), Placement::Below);

    // Remove middle overlay.
    assert!(mgr.pop_overlay(_id2));
    assert_eq!(mgr.count(), 2);

    // Topmost should still be C.
    assert_eq!(mgr.pop_topmost(), Some(id3));
    // Then A.
    assert_eq!(mgr.pop_topmost(), Some(id1));
    assert!(mgr.is_empty());
}

#[test]
fn dismiss_topmost_reveals_overlay_below() {
    // Dismiss topmost → next overlay becomes active and receives events.
    let mut mgr = OverlayManager::new(viewport());
    let id1 = mgr.push_overlay(button_widget("Lower"), anchor(), Placement::Below);
    let _id2 = mgr.push_overlay(label_widget("Upper"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    // Escape removes Upper.
    mgr.process_key_event(key_event(Key::Escape), &MockMeasurer::STANDARD);
    assert_eq!(mgr.count(), 1);

    // Lower overlay should now receive events.
    let rect = mgr.overlay_rect(id1).unwrap();
    let event = mouse_down(rect.x() + 5.0, rect.y() + 5.0);
    let result = mgr.process_mouse_event(&event, &MockMeasurer::STANDARD);
    match result {
        OverlayEventResult::Delivered { overlay_id, .. } => assert_eq!(overlay_id, id1),
        other => panic!("expected Delivered to lower, got {other:?}"),
    }
}

#[test]
fn viewport_resize_relayouts_overlays() {
    // Chromium: window resize must reposition overlays.
    let mut mgr = OverlayManager::new(viewport());
    let id = mgr.push_overlay(label_widget("Popup"), anchor(), Placement::Center);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let rect_before = mgr.overlay_rect(id).unwrap();

    // Shrink viewport.
    let small_vp = Rect::new(0.0, 0.0, 400.0, 300.0);
    mgr.set_viewport(small_vp);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let rect_after = mgr.overlay_rect(id).unwrap();

    // Center placement should shift to new center.
    assert_ne!(
        rect_before.x(),
        rect_after.x(),
        "overlay should reposition on resize"
    );
    assert_ne!(
        rect_before.y(),
        rect_after.y(),
        "overlay should reposition on resize"
    );
    // Verify it's within new viewport.
    assert!(rect_after.x() >= 0.0);
    assert!(rect_after.y() >= 0.0);
    assert!(rect_after.right() <= small_vp.right());
    assert!(rect_after.bottom() <= small_vp.bottom());
}

#[test]
fn non_modal_over_modal_blocks_correctly() {
    // Mixed stack: modal at bottom, non-modal on top.
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_modal(label_widget("Modal Base"), anchor(), Placement::Center);
    mgr.push_overlay(label_widget("Popup"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    // Topmost is non-modal, so has_modal is false (checks topmost only).
    assert!(!mgr.has_modal());

    // Click outside both: topmost is non-modal with dismiss_on_click_outside.
    let event = mouse_down(1.0, 1.0);
    let result = mgr.process_mouse_event(&event, &MockMeasurer::STANDARD);
    // Should dismiss the non-modal popup.
    assert!(matches!(result, OverlayEventResult::Dismissed(_)));
    assert_eq!(mgr.count(), 1);

    // Now topmost is modal — click outside is blocked.
    let result = mgr.process_mouse_event(&event, &MockMeasurer::STANDARD);
    assert!(matches!(result, OverlayEventResult::Blocked));
}

#[test]
fn push_after_clear_works() {
    // Verify clean state after clear_all.
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_overlay(label_widget("A"), anchor(), Placement::Below);
    mgr.push_modal(label_widget("B"), anchor(), Placement::Center);
    mgr.clear_all();

    let id = mgr.push_overlay(label_widget("Fresh"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    assert_eq!(mgr.count(), 1);
    assert!(!mgr.has_modal());
    assert!(mgr.overlay_rect(id).is_some());
}

#[test]
fn mouse_up_outside_does_not_dismiss() {
    // Only Down events dismiss, not Up.
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_overlay(label_widget("Popup"), anchor(), Placement::Below);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(1.0, 1.0),
        modifiers: Modifiers::NONE,
    };
    let result = mgr.process_mouse_event(&up, &MockMeasurer::STANDARD);
    assert!(matches!(result, OverlayEventResult::PassThrough));
    assert_eq!(mgr.count(), 1, "mouse up should not dismiss");
}

#[test]
fn modal_key_delivery_reports_correct_overlay_id() {
    // Verify the overlay_id in Delivered matches the modal.
    let mut mgr = OverlayManager::new(viewport());
    let id = mgr.push_modal(label_widget("Dialog"), anchor(), Placement::Center);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let result = mgr.process_key_event(key_event(Key::ArrowDown), &MockMeasurer::STANDARD);
    match result {
        OverlayEventResult::Delivered { overlay_id, .. } => assert_eq!(overlay_id, id),
        other => panic!("expected Delivered with modal id, got {other:?}"),
    }
}

#[test]
fn draw_stacked_modals_emits_two_dim_rects() {
    // Each modal layer should emit its own dimming rect.
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_modal(label_widget("Outer"), anchor(), Placement::Center);
    mgr.push_modal(label_widget("Inner"), anchor(), Placement::Center);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let anim_flag = Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds: Rect::default(),
        focused_widget: None,
        now: Instant::now(),
        animations_running: &anim_flag,
    };
    mgr.draw_overlays(&mut ctx);

    let dim_rects: Vec<_> = draw_list
        .commands()
        .iter()
        .filter(|c| match c {
            DrawCommand::Rect { style, .. } => style.fill.is_some_and(|f| f.a > 0.0 && f.a < 1.0),
            _ => false,
        })
        .collect();
    assert_eq!(dim_rects.len(), 2, "each modal should emit a dim rect");
}

#[test]
fn label_not_focusable_in_modal() {
    // Labels are not focusable — modal focus order should be empty.
    let mut mgr = OverlayManager::new(viewport());
    mgr.push_modal(label_widget("Text Only"), anchor(), Placement::Center);
    mgr.layout_overlays(&MockMeasurer::STANDARD);

    let order = mgr.modal_focus_order();
    assert!(order.is_some());
    assert!(order.unwrap().is_empty(), "label has no focusable elements");
}
