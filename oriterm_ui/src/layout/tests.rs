//! Tests for the layout engine.

use crate::geometry::{Insets, Rect};

use super::{Align, Direction, Justify, LayoutBox, LayoutConstraints, SizeSpec, compute_layout};

/// Helper: creates a viewport rect at origin.
fn viewport(w: f32, h: f32) -> Rect {
    Rect::new(0.0, 0.0, w, h)
}

/// Helper: asserts two f32 values are approximately equal.
fn assert_approx(a: f32, b: f32, label: &str) {
    assert!((a - b).abs() < 0.01, "{label}: expected {b}, got {a}",);
}

// ── Leaf sizing ──

#[test]
fn leaf_hug_uses_intrinsic_size() {
    let b = LayoutBox::leaf(100.0, 50.0);
    let node = compute_layout(&b, viewport(800.0, 600.0));
    assert_approx(node.rect.width(), 100.0, "width");
    assert_approx(node.rect.height(), 50.0, "height");
}

#[test]
fn leaf_fixed_ignores_intrinsic() {
    let b = LayoutBox::leaf(100.0, 50.0)
        .with_width(SizeSpec::Fixed(200.0))
        .with_height(SizeSpec::Fixed(80.0));
    let node = compute_layout(&b, viewport(800.0, 600.0));
    assert_approx(node.rect.width(), 200.0, "width");
    assert_approx(node.rect.height(), 80.0, "height");
}

#[test]
fn leaf_fill_expands_to_viewport() {
    let b = LayoutBox::leaf(100.0, 50.0)
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill);
    let node = compute_layout(&b, viewport(800.0, 600.0));
    assert_approx(node.rect.width(), 800.0, "width");
    assert_approx(node.rect.height(), 600.0, "height");
}

#[test]
fn leaf_padding_shrinks_content_rect() {
    let b = LayoutBox::leaf(100.0, 50.0).with_padding(Insets::all(10.0));
    let node = compute_layout(&b, viewport(800.0, 600.0));
    // Hug: total size = intrinsic + padding.
    assert_approx(node.rect.width(), 120.0, "outer width");
    assert_approx(node.rect.height(), 70.0, "outer height");
    assert_approx(node.content_rect.width(), 100.0, "content width");
    assert_approx(node.content_rect.height(), 50.0, "content height");
    assert_approx(node.content_rect.x(), 10.0, "content x");
    assert_approx(node.content_rect.y(), 10.0, "content y");
}

#[test]
fn leaf_margin_offsets_position() {
    let b = LayoutBox::leaf(100.0, 50.0).with_margin(Insets::tlbr(5.0, 10.0, 5.0, 10.0));
    let node = compute_layout(&b, viewport(800.0, 600.0));
    assert_approx(node.rect.x(), 10.0, "x offset");
    assert_approx(node.rect.y(), 5.0, "y offset");
}

// ── Row flex: fixed children ──

#[test]
fn row_two_fixed_children() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![LayoutBox::leaf(100.0, 50.0), LayoutBox::leaf(200.0, 60.0)],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    assert_eq!(node.children.len(), 2);

    let c0 = &node.children[0];
    let c1 = &node.children[1];
    assert_approx(c0.rect.x(), 0.0, "child0 x");
    assert_approx(c0.rect.width(), 100.0, "child0 w");
    assert_approx(c1.rect.x(), 100.0, "child1 x");
    assert_approx(c1.rect.width(), 200.0, "child1 w");
}

#[test]
fn row_fixed_plus_fill() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(100.0, 50.0),
            LayoutBox::leaf(0.0, 50.0).with_width(SizeSpec::Fill),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    let c0 = &node.children[0];
    let c1 = &node.children[1];
    assert_approx(c0.rect.width(), 100.0, "fixed child");
    assert_approx(c1.rect.width(), 700.0, "fill child");
    assert_approx(c1.rect.x(), 100.0, "fill child x");
}

#[test]
fn row_equal_fill_children() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(0.0, 50.0).with_width(SizeSpec::Fill),
            LayoutBox::leaf(0.0, 50.0).with_width(SizeSpec::Fill),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    assert_approx(node.children[0].rect.width(), 400.0, "left half");
    assert_approx(node.children[1].rect.width(), 400.0, "right half");
}

#[test]
fn row_weighted_fill_portion() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(0.0, 50.0).with_width(SizeSpec::FillPortion(2)),
            LayoutBox::leaf(0.0, 50.0).with_width(SizeSpec::FillPortion(1)),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill);

    let node = compute_layout(&row, viewport(900.0, 600.0));
    assert_approx(node.children[0].rect.width(), 600.0, "2/3");
    assert_approx(node.children[1].rect.width(), 300.0, "1/3");
}

#[test]
fn row_with_gap() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(100.0, 50.0),
            LayoutBox::leaf(100.0, 50.0),
            LayoutBox::leaf(100.0, 50.0),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill)
    .with_gap(10.0);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    assert_approx(node.children[0].rect.x(), 0.0, "c0 x");
    assert_approx(node.children[1].rect.x(), 110.0, "c1 x");
    assert_approx(node.children[2].rect.x(), 220.0, "c2 x");
}

// ── Column flex ──

#[test]
fn column_two_fixed_children() {
    let col = LayoutBox::flex(
        Direction::Column,
        vec![LayoutBox::leaf(100.0, 40.0), LayoutBox::leaf(100.0, 60.0)],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill);

    let node = compute_layout(&col, viewport(800.0, 600.0));
    assert_approx(node.children[0].rect.y(), 0.0, "c0 y");
    assert_approx(node.children[0].rect.height(), 40.0, "c0 h");
    assert_approx(node.children[1].rect.y(), 40.0, "c1 y");
    assert_approx(node.children[1].rect.height(), 60.0, "c1 h");
}

#[test]
fn column_fill_distributes_vertically() {
    let col = LayoutBox::flex(
        Direction::Column,
        vec![
            LayoutBox::leaf(100.0, 0.0).with_height(SizeSpec::Fill),
            LayoutBox::leaf(100.0, 0.0).with_height(SizeSpec::Fill),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill);

    let node = compute_layout(&col, viewport(800.0, 600.0));
    assert_approx(node.children[0].rect.height(), 300.0, "top half");
    assert_approx(node.children[1].rect.height(), 300.0, "bottom half");
}

// ── Justify ──

#[test]
fn justify_center() {
    let row = LayoutBox::flex(Direction::Row, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_justify(Justify::Center);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    assert_approx(node.children[0].rect.x(), 350.0, "centered x");
}

#[test]
fn justify_end() {
    let row = LayoutBox::flex(Direction::Row, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_justify(Justify::End);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    assert_approx(node.children[0].rect.x(), 700.0, "end x");
}

#[test]
fn justify_space_between() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(50.0, 30.0),
            LayoutBox::leaf(50.0, 30.0),
            LayoutBox::leaf(50.0, 30.0),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill)
    .with_justify(Justify::SpaceBetween);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    // 800 - 150 = 650 free, 2 gaps → 325 each.
    assert_approx(node.children[0].rect.x(), 0.0, "first");
    assert_approx(node.children[1].rect.x(), 375.0, "middle");
    assert_approx(node.children[2].rect.x(), 750.0, "last");
}

#[test]
fn justify_space_around() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![LayoutBox::leaf(100.0, 30.0), LayoutBox::leaf(100.0, 30.0)],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill)
    .with_justify(Justify::SpaceAround);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    // 800 - 200 = 600 free, 2 items → 300 per item.
    // Half-space at edges = 150.
    assert_approx(node.children[0].rect.x(), 150.0, "first");
    assert_approx(node.children[1].rect.x(), 550.0, "second");
}

// ── Align ──

#[test]
fn align_center_cross_axis() {
    let row = LayoutBox::flex(Direction::Row, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_align(Align::Center);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    // Cross axis is vertical: (600 - 50) / 2 = 275.
    assert_approx(node.children[0].rect.y(), 275.0, "centered y");
}

#[test]
fn align_end_cross_axis() {
    let row = LayoutBox::flex(Direction::Row, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_align(Align::End);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    assert_approx(node.children[0].rect.y(), 550.0, "end y");
}

#[test]
fn align_stretch_uses_full_cross() {
    let row = LayoutBox::flex(Direction::Row, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_align(Align::Stretch);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    // Stretch: child gets full cross-axis space.
    assert_approx(node.children[0].rect.y(), 0.0, "stretch y");
}

// ── Nested ──

#[test]
fn nested_row_in_column() {
    let inner_row = LayoutBox::flex(
        Direction::Row,
        vec![LayoutBox::leaf(100.0, 30.0), LayoutBox::leaf(100.0, 30.0)],
    );

    let col = LayoutBox::flex(
        Direction::Column,
        vec![inner_row, LayoutBox::leaf(200.0, 40.0)],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill);

    let node = compute_layout(&col, viewport(800.0, 600.0));
    assert_eq!(node.children.len(), 2);
    // Inner row should have 2 children.
    assert_eq!(node.children[0].children.len(), 2);
    assert_approx(node.children[0].rect.y(), 0.0, "row y");
    assert_approx(node.children[1].rect.y(), 30.0, "leaf y");
}

// ── Hug containers ──

#[test]
fn hug_container_wraps_children() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![LayoutBox::leaf(100.0, 50.0), LayoutBox::leaf(150.0, 60.0)],
    );
    // Default is Hug for both dimensions.
    let node = compute_layout(&row, viewport(800.0, 600.0));
    assert_approx(node.rect.width(), 250.0, "hug width");
    assert_approx(node.rect.height(), 60.0, "hug height = max child");
}

#[test]
fn hug_container_includes_gaps() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![LayoutBox::leaf(100.0, 50.0), LayoutBox::leaf(100.0, 50.0)],
    )
    .with_gap(20.0);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    assert_approx(node.rect.width(), 220.0, "hug width with gap");
}

#[test]
fn hug_container_includes_padding() {
    let row = LayoutBox::flex(Direction::Row, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_padding(Insets::all(10.0));

    let node = compute_layout(&row, viewport(800.0, 600.0));
    assert_approx(node.rect.width(), 120.0, "hug + padding width");
    assert_approx(node.rect.height(), 70.0, "hug + padding height");
}

// ── Edge cases ──

#[test]
fn empty_container() {
    let row = LayoutBox::flex(Direction::Row, vec![]);
    let node = compute_layout(&row, viewport(800.0, 600.0));
    assert!(node.children.is_empty());
    assert_approx(node.rect.width(), 0.0, "empty width");
    assert_approx(node.rect.height(), 0.0, "empty height");
}

#[test]
fn single_child_container() {
    let row = LayoutBox::flex(Direction::Row, vec![LayoutBox::leaf(50.0, 30.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill);

    let node = compute_layout(&row, viewport(400.0, 300.0));
    assert_eq!(node.children.len(), 1);
    assert_approx(node.children[0].rect.x(), 0.0, "single x");
    assert_approx(node.children[0].rect.width(), 50.0, "single w");
}

#[test]
fn min_max_constraints() {
    let b = LayoutBox::leaf(100.0, 50.0)
        .with_width(SizeSpec::Fill)
        .with_max_width(300.0);

    let node = compute_layout(&b, viewport(800.0, 600.0));
    assert_approx(node.rect.width(), 300.0, "max-clamped width");
}

#[test]
fn min_width_enforced() {
    let b = LayoutBox::leaf(10.0, 10.0).with_min_width(200.0);

    let node = compute_layout(&b, viewport(800.0, 600.0));
    assert_approx(node.rect.width(), 200.0, "min-enforced width");
}

// ── Invariant: content_rect == rect.inset(padding) ──

#[test]
fn content_rect_invariant_leaf() {
    let b = LayoutBox::leaf(100.0, 50.0).with_padding(Insets::vh(8.0, 12.0));
    let node = compute_layout(&b, viewport(800.0, 600.0));
    let expected = node.rect.inset(Insets::vh(8.0, 12.0));
    assert_eq!(node.content_rect, expected);
}

#[test]
fn content_rect_invariant_flex() {
    let row = LayoutBox::flex(Direction::Row, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_padding(Insets::all(15.0));

    let node = compute_layout(&row, viewport(800.0, 600.0));
    let expected = node.rect.inset(Insets::all(15.0));
    assert_eq!(node.content_rect, expected);
}

// ── LayoutConstraints unit tests ──

#[test]
fn constraints_tight() {
    let c = LayoutConstraints::tight(100.0, 200.0);
    assert!(c.is_tight());
    assert_eq!(c.constrain(50.0, 300.0), (100.0, 200.0));
}

#[test]
fn constraints_loose() {
    let c = LayoutConstraints::loose(100.0, 200.0);
    assert!(!c.is_tight());
    assert_eq!(c.constrain(50.0, 150.0), (50.0, 150.0));
    assert_eq!(c.constrain(200.0, 300.0), (100.0, 200.0));
}

#[test]
fn constraints_unbounded() {
    let c = LayoutConstraints::unbounded();
    assert!(!c.has_bounded_width());
    assert!(!c.has_bounded_height());
}

#[test]
fn constraints_shrink() {
    let c = LayoutConstraints::tight(100.0, 200.0);
    let s = c.shrink(Insets::all(10.0));
    assert_eq!(s.max_width, 80.0);
    assert_eq!(s.max_height, 180.0);
}

// ── SizeSpec unit tests ──

#[test]
fn size_spec_fill_weight() {
    assert_eq!(SizeSpec::Fill.fill_weight(), 1);
    assert_eq!(SizeSpec::FillPortion(3).fill_weight(), 3);
    assert_eq!(SizeSpec::Fixed(10.0).fill_weight(), 0);
    assert_eq!(SizeSpec::Hug.fill_weight(), 0);
}

#[test]
fn size_spec_is_fill() {
    assert!(SizeSpec::Fill.is_fill());
    assert!(SizeSpec::FillPortion(2).is_fill());
    assert!(!SizeSpec::Fixed(10.0).is_fill());
    assert!(!SizeSpec::Hug.is_fill());
}

// ── Three-level nesting ──

#[test]
fn three_level_nesting() {
    let inner = LayoutBox::flex(Direction::Row, vec![LayoutBox::leaf(50.0, 20.0)]);
    let middle = LayoutBox::flex(Direction::Column, vec![inner, LayoutBox::leaf(60.0, 30.0)]);
    let outer = LayoutBox::flex(Direction::Row, vec![middle, LayoutBox::leaf(80.0, 40.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill);

    let node = compute_layout(&outer, viewport(800.0, 600.0));
    // Outer has 2 children.
    assert_eq!(node.children.len(), 2);
    // First child (middle column) has 2 children.
    assert_eq!(node.children[0].children.len(), 2);
    // First child of middle (inner row) has 1 child.
    assert_eq!(node.children[0].children[0].children.len(), 1);
}

// ── Container with padding and fill children ──

#[test]
fn padded_container_with_fill_child() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![LayoutBox::leaf(0.0, 50.0).with_width(SizeSpec::Fill)],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill)
    .with_padding(Insets::all(20.0));

    let node = compute_layout(&row, viewport(800.0, 600.0));
    // Container is 800x600. Padding 20 each side → content area 760x560.
    assert_approx(node.content_rect.width(), 760.0, "content w");
    // Fill child should get the full content width.
    assert_approx(node.children[0].rect.width(), 760.0, "fill child w");
}

// ── Column with gap and fill ──

#[test]
fn column_gap_and_fill() {
    let col = LayoutBox::flex(
        Direction::Column,
        vec![
            LayoutBox::leaf(100.0, 50.0),
            LayoutBox::leaf(100.0, 0.0).with_height(SizeSpec::Fill),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill)
    .with_gap(10.0);

    let node = compute_layout(&col, viewport(800.0, 600.0));
    // 600 total - 50 fixed - 10 gap = 540 for fill child.
    assert_approx(node.children[0].rect.height(), 50.0, "fixed h");
    assert_approx(node.children[1].rect.height(), 540.0, "fill h");
    assert_approx(node.children[1].rect.y(), 60.0, "fill y");
}

// ── Fractional fill distribution (ratatui rounding pattern) ──

#[test]
fn three_equal_fills_indivisible() {
    // 100px / 3 = 33.333... — children should sum to exactly 100.
    let row = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(0.0, 30.0).with_width(SizeSpec::Fill),
            LayoutBox::leaf(0.0, 30.0).with_width(SizeSpec::Fill),
            LayoutBox::leaf(0.0, 30.0).with_width(SizeSpec::Fill),
        ],
    )
    .with_width(SizeSpec::Fixed(100.0))
    .with_height(SizeSpec::Fixed(30.0));

    let node = compute_layout(&row, viewport(100.0, 30.0));
    let total: f32 = node.children.iter().map(|c| c.rect.width()).sum();
    assert_approx(total, 100.0, "children sum to container");
    // Each child gets 33.33...
    for child in &node.children {
        assert_approx(child.rect.width(), 100.0 / 3.0, "equal third");
    }
}

#[test]
fn fill_portion_uneven_weights() {
    // Weights 1:2:3 in 120px → 20, 40, 60.
    let row = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(0.0, 30.0).with_width(SizeSpec::FillPortion(1)),
            LayoutBox::leaf(0.0, 30.0).with_width(SizeSpec::FillPortion(2)),
            LayoutBox::leaf(0.0, 30.0).with_width(SizeSpec::FillPortion(3)),
        ],
    )
    .with_width(SizeSpec::Fixed(120.0))
    .with_height(SizeSpec::Fixed(30.0));

    let node = compute_layout(&row, viewport(120.0, 30.0));
    assert_approx(node.children[0].rect.width(), 20.0, "1/6");
    assert_approx(node.children[1].rect.width(), 40.0, "2/6");
    assert_approx(node.children[2].rect.width(), 60.0, "3/6");
}

// ── Zero-size viewport ──

#[test]
fn zero_viewport_leaf() {
    let b = LayoutBox::leaf(100.0, 50.0);
    let node = compute_layout(&b, viewport(0.0, 0.0));
    // loose(0,0) constrains max to 0, so Hug resolves to intrinsic then clamps to 0.
    assert_approx(node.rect.width(), 0.0, "hug in zero viewport w");
    assert_approx(node.rect.height(), 0.0, "hug in zero viewport h");
}

#[test]
fn zero_viewport_fill() {
    let b = LayoutBox::leaf(100.0, 50.0)
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill);
    let node = compute_layout(&b, viewport(0.0, 0.0));
    // Fill with zero max → 0.
    assert_approx(node.rect.width(), 0.0, "fill in zero viewport w");
    assert_approx(node.rect.height(), 0.0, "fill in zero viewport h");
}

#[test]
fn zero_viewport_flex_with_children() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![LayoutBox::leaf(50.0, 30.0), LayoutBox::leaf(50.0, 30.0)],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill);

    let node = compute_layout(&row, viewport(0.0, 0.0));
    assert_approx(node.rect.width(), 0.0, "zero viewport row w");
}

// ── Fill in Hug parent (unbounded context) ──

#[test]
fn fill_child_in_hug_parent_uses_bounded_constraint() {
    // A Hug parent passes its bounded constraints (from the viewport) to children.
    // A Fill child sees the finite max and expands to it, then the Hug parent
    // wraps to the children's resulting size.
    let row = LayoutBox::flex(
        Direction::Row,
        vec![LayoutBox::leaf(80.0, 30.0).with_width(SizeSpec::Fill)],
    );
    // Parent is Hug (default), viewport provides bounded constraints.

    let node = compute_layout(&row, viewport(800.0, 600.0));
    // Fill child sees max_width=800 (bounded from viewport) and fills it.
    assert_approx(node.children[0].rect.width(), 800.0, "fill in hug w");
    // Hug parent wraps to child size.
    assert_approx(node.rect.width(), 800.0, "hug parent w");
}

#[test]
fn fill_child_in_truly_unbounded_context() {
    // When constraints are truly unbounded (INFINITY), Fill falls back to intrinsic.
    // Use a very large viewport to simulate unbounded (Size clamps near-zero but
    // not near-infinity).
    let leaf = LayoutBox::leaf(80.0, 30.0).with_width(SizeSpec::Fill);
    let node = compute_layout(&leaf, Rect::new(0.0, 0.0, f32::INFINITY, f32::INFINITY));
    // Fill with infinite max → falls back to intrinsic + padding = 80.
    assert_approx(node.rect.width(), 80.0, "fill unbounded w");
}

// ── Margin overflow (margin > available space) ──

#[test]
fn margin_larger_than_viewport() {
    let b = LayoutBox::leaf(50.0, 30.0).with_margin(Insets::all(500.0));
    let node = compute_layout(&b, viewport(200.0, 200.0));
    // Margin offsets position; constraints shrink (clamped to 0).
    assert_approx(node.rect.x(), 500.0, "margin offset x");
    assert_approx(node.rect.y(), 500.0, "margin offset y");
}

#[test]
fn margin_consumes_all_space_for_fill() {
    let b = LayoutBox::leaf(50.0, 30.0)
        .with_width(SizeSpec::Fill)
        .with_margin(Insets::vh(0.0, 100.0));
    let node = compute_layout(&b, viewport(200.0, 100.0));
    // 200 - 200 (left+right margin) = 0 available for fill.
    assert_approx(node.rect.width(), 0.0, "fill with margin overflow");
}

// ── Justification edge cases: single child ──

#[test]
fn space_between_single_child() {
    let row = LayoutBox::flex(Direction::Row, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_justify(Justify::SpaceBetween);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    // Single child with SpaceBetween → starts at 0 (no gaps to distribute).
    assert_approx(node.children[0].rect.x(), 0.0, "space-between single");
}

#[test]
fn space_around_single_child() {
    let row = LayoutBox::flex(Direction::Row, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_justify(Justify::SpaceAround);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    // Single child with SpaceAround → centered (half-space each side).
    // 800 - 100 = 700 free / 1 item = 700 per. Half = 350.
    assert_approx(node.children[0].rect.x(), 350.0, "space-around single");
}

// ── Column-axis justify and align ──

#[test]
fn column_justify_center() {
    let col = LayoutBox::flex(Direction::Column, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_justify(Justify::Center);

    let node = compute_layout(&col, viewport(800.0, 600.0));
    // Main axis is vertical: (600 - 50) / 2 = 275.
    assert_approx(node.children[0].rect.y(), 275.0, "column center y");
}

#[test]
fn column_justify_end() {
    let col = LayoutBox::flex(Direction::Column, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_justify(Justify::End);

    let node = compute_layout(&col, viewport(800.0, 600.0));
    assert_approx(node.children[0].rect.y(), 550.0, "column end y");
}

#[test]
fn column_justify_space_between() {
    let col = LayoutBox::flex(
        Direction::Column,
        vec![
            LayoutBox::leaf(100.0, 50.0),
            LayoutBox::leaf(100.0, 50.0),
            LayoutBox::leaf(100.0, 50.0),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill)
    .with_justify(Justify::SpaceBetween);

    let node = compute_layout(&col, viewport(800.0, 600.0));
    // 600 - 150 = 450 free, 2 gaps → 225 each.
    assert_approx(node.children[0].rect.y(), 0.0, "col sb first");
    assert_approx(node.children[1].rect.y(), 275.0, "col sb middle");
    assert_approx(node.children[2].rect.y(), 550.0, "col sb last");
}

#[test]
fn column_align_center() {
    let col = LayoutBox::flex(Direction::Column, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_align(Align::Center);

    let node = compute_layout(&col, viewport(800.0, 600.0));
    // Cross axis is horizontal: (800 - 100) / 2 = 350.
    assert_approx(node.children[0].rect.x(), 350.0, "column align center x");
}

#[test]
fn column_align_end() {
    let col = LayoutBox::flex(Direction::Column, vec![LayoutBox::leaf(100.0, 50.0)])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill)
        .with_align(Align::End);

    let node = compute_layout(&col, viewport(800.0, 600.0));
    assert_approx(node.children[0].rect.x(), 700.0, "column align end x");
}

// ── Padding + gap + fill interaction ──

#[test]
fn padding_gap_fill_combined() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(100.0, 30.0),
            LayoutBox::leaf(0.0, 30.0).with_width(SizeSpec::Fill),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill)
    .with_padding(Insets::all(10.0))
    .with_gap(20.0);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    // Content area: 800 - 20 = 780. Gap: 20. Fixed: 100.
    // Fill: 780 - 100 - 20 = 660.
    assert_approx(node.children[0].rect.width(), 100.0, "fixed w");
    assert_approx(node.children[1].rect.width(), 660.0, "fill w");
}

// ── Constraint priority: min > max conflict ──

#[test]
fn min_greater_than_max_clamps_to_min() {
    // When min_width > max_width on the constraints, clamp should use min.
    // (f32::clamp with min > max returns min.)
    let b = LayoutBox::leaf(50.0, 30.0)
        .with_min_width(300.0)
        .with_max_width(100.0);

    let node = compute_layout(&b, viewport(800.0, 600.0));
    // f32::clamp(50, 300, 100) → 300 (min wins per Rust semantics).
    assert_approx(node.rect.width(), 300.0, "min beats max");
}

#[test]
fn min_height_greater_than_max_height() {
    let b = LayoutBox::leaf(50.0, 30.0)
        .with_min_height(200.0)
        .with_max_height(50.0);

    let node = compute_layout(&b, viewport(800.0, 600.0));
    assert_approx(node.rect.height(), 200.0, "min_h beats max_h");
}

// ── Nested fill chains ──

#[test]
fn fill_container_with_fill_children() {
    // Fill container holding fill children — space should cascade.
    let row = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(0.0, 30.0).with_width(SizeSpec::Fill),
            LayoutBox::leaf(0.0, 30.0).with_width(SizeSpec::Fill),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill);

    let outer = LayoutBox::flex(Direction::Column, vec![row])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill);

    let node = compute_layout(&outer, viewport(600.0, 400.0));
    let inner_row = &node.children[0];
    assert_approx(inner_row.rect.width(), 600.0, "fill row w");
    assert_approx(
        inner_row.children[0].rect.width(),
        300.0,
        "nested fill left",
    );
    assert_approx(
        inner_row.children[1].rect.width(),
        300.0,
        "nested fill right",
    );
}

#[test]
fn deeply_nested_fill_propagation() {
    // 4 levels: outer col → row → col → fill leaf.
    let leaf = LayoutBox::leaf(0.0, 0.0)
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill);
    let inner_col = LayoutBox::flex(Direction::Column, vec![leaf])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill);
    let mid_row = LayoutBox::flex(Direction::Row, vec![inner_col])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill);
    let outer = LayoutBox::flex(Direction::Column, vec![mid_row])
        .with_width(SizeSpec::Fill)
        .with_height(SizeSpec::Fill);

    let node = compute_layout(&outer, viewport(500.0, 300.0));
    // Every level should fill the full viewport.
    let deepest = &node.children[0].children[0].children[0];
    assert_approx(deepest.rect.width(), 500.0, "deep fill w");
    assert_approx(deepest.rect.height(), 300.0, "deep fill h");
}

// ── Recursive content_rect invariant ──

/// Recursively verifies that `content_rect == rect.inset(padding)` for every node.
fn assert_content_rect_invariant(node: &super::LayoutNode, padding: Insets, path: &str) {
    let expected = node.rect.inset(padding);
    assert_eq!(
        node.content_rect, expected,
        "content_rect invariant violated at {path}"
    );
}

/// Walks a layout tree verifying the content_rect invariant at every node.
fn walk_invariant(node: &super::LayoutNode, layout_box: &LayoutBox, path: &str) {
    assert_content_rect_invariant(node, layout_box.padding, path);

    if let super::BoxContent::Flex { children, .. } = &layout_box.content {
        for (idx, (child_node, child_box)) in node.children.iter().zip(children.iter()).enumerate()
        {
            let child_path = format!("{path}[{idx}]");
            walk_invariant(child_node, child_box, &child_path);
        }
    }
}

#[test]
fn content_rect_invariant_nested_tree() {
    let inner = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(60.0, 20.0).with_padding(Insets::all(5.0)),
            LayoutBox::leaf(40.0, 20.0).with_padding(Insets::vh(3.0, 7.0)),
        ],
    )
    .with_padding(Insets::all(8.0));

    let outer = LayoutBox::flex(
        Direction::Column,
        vec![
            inner,
            LayoutBox::leaf(100.0, 30.0).with_padding(Insets::all(4.0)),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill)
    .with_padding(Insets::all(12.0));

    let node = compute_layout(&outer, viewport(800.0, 600.0));
    walk_invariant(&node, &outer, "root");
}

// ── Children sum check (no overlap, no gaps for Start justify) ──

#[test]
fn children_positions_are_contiguous_row() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(100.0, 30.0),
            LayoutBox::leaf(150.0, 30.0),
            LayoutBox::leaf(80.0, 30.0),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    // Each child's x should be the previous child's x + width (Justify::Start, no gap).
    for pair in node.children.windows(2) {
        let expected_x = pair[0].rect.x() + pair[0].rect.width();
        assert_approx(pair[1].rect.x(), expected_x, "contiguous row");
    }
}

#[test]
fn children_positions_are_contiguous_column() {
    let col = LayoutBox::flex(
        Direction::Column,
        vec![
            LayoutBox::leaf(100.0, 40.0),
            LayoutBox::leaf(100.0, 60.0),
            LayoutBox::leaf(100.0, 25.0),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill);

    let node = compute_layout(&col, viewport(800.0, 600.0));
    for pair in node.children.windows(2) {
        let expected_y = pair[0].rect.y() + pair[0].rect.height();
        assert_approx(pair[1].rect.y(), expected_y, "contiguous column");
    }
}

// ── Children with gap: positions include gap ──

#[test]
fn children_positions_include_gap_row() {
    let row = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(80.0, 30.0),
            LayoutBox::leaf(60.0, 30.0),
            LayoutBox::leaf(40.0, 30.0),
        ],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill)
    .with_gap(15.0);

    let node = compute_layout(&row, viewport(800.0, 600.0));
    for pair in node.children.windows(2) {
        let expected_x = pair[0].rect.x() + pair[0].rect.width() + 15.0;
        assert_approx(pair[1].rect.x(), expected_x, "gap row");
    }
}

#[test]
fn children_positions_include_gap_column() {
    let col = LayoutBox::flex(
        Direction::Column,
        vec![LayoutBox::leaf(100.0, 30.0), LayoutBox::leaf(100.0, 40.0)],
    )
    .with_width(SizeSpec::Fill)
    .with_height(SizeSpec::Fill)
    .with_gap(10.0);

    let node = compute_layout(&col, viewport(800.0, 600.0));
    let expected_y = node.children[0].rect.y() + node.children[0].rect.height() + 10.0;
    assert_approx(node.children[1].rect.y(), expected_y, "gap column");
}

// ── Non-zero viewport origin ──

#[test]
fn viewport_with_nonzero_origin() {
    let b = LayoutBox::leaf(100.0, 50.0);
    let node = compute_layout(&b, Rect::new(50.0, 30.0, 800.0, 600.0));
    assert_approx(node.rect.x(), 50.0, "origin x");
    assert_approx(node.rect.y(), 30.0, "origin y");
}

// ── Mixed fixed + multiple fills with gap ──

#[test]
fn mixed_fixed_fills_gap() {
    // [Fixed 100] [gap 10] [Fill] [gap 10] [Fill] in 500px.
    // Fill space: 500 - 100 - 20 = 380, split equally = 190 each.
    let row = LayoutBox::flex(
        Direction::Row,
        vec![
            LayoutBox::leaf(100.0, 30.0),
            LayoutBox::leaf(0.0, 30.0).with_width(SizeSpec::Fill),
            LayoutBox::leaf(0.0, 30.0).with_width(SizeSpec::Fill),
        ],
    )
    .with_width(SizeSpec::Fixed(500.0))
    .with_height(SizeSpec::Fixed(30.0))
    .with_gap(10.0);

    let node = compute_layout(&row, viewport(500.0, 30.0));
    assert_approx(node.children[0].rect.width(), 100.0, "fixed");
    assert_approx(node.children[1].rect.width(), 190.0, "fill1");
    assert_approx(node.children[2].rect.width(), 190.0, "fill2");
    assert_approx(node.children[1].rect.x(), 110.0, "fill1 x");
    assert_approx(node.children[2].rect.x(), 310.0, "fill2 x");
}

// ── Constraints shrink floors at zero ──

#[test]
fn constraints_shrink_floors_at_zero() {
    let big = LayoutConstraints::tight(50.0, 50.0);
    let shrunk = big.shrink(Insets::all(100.0));
    assert_eq!(shrunk.min_width, 0.0);
    assert_eq!(shrunk.max_width, 0.0);
    assert_eq!(shrunk.min_height, 0.0);
    assert_eq!(shrunk.max_height, 0.0);
}

// ── FillPortion(0) behaves like Hug ──

#[test]
fn fill_portion_zero_behaves_like_hug() {
    assert_eq!(SizeSpec::FillPortion(0).fill_weight(), 0);
    assert!(!SizeSpec::FillPortion(0).is_fill());
}
