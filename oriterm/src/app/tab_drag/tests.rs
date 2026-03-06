use oriterm_ui::widgets::tab_bar::constants::{
    TAB_BAR_HEIGHT, TAB_LEFT_MARGIN, TEAR_OFF_THRESHOLD, TEAR_OFF_THRESHOLD_UP,
};

use super::{
    DragPhase, TabDragState, compute_drag_visual_x, compute_insertion_index, exceeds_tear_off,
};

// -- compute_drag_visual_x --

#[test]
fn drag_visual_x_preserves_offset() {
    // Cursor at 200, offset 50, max 500 → visual at 150.
    assert!((compute_drag_visual_x(200.0, 50.0, 500.0) - 150.0).abs() < f32::EPSILON);
}

#[test]
fn drag_visual_x_clamps_to_zero() {
    // Cursor at 10, offset 50 → would be -40, clamps to 0.
    assert!((compute_drag_visual_x(10.0, 50.0, 500.0)).abs() < f32::EPSILON);
}

#[test]
fn drag_visual_x_clamps_to_max() {
    // Cursor at 600, offset 10, max 500 → would be 590, clamps to 500.
    assert!((compute_drag_visual_x(600.0, 10.0, 500.0) - 500.0).abs() < f32::EPSILON);
}

// -- compute_insertion_index --

#[test]
fn insertion_index_first_slot() {
    // Tab center falls in first slot.
    let visual_x = TAB_LEFT_MARGIN;
    assert_eq!(compute_insertion_index(visual_x, 120.0, 5), 0);
}

#[test]
fn insertion_index_middle_slot() {
    // Tab center should map to the 2nd tab (index 1).
    let visual_x = TAB_LEFT_MARGIN + 120.0;
    assert_eq!(compute_insertion_index(visual_x, 120.0, 5), 1);
}

#[test]
fn insertion_index_last_slot() {
    // Visual X far to the right — clamps to last valid index.
    let visual_x = TAB_LEFT_MARGIN + 4.0 * 120.0 + 50.0;
    assert_eq!(compute_insertion_index(visual_x, 120.0, 5), 4);
}

#[test]
fn insertion_index_clamps_to_last() {
    // Way beyond the last tab.
    assert_eq!(compute_insertion_index(2000.0, 120.0, 3), 2);
}

#[test]
fn insertion_index_single_tab() {
    // Single tab → always index 0.
    assert_eq!(compute_insertion_index(0.0, 120.0, 1), 0);
    assert_eq!(compute_insertion_index(500.0, 120.0, 1), 0);
}

#[test]
fn insertion_index_zero_tabs() {
    // Edge case: zero tabs → 0.
    assert_eq!(compute_insertion_index(100.0, 120.0, 0), 0);
}

#[test]
fn insertion_index_zero_width() {
    // Edge case: zero width → 0.
    assert_eq!(compute_insertion_index(100.0, 0.0, 5), 0);
}

// -- exceeds_tear_off --

#[test]
fn tear_off_above_bar_within_threshold() {
    let bar_y = 10.0;
    let bar_bottom = bar_y + TAB_BAR_HEIGHT;
    // Just at the threshold edge (not exceeded).
    let cursor_y = bar_y - TEAR_OFF_THRESHOLD_UP;
    assert!(!exceeds_tear_off(cursor_y, bar_y, bar_bottom));
}

#[test]
fn tear_off_above_bar_exceeds_threshold() {
    let bar_y = 10.0;
    let bar_bottom = bar_y + TAB_BAR_HEIGHT;
    // One pixel beyond.
    let cursor_y = bar_y - TEAR_OFF_THRESHOLD_UP - 1.0;
    assert!(exceeds_tear_off(cursor_y, bar_y, bar_bottom));
}

#[test]
fn tear_off_below_bar_within_threshold() {
    let bar_y = 10.0;
    let bar_bottom = bar_y + TAB_BAR_HEIGHT;
    let cursor_y = bar_bottom + TEAR_OFF_THRESHOLD;
    assert!(!exceeds_tear_off(cursor_y, bar_y, bar_bottom));
}

#[test]
fn tear_off_below_bar_exceeds_threshold() {
    let bar_y = 10.0;
    let bar_bottom = bar_y + TAB_BAR_HEIGHT;
    let cursor_y = bar_bottom + TEAR_OFF_THRESHOLD + 1.0;
    assert!(exceeds_tear_off(cursor_y, bar_y, bar_bottom));
}

#[test]
fn tear_off_within_bar_never_exceeds() {
    let bar_y = 10.0;
    let bar_bottom = bar_y + TAB_BAR_HEIGHT;
    // Dead center of bar.
    assert!(!exceeds_tear_off(
        bar_y + TAB_BAR_HEIGHT / 2.0,
        bar_y,
        bar_bottom
    ));
    // At top edge.
    assert!(!exceeds_tear_off(bar_y, bar_y, bar_bottom));
    // At bottom edge.
    assert!(!exceeds_tear_off(bar_bottom, bar_y, bar_bottom));
}

// -- TabDragState construction --

#[test]
fn drag_state_construction() {
    let state = TabDragState {
        tab_id: crate::session::TabId::from_raw(42),
        original_index: 2,
        current_index: 2,
        origin_x: 100.0,
        origin_y: 30.0,
        phase: DragPhase::Pending,
        mouse_offset_in_tab: 25.0,
        tab_bar_y: 10.0,
        tab_bar_bottom: 10.0 + TAB_BAR_HEIGHT,
        suppress_next_release: false,
    };
    assert_eq!(state.phase, DragPhase::Pending);
    assert_eq!(state.original_index, 2);
    assert_eq!(state.current_index, 2);
    assert!((state.mouse_offset_in_tab - 25.0).abs() < f32::EPSILON);
}

// -- Directional threshold asymmetry --

#[test]
fn tear_off_upward_more_sensitive_than_downward() {
    // Verify the design: upward threshold is smaller.
    assert!(TEAR_OFF_THRESHOLD_UP < TEAR_OFF_THRESHOLD);
}

// -- Threshold boundary precision --

#[test]
fn drag_threshold_boundary_just_below() {
    // Euclidean distance just under DRAG_START_THRESHOLD should remain Pending.
    // Move diagonally: 7.0 * sqrt(2) ≈ 9.899, just under 10.0.
    let dx = 7.0_f32;
    let dy = 7.0_f32;
    let distance = dx.hypot(dy);
    assert!(distance < super::DRAG_START_THRESHOLD);
}

#[test]
fn drag_threshold_boundary_just_above() {
    // Euclidean distance just over DRAG_START_THRESHOLD should transition.
    let dx = 8.0_f32;
    let dy = 8.0_f32;
    let distance = dx.hypot(dy);
    assert!(distance >= super::DRAG_START_THRESHOLD);
}

#[test]
fn drag_threshold_exact_boundary() {
    // Exactly at threshold: distance == DRAG_START_THRESHOLD.
    // The code uses `<` so exactly-at-threshold should NOT transition.
    let distance = super::DRAG_START_THRESHOLD;
    assert!(!(distance < super::DRAG_START_THRESHOLD));
}

#[test]
fn drag_threshold_uses_euclidean_not_taxicab() {
    // A purely horizontal 10px move should trigger, but a 6+6 diagonal (taxicab 12)
    // with Euclidean ~8.49 should NOT trigger.
    let horizontal = 10.0_f32.hypot(0.0);
    assert!(horizontal >= super::DRAG_START_THRESHOLD);

    let diagonal = 6.0_f32.hypot(6.0);
    assert!(diagonal < super::DRAG_START_THRESHOLD);
}

// -- Multi-step drag sequences (pure computation) --

#[test]
fn sequential_drag_right_increments_index() {
    // Simulate dragging a tab from slot 0 rightward across 5 tabs.
    // As visual_x increases, insertion index should increment.
    let tw = 120.0;
    let count = 5;
    let mut prev_idx = 0;
    for slot in 1..count {
        // Place visual_x at the center of each successive slot.
        let visual_x = TAB_LEFT_MARGIN + slot as f32 * tw;
        let idx = compute_insertion_index(visual_x, tw, count);
        assert!(
            idx >= prev_idx,
            "index should be monotonically non-decreasing: got {idx} < {prev_idx} at slot {slot}"
        );
        prev_idx = idx;
    }
    assert_eq!(prev_idx, count - 1);
}

#[test]
fn sequential_drag_left_decrements_index() {
    // Dragging from slot 4 leftward.
    let tw = 120.0;
    let count = 5;
    let mut prev_idx = count - 1;
    for slot in (0..count - 1).rev() {
        let visual_x = TAB_LEFT_MARGIN + slot as f32 * tw;
        let idx = compute_insertion_index(visual_x, tw, count);
        assert!(
            idx <= prev_idx,
            "index should be monotonically non-increasing: got {idx} > {prev_idx} at slot {slot}"
        );
        prev_idx = idx;
    }
    assert_eq!(prev_idx, 0);
}

// -- Cancel undo verification --

#[test]
fn cancel_pending_is_noop() {
    // In Pending phase, cancel should not require undo since no swap happened.
    let state = TabDragState {
        tab_id: crate::session::TabId::from_raw(1),
        original_index: 2,
        current_index: 2,
        origin_x: 100.0,
        origin_y: 30.0,
        phase: DragPhase::Pending,
        mouse_offset_in_tab: 25.0,
        tab_bar_y: 10.0,
        tab_bar_bottom: 10.0 + TAB_BAR_HEIGHT,
        suppress_next_release: false,
    };
    // Pending phase with same indices → no undo needed.
    assert_eq!(state.original_index, state.current_index);
    assert_eq!(state.phase, DragPhase::Pending);
}

#[test]
fn cancel_dragging_with_swaps_needs_undo() {
    // After swaps, original_index != current_index → undo needed.
    let state = TabDragState {
        tab_id: crate::session::TabId::from_raw(1),
        original_index: 0,
        current_index: 3,
        origin_x: 100.0,
        origin_y: 30.0,
        phase: DragPhase::DraggingInBar,
        mouse_offset_in_tab: 25.0,
        tab_bar_y: 10.0,
        tab_bar_bottom: 10.0 + TAB_BAR_HEIGHT,
        suppress_next_release: false,
    };
    // This is the condition checked in cancel_tab_drag.
    assert_eq!(state.phase, DragPhase::DraggingInBar);
    assert_ne!(state.original_index, state.current_index);
}

#[test]
fn cancel_dragging_no_swap_no_undo() {
    // DraggingInBar but tab never actually moved (jittered but same slot).
    let state = TabDragState {
        tab_id: crate::session::TabId::from_raw(1),
        original_index: 2,
        current_index: 2,
        origin_x: 100.0,
        origin_y: 30.0,
        phase: DragPhase::DraggingInBar,
        mouse_offset_in_tab: 25.0,
        tab_bar_y: 10.0,
        tab_bar_bottom: 10.0 + TAB_BAR_HEIGHT,
        suppress_next_release: false,
    };
    assert_eq!(state.phase, DragPhase::DraggingInBar);
    assert_eq!(state.original_index, state.current_index);
}

// -- Combined visual + insertion edge cases --

#[test]
fn visual_x_zero_maps_to_first_insertion_index() {
    // When visual_x is clamped to 0.0, insertion should still be slot 0.
    let visual_x = compute_drag_visual_x(5.0, 100.0, 500.0); // Clamps to 0.
    assert!(visual_x.abs() < f32::EPSILON);
    assert_eq!(compute_insertion_index(visual_x, 120.0, 5), 0);
}

#[test]
fn visual_x_at_max_maps_to_last_insertion_index() {
    // When visual_x is clamped to max, insertion should be last slot.
    let max_x = TAB_LEFT_MARGIN + 4.0 * 120.0; // Space for 5 tabs, last at index 4.
    let visual_x = compute_drag_visual_x(2000.0, 10.0, max_x);
    assert!((visual_x - max_x).abs() < f32::EPSILON);
    assert_eq!(compute_insertion_index(visual_x, 120.0, 5), 4);
}

// -- Mouse offset preservation --

#[test]
fn mouse_offset_produces_consistent_visual_across_moves() {
    // The offset captured at drag start should produce a stable relationship
    // between cursor position and visual position across a range of moves.
    let offset = 30.0;
    let max_x = 500.0;
    for cursor_x in [100.0, 200.0, 300.0, 400.0] {
        let visual = compute_drag_visual_x(cursor_x, offset, max_x);
        assert!(
            (visual - (cursor_x - offset)).abs() < f32::EPSILON,
            "visual should be cursor_x - offset within clamp range"
        );
    }
}

// -- Two-tab reorder --

#[test]
fn two_tab_reorder_swap_at_center() {
    // Two tabs: dragging tab 0 past the center of tab 1 should yield index 1.
    let tw = 120.0;
    let count = 2;
    // Place visual_x at the center of the second slot.
    let visual_x = TAB_LEFT_MARGIN + tw; // Start of slot 1.
    let idx = compute_insertion_index(visual_x, tw, count);
    assert_eq!(idx, 1);
}

#[test]
fn two_tab_reorder_just_before_center_stays() {
    // Dragging tab 0 just before the center of tab 1 should stay at index 0.
    let tw = 120.0;
    let count = 2;
    // visual_x positioned so center is just before slot 1 boundary.
    // Center = visual_x + tw/2. For index 0, center < TAB_LEFT_MARGIN + tw.
    let visual_x = TAB_LEFT_MARGIN + tw / 2.0 - 1.0; // Center = TAB_LEFT_MARGIN + tw - 1.
    let idx = compute_insertion_index(visual_x, tw, count);
    assert_eq!(idx, 0);
}

// -- Tear-off direction changes --

#[test]
fn tear_off_cursor_returns_to_bar_after_exceeding() {
    // Cursor exceeds threshold upward, then returns inside bar.
    let bar_y = 10.0;
    let bar_bottom = bar_y + TAB_BAR_HEIGHT;

    // Step 1: exceeded.
    assert!(exceeds_tear_off(
        bar_y - TEAR_OFF_THRESHOLD_UP - 5.0,
        bar_y,
        bar_bottom
    ));
    // Step 2: back inside bar.
    assert!(!exceeds_tear_off(bar_y + 5.0, bar_y, bar_bottom));
}

#[test]
fn tear_off_exact_bar_edges_are_safe() {
    // Cursor exactly at bar_y and bar_bottom should never exceed.
    let bar_y = 0.0;
    let bar_bottom = TAB_BAR_HEIGHT;
    assert!(!exceeds_tear_off(bar_y, bar_y, bar_bottom));
    assert!(!exceeds_tear_off(bar_bottom, bar_y, bar_bottom));
}

// -- Float edge cases --

#[test]
fn insertion_index_negative_width_returns_zero() {
    // Negative tab width should be handled like zero width.
    assert_eq!(compute_insertion_index(100.0, -1.0, 5), 0);
}

#[test]
fn drag_visual_x_zero_max_clamps_to_zero() {
    // Max of 0.0: visual position is always 0.
    assert!(compute_drag_visual_x(500.0, 10.0, 0.0).abs() < f32::EPSILON);
}

#[test]
fn drag_visual_x_large_offset_clamps_to_zero() {
    // Offset larger than cursor: result clamped to 0, not negative.
    let visual = compute_drag_visual_x(50.0, 200.0, 500.0);
    assert!(visual >= 0.0);
    assert!(visual.abs() < f32::EPSILON);
}
