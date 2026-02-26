use super::{LayoutDescriptor, compute_dividers, compute_layout};
use crate::id::PaneId;
use crate::layout::floating::{FloatingLayer, FloatingPane};
use crate::layout::rect::Rect;
use crate::layout::split_tree::{SplitDirection, SplitTree};

fn p(n: u64) -> PaneId {
    PaneId::from_raw(n)
}

/// Standard descriptor: 1000x800 area, 10x20 cell, 2px divider, min 4x2 cells.
fn standard_desc() -> LayoutDescriptor {
    LayoutDescriptor {
        available: Rect {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 800.0,
        },
        cell_width: 10.0,
        cell_height: 20.0,
        divider_px: 2.0,
        min_pane_cells: (4, 2),
    }
}

// ── Single pane ───────────────────────────────────────────────────

#[test]
fn single_pane_fills_available_rect() {
    let tree = SplitTree::leaf(p(1));
    let floating = FloatingLayer::new();
    let desc = standard_desc();

    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts.len(), 1);

    let layout = &layouts[0];
    assert_eq!(layout.pane_id, p(1));
    assert!(layout.is_focused);
    assert!(!layout.is_floating);
    assert_eq!(layout.cols, 100); // 1000 / 10
    assert_eq!(layout.rows, 40); // 800 / 20
}

// ── Horizontal split ──────────────────────────────────────────────

#[test]
fn horizontal_split_50_50() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Horizontal, p(2), 0.5);
    let floating = FloatingLayer::new();
    let desc = standard_desc();

    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts.len(), 2);

    let l1 = &layouts[0];
    let l2 = &layouts[1];
    assert_eq!(l1.pane_id, p(1));
    assert_eq!(l2.pane_id, p(2));

    // Available height: 800 - 2(divider) = 798.
    // First: floor(798 * 0.5) = 399. Second: 798 - 399 = 399.
    assert!(l1.pixel_rect.height > 0.0);
    assert!(l2.pixel_rect.height > 0.0);

    // Panes should be stacked vertically: l2 starts below l1.
    assert!(l2.pixel_rect.y > l1.pixel_rect.y);
}

// ── Vertical split ────────────────────────────────────────────────

#[test]
fn vertical_split_70_30() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.7);
    let floating = FloatingLayer::new();
    let desc = standard_desc();

    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts.len(), 2);

    let l1 = &layouts[0];
    let l2 = &layouts[1];

    // First pane should be wider than second (70/30 split).
    assert!(l1.pixel_rect.width > l2.pixel_rect.width);

    // Panes should be side by side: l2 starts to the right of l1.
    assert!(l2.pixel_rect.x > l1.pixel_rect.x);
}

// ── Nested splits ─────────────────────────────────────────────────

#[test]
fn nested_split_l_shape() {
    // p1 | (p2 / p3) — vertical split, then horizontal on right side.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);
    let floating = FloatingLayer::new();
    let desc = standard_desc();

    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts.len(), 3);

    // All three panes should have positive dimensions.
    for layout in &layouts {
        assert!(layout.cols > 0);
        assert!(layout.rows > 0);
        assert!(layout.pixel_rect.width > 0.0);
        assert!(layout.pixel_rect.height > 0.0);
    }
}

// ── Cell grid snapping ────────────────────────────────────────────

#[test]
fn pixel_rects_align_to_cell_boundaries() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let floating = FloatingLayer::new();
    let desc = standard_desc();

    let layouts = compute_layout(&tree, &floating, p(1), &desc);

    for layout in &layouts {
        if !layout.is_floating {
            // Width should be an exact multiple of cell_width.
            let cells_w = layout.pixel_rect.width / desc.cell_width;
            assert!(
                (cells_w - cells_w.round()).abs() < 0.01,
                "width {} not aligned to cell grid (cell_width={})",
                layout.pixel_rect.width,
                desc.cell_width,
            );
            // Height should be an exact multiple of cell_height.
            let cells_h = layout.pixel_rect.height / desc.cell_height;
            assert!(
                (cells_h - cells_h.round()).abs() < 0.01,
                "height {} not aligned to cell grid (cell_height={})",
                layout.pixel_rect.height,
                desc.cell_height,
            );
        }
    }
}

// ── Dividers ──────────────────────────────────────────────────────

#[test]
fn dividers_for_single_split() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let desc = standard_desc();

    let dividers = compute_dividers(&tree, &desc);
    assert_eq!(dividers.len(), 1);

    let d = &dividers[0];
    assert_eq!(d.direction, SplitDirection::Vertical);
    assert_eq!(d.pane_before, p(1));
    assert_eq!(d.pane_after, p(2));
    // Vertical divider should be full height, divider_px wide.
    assert!((d.rect.height - 800.0).abs() < 0.01);
    assert!((d.rect.width - 2.0).abs() < 0.01);
}

#[test]
fn dividers_for_nested_splits() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);
    let desc = standard_desc();

    let dividers = compute_dividers(&tree, &desc);
    // One for the vertical split (p1 | (p2/p3)), one for the horizontal (p2 / p3).
    assert_eq!(dividers.len(), 2);
}

// ── Minimum pane size ─────────────────────────────────────────────

#[test]
fn minimum_pane_size_enforced() {
    let tree = SplitTree::leaf(p(1));
    // Extreme ratio: 0.99 would make the second pane very small.
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.9);
    let floating = FloatingLayer::new();
    let desc = standard_desc(); // min 4 cols × 2 rows

    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts.len(), 2);

    let l2 = &layouts[1]; // The smaller pane.
    // Should be at least min_pane_cells wide.
    assert!(
        l2.cols >= desc.min_pane_cells.0,
        "second pane cols {} < min {}",
        l2.cols,
        desc.min_pane_cells.0,
    );
}

// ── Floating panes ────────────────────────────────────────────────

#[test]
fn floating_panes_appended_to_layout() {
    let tree = SplitTree::leaf(p(1));
    let floating = FloatingLayer::new();
    let floating = floating.add(FloatingPane {
        pane_id: p(2),
        rect: Rect {
            x: 100.0,
            y: 100.0,
            width: 400.0,
            height: 300.0,
        },
        z_order: 0,
    });
    let desc = standard_desc();

    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts.len(), 2);

    let tiled = &layouts[0];
    let floating = &layouts[1];

    assert!(!tiled.is_floating);
    assert!(floating.is_floating);
    assert_eq!(floating.pane_id, p(2));
    assert_eq!(floating.cols, 40); // 400 / 10
    assert_eq!(floating.rows, 15); // 300 / 20
}

#[test]
fn focused_pane_marked_correctly() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let floating = FloatingLayer::new();
    let desc = standard_desc();

    let layouts = compute_layout(&tree, &floating, p(2), &desc);
    let l1 = layouts.iter().find(|l| l.pane_id == p(1)).unwrap();
    let l2 = layouts.iter().find(|l| l.pane_id == p(2)).unwrap();

    assert!(!l1.is_focused);
    assert!(l2.is_focused);
}

// ── Determinism ───────────────────────────────────────────────────

#[test]
fn layout_is_deterministic() {
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);
    let floating = FloatingLayer::new();
    let desc = standard_desc();

    let layouts1 = compute_layout(&tree, &floating, p(1), &desc);
    let layouts2 = compute_layout(&tree, &floating, p(1), &desc);

    assert_eq!(layouts1, layouts2);
}

// ── Sequential split workflow ─────────────────────────────────────

#[test]
fn sequential_splits_produce_correct_positions() {
    let desc = standard_desc();
    let floating = FloatingLayer::new();

    // Step 1: Single pane fills everything.
    let tree = SplitTree::leaf(p(1));
    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts.len(), 1);
    assert_eq!(layouts[0].cols, 100);
    assert_eq!(layouts[0].rows, 40);

    // Step 2: Vertical split → two side-by-side panes.
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts.len(), 2);

    let l1 = &layouts[0];
    let l2 = &layouts[1];
    // Pane 1 starts at x=0, pane 2 starts after pane 1 + divider.
    assert!((l1.pixel_rect.x).abs() < f32::EPSILON);
    assert!(l2.pixel_rect.x > l1.pixel_rect.x);
    // Both should have full height.
    assert_eq!(l1.rows, 40);
    assert_eq!(l2.rows, 40);

    // Step 3: Split pane 2 horizontally → three panes (L-shape).
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);
    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts.len(), 3);

    // Pane 1 still left side, full height.
    let l1 = layouts.iter().find(|l| l.pane_id == p(1)).unwrap();
    assert_eq!(l1.rows, 40);
    assert!((l1.pixel_rect.x).abs() < f32::EPSILON);

    // Panes 2 and 3 share the right side, stacked vertically.
    let l2 = layouts.iter().find(|l| l.pane_id == p(2)).unwrap();
    let l3 = layouts.iter().find(|l| l.pane_id == p(3)).unwrap();
    assert!(l3.pixel_rect.y > l2.pixel_rect.y);
    assert!((l2.pixel_rect.x - l3.pixel_rect.x).abs() < f32::EPSILON);
    // Each should have roughly half the height (minus divider).
    assert!(l2.rows > 0);
    assert!(l3.rows > 0);
    assert!(l2.rows + l3.rows <= 40);
}

// ── Resize via set_ratio + recompute ──────────────────────────────

#[test]
fn resize_after_splits_updates_layout() {
    let desc = standard_desc();
    let floating = FloatingLayer::new();

    // Build a vertical split.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let before = compute_layout(&tree, &floating, p(1), &desc);
    let l1_before = before.iter().find(|l| l.pane_id == p(1)).unwrap();
    let l2_before = before.iter().find(|l| l.pane_id == p(2)).unwrap();
    let width1_before = l1_before.pixel_rect.width;
    let width2_before = l2_before.pixel_rect.width;

    // Resize to 70/30.
    let tree = tree.set_ratio(p(1), SplitDirection::Vertical, 0.7);
    let after = compute_layout(&tree, &floating, p(1), &desc);
    let l1_after = after.iter().find(|l| l.pane_id == p(1)).unwrap();
    let l2_after = after.iter().find(|l| l.pane_id == p(2)).unwrap();

    // Pane 1 should be wider, pane 2 narrower.
    assert!(l1_after.pixel_rect.width > width1_before);
    assert!(l2_after.pixel_rect.width < width2_before);

    // Both panes should still be within the available area.
    let total = l1_after.pixel_rect.width + l2_after.pixel_rect.width + desc.divider_px;
    assert!(total <= desc.available.width + 1.0);

    // Both should still have positive cols.
    assert!(l1_after.cols > 0);
    assert!(l2_after.cols > 0);
}

// ── Remove pane + recompute ───────────────────────────────────────

#[test]
fn remove_pane_then_recompute_fills_space() {
    let desc = standard_desc();
    let floating = FloatingLayer::new();

    // Build: p1 | (p2 / p3).
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);
    let tree = tree.split_at(p(2), SplitDirection::Horizontal, p(3), 0.5);

    // Remove p2 → tree collapses to p1 | p3.
    let tree = tree.remove(p(2)).expect("should not be None");
    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts.len(), 2);

    let l1 = layouts.iter().find(|l| l.pane_id == p(1)).unwrap();
    let l3 = layouts.iter().find(|l| l.pane_id == p(3)).unwrap();

    // Both panes should have positive dimensions.
    assert!(l1.cols > 0);
    assert!(l1.rows > 0);
    assert!(l3.cols > 0);
    assert!(l3.rows > 0);

    // They should fill most of the available area (minus divider + grid snapping).
    // Each pane loses up to one cell_width from snapping, so allow 2*cell_width slack.
    let total_w = l1.pixel_rect.width + l3.pixel_rect.width + desc.divider_px;
    assert!(total_w <= desc.available.width + 1.0);
    assert!(
        total_w > desc.available.width - 2.0 * desc.cell_width,
        "total width {total_w} too far from available {}",
        desc.available.width,
    );
}

// ── Hit-test consistency after resize ─────────────────────────────

#[test]
fn hit_test_consistent_after_resize() {
    let desc = standard_desc();
    let floating = FloatingLayer::new();

    // Vertical split 50/50.
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let before = compute_layout(&tree, &floating, p(1), &desc);
    let l1 = before.iter().find(|l| l.pane_id == p(1)).unwrap();
    let midpoint_x = l1.pixel_rect.x + l1.pixel_rect.width + desc.divider_px / 2.0;

    // Point just left of divider should be in pane 1.
    let hit_left = super::super::super::nav::nearest_pane(&before, midpoint_x - 5.0, 400.0);
    assert_eq!(hit_left, Some(p(1)));

    // Now resize to 70/30 — divider moves right.
    let tree = tree.set_ratio(p(1), SplitDirection::Vertical, 0.7);
    let after = compute_layout(&tree, &floating, p(1), &desc);

    // Same x that was in pane 1's territory should still be in pane 1
    // (divider moved further right).
    let hit_after = super::super::super::nav::nearest_pane(&after, midpoint_x - 5.0, 400.0);
    assert_eq!(hit_after, Some(p(1)));

    // Point at old divider position should now be inside pane 1.
    let hit_old_div = super::super::super::nav::nearest_pane(&after, midpoint_x, 400.0);
    assert_eq!(hit_old_div, Some(p(1)));
}

// ── Fractional cell dimension rounding ────────────────────────────

#[test]
fn fractional_cell_dimensions_produce_deterministic_layout() {
    // 7px-wide cells in 1000px area → 142.857... cells.
    let desc = LayoutDescriptor {
        available: Rect {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 800.0,
        },
        cell_width: 7.0,
        cell_height: 13.0,
        divider_px: 2.0,
        min_pane_cells: (4, 2),
    };
    let floating = FloatingLayer::new();

    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts.len(), 2);

    for layout in &layouts {
        // Width must be exact multiple of cell_width.
        let remainder_w = layout.pixel_rect.width % desc.cell_width;
        assert!(
            remainder_w.abs() < 0.01 || (desc.cell_width - remainder_w).abs() < 0.01,
            "width {} not aligned to cell_width {}",
            layout.pixel_rect.width,
            desc.cell_width,
        );
        // Height must be exact multiple of cell_height.
        let remainder_h = layout.pixel_rect.height % desc.cell_height;
        assert!(
            remainder_h.abs() < 0.01 || (desc.cell_height - remainder_h).abs() < 0.01,
            "height {} not aligned to cell_height {}",
            layout.pixel_rect.height,
            desc.cell_height,
        );
        assert!(layout.cols > 0);
        assert!(layout.rows > 0);
    }

    // Determinism: same input → same output.
    let layouts2 = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts, layouts2);
}

// ── Zero-size available rect ──────────────────────────────────────

#[test]
fn zero_size_available_rect_does_not_panic() {
    let desc = LayoutDescriptor {
        available: Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        cell_width: 10.0,
        cell_height: 20.0,
        divider_px: 2.0,
        min_pane_cells: (4, 2),
    };
    let floating = FloatingLayer::new();
    let tree = SplitTree::leaf(p(1));
    let tree = tree.split_at(p(1), SplitDirection::Vertical, p(2), 0.5);

    // Should not panic, produce NaN, or infinite values.
    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    assert_eq!(layouts.len(), 2);
    for layout in &layouts {
        assert!(!layout.pixel_rect.width.is_nan());
        assert!(!layout.pixel_rect.height.is_nan());
        assert!(!layout.pixel_rect.x.is_nan());
        assert!(!layout.pixel_rect.y.is_nan());
        assert!(!layout.pixel_rect.width.is_infinite());
        assert!(!layout.pixel_rect.height.is_infinite());
        // cols/rows should be at least 1 (clamped).
        assert!(layout.cols >= 1);
        assert!(layout.rows >= 1);
    }

    // Dividers should also not panic.
    let dividers = compute_dividers(&tree, &desc);
    for d in &dividers {
        assert!(!d.rect.width.is_nan());
        assert!(!d.rect.height.is_nan());
    }
}

// ── Minimum floating pane size enforcement

#[test]
fn floating_pane_below_minimum_is_clamped() {
    let desc = standard_desc();
    let tree = SplitTree::Leaf(p(1));

    // Create a tiny floating pane: 5 cols × 2 rows (below 20×5 minimum).
    let tiny = FloatingPane {
        pane_id: p(2),
        rect: Rect {
            x: 100.0,
            y: 100.0,
            width: 50.0,  // 5 cols at 10px/col.
            height: 40.0, // 2 rows at 20px/row.
        },
        z_order: 0,
    };
    let floating = FloatingLayer::new().add(tiny);

    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    let float_layout = layouts.iter().find(|l| l.pane_id == p(2)).unwrap();

    // Should be clamped to 20 cols × 5 rows.
    assert_eq!(float_layout.cols, 20);
    assert_eq!(float_layout.rows, 5);
    assert!((float_layout.pixel_rect.width - 200.0).abs() < f32::EPSILON);
    assert!((float_layout.pixel_rect.height - 100.0).abs() < f32::EPSILON);
}

#[test]
fn floating_pane_above_minimum_is_unchanged() {
    let desc = standard_desc();
    let tree = SplitTree::Leaf(p(1));

    // Create a floating pane above minimum: 30 cols × 10 rows.
    let big = FloatingPane {
        pane_id: p(2),
        rect: Rect {
            x: 100.0,
            y: 100.0,
            width: 300.0,
            height: 200.0,
        },
        z_order: 0,
    };
    let floating = FloatingLayer::new().add(big);

    let layouts = compute_layout(&tree, &floating, p(1), &desc);
    let float_layout = layouts.iter().find(|l| l.pane_id == p(2)).unwrap();

    assert_eq!(float_layout.cols, 30);
    assert_eq!(float_layout.rows, 10);
}
