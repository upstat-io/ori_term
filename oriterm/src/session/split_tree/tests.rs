use std::sync::Arc;

use super::{SplitDirection, SplitTree};
use oriterm_mux::PaneId;

fn p(n: u64) -> PaneId {
    PaneId::from_raw(n)
}

// Single pane

#[test]
fn single_pane_count() {
    let tree = SplitTree::leaf(p(1));
    assert_eq!(tree.pane_count(), 1);
}

#[test]
fn single_pane_contains() {
    let tree = SplitTree::leaf(p(1));
    assert!(tree.contains(p(1)));
    assert!(!tree.contains(p(2)));
}

#[test]
fn single_pane_depth() {
    let tree = SplitTree::leaf(p(1));
    assert_eq!(tree.depth(), 0);
}

#[test]
fn single_pane_panes_list() {
    let tree = SplitTree::leaf(p(1));
    assert_eq!(tree.panes(), vec![p(1)]);
}

#[test]
fn single_pane_no_parent_split() {
    let tree = SplitTree::leaf(p(1));
    assert_eq!(tree.parent_split(p(1)), None);
}

#[test]
fn single_pane_no_sibling() {
    let tree = SplitTree::leaf(p(1));
    assert_eq!(tree.sibling(p(1)), None);
}

// Split at leaf

#[test]
fn split_at_leaf_produces_split_node() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    assert_eq!(tree.pane_count(), 2);
    assert!(tree.contains(p(1)));
    assert!(tree.contains(p(2)));
    assert_eq!(tree.depth(), 1);
}

#[test]
fn split_at_preserves_order() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    // First child (left) is the original, second (right) is the new pane.
    assert_eq!(tree.panes(), vec![p(1), p(2)]);
}

#[test]
fn split_at_stores_direction_and_ratio() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Horizontal, p(2), 0.7);

    assert_eq!(
        tree.parent_split(p(1)),
        Some((SplitDirection::Horizontal, 0.7))
    );
    assert_eq!(
        tree.parent_split(p(2)),
        Some((SplitDirection::Horizontal, 0.7))
    );
}

#[test]
fn split_at_nonexistent_pane_returns_unchanged() {
    let tree = SplitTree::leaf(p(1));
    let result = tree.split_at(p(99), SplitDirection::Vertical, p(2), 0.5);
    assert_eq!(result, tree);
}

// Nested splits

#[test]
fn nested_split_three_panes() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);

    assert_eq!(tree.pane_count(), 3);
    assert_eq!(tree.depth(), 2);
    assert_eq!(tree.panes(), vec![p(1), p(2), p(3)]);
}

#[test]
fn nested_split_four_panes_grid() {
    // Create a 2x2 grid: split vertically, then split each half horizontally.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(1), SplitDirection::Horizontal, p(3), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(4), 0.5);

    assert_eq!(tree.pane_count(), 4);
    assert_eq!(tree.panes(), vec![p(1), p(3), p(2), p(4)]);
}

// Remove

#[test]
fn remove_last_pane_returns_none() {
    let tree = SplitTree::leaf(p(1));
    assert_eq!(tree.remove(p(1)), None);
}

#[test]
fn remove_from_two_pane_split_collapses_to_sibling() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let after = tree.remove(p(1)).expect("should not be None");
    assert_eq!(after, SplitTree::leaf(p(2)));

    let after = tree.remove(p(2)).expect("should not be None");
    assert_eq!(after, SplitTree::leaf(p(1)));
}

#[test]
fn remove_middle_pane_preserves_remaining() {
    // p1 | (p2 / p3)  ->  remove p2  ->  p1 | p3
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);

    let after = tree.remove(p(2)).expect("should not be None");
    assert_eq!(after.pane_count(), 2);
    assert!(after.contains(p(1)));
    assert!(after.contains(p(3)));
    assert!(!after.contains(p(2)));
}

#[test]
fn remove_nonexistent_pane_returns_unchanged() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let after = tree.remove(p(99)).expect("should not be None");
    assert_eq!(after, tree);
}

// Equalize

#[test]
fn equalize_sets_all_ratios_to_half() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.3);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.8);

    let equalized = tree.equalize();

    // Both splits should now have ratio 0.5.
    assert_eq!(
        equalized.parent_split(p(1)),
        Some((SplitDirection::Vertical, 0.5))
    );
    assert_eq!(
        equalized.parent_split(p(2)),
        Some((SplitDirection::Horizontal, 0.5))
    );
}

#[test]
fn equalize_single_pane_is_noop() {
    let tree = SplitTree::leaf(p(1));
    let equalized = tree.equalize();
    assert_eq!(equalized, tree);
}

// Ratio clamping

#[test]
fn ratio_clamped_below_minimum() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.01);

    if let SplitTree::Split { ratio, .. } = &tree {
        assert!(
            (*ratio - 0.1).abs() < f32::EPSILON,
            "ratio should be clamped to 0.1, got {ratio}"
        );
    } else {
        panic!("expected Split");
    }
}

#[test]
fn ratio_clamped_above_maximum() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.99);

    if let SplitTree::Split { ratio, .. } = &tree {
        assert!(
            (*ratio - 0.9).abs() < f32::EPSILON,
            "ratio should be clamped to 0.9, got {ratio}"
        );
    } else {
        panic!("expected Split");
    }
}

#[test]
fn set_ratio_clamps() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let updated = tree.set_ratio(p(1), SplitDirection::Vertical, 0.0);
    if let SplitTree::Split { ratio, .. } = &updated {
        assert!((*ratio - 0.1).abs() < f32::EPSILON);
    } else {
        panic!("expected Split");
    }
}

// Swap

#[test]
fn swap_exchanges_two_panes() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let swapped = tree.swap(p(1), p(2));
    assert_eq!(swapped.panes(), vec![p(2), p(1)]);
}

#[test]
fn swap_in_nested_tree() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);

    let swapped = tree.swap(p(1), p(3));
    assert_eq!(swapped.panes(), vec![p(3), p(2), p(1)]);
}

#[test]
fn swap_same_pane_is_noop() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let swapped = tree.swap(p(1), p(1));
    assert_eq!(swapped, tree);
}

#[test]
fn swap_nonexistent_pane_returns_unchanged() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let swapped = tree.swap(p(1), p(99));
    assert_eq!(swapped, tree);
}

// Depth-first order

#[test]
fn panes_returns_depth_first_first_child_first() {
    // Build: (p1 | (p2 / p3))
    // Depth-first: p1, p2, p3
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);
    assert_eq!(tree.panes(), vec![p(1), p(2), p(3)]);
}

// Structural sharing

#[test]
fn split_at_shares_unchanged_subtrees() {
    // Start with p1 | p2, then split p2 -> p2 | p3.
    // The "first" subtree (containing p1) should be Arc-shared.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let first_arc = match &tree {
        SplitTree::Split { first, .. } => Arc::clone(first),
        _ => panic!("expected Split"),
    };

    let tree2 = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);

    let first_arc2 = match &tree2 {
        SplitTree::Split { first, .. } => Arc::clone(first),
        _ => panic!("expected Split"),
    };

    // The first subtree should point to the same allocation.
    assert!(Arc::ptr_eq(&first_arc, &first_arc2));
}

// Sibling

#[test]
fn sibling_of_leaf_in_split() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    assert_eq!(tree.sibling(p(1)), Some(p(2)));
    assert_eq!(tree.sibling(p(2)), Some(p(1)));
}

#[test]
fn sibling_returns_none_when_sibling_is_split() {
    // p1 | (p2 / p3) — sibling of p1 is a Split, not a Leaf.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);

    assert_eq!(tree.sibling(p(1)), None);
}

#[test]
fn sibling_of_nonexistent_pane() {
    let tree = SplitTree::leaf(p(1));
    assert_eq!(tree.sibling(p(99)), None);
}

// set_ratio

#[test]
fn set_ratio_updates_matching_direction() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let updated = tree.set_ratio(p(1), SplitDirection::Vertical, 0.7);
    assert_eq!(
        updated.parent_split(p(1)),
        Some((SplitDirection::Vertical, 0.7))
    );
}

#[test]
fn set_ratio_ignores_wrong_direction() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    // Try to set ratio on Horizontal when only Vertical exists.
    let updated = tree.set_ratio(p(1), SplitDirection::Horizontal, 0.7);

    // Should be unchanged — no Horizontal split containing p(1).
    assert_eq!(
        updated.parent_split(p(1)),
        Some((SplitDirection::Vertical, 0.5))
    );
}

// SplitDirection Display

#[test]
fn split_direction_display() {
    assert_eq!(format!("{}", SplitDirection::Horizontal), "Horizontal");
    assert_eq!(format!("{}", SplitDirection::Vertical), "Vertical");
}

// Deep nesting (5+ levels)

/// Build a 6-level deep chain by repeatedly splitting the second child.
fn deep_chain() -> SplitTree {
    let mut tree = SplitTree::leaf(p(1));
    for i in 2..=7 {
        tree = tree.split_at(p(i - 1), SplitDirection::Vertical, p(i), 0.5);
    }
    tree
}

#[test]
fn deep_tree_pane_count() {
    let tree = deep_chain();
    assert_eq!(tree.pane_count(), 7);
}

#[test]
fn deep_tree_depth() {
    let tree = deep_chain();
    assert_eq!(tree.depth(), 6);
}

#[test]
fn deep_tree_contains_all_panes() {
    let tree = deep_chain();
    for i in 1..=7 {
        assert!(tree.contains(p(i)), "missing pane {i}");
    }
}

#[test]
fn deep_tree_remove_middle_preserves_others() {
    let tree = deep_chain();
    let tree = tree.remove(p(4)).expect("should not be None");
    assert_eq!(tree.pane_count(), 6);
    assert!(!tree.contains(p(4)));
    for i in [1, 2, 3, 5, 6, 7] {
        assert!(tree.contains(p(i)), "missing pane {i}");
    }
}

#[test]
fn deep_tree_swap_leaf_and_deep_leaf() {
    let tree = deep_chain();
    let swapped = tree.swap(p(1), p(7));
    let panes = swapped.panes();
    // First and last should be swapped.
    assert_eq!(panes[0], p(7));
    assert_eq!(*panes.last().unwrap(), p(1));
}

#[test]
fn deep_tree_equalize() {
    let tree = deep_chain();
    let equalized = tree.equalize();
    // All panes still present.
    assert_eq!(equalized.pane_count(), 7);
    // Check a mid-level pane has ratio 0.5.
    if let Some((_, ratio)) = equalized.parent_split(p(4)) {
        assert!((ratio - 0.5).abs() < f32::EPSILON);
    }
}

// Duplicate pane IDs

#[test]
fn split_at_with_same_pane_id_creates_two_identical_leaves() {
    // Unusual but defined: splitting pane 1 and placing "pane 1" as second.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(1), 0.5);

    // Both leaves have pane 1 — pane_count counts leaves, not unique IDs.
    assert_eq!(tree.pane_count(), 2);
    assert!(tree.contains(p(1)));
}

#[test]
fn split_at_with_existing_pane_id_from_another_leaf() {
    // Split p1 to get p1|p2, then split p2 with new_pane = p1.
    // This creates a tree with p1 appearing twice.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(1), 0.5);

    // Should have 3 leaves (p1, p2, p1).
    assert_eq!(tree.pane_count(), 3);
    let panes = tree.panes();
    assert_eq!(panes, vec![p(1), p(2), p(1)]);
}

// set_divider_ratio

#[test]
fn set_divider_ratio_simple_split() {
    // p1 | p2 — set the divider between them.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let updated = tree.set_divider_ratio(p(1), p(2), 0.7);
    if let SplitTree::Split { ratio, .. } = &updated {
        assert!((*ratio - 0.7).abs() < f32::EPSILON);
    } else {
        panic!("expected Split");
    }
}

#[test]
fn set_divider_ratio_nested_inner() {
    // Root: (A | B) | C — set inner divider (A-B).
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(3), 0.5);
    // Tree is: Split(V, Split(V, p1, p3), p2)

    let updated = tree.set_divider_ratio(p(1), p(3), 0.3);

    // Inner split (between p1 and p3) should have new ratio.
    assert_eq!(
        updated.parent_split(p(1)),
        Some((SplitDirection::Vertical, 0.3))
    );
    // Outer split should be unchanged.
    if let SplitTree::Split { ratio, .. } = &updated {
        assert!((*ratio - 0.5).abs() < f32::EPSILON);
    }
}

#[test]
fn set_divider_ratio_nested_outer() {
    // Root: Split(V, Split(V, A, B), C)
    // The outer divider is between B (rightmost of first) and C.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(3), 0.5);
    // Tree: Split(V, Split(V, p1, p3), p2)
    // Outer divider: pane_before = p3 (rightmost of first), pane_after = p2.

    let updated = tree.set_divider_ratio(p(3), p(2), 0.8);
    if let SplitTree::Split { ratio, .. } = &updated {
        assert!((*ratio - 0.8).abs() < f32::EPSILON);
    }
}

#[test]
fn set_divider_ratio_clamps() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let updated = tree.set_divider_ratio(p(1), p(2), 0.0);
    if let SplitTree::Split { ratio, .. } = &updated {
        assert!((*ratio - 0.1).abs() < f32::EPSILON);
    } else {
        panic!("expected Split");
    }
}

#[test]
fn set_divider_ratio_nonexistent_panes() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let updated = tree.set_divider_ratio(p(99), p(100), 0.7);
    assert_eq!(updated, tree);
}

#[test]
fn set_divider_ratio_on_leaf() {
    let tree = SplitTree::leaf(p(1));
    let updated = tree.set_divider_ratio(p(1), p(2), 0.7);
    assert_eq!(updated, tree);
}

// Split ratio boundary values

#[test]
fn split_at_with_ratio_zero_clamps_to_minimum() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.0);

    // Ratio=0.0 should be clamped to the minimum (0.1), not accepted literally.
    // A zero ratio would make the first pane invisible — a degenerate layout.
    if let SplitTree::Split { ratio, .. } = &tree {
        assert!(
            (*ratio - 0.1).abs() < f32::EPSILON,
            "ratio=0.0 should clamp to 0.1, got {ratio}"
        );
    } else {
        panic!("expected Split");
    }

    // Both panes should still exist in the tree.
    assert_eq!(tree.pane_count(), 2);
    assert!(tree.contains(p(1)));
    assert!(tree.contains(p(2)));
}

#[test]
fn split_at_with_ratio_one_clamps_to_maximum() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Horizontal, p(2), 1.0);

    // Ratio=1.0 should be clamped to the maximum (0.9), not accepted literally.
    // A ratio of 1.0 would make the second pane invisible.
    if let SplitTree::Split { ratio, .. } = &tree {
        assert!(
            (*ratio - 0.9).abs() < f32::EPSILON,
            "ratio=1.0 should clamp to 0.9, got {ratio}"
        );
    } else {
        panic!("expected Split");
    }

    assert_eq!(tree.pane_count(), 2);
}

// Exhaustive node removal fuzz

#[test]
fn remove_every_leaf_no_panic() {
    // Build a 4-pane tree: (p1 | (p2 / (p3 | p4))).
    // Removing any leaf should produce a valid tree or None (last pane).
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);
    let tree = tree.split_at(p(3), SplitDirection::Vertical, p(4), 0.5);
    assert_eq!(tree.pane_count(), 4);

    // Remove each leaf from a fresh copy and verify no panic.
    for id in 1..=4 {
        let result = tree.remove(p(id));
        let after = result.expect("removing one of 4 panes should not return None");
        assert_eq!(after.pane_count(), 3);
        assert!(!after.contains(p(id)));
        // All other panes still present.
        for other in 1..=4 {
            if other != id {
                assert!(
                    after.contains(p(other)),
                    "pane {other} missing after removing {id}"
                );
            }
        }
    }
}

#[test]
fn remove_every_leaf_deep_chain_no_panic() {
    // Deep chain: p1 → p2 → p3 → p4 → p5 → p6 → p7.
    let tree = deep_chain();
    assert_eq!(tree.pane_count(), 7);

    for id in 1..=7 {
        let result = tree.remove(p(id));
        let after = result.expect("removing one of 7 panes should not return None");
        assert_eq!(after.pane_count(), 6, "wrong count after removing p{id}");
        assert!(!after.contains(p(id)));
    }
}

// resize_toward

#[test]
fn resize_toward_right_pane_in_first() {
    // p1 | p2 — resize p1 rightward (grow first).
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let resized = tree.resize_toward(p(1), SplitDirection::Vertical, true, 0.05);
    if let SplitTree::Split { ratio, .. } = &resized {
        assert!((*ratio - 0.55).abs() < f32::EPSILON);
    } else {
        panic!("expected Split");
    }
}

#[test]
fn resize_toward_left_pane_in_second() {
    // p1 | p2 — resize p2 leftward (shrink first, grow second).
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let resized = tree.resize_toward(p(2), SplitDirection::Vertical, false, -0.05);
    if let SplitTree::Split { ratio, .. } = &resized {
        assert!((*ratio - 0.45).abs() < f32::EPSILON);
    } else {
        panic!("expected Split");
    }
}

#[test]
fn resize_toward_no_matching_split() {
    // p1 | p2 (Vertical) — try resizing with Horizontal axis.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let resized = tree.resize_toward(p(1), SplitDirection::Horizontal, true, 0.05);
    assert_eq!(resized, tree);
}

#[test]
fn resize_toward_wrong_side_noop() {
    // p1 | p2 — p2 is in second, but pane_in_first=true → no match.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let resized = tree.resize_toward(p(2), SplitDirection::Vertical, true, 0.05);
    assert_eq!(resized, tree);
}

#[test]
fn resize_toward_nested_finds_deepest() {
    // Root: Split(V, Split(V, A, B), C)
    // Resize A rightward — should adjust the INNER split (nearest to A).
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(3), 0.5);
    // Tree: Split(V, 0.5, Split(V, 0.5, p1, p3), p2)

    let resized = tree.resize_toward(p(1), SplitDirection::Vertical, true, 0.1);

    // Inner split should be adjusted.
    assert_eq!(
        resized.parent_split(p(1)),
        Some((SplitDirection::Vertical, 0.6))
    );
    // Outer split unchanged.
    if let SplitTree::Split { ratio, .. } = &resized {
        assert!((*ratio - 0.5).abs() < f32::EPSILON);
    }
}

#[test]
fn resize_toward_nested_outer_when_inner_wrong_side() {
    // Root: Split(V, Split(V, A, B), C)
    // Resize B rightward — B is in SECOND of inner split, so inner doesn't
    // qualify (pane_in_first=true). Outer qualifies: B is in first subtree.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(3), 0.5);
    // Tree: Split(V, 0.5, Split(V, 0.5, p1, p3), p2)

    let resized = tree.resize_toward(p(3), SplitDirection::Vertical, true, 0.1);

    // Inner split unchanged (p3 is second child).
    assert_eq!(
        resized.parent_split(p(1)),
        Some((SplitDirection::Vertical, 0.5))
    );
    // Outer split adjusted (p3 is in first subtree of outer).
    if let SplitTree::Split { ratio, .. } = &resized {
        assert!((*ratio - 0.6).abs() < f32::EPSILON);
    }
}

#[test]
fn resize_toward_clamps_at_bounds() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.15);

    let resized = tree.resize_toward(p(2), SplitDirection::Vertical, false, -0.1);
    if let SplitTree::Split { ratio, .. } = &resized {
        assert!((*ratio - 0.1).abs() < f32::EPSILON, "got {ratio}");
    } else {
        panic!("expected Split");
    }
}

#[test]
fn resize_toward_on_leaf_noop() {
    let tree = SplitTree::leaf(p(1));
    let resized = tree.resize_toward(p(1), SplitDirection::Vertical, true, 0.1);
    assert_eq!(resized, tree);
}

#[test]
fn resize_toward_mixed_directions() {
    // Root: Split(V, Split(H, A, B), C)
    // Resize A downward (Horizontal, pane_in_first=true).
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(1), SplitDirection::Horizontal, p(3), 0.5);
    // Tree: Split(V, 0.5, Split(H, 0.5, p1, p3), p2)

    let resized = tree.resize_toward(p(1), SplitDirection::Horizontal, true, 0.1);

    // Inner horizontal split adjusted.
    assert_eq!(
        resized.parent_split(p(1)),
        Some((SplitDirection::Horizontal, 0.6))
    );
    // Outer vertical split unchanged.
    if let SplitTree::Split { ratio, .. } = &resized {
        assert!((*ratio - 0.5).abs() < f32::EPSILON);
    }
}
