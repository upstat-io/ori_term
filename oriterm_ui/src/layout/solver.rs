//! Two-pass flex layout solver.
//!
//! Computes concrete positions and sizes from a [`LayoutBox`] descriptor tree.
//! Pass 1 measures children along the main axis, distributing remaining space
//! to `Fill`/`FillPortion` children proportionally. Pass 2 arranges children
//! at computed positions with justification and cross-axis alignment.

use crate::geometry::Rect;

use super::constraints::LayoutConstraints;
use super::flex::{Align, Direction, Justify};
use super::layout_box::{BoxContent, LayoutBox};
use super::layout_node::LayoutNode;
use super::size_spec::SizeSpec;

/// Computes a layout tree from a root descriptor and viewport rectangle.
///
/// The viewport provides maximum available space. The root box's `SizeSpec`
/// determines whether it fills that space or shrinks to content.
pub fn compute_layout(root: &LayoutBox, viewport: Rect) -> LayoutNode {
    let constraints = LayoutConstraints::loose(viewport.width(), viewport.height());
    solve(root, constraints, viewport.x(), viewport.y())
}

/// Recursively solves layout for a single box at a given position.
fn solve(
    layout_box: &LayoutBox,
    constraints: LayoutConstraints,
    pos_x: f32,
    pos_y: f32,
) -> LayoutNode {
    // Apply margin: offset position, shrink available space.
    let mx = pos_x + layout_box.margin.left;
    let my = pos_y + layout_box.margin.top;
    let inner = constraints.shrink(layout_box.margin);

    // Merge box-level min/max with incoming constraints.
    let constrained = LayoutConstraints {
        min_width: inner.min_width.max(layout_box.min_width),
        max_width: inner.max_width.min(layout_box.max_width),
        min_height: inner.min_height.max(layout_box.min_height),
        max_height: inner.max_height.min(layout_box.max_height),
    };

    match &layout_box.content {
        BoxContent::Leaf { .. } => solve_leaf(layout_box, &constrained, mx, my),
        BoxContent::Flex {
            direction,
            align,
            justify,
            gap,
            children,
        } => solve_flex(
            layout_box,
            &constrained,
            mx,
            my,
            *direction,
            *align,
            *justify,
            *gap,
            children,
        ),
    }
}

/// Solves a leaf node: resolves `SizeSpec` against constraints + intrinsic size.
fn solve_leaf(
    layout_box: &LayoutBox,
    constraints: &LayoutConstraints,
    pos_x: f32,
    pos_y: f32,
) -> LayoutNode {
    let BoxContent::Leaf {
        intrinsic_width,
        intrinsic_height,
    } = &layout_box.content
    else {
        debug_assert!(false, "solve_leaf called on non-leaf");
        return LayoutNode::new(Rect::default(), Rect::default());
    };
    let (iw, ih) = (*intrinsic_width, *intrinsic_height);

    let width = resolve_size(
        layout_box.width,
        constraints.max_width,
        iw + layout_box.padding.width(),
    );
    let height = resolve_size(
        layout_box.height,
        constraints.max_height,
        ih + layout_box.padding.height(),
    );
    let width = constraints.constrain_width(width);
    let height = constraints.constrain_height(height);

    let rect = Rect::new(pos_x, pos_y, width, height);
    let content_rect = rect.inset(layout_box.padding);
    let mut node = LayoutNode::new(rect, content_rect);
    node.widget_id = layout_box.widget_id;
    node
}

/// Resolves a `SizeSpec` to a concrete pixel value.
fn resolve_size(spec: SizeSpec, available: f32, intrinsic: f32) -> f32 {
    match spec {
        SizeSpec::Fixed(val) => val,
        SizeSpec::Fill | SizeSpec::FillPortion(_) => {
            if available.is_finite() {
                available
            } else {
                intrinsic
            }
        }
        SizeSpec::Hug => intrinsic,
    }
}

/// Returns the main-axis `SizeSpec` for a box in the given direction.
fn main_axis_spec(layout_box: &LayoutBox, dir: Direction) -> SizeSpec {
    match dir {
        Direction::Row => layout_box.width,
        Direction::Column => layout_box.height,
    }
}

/// Solves a flex container using a two-pass algorithm.
#[expect(clippy::too_many_arguments, reason = "extracted flex params from enum")]
fn solve_flex(
    layout_box: &LayoutBox,
    constraints: &LayoutConstraints,
    pos_x: f32,
    pos_y: f32,
    dir: Direction,
    align: Align,
    justify: Justify,
    gap: f32,
    children: &[LayoutBox],
) -> LayoutNode {
    if children.is_empty() {
        return solve_empty(layout_box, constraints, pos_x, pos_y);
    }

    let pad_main = dir.main_insets(layout_box.padding);
    let pad_cross = dir.cross_insets(layout_box.padding);
    let avail_main = dir.main(constraints.max_width, constraints.max_height);
    let avail_cross = dir.cross(constraints.max_width, constraints.max_height);

    // Content space = available minus padding.
    let content_main = if avail_main.is_finite() {
        avail_main - pad_main
    } else {
        f32::INFINITY
    };
    let content_cross = if avail_cross.is_finite() {
        avail_cross - pad_cross
    } else {
        f32::INFINITY
    };

    let measured = measure_children(children, dir, content_main, content_cross, gap);

    // Resolve container's own size.
    let container_main = resolve_container_main(
        layout_box,
        dir,
        constraints,
        measured.children_main + pad_main,
    );
    let container_cross =
        resolve_container_cross(layout_box, dir, constraints, measured.max_cross + pad_cross);

    // Pass 2: Position children.
    arrange_children(
        layout_box,
        pos_x,
        pos_y,
        dir,
        align,
        justify,
        gap,
        children,
        &measured.child_mains,
        measured.children_main,
        container_main,
        container_cross,
        pad_main,
        pad_cross,
    )
}

/// Results from the measurement pass.
struct MeasureResult {
    /// Main-axis size for each child.
    child_mains: Vec<f32>,
    /// Total main-axis extent of all children including gaps.
    children_main: f32,
    /// Maximum cross-axis extent among children.
    max_cross: f32,
}

/// Pass 1: Measures non-fill children and distributes space to fill children.
fn measure_children(
    children: &[LayoutBox],
    dir: Direction,
    content_main: f32,
    content_cross: f32,
    gap: f32,
) -> MeasureResult {
    let total_gap = if children.len() > 1 {
        gap * (children.len() - 1) as f32
    } else {
        0.0
    };

    let mut child_mains = vec![0.0_f32; children.len()];
    let mut used_main = total_gap;
    let mut total_fill: u32 = 0;
    let mut max_cross: f32 = 0.0;

    for (idx, child) in children.iter().enumerate() {
        let spec = main_axis_spec(child, dir);
        if spec.is_fill() {
            total_fill += spec.fill_weight();
        } else {
            let child_avail = if content_main.is_finite() {
                content_main - used_main
            } else {
                f32::INFINITY
            };
            let (cw, ch) = dir.compose(child_avail.max(0.0), content_cross);
            let measured = solve(child, LayoutConstraints::loose(cw, ch), 0.0, 0.0);
            let main_size = dir.main(measured.rect.width(), measured.rect.height());
            let cross_size = dir.cross(measured.rect.width(), measured.rect.height());
            child_mains[idx] = main_size;
            used_main += main_size;
            max_cross = max_cross.max(cross_size);
        }
    }

    // Distribute remaining space to fill children.
    if total_fill > 0 {
        let remaining = if content_main.is_finite() {
            (content_main - used_main).max(0.0)
        } else {
            0.0
        };
        let per_unit = remaining / total_fill as f32;
        for (idx, child) in children.iter().enumerate() {
            let spec = main_axis_spec(child, dir);
            if spec.is_fill() {
                child_mains[idx] = per_unit * spec.fill_weight() as f32;
            }
        }
    }

    let children_main: f32 = child_mains.iter().sum::<f32>() + total_gap;

    MeasureResult {
        child_mains,
        children_main,
        max_cross,
    }
}

/// Pass 2: Positions children with justification and alignment.
#[expect(clippy::too_many_arguments, reason = "pass-2 needs all layout context")]
fn arrange_children(
    layout_box: &LayoutBox,
    pos_x: f32,
    pos_y: f32,
    dir: Direction,
    align: Align,
    justify: Justify,
    gap: f32,
    children: &[LayoutBox],
    child_mains: &[f32],
    children_main: f32,
    container_main: f32,
    container_cross: f32,
    pad_main: f32,
    pad_cross: f32,
) -> LayoutNode {
    let (start_offset, between) = compute_justification(
        justify,
        container_main - pad_main,
        children_main,
        children.len(),
    );

    let pad_main_start = dir.main_start(layout_box.padding);
    let pad_cross_start = dir.cross_start(layout_box.padding);
    let mut cursor = pad_main_start + start_offset;
    let child_cross_avail = container_cross - pad_cross;

    let mut child_nodes = Vec::with_capacity(children.len());

    for (idx, child) in children.iter().enumerate() {
        let child_main = child_mains[idx];

        // Solve child at cross-axis start position.
        let (cw, ch) = dir.compose(child_main, child_cross_avail);
        let child_constraints = LayoutConstraints::loose(cw, ch);
        let (cx, cy) = dir.compose(cursor, pad_cross_start);
        let mut node = solve(child, child_constraints, pos_x + cx, pos_y + cy);

        // Compute alignment offset using actual solved dimensions.
        let actual_cross = dir.cross(node.rect.width(), node.rect.height());
        let cross_offset = match align {
            Align::Start | Align::Stretch => 0.0,
            Align::Center => (child_cross_avail - actual_cross) / 2.0,
            Align::End => child_cross_avail - actual_cross,
        };
        if cross_offset.abs() > f32::EPSILON {
            offset_node_cross(&mut node, dir, cross_offset);
        }

        child_nodes.push(node);

        cursor += child_main + gap + between;
    }

    let (width, height) = dir.compose(container_main, container_cross);
    let rect = Rect::new(pos_x, pos_y, width, height);
    let content_rect = rect.inset(layout_box.padding);
    let mut node = LayoutNode::new(rect, content_rect).with_children(child_nodes);
    node.widget_id = layout_box.widget_id;
    node
}

/// Solves an empty flex container.
fn solve_empty(
    layout_box: &LayoutBox,
    constraints: &LayoutConstraints,
    pos_x: f32,
    pos_y: f32,
) -> LayoutNode {
    let width = resolve_size(
        layout_box.width,
        constraints.max_width,
        layout_box.padding.width(),
    );
    let height = resolve_size(
        layout_box.height,
        constraints.max_height,
        layout_box.padding.height(),
    );
    let width = constraints.constrain_width(width);
    let height = constraints.constrain_height(height);
    let rect = Rect::new(pos_x, pos_y, width, height);
    let content_rect = rect.inset(layout_box.padding);
    let mut node = LayoutNode::new(rect, content_rect);
    node.widget_id = layout_box.widget_id;
    node
}

/// Resolves the container's main-axis size.
fn resolve_container_main(
    layout_box: &LayoutBox,
    dir: Direction,
    constraints: &LayoutConstraints,
    children_with_padding: f32,
) -> f32 {
    let spec = main_axis_spec(layout_box, dir);
    let avail = dir.main(constraints.max_width, constraints.max_height);
    let raw = resolve_size(spec, avail, children_with_padding);
    match dir {
        Direction::Row => constraints.constrain_width(raw),
        Direction::Column => constraints.constrain_height(raw),
    }
}

/// Resolves the container's cross-axis size.
fn resolve_container_cross(
    layout_box: &LayoutBox,
    dir: Direction,
    constraints: &LayoutConstraints,
    content_with_padding: f32,
) -> f32 {
    let spec = match dir {
        Direction::Row => layout_box.height,
        Direction::Column => layout_box.width,
    };
    let avail = dir.cross(constraints.max_width, constraints.max_height);
    let raw = resolve_size(spec, avail, content_with_padding);
    match dir {
        Direction::Row => constraints.constrain_height(raw),
        Direction::Column => constraints.constrain_width(raw),
    }
}

/// Computes start offset and extra between-child spacing for justification.
fn compute_justification(justify: Justify, available: f32, used: f32, count: usize) -> (f32, f32) {
    let free = (available - used).max(0.0);
    match justify {
        Justify::Start => (0.0, 0.0),
        Justify::Center => (free / 2.0, 0.0),
        Justify::End => (free, 0.0),
        Justify::SpaceBetween => {
            if count <= 1 {
                (0.0, 0.0)
            } else {
                (0.0, free / (count - 1) as f32)
            }
        }
        Justify::SpaceAround => {
            if count == 0 {
                (0.0, 0.0)
            } else {
                let per = free / count as f32;
                (per / 2.0, per)
            }
        }
    }
}

/// Offsets a solved node and all descendants along the cross axis.
fn offset_node_cross(node: &mut LayoutNode, dir: Direction, delta: f32) {
    let (dx, dy) = match dir {
        Direction::Row => (0.0, delta),
        Direction::Column => (delta, 0.0),
    };
    node.rect = node.rect.offset(dx, dy);
    node.content_rect = node.content_rect.offset(dx, dy);
    for child in &mut node.children {
        offset_node_cross(child, dir, delta);
    }
}
