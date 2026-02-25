use super::{FloatingLayer, FloatingPane};
use crate::id::PaneId;

fn p(n: u64) -> PaneId {
    PaneId::from_raw(n)
}

fn floating(id: u64, x: f32, y: f32, w: f32, h: f32, z: u32) -> FloatingPane {
    FloatingPane {
        pane_id: p(id),
        x,
        y,
        width: w,
        height: h,
        z_order: z,
    }
}

// ── Add / Remove / Contains ───────────────────────────────────────

#[test]
fn empty_layer() {
    let layer = FloatingLayer::new();
    assert!(layer.is_empty());
    assert!(!layer.contains(p(1)));
}

#[test]
fn add_pane_appears_in_layer() {
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(1, 10.0, 20.0, 300.0, 200.0, 0));

    assert!(!layer.is_empty());
    assert!(layer.contains(p(1)));
    assert_eq!(layer.panes().len(), 1);
}

#[test]
fn remove_pane_disappears() {
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(1, 10.0, 20.0, 300.0, 200.0, 0));
    let layer = layer.add(floating(2, 50.0, 60.0, 200.0, 150.0, 1));

    let layer = layer.remove(p(1));
    assert!(!layer.contains(p(1)));
    assert!(layer.contains(p(2)));
    assert_eq!(layer.panes().len(), 1);
}

#[test]
fn remove_nonexistent_pane_is_harmless() {
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(1, 0.0, 0.0, 100.0, 100.0, 0));

    let layer = layer.remove(p(99));
    assert_eq!(layer.panes().len(), 1);
}

// ── Hit testing ───────────────────────────────────────────────────

#[test]
fn hit_test_returns_topmost_pane() {
    let layer = FloatingLayer::new();
    // Two overlapping panes. Pane 2 has higher z-order.
    let layer = layer.add(floating(1, 0.0, 0.0, 200.0, 200.0, 0));
    let layer = layer.add(floating(2, 50.0, 50.0, 200.0, 200.0, 1));

    // Point in overlap region — topmost (z=1) wins.
    assert_eq!(layer.hit_test(100.0, 100.0), Some(p(2)));
}

#[test]
fn hit_test_returns_only_pane_under_point() {
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(1, 0.0, 0.0, 100.0, 100.0, 0));
    let layer = layer.add(floating(2, 200.0, 200.0, 100.0, 100.0, 1));

    // Point only in pane 1.
    assert_eq!(layer.hit_test(50.0, 50.0), Some(p(1)));
    // Point only in pane 2.
    assert_eq!(layer.hit_test(250.0, 250.0), Some(p(2)));
}

#[test]
fn hit_test_returns_none_outside_all_panes() {
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(1, 10.0, 10.0, 50.0, 50.0, 0));

    assert_eq!(layer.hit_test(0.0, 0.0), None);
    assert_eq!(layer.hit_test(100.0, 100.0), None);
}

// ── Raise / Lower ─────────────────────────────────────────────────

#[test]
fn raise_moves_pane_to_front() {
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(1, 0.0, 0.0, 100.0, 100.0, 0));
    let layer = layer.add(floating(2, 0.0, 0.0, 100.0, 100.0, 1));

    let layer = layer.raise(p(1));

    // After raising p(1), it should be the topmost — hit test in overlap wins.
    assert_eq!(layer.hit_test(50.0, 50.0), Some(p(1)));
}

#[test]
fn lower_moves_pane_to_back() {
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(1, 0.0, 0.0, 100.0, 100.0, 0));
    let layer = layer.add(floating(2, 0.0, 0.0, 100.0, 100.0, 1));

    let layer = layer.lower(p(2));

    // After lowering p(2), p(1) should be on top.
    assert_eq!(layer.hit_test(50.0, 50.0), Some(p(1)));
}

// ── Move / Resize ─────────────────────────────────────────────────

#[test]
fn move_pane_updates_position() {
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(1, 0.0, 0.0, 100.0, 100.0, 0));

    let layer = layer.move_pane(p(1), 200.0, 300.0);

    let rect = layer.pane_rect(p(1)).expect("pane should exist");
    assert!((rect.x - 200.0).abs() < f32::EPSILON);
    assert!((rect.y - 300.0).abs() < f32::EPSILON);
    assert!((rect.width - 100.0).abs() < f32::EPSILON);
    assert!((rect.height - 100.0).abs() < f32::EPSILON);
}

#[test]
fn resize_pane_updates_dimensions() {
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(1, 10.0, 20.0, 100.0, 100.0, 0));

    let layer = layer.resize_pane(p(1), 500.0, 400.0);

    let rect = layer.pane_rect(p(1)).expect("pane should exist");
    assert!((rect.x - 10.0).abs() < f32::EPSILON);
    assert!((rect.y - 20.0).abs() < f32::EPSILON);
    assert!((rect.width - 500.0).abs() < f32::EPSILON);
    assert!((rect.height - 400.0).abs() < f32::EPSILON);
}

// ── pane_rect ─────────────────────────────────────────────────────

#[test]
fn pane_rect_returns_correct_bounds() {
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(1, 10.0, 20.0, 300.0, 200.0, 0));

    let rect = layer.pane_rect(p(1)).expect("pane should exist");
    assert!((rect.x - 10.0).abs() < f32::EPSILON);
    assert!((rect.y - 20.0).abs() < f32::EPSILON);
    assert!((rect.width - 300.0).abs() < f32::EPSILON);
    assert!((rect.height - 200.0).abs() < f32::EPSILON);
}

#[test]
fn pane_rect_returns_none_for_nonexistent() {
    let layer = FloatingLayer::new();
    assert_eq!(layer.pane_rect(p(99)), None);
}

// ── Z-order invariant ─────────────────────────────────────────────

#[test]
fn panes_sorted_by_z_order() {
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(3, 0.0, 0.0, 50.0, 50.0, 10));
    let layer = layer.add(floating(1, 0.0, 0.0, 50.0, 50.0, 0));
    let layer = layer.add(floating(2, 0.0, 0.0, 50.0, 50.0, 5));

    let z_orders: Vec<u32> = layer.panes().iter().map(|p| p.z_order).collect();
    assert!(
        z_orders.windows(2).all(|w| w[0] <= w[1]),
        "panes should be sorted by z_order: {z_orders:?}"
    );
}
