//! Tests for compositor-driven tab slide animations.

use std::time::{Duration, Instant};

use crate::compositor::layer_animator::LayerAnimator;
use crate::compositor::layer_tree::LayerTree;
use crate::geometry::Rect;

use super::{SlideContext, TabBarWidget, TabSlideState};

/// Creates a tree + animator for testing.
fn make_test_env() -> (LayerTree, LayerAnimator) {
    let tree = LayerTree::new(Rect::new(0.0, 0.0, 1200.0, 46.0));
    let animator = LayerAnimator::new();
    (tree, animator)
}

#[test]
fn new_state_has_no_active() {
    let state = TabSlideState::new();
    assert!(!state.has_active());
}

#[test]
fn close_creates_layers_for_displaced_tabs() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    // Close tab 1 out of 4 remaining tabs → tabs 1,2,3 slide.
    state.start_close_slide(1, 200.0, 4, &mut cx);

    assert!(state.has_active());
    // Should have 3 active animations (indices 1, 2, 3).
    assert_eq!(state.active.len(), 3);
}

#[test]
fn close_last_index_creates_no_layers() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    // Close at index == tab_count → range is empty.
    state.start_close_slide(4, 200.0, 4, &mut cx);

    assert!(!state.has_active());
}

#[test]
fn reorder_creates_layers() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    // Move tab 0 → tab 2: indices 0, 1 get +tab_width.
    state.start_reorder_slide(0, 2, 200.0, &mut cx);

    assert!(state.has_active());
    assert_eq!(state.active.len(), 2);
}

#[test]
fn reorder_same_index_is_noop() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    state.start_reorder_slide(2, 2, 200.0, &mut cx);

    assert!(!state.has_active());
}

#[test]
fn reorder_direction_from_greater_than_to() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    // Move tab 3 → tab 1: indices 2, 3 get -tab_width.
    state.start_reorder_slide(3, 1, 200.0, &mut cx);

    assert!(state.has_active());
    assert_eq!(state.active.len(), 2);

    // Verify layers have negative initial translation.
    for &layer_id in state.active.values() {
        let layer = tree.get(layer_id).unwrap();
        assert!(
            layer.properties().transform.translation_x() < 0.0,
            "from > to should create negative offset"
        );
    }
}

#[test]
fn cleanup_removes_finished_layers() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    state.start_close_slide(0, 200.0, 2, &mut cx);
    assert!(state.has_active());

    // Tick past the animation duration.
    let after = now + Duration::from_millis(200);
    animator.tick(&mut tree, after);

    state.cleanup(&mut tree, &animator);
    assert!(!state.has_active());
}

#[test]
fn sync_populates_offsets() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut widget = TabBarWidget::new(1200.0);
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    state.start_close_slide(1, 200.0, 3, &mut cx);

    // Before any tick, transforms are at their initial values.
    state.sync_to_widget(3, &tree, &mut widget);

    // Tab 0 should have no offset; tabs 1 and 2 should have 200px.
    let mut readback = Vec::new();
    widget.swap_anim_offsets(&mut readback);
    assert_eq!(readback.len(), 3);
    assert!((readback[0] - 0.0).abs() < f32::EPSILON, "tab 0 untouched");
    assert!(
        (readback[1] - 200.0).abs() < f32::EPSILON,
        "tab 1 at initial offset: got {}",
        readback[1]
    );
    assert!(
        (readback[2] - 200.0).abs() < f32::EPSILON,
        "tab 2 at initial offset: got {}",
        readback[2]
    );
}

#[test]
fn sync_idle_is_noop() {
    let mut state = TabSlideState::new();
    let tree = LayerTree::new(Rect::new(0.0, 0.0, 1200.0, 46.0));
    let mut widget = TabBarWidget::new(1200.0);

    state.sync_to_widget(3, &tree, &mut widget);

    // All offsets should be zero.
    let mut readback = Vec::new();
    widget.swap_anim_offsets(&mut readback);
    assert_eq!(readback.len(), 3);
    assert!(readback.iter().all(|&v| v == 0.0));
}

#[test]
fn cancel_removes_all() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    state.start_close_slide(0, 200.0, 3, &mut cx);
    assert!(state.has_active());

    state.cancel_all(&mut tree, &mut animator);
    assert!(!state.has_active());
    assert!(!animator.is_any_animating());
}

#[test]
fn rapid_close_cancels_previous() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    // First close.
    state.start_close_slide(0, 200.0, 4, &mut cx);
    let first_count = state.active.len();
    assert_eq!(first_count, 4);

    // Rapid second close (before first finishes) — should cancel previous.
    let slightly_later = now + Duration::from_millis(10);
    let mut cx2 = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now: slightly_later,
    };
    state.start_close_slide(1, 200.0, 3, &mut cx2);

    // Previous animations were cancelled; new set started.
    assert_eq!(state.active.len(), 2);
}

#[test]
fn close_slide_mid_animation_offset_decreasing() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    state.start_close_slide(0, 200.0, 2, &mut cx);

    // Tick to ~50% through the animation.
    let mid = now + Duration::from_millis(75);
    animator.tick(&mut tree, mid);

    // Both layers should have translation_x between 0 and 200.
    for &layer_id in state.active.values() {
        let tx = tree
            .get(layer_id)
            .unwrap()
            .properties()
            .transform
            .translation_x();
        assert!(
            tx > 0.0 && tx < 200.0,
            "mid-animation offset should be between 0 and 200, got {tx}"
        );
    }
}

// --- Gap analysis: high priority ---

#[test]
fn zero_offset_slide_creates_identity_layers() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    // Close with tab_width=0.0 — offset is zero, so transforms start at identity.
    state.start_close_slide(0, 0.0, 3, &mut cx);

    // Layers should be created (indices 0, 1, 2) even with zero offset.
    assert_eq!(state.active.len(), 3);

    // Each layer's initial transform should be translate(0, 0) == identity.
    for &layer_id in state.active.values() {
        let tx = tree
            .get(layer_id)
            .unwrap()
            .properties()
            .transform
            .translation_x();
        assert!(
            tx.abs() < f32::EPSILON,
            "zero-offset slide should have zero translation, got {tx}"
        );
    }
}

#[test]
fn reorder_across_full_range() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    // Move first tab to last position: from=0, to=4 (5 tabs total).
    // Displaced range: 0..4, all get +tab_width.
    state.start_reorder_slide(0, 4, 200.0, &mut cx);

    assert!(state.has_active());
    assert_eq!(
        state.active.len(),
        4,
        "should displace 4 tabs (indices 0..4)"
    );

    // All displaced tabs should have positive initial offset.
    for &layer_id in state.active.values() {
        let tx = tree
            .get(layer_id)
            .unwrap()
            .properties()
            .transform
            .translation_x();
        assert!(
            tx > 0.0,
            "from < to: displaced tabs should have positive offset, got {tx}"
        );
    }
}

#[test]
fn animation_completes_to_identity() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    state.start_close_slide(0, 200.0, 3, &mut cx);

    // Tick well past the animation duration (150ms default + margin).
    let after = now + Duration::from_millis(300);
    animator.tick(&mut tree, after);

    // All layers should have converged to identity transform.
    for &layer_id in state.active.values() {
        let props = tree.get(layer_id).unwrap().properties();
        assert!(
            props.transform.is_identity(),
            "completed animation should yield identity, got {:?}",
            props.transform
        );
    }
}

#[test]
fn close_first_tab_shifts_all_remaining() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    // Close tab 0 out of 5 remaining → indices 0..5 should all slide.
    state.start_close_slide(0, 200.0, 5, &mut cx);

    assert_eq!(state.active.len(), 5, "all 5 tabs should animate");

    // Every layer should have a positive initial X offset.
    for (&idx, &layer_id) in &state.active {
        let tx = tree
            .get(layer_id)
            .unwrap()
            .properties()
            .transform
            .translation_x();
        assert!(
            tx > 0.0,
            "tab {idx} should have positive offset after closing tab 0, got {tx}"
        );
    }
}

// --- Gap analysis: medium priority ---

#[test]
fn cleanup_mid_animation_retains_active() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    // Start a close slide with 2 displaced tabs.
    state.start_close_slide(0, 200.0, 2, &mut cx);
    assert_eq!(state.active.len(), 2);

    // Tick to mid-animation (< 150ms duration).
    let mid = now + Duration::from_millis(50);
    animator.tick(&mut tree, mid);

    // Cleanup mid-animation: nothing should be removed since both are still animating.
    state.cleanup(&mut tree, &animator);
    assert_eq!(
        state.active.len(),
        2,
        "mid-animation cleanup should retain all active layers"
    );

    // Now tick past completion.
    let after = now + Duration::from_millis(300);
    animator.tick(&mut tree, after);

    // Cleanup after completion: both should be removed.
    state.cleanup(&mut tree, &animator);
    assert!(
        !state.has_active(),
        "post-completion cleanup should remove all"
    );
}

#[test]
fn sync_with_smaller_tab_count_skips_out_of_range() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut widget = TabBarWidget::new(1200.0);
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    // Start with 4 tabs, close creates layers for indices 0..4.
    state.start_close_slide(0, 200.0, 4, &mut cx);
    assert_eq!(state.active.len(), 4);

    // Sync with only 2 tabs (simulates rapid close reducing count).
    state.sync_to_widget(2, &tree, &mut widget);

    let mut readback = Vec::new();
    widget.swap_anim_offsets(&mut readback);
    assert_eq!(readback.len(), 2, "should produce exactly 2 offsets");
    // Both should be valid (no panic from out-of-range).
    assert!(readback[0].is_finite());
    assert!(readback[1].is_finite());
}

#[test]
fn double_cancel_is_safe() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    state.start_close_slide(0, 200.0, 3, &mut cx);
    assert!(state.has_active());

    // First cancel.
    state.cancel_all(&mut tree, &mut animator);
    assert!(!state.has_active());

    // Second cancel — should be a no-op, no panic.
    state.cancel_all(&mut tree, &mut animator);
    assert!(!state.has_active());
    assert!(!animator.is_any_animating());
}

#[test]
fn reorder_adjacent_tabs_creates_single_layer() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    // Move tab 2 → tab 3 (adjacent): displaced range is 2..3, exactly 1 tab.
    state.start_reorder_slide(2, 3, 200.0, &mut cx);

    assert!(state.has_active());
    assert_eq!(
        state.active.len(),
        1,
        "adjacent reorder should displace exactly 1 tab"
    );
}

// --- Gap analysis: low priority ---

#[test]
fn large_tab_count_slide() {
    let mut state = TabSlideState::new();
    let (mut tree, mut animator) = make_test_env();
    let now = Instant::now();
    let mut cx = SlideContext {
        tree: &mut tree,
        animator: &mut animator,
        now,
    };

    // Close tab 0 with 50 remaining tabs.
    state.start_close_slide(0, 100.0, 50, &mut cx);
    assert_eq!(state.active.len(), 50);

    // Tick to completion.
    let after = now + Duration::from_millis(300);
    animator.tick(&mut tree, after);

    // Cleanup should remove all 50.
    state.cleanup(&mut tree, &animator);
    assert!(!state.has_active());
}
