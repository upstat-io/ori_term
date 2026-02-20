//! Computed layout output node.

use crate::geometry::Rect;

/// A computed layout node — the output of the layout solver.
///
/// Each node stores its outer rectangle (including margin offset), the
/// content rectangle (outer rect inset by padding), and child nodes
/// for flex containers.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutNode {
    /// Outer bounding rectangle (position relative to parent's content area).
    pub rect: Rect,
    /// Content area (rect inset by padding).
    pub content_rect: Rect,
    /// Child layout nodes (empty for leaves).
    pub children: Vec<Self>,
}

impl LayoutNode {
    /// Creates a leaf node with no children.
    pub fn new(rect: Rect, content_rect: Rect) -> Self {
        Self {
            rect,
            content_rect,
            children: Vec::new(),
        }
    }

    /// Attaches children to this node.
    #[must_use]
    pub fn with_children(mut self, children: Vec<Self>) -> Self {
        self.children = children;
        self
    }
}
