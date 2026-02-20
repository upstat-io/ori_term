//! Tests for widget-level hit testing and input routing.

use crate::geometry::{Point, Rect};
use crate::layout::LayoutNode;
use crate::widget_id::WidgetId;

use super::event::{
    EventResponse, HoverEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind, ScrollDelta,
};
use super::hit_test::{layout_hit_test, layout_hit_test_clipped};
use super::routing::{InputState, RouteAction};

// ── Helpers ──────────────────────────────────────────────────────────

fn make_node(x: f32, y: f32, w: f32, h: f32, id: Option<WidgetId>) -> LayoutNode {
    let rect = Rect::new(x, y, w, h);
    LayoutNode {
        rect,
        content_rect: rect,
        children: Vec::new(),
        widget_id: id,
    }
}

fn mouse_move(x: f32, y: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Move,
        pos: Point::new(x, y),
        modifiers: Modifiers::NONE,
    }
}

fn mouse_down(x: f32, y: f32, button: MouseButton) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(button),
        pos: Point::new(x, y),
        modifiers: Modifiers::NONE,
    }
}

fn mouse_up(x: f32, y: f32, button: MouseButton) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(button),
        pos: Point::new(x, y),
        modifiers: Modifiers::NONE,
    }
}

fn mouse_scroll(x: f32, y: f32, dy: f32) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Scroll(ScrollDelta::Lines { x: 0.0, y: dy }),
        pos: Point::new(x, y),
        modifiers: Modifiers::NONE,
    }
}

fn two_widget_tree() -> (LayoutNode, WidgetId, WidgetId) {
    let a = WidgetId::next();
    let b = WidgetId::next();
    let child_a = make_node(0.0, 0.0, 50.0, 100.0, Some(a));
    let child_b = make_node(50.0, 0.0, 50.0, 100.0, Some(b));
    let mut root = make_node(0.0, 0.0, 100.0, 100.0, None);
    root.children.push(child_a);
    root.children.push(child_b);
    (root, a, b)
}

fn hover_count(actions: &[RouteAction]) -> usize {
    actions
        .iter()
        .filter(|a| matches!(a, RouteAction::Hover { .. }))
        .count()
}

fn delivers_to(actions: &[RouteAction], target: WidgetId) -> bool {
    actions
        .iter()
        .any(|a| matches!(a, RouteAction::Deliver { target: t, .. } if *t == target))
}

// ── Hit Testing ──────────────────────────────────────────────────────

#[test]
fn hit_test_single_leaf() {
    let id = WidgetId::next();
    let root = make_node(0.0, 0.0, 100.0, 50.0, Some(id));

    assert_eq!(layout_hit_test(&root, Point::new(50.0, 25.0)), Some(id));
    assert_eq!(layout_hit_test(&root, Point::new(0.0, 0.0)), Some(id));
    // Half-open: right/bottom edge is outside.
    assert_eq!(layout_hit_test(&root, Point::new(100.0, 25.0)), None);
    assert_eq!(layout_hit_test(&root, Point::new(50.0, 50.0)), None);
}

#[test]
fn hit_test_miss_returns_none() {
    let id = WidgetId::next();
    let root = make_node(10.0, 10.0, 50.0, 50.0, Some(id));

    assert_eq!(layout_hit_test(&root, Point::new(0.0, 0.0)), None);
    assert_eq!(layout_hit_test(&root, Point::new(5.0, 30.0)), None);
}

#[test]
fn hit_test_no_widget_id() {
    let root = make_node(0.0, 0.0, 100.0, 100.0, None);
    assert_eq!(layout_hit_test(&root, Point::new(50.0, 50.0)), None);
}

#[test]
fn hit_test_child_takes_priority() {
    let parent_id = WidgetId::next();
    let child_id = WidgetId::next();

    let child = make_node(20.0, 20.0, 30.0, 30.0, Some(child_id));
    let mut parent = make_node(0.0, 0.0, 100.0, 100.0, Some(parent_id));
    parent.children.push(child);

    // Point inside child → child wins.
    assert_eq!(
        layout_hit_test(&parent, Point::new(35.0, 35.0)),
        Some(child_id)
    );
    // Point outside child but inside parent → parent wins.
    assert_eq!(
        layout_hit_test(&parent, Point::new(5.0, 5.0)),
        Some(parent_id)
    );
}

#[test]
fn hit_test_last_child_is_frontmost() {
    let parent_id = WidgetId::next();
    let back_id = WidgetId::next();
    let front_id = WidgetId::next();

    // Two overlapping children at the same position.
    let back = make_node(10.0, 10.0, 40.0, 40.0, Some(back_id));
    let front = make_node(10.0, 10.0, 40.0, 40.0, Some(front_id));

    let mut parent = make_node(0.0, 0.0, 100.0, 100.0, Some(parent_id));
    parent.children.push(back);
    parent.children.push(front);

    // Last child (front) wins.
    assert_eq!(
        layout_hit_test(&parent, Point::new(25.0, 25.0)),
        Some(front_id)
    );
}

#[test]
fn hit_test_deeply_nested() {
    let root_id = WidgetId::next();
    let mid_id = WidgetId::next();
    let leaf_id = WidgetId::next();

    let leaf = make_node(30.0, 30.0, 10.0, 10.0, Some(leaf_id));
    let mut mid = make_node(20.0, 20.0, 40.0, 40.0, Some(mid_id));
    mid.children.push(leaf);
    let mut root = make_node(0.0, 0.0, 100.0, 100.0, Some(root_id));
    root.children.push(mid);

    // Deepest node wins.
    assert_eq!(
        layout_hit_test(&root, Point::new(35.0, 35.0)),
        Some(leaf_id)
    );
    // Between mid and leaf → mid.
    assert_eq!(layout_hit_test(&root, Point::new(22.0, 22.0)), Some(mid_id));
    // Outside mid → root.
    assert_eq!(layout_hit_test(&root, Point::new(5.0, 5.0)), Some(root_id));
}

#[test]
fn hit_test_child_without_id_falls_through_to_parent() {
    let parent_id = WidgetId::next();
    // Child has no widget_id.
    let child = make_node(20.0, 20.0, 30.0, 30.0, None);
    let mut parent = make_node(0.0, 0.0, 100.0, 100.0, Some(parent_id));
    parent.children.push(child);

    // Point in child area falls through to parent.
    assert_eq!(
        layout_hit_test(&parent, Point::new(35.0, 35.0)),
        Some(parent_id)
    );
}

#[test]
fn hit_test_clipped_excludes_outside_clip() {
    let id = WidgetId::next();
    let root = make_node(0.0, 0.0, 200.0, 200.0, Some(id));
    let clip = Rect::new(50.0, 50.0, 100.0, 100.0);

    // Inside both rect and clip.
    assert_eq!(
        layout_hit_test_clipped(&root, Point::new(75.0, 75.0), Some(clip)),
        Some(id)
    );
    // Inside rect but outside clip.
    assert_eq!(
        layout_hit_test_clipped(&root, Point::new(10.0, 10.0), Some(clip)),
        None
    );
    // No clip → normal hit test.
    assert_eq!(
        layout_hit_test_clipped(&root, Point::new(10.0, 10.0), None),
        Some(id)
    );
}

// ── Event Response ───────────────────────────────────────────────────

#[test]
fn event_response_is_handled() {
    assert!(EventResponse::Handled.is_handled());
    assert!(EventResponse::RequestFocus.is_handled());
    assert!(EventResponse::RequestRedraw.is_handled());
    assert!(!EventResponse::Ignored.is_handled());
}

// ── Input Routing ────────────────────────────────────────────────────

#[test]
fn routing_hover_enter_leave() {
    let a = WidgetId::next();
    let b = WidgetId::next();

    let child_a = make_node(0.0, 0.0, 50.0, 100.0, Some(a));
    let child_b = make_node(50.0, 0.0, 50.0, 100.0, Some(b));
    let mut root = make_node(0.0, 0.0, 100.0, 100.0, None);
    root.children.push(child_a);
    root.children.push(child_b);

    let mut state = InputState::new();

    // Move into widget A.
    let actions = state.process_mouse_event(mouse_move(25.0, 50.0), &root);
    assert!(actions.contains(&RouteAction::Hover {
        target: a,
        kind: HoverEvent::Enter,
    }));
    assert_eq!(state.hovered(), Some(a));

    // Move into widget B → Leave A, Enter B.
    let actions = state.process_mouse_event(mouse_move(75.0, 50.0), &root);
    assert!(actions.contains(&RouteAction::Hover {
        target: a,
        kind: HoverEvent::Leave,
    }));
    assert!(actions.contains(&RouteAction::Hover {
        target: b,
        kind: HoverEvent::Enter,
    }));
    assert_eq!(state.hovered(), Some(b));
}

#[test]
fn routing_mouse_capture_on_down() {
    let id = WidgetId::next();
    let root = make_node(0.0, 0.0, 100.0, 100.0, Some(id));
    let mut state = InputState::new();

    // Mouse down → auto-capture.
    state.process_mouse_event(mouse_down(50.0, 50.0, MouseButton::Left), &root);
    assert_eq!(state.captured(), Some(id));

    // Mouse up → release capture.
    state.process_mouse_event(mouse_up(50.0, 50.0, MouseButton::Left), &root);
    assert_eq!(state.captured(), None);
}

#[test]
fn routing_captured_widget_receives_events_outside_bounds() {
    let a = WidgetId::next();
    let b = WidgetId::next();

    let child_a = make_node(0.0, 0.0, 50.0, 100.0, Some(a));
    let child_b = make_node(50.0, 0.0, 50.0, 100.0, Some(b));
    let mut root = make_node(0.0, 0.0, 100.0, 100.0, None);
    root.children.push(child_a);
    root.children.push(child_b);

    let mut state = InputState::new();

    // Move to A, then mouse down to capture.
    state.process_mouse_event(mouse_move(25.0, 50.0), &root);
    state.process_mouse_event(mouse_down(25.0, 50.0, MouseButton::Left), &root);
    assert_eq!(state.captured(), Some(a));

    // Move cursor over B while captured → event delivered to A (captured).
    let actions = state.process_mouse_event(mouse_move(75.0, 50.0), &root);
    let delivered = actions.iter().any(|action| {
        matches!(
            action,
            RouteAction::Deliver { target, .. } if *target == a
        )
    });
    assert!(delivered, "captured widget should receive the event");
}

#[test]
fn routing_cursor_left_generates_leave() {
    let id = WidgetId::next();
    let root = make_node(0.0, 0.0, 100.0, 100.0, Some(id));
    let mut state = InputState::new();

    // Enter.
    state.process_mouse_event(mouse_move(50.0, 50.0), &root);
    assert_eq!(state.hovered(), Some(id));

    // Cursor leaves window.
    let actions = state.process_cursor_left();
    assert!(actions.contains(&RouteAction::Hover {
        target: id,
        kind: HoverEvent::Leave,
    }));
    assert_eq!(state.hovered(), None);
    assert_eq!(state.cursor_pos(), None);
}

#[test]
fn routing_no_actions_on_empty_tree() {
    let root = make_node(0.0, 0.0, 100.0, 100.0, None);
    let mut state = InputState::new();
    let actions = state.process_mouse_event(mouse_move(50.0, 50.0), &root);
    // No widget_id anywhere → no actions.
    assert!(actions.is_empty());
}

#[test]
fn routing_move_within_same_widget_no_hover_events() {
    let id = WidgetId::next();
    let root = make_node(0.0, 0.0, 100.0, 100.0, Some(id));
    let mut state = InputState::new();

    // First move → Enter.
    let actions = state.process_mouse_event(mouse_move(25.0, 25.0), &root);
    assert_eq!(
        actions
            .iter()
            .filter(|a| matches!(a, RouteAction::Hover { .. }))
            .count(),
        1,
    );

    // Second move within same widget → no hover events.
    let actions = state.process_mouse_event(mouse_move(75.0, 75.0), &root);
    let hover_count = actions
        .iter()
        .filter(|a| matches!(a, RouteAction::Hover { .. }))
        .count();
    assert_eq!(hover_count, 0, "no hover change within same widget");
}

#[test]
fn routing_manual_capture_overrides_hit() {
    let a = WidgetId::next();
    let b = WidgetId::next();

    let child_a = make_node(0.0, 0.0, 50.0, 100.0, Some(a));
    let child_b = make_node(50.0, 0.0, 50.0, 100.0, Some(b));
    let mut root = make_node(0.0, 0.0, 100.0, 100.0, None);
    root.children.push(child_a);
    root.children.push(child_b);

    let mut state = InputState::new();
    state.set_capture(a);

    // Move over B with A captured → event delivered to A.
    let actions = state.process_mouse_event(mouse_move(75.0, 50.0), &root);
    let delivered_to_a = actions.iter().any(|action| {
        matches!(
            action,
            RouteAction::Deliver { target, .. } if *target == a
        )
    });
    assert!(delivered_to_a);

    state.release_capture();
    assert_eq!(state.captured(), None);
}

#[test]
fn routing_mouse_down_outside_all_widgets() {
    let id = WidgetId::next();
    let root = make_node(10.0, 10.0, 50.0, 50.0, Some(id));
    let mut state = InputState::new();

    // Click outside any widget.
    let actions = state.process_mouse_event(mouse_down(5.0, 5.0, MouseButton::Left), &root);
    assert!(actions.is_empty());
    assert_eq!(state.captured(), None);
}

// ── Chromium-Inspired Edge Cases ─────────────────────────────────────

#[test]
fn hit_test_zero_size_widget_not_hittable() {
    // Zero-width widget: half-open rect [10, 10+0) has no interior.
    let id = WidgetId::next();
    let root = make_node(10.0, 10.0, 0.0, 50.0, Some(id));
    assert_eq!(layout_hit_test(&root, Point::new(10.0, 25.0)), None);

    // Zero-height.
    let root = make_node(10.0, 10.0, 50.0, 0.0, Some(id));
    assert_eq!(layout_hit_test(&root, Point::new(25.0, 10.0)), None);

    // Zero both.
    let root = make_node(10.0, 10.0, 0.0, 0.0, Some(id));
    assert_eq!(layout_hit_test(&root, Point::new(10.0, 10.0)), None);
}

#[test]
fn hit_test_one_pixel_widget() {
    let id = WidgetId::next();
    let root = make_node(10.0, 10.0, 1.0, 1.0, Some(id));

    // Exact top-left corner is inside.
    assert_eq!(layout_hit_test(&root, Point::new(10.0, 10.0)), Some(id));
    // Right/bottom edge is outside (half-open).
    assert_eq!(layout_hit_test(&root, Point::new(11.0, 10.0)), None);
    assert_eq!(layout_hit_test(&root, Point::new(10.0, 11.0)), None);
}

#[test]
fn hit_test_negative_coordinates() {
    let id = WidgetId::next();
    let root = make_node(-50.0, -50.0, 100.0, 100.0, Some(id));

    assert_eq!(layout_hit_test(&root, Point::new(-25.0, -25.0)), Some(id));
    assert_eq!(layout_hit_test(&root, Point::new(25.0, 25.0)), Some(id));
    assert_eq!(layout_hit_test(&root, Point::new(50.0, 0.0)), None);
}

#[test]
fn hit_test_child_extends_beyond_parent() {
    // Child extends past parent's right edge.
    // Our hit test checks parent bounds first, so the child's out-of-bounds
    // portion is not reachable (implicit clipping by parent rect).
    let parent_id = WidgetId::next();
    let child_id = WidgetId::next();

    let child = make_node(50.0, 0.0, 100.0, 50.0, Some(child_id));
    let mut parent = make_node(0.0, 0.0, 100.0, 50.0, Some(parent_id));
    parent.children.push(child);

    // Inside child AND parent → child.
    assert_eq!(
        layout_hit_test(&parent, Point::new(75.0, 25.0)),
        Some(child_id)
    );
    // Inside child but OUTSIDE parent → miss (parent clips).
    assert_eq!(layout_hit_test(&parent, Point::new(125.0, 25.0)), None);
}

#[test]
fn hit_test_three_overlapping_siblings() {
    let parent_id = WidgetId::next();
    let a = WidgetId::next();
    let b = WidgetId::next();
    let c = WidgetId::next();

    // All overlap at (20,20)-(40,40).
    let child_a = make_node(10.0, 10.0, 40.0, 40.0, Some(a));
    let child_b = make_node(20.0, 20.0, 40.0, 40.0, Some(b));
    let child_c = make_node(15.0, 15.0, 30.0, 30.0, Some(c));

    let mut parent = make_node(0.0, 0.0, 100.0, 100.0, Some(parent_id));
    parent.children.push(child_a);
    parent.children.push(child_b);
    parent.children.push(child_c);

    // Last child (c) wins in overlap region.
    assert_eq!(layout_hit_test(&parent, Point::new(30.0, 30.0)), Some(c));
    // Only b covers (55, 55).
    assert_eq!(layout_hit_test(&parent, Point::new(55.0, 55.0)), Some(b));
    // Only a covers (12, 12).
    assert_eq!(layout_hit_test(&parent, Point::new(12.0, 12.0)), Some(a));
}

#[test]
fn hit_test_no_id_middle_layer_falls_through() {
    // Root (ID) → Middle (no ID) → Leaf (ID).
    // Hit in leaf area → leaf. Hit in middle-only area → root (skip middle).
    let root_id = WidgetId::next();
    let leaf_id = WidgetId::next();

    let leaf = make_node(30.0, 30.0, 20.0, 20.0, Some(leaf_id));
    let mut middle = make_node(10.0, 10.0, 60.0, 60.0, None);
    middle.children.push(leaf);
    let mut root = make_node(0.0, 0.0, 100.0, 100.0, Some(root_id));
    root.children.push(middle);

    assert_eq!(
        layout_hit_test(&root, Point::new(35.0, 35.0)),
        Some(leaf_id)
    );
    // Inside middle but not leaf → falls through middle (no ID) to root.
    assert_eq!(
        layout_hit_test(&root, Point::new(15.0, 15.0)),
        Some(root_id)
    );
}

#[test]
fn routing_no_hover_change_while_captured() {
    // Chromium pattern: suppress hover transitions during capture.
    let (root, a, _b) = two_widget_tree();
    let mut state = InputState::new();

    // Hover A, then capture A.
    state.process_mouse_event(mouse_move(25.0, 50.0), &root);
    state.process_mouse_event(mouse_down(25.0, 50.0, MouseButton::Left), &root);
    assert_eq!(state.captured(), Some(a));
    assert_eq!(state.hovered(), Some(a));

    // Move over B while captured → NO hover Leave(A)/Enter(B).
    let actions = state.process_mouse_event(mouse_move(75.0, 50.0), &root);
    assert_eq!(hover_count(&actions), 0, "hover suppressed during capture");
    assert_eq!(state.hovered(), Some(a), "hovered frozen during capture");
    assert!(delivers_to(&actions, a), "captured widget receives event");
}

#[test]
fn routing_capture_release_restores_hover() {
    // After mouseup releases capture, hover should update to the widget
    // under the cursor (which may have changed during the drag).
    let (root, a, b) = two_widget_tree();
    let mut state = InputState::new();

    // Hover A, capture A.
    state.process_mouse_event(mouse_move(25.0, 50.0), &root);
    state.process_mouse_event(mouse_down(25.0, 50.0, MouseButton::Left), &root);

    // Drag to B.
    state.process_mouse_event(mouse_move(75.0, 50.0), &root);
    assert_eq!(state.hovered(), Some(a), "still frozen");

    // Release over B → hover transitions fire: Leave(A), Enter(B).
    let actions = state.process_mouse_event(mouse_up(75.0, 50.0, MouseButton::Left), &root);
    assert!(
        actions.contains(&RouteAction::Hover {
            target: a,
            kind: HoverEvent::Leave,
        }),
        "Leave(A) on capture release"
    );
    assert!(
        actions.contains(&RouteAction::Hover {
            target: b,
            kind: HoverEvent::Enter,
        }),
        "Enter(B) on capture release"
    );
    assert_eq!(state.hovered(), Some(b));
    assert_eq!(state.captured(), None);
}

#[test]
fn routing_captured_move_outside_all_bounds() {
    let id = WidgetId::next();
    let root = make_node(10.0, 10.0, 50.0, 50.0, Some(id));
    let mut state = InputState::new();

    // Hover, capture.
    state.process_mouse_event(mouse_move(25.0, 25.0), &root);
    state.process_mouse_event(mouse_down(25.0, 25.0, MouseButton::Left), &root);
    assert_eq!(state.captured(), Some(id));

    // Move outside all bounds → still delivered to captured widget.
    let actions = state.process_mouse_event(mouse_move(200.0, 200.0), &root);
    assert!(
        delivers_to(&actions, id),
        "captured widget receives even outside bounds"
    );
    assert_eq!(hover_count(&actions), 0, "no hover change while captured");
}

#[test]
fn routing_scroll_routes_to_hovered() {
    let (root, a, _b) = two_widget_tree();
    let mut state = InputState::new();

    // Hover A.
    state.process_mouse_event(mouse_move(25.0, 50.0), &root);

    // Scroll on A.
    let actions = state.process_mouse_event(mouse_scroll(25.0, 50.0, -3.0), &root);
    assert!(
        delivers_to(&actions, a),
        "scroll delivered to hovered widget"
    );
    // Scroll should not capture.
    assert_eq!(state.captured(), None, "scroll does not set capture");
}

#[test]
fn routing_scroll_routes_to_captured() {
    let (root, a, _b) = two_widget_tree();
    let mut state = InputState::new();

    // Hover A, capture A.
    state.process_mouse_event(mouse_move(25.0, 50.0), &root);
    state.process_mouse_event(mouse_down(25.0, 50.0, MouseButton::Left), &root);

    // Scroll over B while A is captured → delivered to A.
    let actions = state.process_mouse_event(mouse_scroll(75.0, 50.0, -3.0), &root);
    assert!(delivers_to(&actions, a), "scroll goes to captured widget");
}

#[test]
fn routing_rapid_down_up_sequence() {
    let id = WidgetId::next();
    let root = make_node(0.0, 0.0, 100.0, 100.0, Some(id));
    let mut state = InputState::new();

    // Hover.
    state.process_mouse_event(mouse_move(50.0, 50.0), &root);

    // Rapid click: down, up, down, up.
    state.process_mouse_event(mouse_down(50.0, 50.0, MouseButton::Left), &root);
    assert_eq!(state.captured(), Some(id));
    state.process_mouse_event(mouse_up(50.0, 50.0, MouseButton::Left), &root);
    assert_eq!(state.captured(), None);
    state.process_mouse_event(mouse_down(50.0, 50.0, MouseButton::Left), &root);
    assert_eq!(state.captured(), Some(id));
    state.process_mouse_event(mouse_up(50.0, 50.0, MouseButton::Left), &root);
    assert_eq!(state.captured(), None);

    // Hover should still be correct.
    assert_eq!(state.hovered(), Some(id));
}

#[test]
fn routing_move_outside_all_then_leave() {
    // Move to widget, then move outside all bounds (still in window),
    // then cursor leaves window.
    let id = WidgetId::next();
    let root = make_node(10.0, 10.0, 50.0, 50.0, Some(id));
    let mut state = InputState::new();

    // Enter widget.
    state.process_mouse_event(mouse_move(30.0, 30.0), &root);
    assert_eq!(state.hovered(), Some(id));

    // Move outside all widget bounds but still in window.
    let actions = state.process_mouse_event(mouse_move(5.0, 5.0), &root);
    assert!(
        actions.contains(&RouteAction::Hover {
            target: id,
            kind: HoverEvent::Leave,
        }),
        "Leave when moving out of widget"
    );
    assert_eq!(state.hovered(), None);

    // Cursor leaves window — no duplicate leave.
    let actions = state.process_cursor_left();
    assert_eq!(hover_count(&actions), 0, "no widget to leave");
}

#[test]
fn routing_cursor_pos_always_latest() {
    let root = make_node(0.0, 0.0, 100.0, 100.0, None);
    let mut state = InputState::new();

    state.process_mouse_event(mouse_move(10.0, 20.0), &root);
    assert_eq!(state.cursor_pos(), Some(Point::new(10.0, 20.0)));

    state.process_mouse_event(mouse_move(50.0, 60.0), &root);
    assert_eq!(state.cursor_pos(), Some(Point::new(50.0, 60.0)));

    state.process_cursor_left();
    assert_eq!(state.cursor_pos(), None);
}

#[test]
fn modifiers_bitmask_operations() {
    let ctrl_shift = Modifiers::CTRL_ONLY.union(Modifiers::SHIFT_ONLY);
    assert!(ctrl_shift.ctrl());
    assert!(ctrl_shift.shift());
    assert!(!ctrl_shift.alt());
    assert!(!ctrl_shift.logo());

    assert_eq!(Modifiers::NONE, Modifiers::default());
    assert!(!Modifiers::NONE.shift());
}
