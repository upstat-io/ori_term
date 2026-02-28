use std::cell::Cell;
use std::time::{Duration, Instant};

use crate::compositor::layer_animator::LayerAnimator;
use crate::compositor::layer_tree::LayerTree;
use crate::draw::{DrawCommand, DrawList};
use crate::geometry::{Point, Rect, Size};
use crate::input::{Key, KeyEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind};
use crate::theme::UiTheme;
use crate::widgets::button::ButtonWidget;
use crate::widgets::flex::FlexWidget;
use crate::widgets::label::LabelWidget;
use crate::widgets::tests::MockMeasurer;
use crate::widgets::{DrawCtx, Widget, WidgetAction};

use super::OverlayManager;

const TEST_THEME: UiTheme = UiTheme::dark();
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

fn test_tree() -> LayerTree {
    LayerTree::new(viewport())
}

/// Advance time past all fade animations and remove completed dismissals.
fn complete_animations(
    mgr: &mut OverlayManager,
    tree: &mut LayerTree,
    animator: &mut LayerAnimator,
) {
    let future = Instant::now() + Duration::from_secs(1);
    animator.tick(tree, future);
    mgr.cleanup_dismissed(tree, animator);
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
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_overlay(
        label_widget("A"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    assert_eq!(mgr.count(), 1);
    assert!(!mgr.is_empty());
}

#[test]
fn push_modal_sets_has_modal() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_modal(
        label_widget("Modal"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(mgr.has_modal());
}

#[test]
fn dismiss_overlay_by_id() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id1 = mgr.push_overlay(
        label_widget("A"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    let _id2 = mgr.push_overlay(
        label_widget("B"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );

    assert!(mgr.begin_dismiss(id1, &mut tree, &mut animator, now));
    assert_eq!(mgr.count(), 1);
    // Can't dismiss again.
    assert!(!mgr.begin_dismiss(id1, &mut tree, &mut animator, now));
}

#[test]
fn dismiss_topmost() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let _id1 = mgr.push_overlay(
        label_widget("A"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    let id2 = mgr.push_overlay(
        label_widget("B"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );

    assert_eq!(
        mgr.begin_dismiss_topmost(&mut tree, &mut animator, now),
        Some(id2)
    );
    assert_eq!(mgr.count(), 1);
}

#[test]
fn dismiss_topmost_empty() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    assert_eq!(
        mgr.begin_dismiss_topmost(&mut tree, &mut animator, now),
        None
    );
}

#[test]
fn clear_all() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_overlay(
        label_widget("A"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.push_overlay(
        label_widget("B"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.push_modal(
        label_widget("C"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );

    mgr.clear_all(&mut tree, &mut animator);
    assert!(mgr.is_empty());
    assert!(!mgr.has_modal());
}

#[test]
fn dismiss_nonexistent_id() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();
    let fake_id = OverlayId::next();

    assert!(!mgr.begin_dismiss(fake_id, &mut tree, &mut animator, now));
}

#[test]
fn multiple_overlays_ordering() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let _id1 = mgr.push_overlay(
        label_widget("A"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    let _id2 = mgr.push_overlay(
        label_widget("B"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    let id3 = mgr.push_overlay(
        label_widget("C"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );

    // Topmost is the last pushed.
    assert_eq!(
        mgr.begin_dismiss_topmost(&mut tree, &mut animator, now),
        Some(id3)
    );
    assert_eq!(mgr.count(), 2);
}

#[test]
fn overlay_rect_accessor() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id = mgr.push_overlay(
        label_widget("Test"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

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
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let event = mouse_down(50.0, 50.0);
    let result = mgr.process_mouse_event(
        &event,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(result, OverlayEventResult::PassThrough));
}

#[test]
fn mouse_click_inside_overlay_delivers() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id = mgr.push_overlay(
        button_widget("Click"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let rect = mgr.overlay_rect(id).unwrap();
    let event = mouse_down(rect.x() + 5.0, rect.y() + 5.0);
    let result = mgr.process_mouse_event(
        &event,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(result, OverlayEventResult::Delivered { .. }));
}

#[test]
fn mouse_click_outside_dismisses() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id = mgr.push_overlay(
        label_widget("Popup"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    // Click far from the overlay.
    let event = mouse_down(1.0, 1.0);
    let result = mgr.process_mouse_event(
        &event,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );

    match result {
        OverlayEventResult::Dismissed(dismissed_id) => assert_eq!(dismissed_id, id),
        other => panic!("expected Dismissed, got {other:?}"),
    }
    // Active count drops immediately; overlay is now in dismissing vec.
    assert_eq!(mgr.count(), 0);
    // Complete fade-out to fully clean up.
    complete_animations(&mut mgr, &mut tree, &mut animator);
    assert!(mgr.is_empty());
}

#[test]
fn mouse_move_outside_does_not_dismiss() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_overlay(
        label_widget("Popup"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    // Mouse move (not click) outside should pass through, not dismiss.
    let event = mouse_move(1.0, 1.0);
    let result = mgr.process_mouse_event(
        &event,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(result, OverlayEventResult::PassThrough));
    assert_eq!(mgr.count(), 1);
}

#[test]
fn mouse_click_outside_modal_blocks() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_modal(
        label_widget("Modal"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let event = mouse_down(1.0, 1.0);
    let result = mgr.process_mouse_event(
        &event,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(result, OverlayEventResult::Blocked));
    // Modal should still be there.
    assert_eq!(mgr.count(), 1);
}

#[test]
fn mouse_topmost_overlay_wins() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    // Two overlays at the same position.
    let _id1 = mgr.push_overlay(
        button_widget("Back"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    let id2 = mgr.push_overlay(
        button_widget("Front"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let rect = mgr.overlay_rect(id2).unwrap();
    let event = mouse_down(rect.x() + 5.0, rect.y() + 5.0);
    let result = mgr.process_mouse_event(
        &event,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );

    match result {
        OverlayEventResult::Delivered { overlay_id, .. } => assert_eq!(overlay_id, id2),
        other => panic!("expected Delivered to topmost, got {other:?}"),
    }
}

// --- Key routing tests ---

#[test]
fn key_pass_through_when_empty() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let result = mgr.process_key_event(
        key_event(Key::Enter),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(result, OverlayEventResult::PassThrough));
}

#[test]
fn escape_dismisses_topmost() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let _id1 = mgr.push_overlay(
        label_widget("A"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    let id2 = mgr.push_overlay(
        label_widget("B"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let result = mgr.process_key_event(
        key_event(Key::Escape),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    match result {
        OverlayEventResult::Dismissed(id) => assert_eq!(id, id2),
        other => panic!("expected Dismissed, got {other:?}"),
    }
    assert_eq!(mgr.count(), 1);
}

#[test]
fn escape_dismisses_modal() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id = mgr.push_modal(
        label_widget("Modal"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let result = mgr.process_key_event(
        key_event(Key::Escape),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    match result {
        OverlayEventResult::Dismissed(dismissed_id) => assert_eq!(dismissed_id, id),
        other => panic!("expected Dismissed, got {other:?}"),
    }
}

#[test]
fn modal_never_passes_key_through() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_modal(
        label_widget("Modal"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    // A random key that the label won't handle.
    let result = mgr.process_key_event(
        key_event(Key::ArrowDown),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    // Modal should deliver (even if Ignored by widget), never PassThrough.
    assert!(matches!(result, OverlayEventResult::Delivered { .. }));
}

#[test]
fn non_modal_key_can_pass_through() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_overlay(
        label_widget("Popup"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    // Labels don't handle key events → should pass through.
    let result = mgr.process_key_event(
        key_event(Key::ArrowDown),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(result, OverlayEventResult::PassThrough));
}

// --- Hover routing tests ---

#[test]
fn hover_pass_through_when_empty() {
    let mut mgr = OverlayManager::new(viewport());
    let result = mgr.process_hover_event(
        Point::new(50.0, 50.0),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
    );
    assert!(matches!(result, OverlayEventResult::PassThrough));
}

#[test]
fn hover_inside_overlay_delivers() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id = mgr.push_overlay(
        button_widget("Btn"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let rect = mgr.overlay_rect(id).unwrap();
    let result = mgr.process_hover_event(
        Point::new(rect.x() + 5.0, rect.y() + 5.0),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
    );
    assert!(matches!(result, OverlayEventResult::Delivered { .. }));
}

#[test]
fn hover_outside_modal_blocks() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_modal(
        label_widget("Modal"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let result = mgr.process_hover_event(
        Point::new(1.0, 1.0),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
    );
    assert!(matches!(result, OverlayEventResult::Blocked));
}

#[test]
fn hover_outside_non_modal_passes_through() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_overlay(
        label_widget("Popup"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let result = mgr.process_hover_event(
        Point::new(1.0, 1.0),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
    );
    assert!(matches!(result, OverlayEventResult::PassThrough));
}

#[test]
fn hover_transition_sends_leave_to_old_overlay() {
    // Two overlays at different positions. Hover first, then move to second.
    // Old overlay's widget should receive Leave.
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let btn_a = button_widget("A");
    let anchor_a = Rect::new(50.0, 50.0, 80.0, 30.0);
    let id_a = mgr.push_overlay(
        btn_a,
        anchor_a,
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );

    let btn_b = button_widget("B");
    let anchor_b = Rect::new(300.0, 50.0, 80.0, 30.0);
    let id_b = mgr.push_overlay(
        btn_b,
        anchor_b,
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );

    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let rect_a = mgr.overlay_rect(id_a).unwrap();
    let rect_b = mgr.overlay_rect(id_b).unwrap();

    // Hover into overlay A.
    let result = mgr.process_hover_event(
        Point::new(rect_a.x() + 5.0, rect_a.y() + 5.0),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
    );
    assert!(matches!(
        result,
        OverlayEventResult::Delivered {
            overlay_id,
            ..
        } if overlay_id == id_a
    ));

    // Hover into overlay B — should send Leave to A internally.
    let result = mgr.process_hover_event(
        Point::new(rect_b.x() + 5.0, rect_b.y() + 5.0),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
    );
    assert!(matches!(
        result,
        OverlayEventResult::Delivered {
            overlay_id,
            ..
        } if overlay_id == id_b
    ));

    // Hover outside both — should clear tracking.
    let result = mgr.process_hover_event(
        Point::new(1.0, 1.0),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
    );
    assert!(matches!(result, OverlayEventResult::PassThrough));
}

// --- Drawing tests ---

#[test]
fn draw_empty_is_noop() {
    let mgr = OverlayManager::new(viewport());
    assert_eq!(mgr.draw_count(), 0);
}

#[test]
fn draw_non_modal_no_dimming() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_overlay(
        label_widget("A"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let anim_flag = Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds: Rect::default(),
        focused_widget: None,
        now,
        animations_running: &anim_flag,
        theme: &TEST_THEME,
    };

    assert_eq!(mgr.draw_count(), 1);
    let _opacity = mgr.draw_overlay_at(0, &mut ctx, &tree);

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
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_modal(
        label_widget("Modal"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    // Tick the animator so opacity reaches target (1.0 for fade-in).
    let future = now + Duration::from_secs(1);
    animator.tick(&mut tree, future);

    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let anim_flag = Cell::new(false);
    let mut ctx = DrawCtx {
        measurer: &measurer,
        draw_list: &mut draw_list,
        bounds: Rect::default(),
        focused_widget: None,
        now: future,
        animations_running: &anim_flag,
        theme: &TEST_THEME,
    };

    assert_eq!(mgr.draw_count(), 1);
    let _opacity = mgr.draw_overlay_at(0, &mut ctx, &tree);

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
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    // Use different labels so we can distinguish them by glyph count.
    mgr.push_overlay(
        label_widget("AB"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.push_overlay(
        label_widget("ABCDE"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let anim_flag = Cell::new(false);

    // Draw all overlays into the same draw list to verify order.
    for i in 0..mgr.draw_count() {
        let mut ctx = DrawCtx {
            measurer: &measurer,
            draw_list: &mut draw_list,
            bounds: Rect::default(),
            focused_widget: None,
            now,
            animations_running: &anim_flag,
            theme: &TEST_THEME,
        };
        mgr.draw_overlay_at(i, &mut ctx, &tree);
    }

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
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let btn = button_widget("Focus Me");
    let btn_id = btn.id();
    mgr.push_modal(
        btn,
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

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
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_overlay(
        button_widget("Btn"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
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
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let btn = button_widget("Go");
    let btn_id = btn.id();
    let id = mgr.push_overlay(
        btn,
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let rect = mgr.overlay_rect(id).unwrap();

    // Down then up = click.
    let down = mouse_down(rect.x() + 5.0, rect.y() + 5.0);
    mgr.process_mouse_event(
        &down,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(rect.x() + 5.0, rect.y() + 5.0),
        modifiers: Modifiers::NONE,
    };
    let result = mgr.process_mouse_event(
        &up,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );

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
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id1 = mgr.push_modal(
        label_widget("Outer"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    let id2 = mgr.push_modal(
        label_widget("Inner"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    assert_eq!(mgr.count(), 2);
    assert!(mgr.has_modal());

    // Escape dismisses inner modal.
    let result = mgr.process_key_event(
        key_event(Key::Escape),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    match result {
        OverlayEventResult::Dismissed(id) => assert_eq!(id, id2),
        other => panic!("expected inner dismissed, got {other:?}"),
    }

    // Outer modal is still active.
    assert_eq!(mgr.count(), 1);
    assert!(mgr.has_modal());

    // Click outside is still blocked by outer modal.
    let event = mouse_down(1.0, 1.0);
    let result = mgr.process_mouse_event(
        &event,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(result, OverlayEventResult::Blocked));

    // Second escape dismisses outer.
    let result = mgr.process_key_event(
        key_event(Key::Escape),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    match result {
        OverlayEventResult::Dismissed(id) => assert_eq!(id, id1),
        other => panic!("expected outer dismissed, got {other:?}"),
    }
    assert_eq!(mgr.count(), 0);
    complete_animations(&mut mgr, &mut tree, &mut animator);
    assert!(mgr.is_empty());
}

#[test]
fn multiple_escapes_dismiss_stack_one_at_a_time() {
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id1 = mgr.push_overlay(
        label_widget("A"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    let id2 = mgr.push_overlay(
        label_widget("B"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    let id3 = mgr.push_overlay(
        label_widget("C"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    // Three escapes should dismiss C, B, A in order.
    let r = mgr.process_key_event(
        key_event(Key::Escape),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(r, OverlayEventResult::Dismissed(id) if id == id3));
    assert_eq!(mgr.count(), 2);

    let r = mgr.process_key_event(
        key_event(Key::Escape),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(r, OverlayEventResult::Dismissed(id) if id == id2));
    assert_eq!(mgr.count(), 1);

    let r = mgr.process_key_event(
        key_event(Key::Escape),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(r, OverlayEventResult::Dismissed(id) if id == id1));
    assert_eq!(mgr.count(), 0);
    complete_animations(&mut mgr, &mut tree, &mut animator);
    assert!(mgr.is_empty());

    // Fourth escape passes through (stack empty).
    let r = mgr.process_key_event(
        key_event(Key::Escape),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(r, OverlayEventResult::PassThrough));
}

#[test]
fn scroll_outside_overlay_does_not_dismiss() {
    // Scroll events are not clicks — should not dismiss.
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_overlay(
        label_widget("Menu"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let scroll = MouseEvent {
        kind: MouseEventKind::Scroll(crate::input::ScrollDelta::Lines { x: 0.0, y: -3.0 }),
        pos: Point::new(1.0, 1.0),
        modifiers: Modifiers::NONE,
    };
    let result = mgr.process_mouse_event(
        &scroll,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(result, OverlayEventResult::PassThrough));
    assert_eq!(mgr.count(), 1, "scroll should not dismiss overlay");
}

#[test]
fn right_click_outside_also_dismisses() {
    // Right-click is also a Down event — should dismiss non-modal overlay.
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id = mgr.push_overlay(
        label_widget("Context"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let right_click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        pos: Point::new(1.0, 1.0),
        modifiers: Modifiers::NONE,
    };
    let result = mgr.process_mouse_event(
        &right_click,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    match result {
        OverlayEventResult::Dismissed(dismissed_id) => assert_eq!(dismissed_id, id),
        other => panic!("expected Dismissed on right-click, got {other:?}"),
    }
}

#[test]
fn dismiss_middle_overlay_preserves_stack() {
    // Remove by ID when not topmost. Chromium tests stacking integrity.
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id1 = mgr.push_overlay(
        label_widget("A"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    let id2 = mgr.push_overlay(
        label_widget("B"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    let id3 = mgr.push_overlay(
        label_widget("C"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );

    // Remove middle overlay.
    assert!(mgr.begin_dismiss(id2, &mut tree, &mut animator, now));
    assert_eq!(mgr.count(), 2);

    // Topmost should still be C.
    assert_eq!(
        mgr.begin_dismiss_topmost(&mut tree, &mut animator, now),
        Some(id3)
    );
    // Then A.
    assert_eq!(
        mgr.begin_dismiss_topmost(&mut tree, &mut animator, now),
        Some(id1)
    );
    assert_eq!(mgr.count(), 0);
    complete_animations(&mut mgr, &mut tree, &mut animator);
    assert!(mgr.is_empty());
}

#[test]
fn dismiss_topmost_reveals_overlay_below() {
    // Dismiss topmost → next overlay becomes active and receives events.
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id1 = mgr.push_overlay(
        button_widget("Lower"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    let _id2 = mgr.push_overlay(
        label_widget("Upper"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    // Escape removes Upper.
    mgr.process_key_event(
        key_event(Key::Escape),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert_eq!(mgr.count(), 1);

    // Lower overlay should now receive events.
    let rect = mgr.overlay_rect(id1).unwrap();
    let event = mouse_down(rect.x() + 5.0, rect.y() + 5.0);
    let result = mgr.process_mouse_event(
        &event,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    match result {
        OverlayEventResult::Delivered { overlay_id, .. } => assert_eq!(overlay_id, id1),
        other => panic!("expected Delivered to lower, got {other:?}"),
    }
}

#[test]
fn viewport_resize_relayouts_overlays() {
    // Chromium: window resize must reposition overlays.
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id = mgr.push_overlay(
        label_widget("Popup"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let rect_before = mgr.overlay_rect(id).unwrap();

    // Shrink viewport.
    let small_vp = Rect::new(0.0, 0.0, 400.0, 300.0);
    mgr.set_viewport(small_vp);
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

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
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_modal(
        label_widget("Modal Base"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.push_overlay(
        label_widget("Popup"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    // Topmost is non-modal, so has_modal is false (checks topmost only).
    assert!(!mgr.has_modal());

    // Click outside both: topmost is non-modal with dismiss_on_click_outside.
    let event = mouse_down(1.0, 1.0);
    let result = mgr.process_mouse_event(
        &event,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    // Should dismiss the non-modal popup.
    assert!(matches!(result, OverlayEventResult::Dismissed(_)));
    assert_eq!(mgr.count(), 1);

    // Now topmost is modal — click outside is blocked.
    let result = mgr.process_mouse_event(
        &event,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(result, OverlayEventResult::Blocked));
}

#[test]
fn push_after_clear_works() {
    // Verify clean state after clear_all.
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_overlay(
        label_widget("A"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.push_modal(
        label_widget("B"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.clear_all(&mut tree, &mut animator);

    let id = mgr.push_overlay(
        label_widget("Fresh"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    assert_eq!(mgr.count(), 1);
    assert!(!mgr.has_modal());
    assert!(mgr.overlay_rect(id).is_some());
}

#[test]
fn mouse_up_outside_does_not_dismiss() {
    // Only Down events dismiss, not Up.
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_overlay(
        label_widget("Popup"),
        anchor(),
        Placement::Below,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        pos: Point::new(1.0, 1.0),
        modifiers: Modifiers::NONE,
    };
    let result = mgr.process_mouse_event(
        &up,
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    assert!(matches!(result, OverlayEventResult::PassThrough));
    assert_eq!(mgr.count(), 1, "mouse up should not dismiss");
}

#[test]
fn modal_key_delivery_reports_correct_overlay_id() {
    // Verify the overlay_id in Delivered matches the modal.
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let id = mgr.push_modal(
        label_widget("Dialog"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let result = mgr.process_key_event(
        key_event(Key::ArrowDown),
        &MockMeasurer::STANDARD,
        &TEST_THEME,
        None,
        &mut tree,
        &mut animator,
        now,
    );
    match result {
        OverlayEventResult::Delivered { overlay_id, .. } => assert_eq!(overlay_id, id),
        other => panic!("expected Delivered with modal id, got {other:?}"),
    }
}

#[test]
fn draw_stacked_modals_emits_two_dim_rects() {
    // Each modal layer should emit its own dimming rect.
    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_modal(
        label_widget("Outer"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.push_modal(
        label_widget("Inner"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    // Advance time so fade-in animations complete.
    let future = now + Duration::from_secs(1);
    animator.tick(&mut tree, future);

    let measurer = MockMeasurer::STANDARD;
    let mut draw_list = DrawList::new();
    let anim_flag = Cell::new(false);

    for i in 0..mgr.draw_count() {
        let mut ctx = DrawCtx {
            measurer: &measurer,
            draw_list: &mut draw_list,
            bounds: Rect::default(),
            focused_widget: None,
            now: future,
            animations_running: &anim_flag,
            theme: &TEST_THEME,
        };
        mgr.draw_overlay_at(i, &mut ctx, &tree);
    }

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
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_modal(
        label_widget("Text Only"),
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let order = mgr.modal_focus_order();
    assert!(order.is_some());
    assert!(order.unwrap().is_empty(), "label has no focusable elements");
}

#[test]
fn modal_focus_order_traverses_containers() {
    // Flex wrapping two buttons — focus order should find both.
    let btn1 = ButtonWidget::new("OK");
    let btn1_id = btn1.id();
    let btn2 = ButtonWidget::new("Cancel");
    let btn2_id = btn2.id();
    let flex: Box<dyn Widget> = Box::new(FlexWidget::row(vec![Box::new(btn1), Box::new(btn2)]));

    let mut mgr = OverlayManager::new(viewport());
    let mut tree = test_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    mgr.push_modal(
        flex,
        anchor(),
        Placement::Center,
        &mut tree,
        &mut animator,
        now,
    );
    mgr.layout_overlays(&MockMeasurer::STANDARD, &TEST_THEME);

    let ids = mgr.modal_focus_order().expect("modal present");
    assert!(ids.contains(&btn1_id), "should find first button");
    assert!(ids.contains(&btn2_id), "should find second button");
    assert_eq!(ids.len(), 2);
}
