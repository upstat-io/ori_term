use super::{Direction, cycle, navigate, nearest_pane};
use crate::id::PaneId;
use crate::layout::compute::PaneLayout;
use crate::layout::floating::Rect;

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
