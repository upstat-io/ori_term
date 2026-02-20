//! Widget-level hit testing on a layout tree.
//!
//! Walks a [`LayoutNode`] tree back-to-front (last child = frontmost) and
//! returns the deepest widget whose rect contains the test point. This is
//! the standard approach used by Chromium's `WindowTargeter` and Druid's
//! `WidgetPod`.

use crate::geometry::{Point, Rect};
use crate::layout::LayoutNode;
use crate::widget_id::WidgetId;

/// Finds the deepest widget under `point` in a layout tree.
///
/// Traversal is back-to-front: the last child in the children list is
/// considered frontmost (painter's algorithm). The first hit in reverse
/// order wins because it is visually on top.
///
/// Returns `None` if no widget with a `widget_id` contains the point.
pub fn layout_hit_test(root: &LayoutNode, point: Point) -> Option<WidgetId> {
    hit_test_node(root, point, None)
}

/// Finds the deepest widget under `point`, respecting a clip rectangle.
///
/// Widgets outside the clip rect are not hittable. Pass `None` for no clip.
pub fn layout_hit_test_clipped(
    root: &LayoutNode,
    point: Point,
    clip: Option<Rect>,
) -> Option<WidgetId> {
    hit_test_node(root, point, clip)
}

/// Recursive hit test on a single node.
///
/// Returns the deepest `WidgetId` whose rect contains `point`, or `None`.
fn hit_test_node(node: &LayoutNode, point: Point, clip: Option<Rect>) -> Option<WidgetId> {
    // Early out: point outside this node's rect.
    if !node.rect.contains(point) {
        return None;
    }

    // Early out: point outside clip rect.
    if let Some(clip) = clip {
        if !clip.contains(point) {
            return None;
        }
    }

    // Walk children back-to-front (last child = frontmost).
    for child in node.children.iter().rev() {
        if let Some(id) = hit_test_node(child, point, clip) {
            return Some(id);
        }
    }

    // No child claimed it — return this node's widget_id (if any).
    node.widget_id
}
