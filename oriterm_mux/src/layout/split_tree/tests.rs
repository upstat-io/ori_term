use std::sync::Arc;

use super::{SplitDirection, SplitTree};
use crate::id::PaneId;

fn p(n: u64) -> PaneId {
    PaneId::from_raw(n)
}

// ── Single pane ───────────────────────────────────────────────────

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

// ── Split at leaf ─────────────────────────────────────────────────

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

// ── Nested splits ─────────────────────────────────────────────────

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

// ── Remove ────────────────────────────────────────────────────────

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

// ── Equalize ──────────────────────────────────────────────────────

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

// ── Ratio clamping ────────────────────────────────────────────────

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

// ── Swap ──────────────────────────────────────────────────────────

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

// ── Depth-first order ─────────────────────────────────────────────

#[test]
fn panes_returns_depth_first_first_child_first() {
    // Build: (p1 | (p2 / p3))
    // Depth-first: p1, p2, p3
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);
    assert_eq!(tree.panes(), vec![p(1), p(2), p(3)]);
}

// ── Structural sharing ───────────────────────────────────────────

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

// ── Sibling ───────────────────────────────────────────────────────

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

// ── set_ratio ─────────────────────────────────────────────────────

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

// ── SplitDirection Display ────────────────────────────────────────

#[test]
fn split_direction_display() {
    assert_eq!(format!("{}", SplitDirection::Horizontal), "Horizontal");
    assert_eq!(format!("{}", SplitDirection::Vertical), "Vertical");
}
