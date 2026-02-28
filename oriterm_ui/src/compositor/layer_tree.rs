//! Parent-child layer hierarchy with z-ordering.
//!
//! The [`LayerTree`] owns all layers and manages their parent-child
//! relationships, paint order (z-order), and dirty flag tracking.
//! Property mutations go through the tree so dirty flags are set
//! correctly.

use std::collections::HashMap;

use crate::geometry::Rect;

use super::Transform2D;
use super::layer::{Layer, LayerId, LayerProperties, LayerType};

/// A tree of compositor layers.
///
/// Owns all layers in a `HashMap` keyed by `LayerId`. The root is
/// always a `Group` layer covering the viewport. Children within a
/// parent are ordered back-to-front (last child is frontmost).
pub struct LayerTree {
    layers: HashMap<LayerId, Layer>,
    root: LayerId,
    next_id: u64,
}

impl LayerTree {
    // --- Constructors ---

    /// Creates a new tree with a root group layer spanning `viewport`.
    pub fn new(viewport: Rect) -> Self {
        let root_id = LayerId::new(1);
        let root = Layer::new(
            root_id,
            LayerType::Group,
            LayerProperties {
                bounds: viewport,
                ..LayerProperties::default()
            },
        );
        let mut layers = HashMap::new();
        layers.insert(root_id, root);
        Self {
            layers,
            root: root_id,
            next_id: 2,
        }
    }

    // --- Accessors ---

    /// Returns the root layer's ID.
    pub fn root(&self) -> LayerId {
        self.root
    }

    /// Returns a reference to a layer, or `None` if not found.
    pub fn get(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(&id)
    }

    /// Returns a mutable reference to a layer, or `None` if not found.
    pub fn get_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.get_mut(&id)
    }

    /// Returns the number of layers (including root).
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Returns `true` if the tree contains only the root.
    pub fn is_empty(&self) -> bool {
        self.layers.len() <= 1
    }

    // --- Tree mutation ---

    /// Adds a new layer as a child of `parent`. Returns the new layer's ID.
    ///
    /// The new layer is appended to the end of the parent's children
    /// (frontmost in paint order).
    pub fn add(
        &mut self,
        parent: LayerId,
        kind: LayerType,
        properties: LayerProperties,
    ) -> LayerId {
        let id = self.alloc_id();
        let mut layer = Layer::new(id, kind, properties);
        layer.parent = Some(parent);
        self.layers.insert(id, layer);

        if let Some(parent_layer) = self.layers.get_mut(&parent) {
            parent_layer.children.push(id);
        }

        id
    }

    /// Removes a layer, reparenting its children to its parent.
    ///
    /// Returns `true` if the layer existed. Cannot remove the root.
    pub fn remove(&mut self, id: LayerId) -> bool {
        if id == self.root {
            return false;
        }

        let Some(layer) = self.layers.remove(&id) else {
            return false;
        };

        let parent = layer.parent;
        let children = layer.children;

        // Reparent children to the removed layer's parent.
        if let Some(parent_id) = parent {
            for &child_id in &children {
                if let Some(child) = self.layers.get_mut(&child_id) {
                    child.parent = Some(parent_id);
                }
            }

            if let Some(parent_layer) = self.layers.get_mut(&parent_id) {
                // Replace the removed layer with its children in-place.
                if let Some(pos) = parent_layer.children.iter().position(|&c| c == id) {
                    parent_layer.children.splice(pos..=pos, children);
                }
            }
        }

        true
    }

    /// Removes a layer and all its descendants.
    ///
    /// Cannot remove the root.
    pub fn remove_subtree(&mut self, id: LayerId) {
        if id == self.root {
            return;
        }

        // Detach from parent first.
        if let Some(layer) = self.layers.get(&id) {
            let parent = layer.parent;
            if let Some(parent_id) = parent {
                if let Some(parent_layer) = self.layers.get_mut(&parent_id) {
                    parent_layer.children.retain(|&c| c != id);
                }
            }
        }

        // Collect all IDs to remove via DFS.
        let mut to_remove = Vec::new();
        self.collect_subtree(id, &mut to_remove);
        for remove_id in to_remove {
            self.layers.remove(&remove_id);
        }
    }

    /// Moves a layer to a different parent.
    ///
    /// The layer is appended to the end of the new parent's children.
    pub fn reparent(&mut self, id: LayerId, new_parent: LayerId) {
        if id == self.root {
            return;
        }

        // Detach from old parent.
        if let Some(layer) = self.layers.get(&id) {
            let old_parent = layer.parent;
            if let Some(old_parent_id) = old_parent {
                if let Some(old_parent_layer) = self.layers.get_mut(&old_parent_id) {
                    old_parent_layer.children.retain(|&c| c != id);
                }
            }
        }

        // Attach to new parent.
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.parent = Some(new_parent);
        }
        if let Some(parent_layer) = self.layers.get_mut(&new_parent) {
            parent_layer.children.push(id);
        }
    }

    // --- Property setters ---

    /// Sets a layer's opacity, marking it for re-composite.
    pub fn set_opacity(&mut self, id: LayerId, opacity: f32) {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.set_opacity(opacity);
        }
    }

    /// Sets a layer's transform, marking it for re-composite.
    pub fn set_transform(&mut self, id: LayerId, transform: Transform2D) {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.set_transform(transform);
        }
    }

    /// Sets a layer's bounds, marking it for re-composite.
    pub fn set_bounds(&mut self, id: LayerId, bounds: Rect) {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.set_bounds(bounds);
        }
    }

    /// Sets a layer's visibility, marking it for re-composite.
    pub fn set_visible(&mut self, id: LayerId, visible: bool) {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.set_visible(visible);
        }
    }

    /// Marks a layer's content as needing re-render.
    pub fn schedule_paint(&mut self, id: LayerId) {
        if let Some(layer) = self.layers.get_mut(&id) {
            layer.schedule_paint();
        }
    }

    // --- Z-order ---

    /// Moves `id` above `sibling` in their shared parent's child list.
    ///
    /// Both layers must share the same parent. No-op if they don't.
    pub fn stack_above(&mut self, id: LayerId, sibling: LayerId) {
        self.reorder_sibling(id, sibling, true);
    }

    /// Moves `id` below `sibling` in their shared parent's child list.
    ///
    /// Both layers must share the same parent. No-op if they don't.
    pub fn stack_below(&mut self, id: LayerId, sibling: LayerId) {
        self.reorder_sibling(id, sibling, false);
    }

    // --- Traversal ---

    /// Iterates all layers in depth-first back-to-front paint order.
    ///
    /// Children are visited in order (first child = backmost). Each
    /// node is visited after all its descendants.
    pub fn iter_back_to_front(&self) -> Vec<LayerId> {
        let mut result = Vec::with_capacity(self.layers.len());
        self.iter_back_to_front_into(&mut result);
        result
    }

    /// Fills `out` with all layer IDs in depth-first back-to-front order.
    ///
    /// Clears `out` before filling. Callers can reuse the same `Vec` across
    /// frames to avoid per-frame allocation.
    pub fn iter_back_to_front_into(&self, out: &mut Vec<LayerId>) {
        out.clear();
        self.visit_back_to_front(self.root, out);
    }

    /// Computes the accumulated opacity from root to `id`.
    ///
    /// Multiplies opacity values along the ancestor chain.
    pub fn accumulated_opacity(&self, id: LayerId) -> f32 {
        let mut opacity = 1.0_f32;
        let mut current = Some(id);
        while let Some(cid) = current {
            if let Some(layer) = self.layers.get(&cid) {
                opacity *= layer.properties().opacity;
                current = layer.parent();
            } else {
                break;
            }
        }
        opacity
    }

    /// Computes the accumulated transform from root to `id`.
    ///
    /// Concatenates transforms along the ancestor chain (root first).
    /// Walks child→root, prepending each ancestor's transform to produce
    /// the same root-first composition without allocating.
    pub fn accumulated_transform(&self, id: LayerId) -> Transform2D {
        let mut result = Transform2D::identity();
        let mut current = Some(id);
        while let Some(cid) = current {
            if let Some(layer) = self.layers.get(&cid) {
                result = layer.properties().transform.concat(&result);
                current = layer.parent();
            } else {
                break;
            }
        }
        result
    }

    // --- Dirty queries ---

    /// Returns all layer IDs that need their content re-painted.
    pub fn layers_needing_paint(&self) -> Vec<LayerId> {
        let mut out = Vec::new();
        self.layers_needing_paint_into(&mut out);
        out
    }

    /// Fills `out` with layer IDs that need their content re-painted.
    ///
    /// Clears `out` before filling. Callers can reuse the same `Vec` across
    /// frames to avoid per-frame allocation.
    pub fn layers_needing_paint_into(&self, out: &mut Vec<LayerId>) {
        out.clear();
        out.extend(
            self.layers
                .values()
                .filter(|l| l.needs_paint())
                .map(Layer::id),
        );
    }

    /// Returns all layer IDs that need re-compositing.
    pub fn layers_needing_composite(&self) -> Vec<LayerId> {
        let mut out = Vec::new();
        self.layers_needing_composite_into(&mut out);
        out
    }

    /// Fills `out` with layer IDs that need re-compositing.
    ///
    /// Clears `out` before filling. Callers can reuse the same `Vec` across
    /// frames to avoid per-frame allocation.
    pub fn layers_needing_composite_into(&self, out: &mut Vec<LayerId>) {
        out.clear();
        out.extend(
            self.layers
                .values()
                .filter(|l| l.needs_composite())
                .map(Layer::id),
        );
    }

    /// Clears all dirty flags on all layers. Call after each frame.
    pub fn clear_dirty_flags(&mut self) {
        for layer in self.layers.values_mut() {
            layer.clear_dirty_flags();
        }
    }

    // --- Private helpers ---

    /// Allocates a new unique `LayerId`.
    fn alloc_id(&mut self) -> LayerId {
        let id = LayerId::new(self.next_id);
        self.next_id += 1;
        id
    }

    /// Collects all IDs in the subtree rooted at `id` (DFS).
    fn collect_subtree(&self, id: LayerId, out: &mut Vec<LayerId>) {
        out.push(id);
        if let Some(layer) = self.layers.get(&id) {
            for &child in &layer.children {
                self.collect_subtree(child, out);
            }
        }
    }

    /// Depth-first back-to-front traversal.
    fn visit_back_to_front(&self, id: LayerId, out: &mut Vec<LayerId>) {
        if let Some(layer) = self.layers.get(&id) {
            for &child in &layer.children {
                self.visit_back_to_front(child, out);
            }
        }
        out.push(id);
    }

    /// Reorders `id` relative to `sibling`. If `above` is true, `id`
    /// is placed after `sibling`; otherwise before.
    fn reorder_sibling(&mut self, id: LayerId, sibling: LayerId, above: bool) {
        if id == sibling {
            return;
        }

        // Find shared parent.
        let parent_id = match self.layers.get(&id).and_then(Layer::parent) {
            Some(pid) => pid,
            None => return,
        };
        let sib_parent = self.layers.get(&sibling).and_then(Layer::parent);
        if sib_parent != Some(parent_id) {
            return;
        }

        let parent = match self.layers.get_mut(&parent_id) {
            Some(p) => p,
            None => return,
        };

        // Remove `id` from children.
        parent.children.retain(|&c| c != id);

        // Find sibling position and insert relative to it.
        if let Some(sib_pos) = parent.children.iter().position(|&c| c == sibling) {
            let insert_pos = if above { sib_pos + 1 } else { sib_pos };
            parent.children.insert(insert_pos, id);
        }
    }
}
