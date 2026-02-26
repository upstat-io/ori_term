use super::{FloatingLayer, FloatingPane, snap_to_edge};
use crate::id::PaneId;
use crate::layout::rect::Rect;

fn p(n: u64) -> PaneId {
    PaneId::from_raw(n)
}

fn floating(id: u64, x: f32, y: f32, w: f32, h: f32, z: u32) -> FloatingPane {
    FloatingPane {
        pane_id: p(id),
        rect: Rect {
            x,
            y,
            width: w,
            height: h,
        },
        z_order: z,
    }
}

// ── Add / Remove / Contains

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

// ── Hit testing

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

// ── Raise / Lower

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

// ── Move / Resize

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

// ── pane_rect

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

// ── Z-order invariant

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

// ── Z-order stability across mutations

#[test]
fn floating_z_order_stable_after_add_remove() {
    // Start with two floating panes.
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(1, 0.0, 0.0, 100.0, 100.0, 0));
    let layer = layer.add(floating(2, 0.0, 0.0, 100.0, 100.0, 5));
    let layer = layer.add(floating(3, 0.0, 0.0, 100.0, 100.0, 10));

    // Record z-orders.
    let z_before: Vec<(PaneId, u32)> = layer
        .panes()
        .iter()
        .map(|p| (p.pane_id, p.z_order))
        .collect();

    // Add and remove a different pane — existing z-orders unchanged.
    let layer = layer.add(floating(4, 200.0, 200.0, 50.0, 50.0, 7));
    let layer = layer.remove(p(4));

    let z_after: Vec<(PaneId, u32)> = layer
        .panes()
        .iter()
        .map(|p| (p.pane_id, p.z_order))
        .collect();

    assert_eq!(z_before, z_after);
}

#[test]
fn floating_z_order_stable_after_move_and_resize() {
    let layer = FloatingLayer::new();
    let layer = layer.add(floating(1, 0.0, 0.0, 100.0, 100.0, 0));
    let layer = layer.add(floating(2, 50.0, 50.0, 100.0, 100.0, 1));

    let z_before: Vec<u32> = layer.panes().iter().map(|p| p.z_order).collect();

    // Move and resize pane 1 — z-orders should not change.
    let layer = layer.move_pane(p(1), 300.0, 300.0);
    let layer = layer.resize_pane(p(1), 200.0, 200.0);

    let z_after: Vec<u32> = layer.panes().iter().map(|p| p.z_order).collect();
    assert_eq!(z_before, z_after);

    // Hit test should still respect z-order (pane 2 on top if overlapping).
    // After move, pane 1 is at (300,300), pane 2 at (50,50) — no overlap.
    assert_eq!(layer.hit_test(350.0, 350.0), Some(p(1)));
    assert_eq!(layer.hit_test(75.0, 75.0), Some(p(2)));
}

// ── Default centered size

/// Tolerance for floating point comparisons involving 0.6 multiplication
/// (0.6 is not exactly representable in IEEE 754 binary32).
const FLOAT_TOL: f32 = 0.01;

#[test]
fn centered_pane_is_60_percent_of_available() {
    let available = Rect {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 800.0,
    };
    let fp = FloatingPane::centered(p(1), &available, 0);

    assert!((fp.rect.width - 600.0).abs() < FLOAT_TOL);
    assert!((fp.rect.height - 480.0).abs() < FLOAT_TOL);
}

#[test]
fn centered_pane_is_centered_in_available() {
    let available = Rect {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 800.0,
    };
    let fp = FloatingPane::centered(p(1), &available, 0);

    // 60% of 1000 = 600, margin = (1000-600)/2 = 200.
    assert!((fp.rect.x - 200.0).abs() < FLOAT_TOL);
    // 60% of 800 = 480, margin = (800-480)/2 = 160.
    assert!((fp.rect.y - 160.0).abs() < FLOAT_TOL);
}

#[test]
fn centered_pane_respects_available_offset() {
    let available = Rect {
        x: 50.0,
        y: 100.0,
        width: 1000.0,
        height: 800.0,
    };
    let fp = FloatingPane::centered(p(1), &available, 5);

    // Center within offset area.
    assert!((fp.rect.x - 250.0).abs() < FLOAT_TOL);
    assert!((fp.rect.y - 260.0).abs() < FLOAT_TOL);
    assert_eq!(fp.z_order, 5);
}

// ── Snap-to-edge

#[test]
fn snap_to_left_edge() {
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 800.0,
    };
    // Pane at x=8 (within 10px of left edge) should snap to x=0.
    let (sx, sy) = snap_to_edge(8.0, 100.0, 200.0, 150.0, &bounds);
    assert!((sx - 0.0).abs() < f32::EPSILON);
    assert!((sy - 100.0).abs() < f32::EPSILON);
}

#[test]
fn snap_to_right_edge() {
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 800.0,
    };
    // Pane right edge at x=200 + w=200 = 993, gap to 1000 = 7px → snap.
    let (sx, _) = snap_to_edge(793.0, 100.0, 200.0, 150.0, &bounds);
    assert!((sx - 800.0).abs() < f32::EPSILON);
}

#[test]
fn snap_to_top_edge() {
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 800.0,
    };
    let (_, sy) = snap_to_edge(100.0, 5.0, 200.0, 150.0, &bounds);
    assert!((sy - 0.0).abs() < f32::EPSILON);
}

#[test]
fn snap_to_bottom_edge() {
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 800.0,
    };
    // Pane bottom at y=100 + h=150 = 645, gap to 800 = 155 → no snap.
    // Pane bottom at y=643 + h=150 = 793, gap to 800 = 7 → snap.
    let (_, sy) = snap_to_edge(100.0, 643.0, 200.0, 150.0, &bounds);
    assert!((sy - 650.0).abs() < f32::EPSILON);
}

#[test]
fn snap_to_corner() {
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 800.0,
    };
    // Near top-left corner: both x and y within threshold.
    let (sx, sy) = snap_to_edge(3.0, 7.0, 200.0, 150.0, &bounds);
    assert!((sx - 0.0).abs() < f32::EPSILON);
    assert!((sy - 0.0).abs() < f32::EPSILON);
}

#[test]
fn no_snap_when_far_from_edges() {
    let bounds = Rect {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 800.0,
    };
    let (sx, sy) = snap_to_edge(200.0, 300.0, 200.0, 150.0, &bounds);
    assert!((sx - 200.0).abs() < f32::EPSILON);
    assert!((sy - 300.0).abs() < f32::EPSILON);
}

#[test]
fn snap_respects_bounds_offset() {
    let bounds = Rect {
        x: 50.0,
        y: 100.0,
        width: 900.0,
        height: 600.0,
    };
    // Near left edge of offset bounds: x=55, within 10px of bounds.x=50 → snap to 50.
    let (sx, _) = snap_to_edge(55.0, 300.0, 200.0, 150.0, &bounds);
    assert!((sx - 50.0).abs() < f32::EPSILON);
}
