use super::{Direction, cycle, navigate, nearest_pane};
use crate::id::PaneId;
use crate::layout::compute::PaneLayout;
use crate::layout::rect::Rect;

fn p(n: u64) -> PaneId {
    PaneId::from_raw(n)
}

fn tiled(id: u64, x: f32, y: f32, w: f32, h: f32) -> PaneLayout {
    PaneLayout {
        pane_id: p(id),
        pixel_rect: Rect {
            x,
            y,
            width: w,
            height: h,
        },
        cols: (w / 10.0) as u16,
        rows: (h / 20.0) as u16,
        is_focused: false,
        is_floating: false,
    }
}

fn floating(id: u64, x: f32, y: f32, w: f32, h: f32) -> PaneLayout {
    PaneLayout {
        pane_id: p(id),
        pixel_rect: Rect {
            x,
            y,
            width: w,
            height: h,
        },
        cols: (w / 10.0) as u16,
        rows: (h / 20.0) as u16,
        is_focused: false,
        is_floating: true,
    }
}

/// Create a 2x2 grid layout:
/// ```text
///   p1 (0,0,500,400)    | p2 (500,0,500,400)
///   p3 (0,400,500,400)  | p4 (500,400,500,400)
/// ```
fn grid_2x2() -> Vec<PaneLayout> {
    vec![
        tiled(1, 0.0, 0.0, 500.0, 400.0),
        tiled(2, 500.0, 0.0, 500.0, 400.0),
        tiled(3, 0.0, 400.0, 500.0, 400.0),
        tiled(4, 500.0, 400.0, 500.0, 400.0),
    ]
}

// ── Navigate: 2x2 grid ────────────────────────────────────────────

#[test]
fn navigate_right_from_top_left() {
    let layouts = grid_2x2();
    assert_eq!(navigate(&layouts, p(1), Direction::Right), Some(p(2)));
}

#[test]
fn navigate_down_from_top_left() {
    let layouts = grid_2x2();
    assert_eq!(navigate(&layouts, p(1), Direction::Down), Some(p(3)));
}

#[test]
fn navigate_left_from_top_right() {
    let layouts = grid_2x2();
    assert_eq!(navigate(&layouts, p(2), Direction::Left), Some(p(1)));
}

#[test]
fn navigate_up_from_bottom_left() {
    let layouts = grid_2x2();
    assert_eq!(navigate(&layouts, p(3), Direction::Up), Some(p(1)));
}

#[test]
fn navigate_right_from_rightmost_returns_none() {
    let layouts = grid_2x2();
    assert_eq!(navigate(&layouts, p(2), Direction::Right), None);
}

#[test]
fn navigate_up_from_topmost_returns_none() {
    let layouts = grid_2x2();
    assert_eq!(navigate(&layouts, p(1), Direction::Up), None);
}

#[test]
fn navigate_diagonal_picks_nearest() {
    // From bottom-right, navigate up — should pick top-right (directly above).
    let layouts = grid_2x2();
    assert_eq!(navigate(&layouts, p(4), Direction::Up), Some(p(2)));
}

// ── Cycle ─────────────────────────────────────────────────────────

#[test]
fn cycle_forward_visits_in_order() {
    let layouts = grid_2x2();
    assert_eq!(cycle(&layouts, p(1), true), Some(p(2)));
    assert_eq!(cycle(&layouts, p(2), true), Some(p(3)));
    assert_eq!(cycle(&layouts, p(3), true), Some(p(4)));
}

#[test]
fn cycle_forward_wraps_to_first() {
    let layouts = grid_2x2();
    assert_eq!(cycle(&layouts, p(4), true), Some(p(1)));
}

#[test]
fn cycle_backward_visits_reverse_order() {
    let layouts = grid_2x2();
    assert_eq!(cycle(&layouts, p(4), false), Some(p(3)));
    assert_eq!(cycle(&layouts, p(3), false), Some(p(2)));
}

#[test]
fn cycle_backward_wraps_to_last() {
    let layouts = grid_2x2();
    assert_eq!(cycle(&layouts, p(1), false), Some(p(4)));
}

#[test]
fn cycle_single_pane_returns_self() {
    let layouts = vec![tiled(1, 0.0, 0.0, 1000.0, 800.0)];
    assert_eq!(cycle(&layouts, p(1), true), Some(p(1)));
    assert_eq!(cycle(&layouts, p(1), false), Some(p(1)));
}

// ── nearest_pane ──────────────────────────────────────────────────

#[test]
fn nearest_pane_finds_tiled() {
    let layouts = grid_2x2();
    assert_eq!(nearest_pane(&layouts, 250.0, 200.0), Some(p(1)));
    assert_eq!(nearest_pane(&layouts, 750.0, 200.0), Some(p(2)));
    assert_eq!(nearest_pane(&layouts, 250.0, 600.0), Some(p(3)));
    assert_eq!(nearest_pane(&layouts, 750.0, 600.0), Some(p(4)));
}

#[test]
fn nearest_pane_prefers_floating_over_tiled() {
    let mut layouts = grid_2x2();
    // Add a floating pane overlapping p1's area.
    layouts.push(floating(10, 100.0, 100.0, 200.0, 200.0));

    // Point in overlap region — floating pane should win.
    assert_eq!(nearest_pane(&layouts, 200.0, 200.0), Some(p(10)));
}

#[test]
fn nearest_pane_returns_none_outside_all() {
    let layouts = vec![tiled(1, 100.0, 100.0, 200.0, 200.0)];
    assert_eq!(nearest_pane(&layouts, 0.0, 0.0), None);
}

// ── Navigate with floating panes ──────────────────────────────────

#[test]
fn navigate_from_tiled_to_floating() {
    let mut layouts = vec![tiled(1, 0.0, 0.0, 500.0, 800.0)];
    // Floating pane to the right.
    layouts.push(floating(2, 600.0, 200.0, 200.0, 200.0));

    assert_eq!(navigate(&layouts, p(1), Direction::Right), Some(p(2)));
}

// ── Navigate: uneven splits ──────────────────────────────────────

/// Uneven 60/40 vertical split: pane 1 is wider, pane 2 is narrower.
fn uneven_split() -> Vec<PaneLayout> {
    vec![
        tiled(1, 0.0, 0.0, 600.0, 800.0),
        tiled(2, 602.0, 0.0, 398.0, 800.0),
    ]
}

#[test]
fn navigate_right_in_uneven_split() {
    let layouts = uneven_split();
    assert_eq!(navigate(&layouts, p(1), Direction::Right), Some(p(2)));
}

#[test]
fn navigate_left_in_uneven_split() {
    let layouts = uneven_split();
    assert_eq!(navigate(&layouts, p(2), Direction::Left), Some(p(1)));
}

/// Uneven 2x2: top-left is 70% wide, bottom-right is 30% wide.
#[test]
fn navigate_in_uneven_2x2_grid() {
    let layouts = vec![
        tiled(1, 0.0, 0.0, 700.0, 400.0),
        tiled(2, 702.0, 0.0, 298.0, 400.0),
        tiled(3, 0.0, 402.0, 700.0, 398.0),
        tiled(4, 702.0, 402.0, 298.0, 398.0),
    ];

    assert_eq!(navigate(&layouts, p(1), Direction::Right), Some(p(2)));
    assert_eq!(navigate(&layouts, p(1), Direction::Down), Some(p(3)));
    assert_eq!(navigate(&layouts, p(4), Direction::Up), Some(p(2)));
    assert_eq!(navigate(&layouts, p(4), Direction::Left), Some(p(3)));
}

// ── Empty / single-element edge cases ────────────────────────────

#[test]
fn navigate_empty_layouts_returns_none() {
    let layouts: Vec<PaneLayout> = vec![];
    assert_eq!(navigate(&layouts, p(1), Direction::Right), None);
}

#[test]
fn cycle_empty_layouts_returns_none() {
    let layouts: Vec<PaneLayout> = vec![];
    assert_eq!(cycle(&layouts, p(1), true), None);
    assert_eq!(cycle(&layouts, p(1), false), None);
}

#[test]
fn nearest_pane_empty_layouts_returns_none() {
    let layouts: Vec<PaneLayout> = vec![];
    assert_eq!(nearest_pane(&layouts, 500.0, 400.0), None);
}

#[test]
fn navigate_from_nonexistent_pane_returns_none() {
    let layouts = grid_2x2();
    assert_eq!(navigate(&layouts, p(99), Direction::Right), None);
}

#[test]
fn cycle_from_nonexistent_pane_returns_none() {
    let layouts = grid_2x2();
    assert_eq!(cycle(&layouts, p(99), true), None);
}

// ── Asymmetric T/L-shape navigation ─────────────────────────────

/// T-shape: two panes on top, one full-width pane below.
/// ```text
///   p1 (0,0,500,400)    | p2 (500,0,500,400)
///   p3 (0,400,1000,400) — full width
/// ```
fn t_shape() -> Vec<PaneLayout> {
    vec![
        tiled(1, 0.0, 0.0, 500.0, 400.0),
        tiled(2, 500.0, 0.0, 500.0, 400.0),
        tiled(3, 0.0, 400.0, 1000.0, 400.0),
    ]
}

#[test]
fn navigate_up_from_wide_bottom_pane_picks_nearest_above() {
    // p3 center is (500, 600). Both p1 (250,200) and p2 (750,200) are above.
    // p1 perp_dist=250, p2 perp_dist=250 — equal distance; first-in-list wins.
    let layouts = t_shape();
    let result = navigate(&layouts, p(3), Direction::Up);
    assert!(
        result == Some(p(1)) || result == Some(p(2)),
        "should navigate to one of the top panes, got {result:?}",
    );
}

#[test]
fn navigate_down_to_wide_bottom_pane() {
    let layouts = t_shape();
    assert_eq!(navigate(&layouts, p(1), Direction::Down), Some(p(3)));
    assert_eq!(navigate(&layouts, p(2), Direction::Down), Some(p(3)));
}

/// L-shape: tall left pane, two short panes stacked on the right.
/// ```text
///   p1 (0,0,500,800)  | p2 (500,0,500,400)
///                      | p3 (500,400,500,400)
/// ```
fn l_shape() -> Vec<PaneLayout> {
    vec![
        tiled(1, 0.0, 0.0, 500.0, 800.0),
        tiled(2, 500.0, 0.0, 500.0, 400.0),
        tiled(3, 500.0, 400.0, 500.0, 400.0),
    ]
}

#[test]
fn navigate_left_from_short_pane_to_tall_pane() {
    // p3 center is (750,600). Navigating left: only p1 (250,400) is to the left.
    let layouts = l_shape();
    assert_eq!(navigate(&layouts, p(3), Direction::Left), Some(p(1)));
}

#[test]
fn navigate_right_from_tall_to_nearest_short() {
    // p1 center is (250,400). p2 center is (750,200), p3 center is (750,600).
    // Both are at primary_dist=500, but p2 perp_dist=200, p3 perp_dist=200 — tie.
    // First-in-list (p2) wins.
    let layouts = l_shape();
    let result = navigate(&layouts, p(1), Direction::Right);
    assert!(
        result == Some(p(2)) || result == Some(p(3)),
        "should navigate to one of the right panes, got {result:?}",
    );
}

// ── 3-pane nested split ─────────────────────────────────────────

/// 3-pane: left half split horizontally, right half is one pane.
/// ```text
///   p1 (0,0,500,400)    | p3 (500,0,500,800)
///   p2 (0,400,500,400)  |
/// ```
fn nested_3_pane() -> Vec<PaneLayout> {
    vec![
        tiled(1, 0.0, 0.0, 500.0, 400.0),
        tiled(2, 0.0, 400.0, 500.0, 400.0),
        tiled(3, 500.0, 0.0, 500.0, 800.0),
    ]
}

#[test]
fn navigate_3_pane_all_directions() {
    let layouts = nested_3_pane();
    // p1 → right → p3.
    assert_eq!(navigate(&layouts, p(1), Direction::Right), Some(p(3)));
    // p1 → down → p2.
    assert_eq!(navigate(&layouts, p(1), Direction::Down), Some(p(2)));
    // p2 → up → p1.
    assert_eq!(navigate(&layouts, p(2), Direction::Up), Some(p(1)));
    // p2 → right → p3.
    assert_eq!(navigate(&layouts, p(2), Direction::Right), Some(p(3)));
    // p3 → left → p1 or p2 (depends on centroid proximity).
    let left = navigate(&layouts, p(3), Direction::Left);
    assert!(
        left == Some(p(1)) || left == Some(p(2)),
        "p3 left should reach p1 or p2, got {left:?}",
    );
}

#[test]
fn cycle_3_pane_visits_all() {
    let layouts = nested_3_pane();
    assert_eq!(cycle(&layouts, p(1), true), Some(p(2)));
    assert_eq!(cycle(&layouts, p(2), true), Some(p(3)));
    assert_eq!(cycle(&layouts, p(3), true), Some(p(1)));
}

// ── Exhaustive removal ──────────────────────────────────────────

#[test]
fn navigate_after_progressive_pane_removal() {
    // Start with 3 panes, remove them one by one. Navigation should never panic.
    let mut layouts = nested_3_pane();

    // Remove p2 — now p1 and p3 remain.
    // p1 center (250,200), p3 center (750,400) — p3 is right and below.
    layouts.retain(|l| l.pane_id != p(2));
    assert_eq!(navigate(&layouts, p(1), Direction::Right), Some(p(3)));
    assert_eq!(navigate(&layouts, p(1), Direction::Down), Some(p(3)));

    // Remove p3 — only p1 remains.
    layouts.retain(|l| l.pane_id != p(3));
    assert_eq!(navigate(&layouts, p(1), Direction::Right), None);
    assert_eq!(navigate(&layouts, p(1), Direction::Left), None);
}

#[test]
fn cycle_after_progressive_pane_removal() {
    let mut layouts = nested_3_pane();

    // Remove p2.
    layouts.retain(|l| l.pane_id != p(2));
    assert_eq!(cycle(&layouts, p(1), true), Some(p(3)));
    assert_eq!(cycle(&layouts, p(3), true), Some(p(1)));

    // Remove p3 — single pane wraps to self.
    layouts.retain(|l| l.pane_id != p(3));
    assert_eq!(cycle(&layouts, p(1), true), Some(p(1)));
    assert_eq!(cycle(&layouts, p(1), false), Some(p(1)));
}

// ── Border and tie-breaking ─────────────────────────────────────

#[test]
fn nearest_pane_on_exact_border_goes_to_right_neighbor() {
    // Half-open intervals: x=500.0 is the first pixel of p2 (p1 ends at 500.0 exclusive).
    let layouts = grid_2x2();
    assert_eq!(nearest_pane(&layouts, 500.0, 200.0), Some(p(2)));
}

#[test]
fn nearest_pane_just_inside_left_edge() {
    let layouts = grid_2x2();
    // x=499.9 is inside p1; x=500.0 is inside p2.
    assert_eq!(nearest_pane(&layouts, 499.9, 200.0), Some(p(1)));
}

#[test]
fn navigate_equidistant_candidates_is_deterministic() {
    // 3 columns, equal width. From the center column, navigate right:
    // only one candidate (p3). Navigate left: only one (p1).
    let layouts = vec![
        tiled(1, 0.0, 0.0, 333.0, 800.0),
        tiled(2, 334.0, 0.0, 333.0, 800.0),
        tiled(3, 668.0, 0.0, 332.0, 800.0),
    ];
    assert_eq!(navigate(&layouts, p(2), Direction::Right), Some(p(3)));
    assert_eq!(navigate(&layouts, p(2), Direction::Left), Some(p(1)));

    // From p1, navigate right: p2 and p3 are both to the right.
    // p2 (primary_dist=168) beats p3 (primary_dist=502).
    assert_eq!(navigate(&layouts, p(1), Direction::Right), Some(p(2)));
}

// ── Cycle with floating + tiled mixed ───────────────────────────

#[test]
fn cycle_with_floating_panes_visits_all() {
    let layouts = vec![
        tiled(1, 0.0, 0.0, 500.0, 800.0),
        tiled(2, 500.0, 0.0, 500.0, 800.0),
        floating(10, 200.0, 200.0, 300.0, 300.0),
    ];
    // Cycle visits in layout order: p1 → p2 → p10 → p1.
    assert_eq!(cycle(&layouts, p(1), true), Some(p(2)));
    assert_eq!(cycle(&layouts, p(2), true), Some(p(10)));
    assert_eq!(cycle(&layouts, p(10), true), Some(p(1)));

    // Backward: p1 → p10 → p2 → p1.
    assert_eq!(cycle(&layouts, p(1), false), Some(p(10)));
    assert_eq!(cycle(&layouts, p(10), false), Some(p(2)));
}

// ── Navigate: single pane ────────────────────────────────────────

#[test]
fn navigate_single_pane_returns_none_all_directions() {
    let layouts = vec![tiled(1, 0.0, 0.0, 1000.0, 800.0)];
    assert_eq!(navigate(&layouts, p(1), Direction::Up), None);
    assert_eq!(navigate(&layouts, p(1), Direction::Down), None);
    assert_eq!(navigate(&layouts, p(1), Direction::Left), None);
    assert_eq!(navigate(&layouts, p(1), Direction::Right), None);
}

// ── Navigate: floating → tiled ──────────────────────────────────

#[test]
fn navigate_from_floating_to_tiled() {
    let layouts = vec![
        tiled(1, 0.0, 0.0, 500.0, 800.0),
        tiled(2, 500.0, 0.0, 500.0, 800.0),
        floating(10, 200.0, 200.0, 200.0, 200.0),
    ];
    // Floating p10 center (300,300). Navigate right → p2 center (750,400).
    assert_eq!(navigate(&layouts, p(10), Direction::Right), Some(p(2)));
    // Navigate left → p1 center (250,400). p1's center.x < p10's center.x.
    assert_eq!(navigate(&layouts, p(10), Direction::Left), Some(p(1)));
}

#[test]
fn navigate_from_floating_to_floating() {
    // Two floating panes side-by-side with no tiled pane between them.
    let layouts = vec![
        floating(10, 100.0, 100.0, 200.0, 200.0),
        floating(11, 600.0, 100.0, 200.0, 200.0),
    ];
    // p10 center (200,200), p11 center (700,200). Navigate right from p10.
    assert_eq!(navigate(&layouts, p(10), Direction::Right), Some(p(11)));
    assert_eq!(navigate(&layouts, p(11), Direction::Left), Some(p(10)));
}

// ── Multiple overlapping floating panes (z-order) ───────────────

#[test]
fn nearest_pane_prefers_topmost_floating_in_overlap() {
    let layouts = vec![
        tiled(1, 0.0, 0.0, 1000.0, 800.0),
        floating(10, 100.0, 100.0, 300.0, 300.0),
        floating(11, 150.0, 150.0, 300.0, 300.0), // later = higher z
    ];
    // Click in overlap region of both floats — p11 (higher z) should win.
    assert_eq!(nearest_pane(&layouts, 250.0, 250.0), Some(p(11)));
}

#[test]
fn nearest_pane_falls_through_to_lower_float() {
    let layouts = vec![
        tiled(1, 0.0, 0.0, 1000.0, 800.0),
        floating(10, 100.0, 100.0, 300.0, 300.0),
        floating(11, 500.0, 500.0, 200.0, 200.0),
    ];
    // Click in p10 only (not in p11 region) — p10 should win.
    assert_eq!(nearest_pane(&layouts, 200.0, 200.0), Some(p(10)));
}

// ── Partial overlap edge adjacency ──────────────────────────────

#[test]
fn navigate_with_partial_vertical_overlap() {
    // Pane 2 only partially overlaps with pane 1's vertical range.
    // p1: (0,0, 500, 400), p2: (500, 200, 500, 400)
    // p2 starts at y=200, so only overlaps bottom half of p1's height.
    let layouts = vec![
        tiled(1, 0.0, 0.0, 500.0, 400.0),
        tiled(2, 500.0, 200.0, 500.0, 400.0),
    ];
    // p1 center (250,200), p2 center (750,400). Navigate right: p2 is to the right.
    assert_eq!(navigate(&layouts, p(1), Direction::Right), Some(p(2)));
    assert_eq!(navigate(&layouts, p(2), Direction::Left), Some(p(1)));
}

#[test]
fn navigate_with_no_vertical_overlap() {
    // Pane 2 is to the right but completely below pane 1's vertical range.
    // Still reachable via centroid navigation.
    let layouts = vec![
        tiled(1, 0.0, 0.0, 500.0, 200.0),
        tiled(2, 500.0, 400.0, 500.0, 200.0),
    ];
    // p1 center (250,100), p2 center (750,500).
    // Navigate right from p1: p2's center.x > p1's center.x → reachable.
    assert_eq!(navigate(&layouts, p(1), Direction::Right), Some(p(2)));
    // Navigate down from p1: p2's center.y > p1's center.y → reachable.
    assert_eq!(navigate(&layouts, p(1), Direction::Down), Some(p(2)));
}

// ── Degenerate geometry ─────────────────────────────────────────

#[test]
fn navigate_with_zero_width_pane_does_not_panic() {
    let layouts = vec![
        tiled(1, 0.0, 0.0, 500.0, 400.0),
        tiled(2, 500.0, 0.0, 0.0, 400.0), // zero-width (transient during resize)
    ];
    // Should not panic. p2's center is (500, 200). p1's center is (250, 200).
    // Navigate right from p1: p2's center.x > p1's center.x → reachable.
    let _ = navigate(&layouts, p(1), Direction::Right);
    let _ = navigate(&layouts, p(2), Direction::Left);
}

#[test]
fn navigate_with_zero_height_pane_does_not_panic() {
    let layouts = vec![
        tiled(1, 0.0, 0.0, 500.0, 400.0),
        tiled(2, 0.0, 400.0, 500.0, 0.0), // zero-height
    ];
    let _ = navigate(&layouts, p(1), Direction::Down);
    let _ = navigate(&layouts, p(2), Direction::Up);
}

#[test]
fn nearest_pane_with_zero_size_pane_does_not_panic() {
    let layouts = vec![
        tiled(1, 0.0, 0.0, 500.0, 400.0),
        tiled(2, 500.0, 0.0, 0.0, 0.0),
    ];
    // Click in p1's area — should find p1.
    assert_eq!(nearest_pane(&layouts, 250.0, 200.0), Some(p(1)));
    // Click at p2's origin — zero-size pane contains nothing.
    assert_eq!(nearest_pane(&layouts, 500.0, 0.0), None);
}

// ── nearest_pane: floating-only layouts ─────────────────────────

#[test]
fn nearest_pane_with_only_floating_panes() {
    let layouts = vec![
        floating(10, 100.0, 100.0, 200.0, 200.0),
        floating(11, 500.0, 500.0, 200.0, 200.0),
    ];
    assert_eq!(nearest_pane(&layouts, 200.0, 200.0), Some(p(10)));
    assert_eq!(nearest_pane(&layouts, 600.0, 600.0), Some(p(11)));
    // Outside all floats.
    assert_eq!(nearest_pane(&layouts, 0.0, 0.0), None);
}
