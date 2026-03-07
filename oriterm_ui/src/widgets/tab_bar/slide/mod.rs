//! Compositor-driven tab sliding animations.
//!
//! [`TabSlideState`] manages ephemeral `Group` layers that carry `Transform2D`
//! translations for tab close and reorder animations. Each animated tab gets a
//! layer whose transform is interpolated from an initial offset to `identity()`
//! by the [`LayerAnimator`]. Zero overhead when idle — no layers exist.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::animation::Easing;
use crate::compositor::layer::{LayerProperties, LayerType};
use crate::compositor::layer_animator::{AnimationParams, LayerAnimator};
use crate::compositor::layer_tree::LayerTree;
use crate::geometry::{LayerId, Transform2D};

use super::TabBarWidget;

/// Compute slide duration proportional to pixel distance.
///
/// Base: 80ms. Scales up to 200ms for large distances (5+ tab widths).
/// Clamped to [80ms, 200ms] range.
fn slide_duration(distance_px: f32, tab_width: f32) -> Duration {
    let slots = (distance_px.abs() / tab_width).max(1.0);
    let ms = 80.0 + slots * 25.0;
    Duration::from_millis(ms.clamp(80.0, 200.0) as u64)
}

/// Bundles the compositor state needed to start a slide animation.
pub struct SlideContext<'a> {
    /// Layer tree to create ephemeral group layers in.
    pub tree: &'a mut LayerTree,
    /// Animator that drives the transform interpolation.
    pub animator: &'a mut LayerAnimator,
    /// Current frame timestamp.
    pub now: Instant,
}

/// Manages compositor layers for tab slide animations.
///
/// Each entry maps a tab index to its animation `LayerId`. Layers are
/// ephemeral `Group` nodes — no rendering, just property containers for
/// `Transform2D` interpolation. Created on slide start, removed when the
/// animation completes.
pub struct TabSlideState {
    /// Active animations: tab index → layer ID.
    active: HashMap<usize, LayerId>,
    /// Reusable buffer for syncing offsets to the widget.
    offset_buf: Vec<f32>,
}

impl TabSlideState {
    /// Creates a new idle slide state.
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
            offset_buf: Vec::new(),
        }
    }

    /// Returns `true` if any slide animations are active.
    pub fn has_active(&self) -> bool {
        !self.active.is_empty()
    }

    /// Starts a close-slide animation.
    ///
    /// When a tab at `closed_idx` is closed, tabs to its right slide left
    /// by one `tab_width`. Creates `Group` layers for indices
    /// `closed_idx..tab_count` with `translate(tab_width, 0)`, animated
    /// to `identity()`.
    pub fn start_close_slide(
        &mut self,
        closed_idx: usize,
        tab_width: f32,
        tab_count: usize,
        cx: &mut SlideContext<'_>,
    ) {
        // Cancel any existing animations first.
        self.cancel_all(cx.tree, cx.animator);

        // Tabs at closed_idx..tab_count need to slide left from +tab_width.
        let duration = slide_duration(tab_width, tab_width);
        self.create_slide_layers(closed_idx..tab_count, tab_width, duration, cx);
    }

    /// Starts a reorder-slide animation.
    ///
    /// When a tab moves from `from` to `to`, displaced tabs slide to make
    /// room. If `from < to`: indices `from..to` get `+tab_width`
    /// (slide right). If `from > to`: indices `to+1..=from` get
    /// `-tab_width` (slide left). All animate to `identity()`.
    pub fn start_reorder_slide(
        &mut self,
        from: usize,
        to: usize,
        tab_width: f32,
        cx: &mut SlideContext<'_>,
    ) {
        if from == to {
            return;
        }

        // Cancel any existing animations first.
        self.cancel_all(cx.tree, cx.animator);

        let (range, offset) = if from < to {
            (from..to, tab_width)
        } else {
            (to + 1..from + 1, -tab_width)
        };

        let distance = (to as f32 - from as f32).abs() * tab_width;
        let duration = slide_duration(distance, tab_width);
        self.create_slide_layers(range, offset, duration, cx);
    }

    /// Removes finished animation layers from the tree.
    ///
    /// Retains only layers where the animator is still interpolating the
    /// `Transform` property.
    pub fn cleanup(&mut self, tree: &mut LayerTree, animator: &LayerAnimator) {
        use crate::animation::AnimatableProperty;
        self.active.retain(|_idx, layer_id| {
            if animator.is_animating(*layer_id, AnimatableProperty::Transform) {
                true
            } else {
                tree.remove(*layer_id);
                false
            }
        });
    }

    /// Syncs compositor transform offsets to the tab bar widget.
    ///
    /// Reads the current `translation_x()` from each active layer's
    /// transform and populates the widget's animation offsets via
    /// buffer swap.
    pub fn sync_to_widget(
        &mut self,
        tab_count: usize,
        tree: &LayerTree,
        widget: &mut TabBarWidget,
    ) {
        self.offset_buf.clear();
        self.offset_buf.resize(tab_count, 0.0);

        for (&idx, &layer_id) in &self.active {
            if idx < tab_count {
                if let Some(layer) = tree.get(layer_id) {
                    self.offset_buf[idx] = layer.properties().transform.translation_x();
                }
            }
        }

        widget.swap_anim_offsets(&mut self.offset_buf);
    }

    /// Cancels all active animations and removes their layers.
    pub fn cancel_all(&mut self, tree: &mut LayerTree, animator: &mut LayerAnimator) {
        for (_idx, layer_id) in self.active.drain() {
            animator.cancel_all(layer_id);
            tree.remove(layer_id);
        }
    }

    // Private helpers

    /// Creates `Group` layers for each index in `range` with `translate(offset, 0)`,
    /// each animated to `identity()`.
    fn create_slide_layers(
        &mut self,
        range: std::ops::Range<usize>,
        offset: f32,
        duration: Duration,
        cx: &mut SlideContext<'_>,
    ) {
        let root = cx.tree.root();
        for idx in range {
            let layer_id = cx.tree.add(
                root,
                LayerType::Group,
                LayerProperties {
                    transform: Transform2D::translate(offset, 0.0),
                    ..LayerProperties::default()
                },
            );
            let params = AnimationParams {
                duration,
                easing: Easing::EaseOut,
                tree: cx.tree,
                now: cx.now,
            };
            cx.animator
                .animate_transform(layer_id, Transform2D::identity(), &params);
            self.active.insert(idx, layer_id);
        }
    }
}

impl Default for TabSlideState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
