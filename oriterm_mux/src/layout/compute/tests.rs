use super::{LayoutDescriptor, compute_dividers, compute_layout};
use crate::id::PaneId;
use crate::layout::floating::{FloatingLayer, FloatingPane, Rect};
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
        x: 100.0,
        y: 100.0,
        width: 400.0,
        height: 300.0,
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
