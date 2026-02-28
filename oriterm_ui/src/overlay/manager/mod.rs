//! Overlay manager — lifecycle, event routing, and drawing for floating layers.
//!
//! Sits alongside the widget tree (not inside it). The application layer calls
//! into the manager at specific frame-loop points: events before the main tree,
//! layout after the main tree, drawing after the main tree.

mod event_routing;

use std::time::{Duration, Instant};

use crate::animation::Easing;
use crate::color::Color;
use crate::compositor::layer::{LayerId, LayerProperties, LayerType};
use crate::compositor::layer_animator::{AnimationParams, LayerAnimator};
use crate::compositor::layer_tree::LayerTree;
use crate::draw::RectStyle;
use crate::geometry::{Rect, Size};
use crate::layout::compute_layout;
use crate::theme::UiTheme;
use crate::widget_id::WidgetId;
use crate::widgets::{DrawCtx, LayoutCtx, Widget, WidgetResponse};

use super::overlay_id::OverlayId;
use super::placement::{Placement, compute_overlay_rect};

/// Semi-transparent black for modal dimming.
const MODAL_DIM_COLOR: Color = Color::rgba(0.0, 0.0, 0.0, 0.5);

/// Duration for overlay fade-in and fade-out animations.
const FADE_DURATION: Duration = Duration::from_millis(150);

/// Discriminates overlay behavior: popup vs. modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::overlay) enum OverlayKind {
    /// Non-modal popup — dismissed on click outside.
    Popup,
    /// Modal dialog — blocks interaction below, not dismissable by click outside.
    Modal,
}

/// A floating overlay containing a widget.
pub(in crate::overlay) struct Overlay {
    /// Unique identifier for this overlay.
    pub(in crate::overlay) id: OverlayId,
    /// The widget displayed in this overlay.
    pub(in crate::overlay) widget: Box<dyn Widget>,
    /// Anchor rectangle for placement computation.
    pub(in crate::overlay) anchor: Rect,
    /// Placement strategy relative to anchor.
    pub(in crate::overlay) placement: Placement,
    /// Popup vs. modal behavior.
    pub(in crate::overlay) kind: OverlayKind,
    /// Computed screen-space rectangle (set by `layout_overlays`).
    pub(in crate::overlay) computed_rect: Rect,

    // Compositor integration.
    /// Compositor layer for this overlay's content.
    pub(in crate::overlay) layer_id: LayerId,
    /// Compositor layer for modal dimming (modals only).
    pub(in crate::overlay) dim_layer_id: Option<LayerId>,
}

/// Result of routing an event through the overlay stack.
#[derive(Debug)]
pub enum OverlayEventResult {
    /// Event was delivered to an overlay widget.
    Delivered {
        /// Which overlay received the event.
        overlay_id: OverlayId,
        /// The widget's response.
        response: WidgetResponse,
    },
    /// A click outside dismissed the topmost overlay.
    Dismissed(OverlayId),
    /// A modal overlay blocked the event (consumed without delivery).
    Blocked,
    /// No overlay intercepted the event — deliver to the main widget tree.
    PassThrough,
}

/// Manages a stack of floating overlays above the main widget tree.
///
/// Overlays are ordered back-to-front: the last overlay in the stack is
/// topmost (drawn last, receives events first).
pub struct OverlayManager {
    pub(in crate::overlay) overlays: Vec<Overlay>,
    /// Overlays being faded out — still drawn, but excluded from event routing.
    pub(in crate::overlay) dismissing: Vec<Overlay>,
    pub(in crate::overlay) viewport: Rect,
    /// Index of the overlay currently under the cursor.
    ///
    /// Tracked across `process_hover_event` calls so we can send
    /// `HoverEvent::Leave` to the old overlay when hover transitions.
    pub(in crate::overlay) hovered_overlay: Option<usize>,
}

impl OverlayManager {
    // Constructors

    /// Creates a new overlay manager with the given viewport bounds.
    pub fn new(viewport: Rect) -> Self {
        Self {
            overlays: Vec::new(),
            dismissing: Vec::new(),
            viewport,
            hovered_overlay: None,
        }
    }

    // Accessors

    /// Updates the viewport bounds (e.g. on window resize).
    pub fn set_viewport(&mut self, viewport: Rect) {
        self.viewport = viewport;
    }

    /// Returns the current viewport.
    pub fn viewport(&self) -> Rect {
        self.viewport
    }

    // Predicates

    /// Returns `true` if no overlays are active or dismissing.
    pub fn is_empty(&self) -> bool {
        self.overlays.is_empty() && self.dismissing.is_empty()
    }

    /// Returns `true` if no overlays are active (excludes dismissing).
    pub fn is_active_empty(&self) -> bool {
        self.overlays.is_empty()
    }

    /// Returns the number of active overlays.
    pub fn count(&self) -> usize {
        self.overlays.len()
    }

    /// Returns `true` if the topmost overlay is modal.
    pub fn has_modal(&self) -> bool {
        self.overlays
            .last()
            .is_some_and(|o| o.kind == OverlayKind::Modal)
    }

    /// Returns the computed screen-space rectangle for an overlay.
    ///
    /// Returns `None` if the ID is not found. The rect is only valid
    /// after calling [`layout_overlays`](Self::layout_overlays).
    pub fn overlay_rect(&self, id: OverlayId) -> Option<Rect> {
        self.overlays
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.computed_rect)
    }

    // Lifecycle API

    /// Pushes a non-modal overlay that dismisses on click-outside.
    ///
    /// Creates a `Textured` compositor layer and starts a fade-in animation
    /// (opacity `0→1`, 150ms `EaseOut`).
    #[expect(
        clippy::too_many_arguments,
        reason = "lifecycle: widget, anchor, placement, tree, animator, now"
    )]
    pub fn push_overlay(
        &mut self,
        widget: Box<dyn Widget>,
        anchor: Rect,
        placement: Placement,
        tree: &mut LayerTree,
        animator: &mut LayerAnimator,
        now: Instant,
    ) -> OverlayId {
        let id = OverlayId::next();
        let root = tree.root();

        let layer_id = tree.add(
            root,
            LayerType::Textured,
            LayerProperties {
                opacity: 0.0,
                ..LayerProperties::default()
            },
        );

        let params = AnimationParams {
            duration: FADE_DURATION,
            easing: Easing::EaseOut,
            tree,
            now,
        };
        animator.animate_opacity(layer_id, 1.0, &params);

        self.overlays.push(Overlay {
            id,
            widget,
            anchor,
            placement,
            kind: OverlayKind::Popup,
            computed_rect: Rect::default(),
            layer_id,
            dim_layer_id: None,
        });
        id
    }

    /// Pushes a modal overlay (blocks interaction below, no click-outside dismiss).
    ///
    /// Creates a `SolidColor` dim layer and a `Textured` content layer,
    /// both with fade-in animations (opacity `0→1`, 150ms `EaseOut`).
    #[expect(
        clippy::too_many_arguments,
        reason = "lifecycle: widget, anchor, placement, tree, animator, now"
    )]
    pub fn push_modal(
        &mut self,
        widget: Box<dyn Widget>,
        anchor: Rect,
        placement: Placement,
        tree: &mut LayerTree,
        animator: &mut LayerAnimator,
        now: Instant,
    ) -> OverlayId {
        let id = OverlayId::next();
        let root = tree.root();

        // Dim layer (SolidColor) — drawn behind content.
        let dim_layer_id = tree.add(
            root,
            LayerType::SolidColor(MODAL_DIM_COLOR),
            LayerProperties {
                bounds: self.viewport,
                opacity: 0.0,
                ..LayerProperties::default()
            },
        );

        // Content layer (Textured).
        let layer_id = tree.add(
            root,
            LayerType::Textured,
            LayerProperties {
                opacity: 0.0,
                ..LayerProperties::default()
            },
        );

        // Animate both layers opacity 0→1.
        let params = AnimationParams {
            duration: FADE_DURATION,
            easing: Easing::EaseOut,
            tree,
            now,
        };
        animator.animate_opacity(dim_layer_id, 1.0, &params);
        animator.animate_opacity(layer_id, 1.0, &params);

        self.overlays.push(Overlay {
            id,
            widget,
            anchor,
            placement,
            kind: OverlayKind::Modal,
            computed_rect: Rect::default(),
            layer_id,
            dim_layer_id: Some(dim_layer_id),
        });
        id
    }

    /// Begins dismissing a specific overlay by ID with a fade-out animation.
    ///
    /// The overlay is moved from the active stack to the dismissing list
    /// and becomes invisible to event routing. Returns `true` if found.
    pub fn begin_dismiss(
        &mut self,
        id: OverlayId,
        tree: &LayerTree,
        animator: &mut LayerAnimator,
        now: Instant,
    ) -> bool {
        let Some(idx) = self.overlays.iter().position(|o| o.id == id) else {
            return false;
        };
        let overlay = self.overlays.remove(idx);
        Self::start_fade_out(&overlay, tree, animator, now);
        self.dismissing.push(overlay);
        self.hovered_overlay = None;
        true
    }

    /// Begins dismissing the topmost overlay with a fade-out animation.
    ///
    /// Returns the dismissed overlay's ID, or `None` if the stack is empty.
    pub fn begin_dismiss_topmost(
        &mut self,
        tree: &LayerTree,
        animator: &mut LayerAnimator,
        now: Instant,
    ) -> Option<OverlayId> {
        let overlay = self.overlays.pop()?;
        let id = overlay.id;
        Self::start_fade_out(&overlay, tree, animator, now);
        self.dismissing.push(overlay);
        self.hovered_overlay = None;
        Some(id)
    }

    /// Removes all overlays instantly, canceling any running animations.
    pub fn clear_all(&mut self, tree: &mut LayerTree, animator: &mut LayerAnimator) {
        for overlay in self.overlays.drain(..).chain(self.dismissing.drain(..)) {
            animator.cancel_all(overlay.layer_id);
            tree.remove_subtree(overlay.layer_id);
            if let Some(dim_id) = overlay.dim_layer_id {
                animator.cancel_all(dim_id);
                tree.remove_subtree(dim_id);
            }
        }
        self.hovered_overlay = None;
    }

    /// Removes dismissing overlays whose fade-out animations have completed.
    ///
    /// Call after [`LayerAnimator::tick`] each frame. Removes compositor layers
    /// for fully faded overlays.
    pub fn cleanup_dismissed(&mut self, tree: &mut LayerTree, animator: &LayerAnimator) {
        self.dismissing.retain(|overlay| {
            let still_fading = animator.is_animating(
                overlay.layer_id,
                crate::animation::AnimatableProperty::Opacity,
            );
            if !still_fading {
                tree.remove_subtree(overlay.layer_id);
                if let Some(dim_id) = overlay.dim_layer_id {
                    tree.remove_subtree(dim_id);
                }
            }
            still_fading
        });
    }

    // Frame-loop API

    /// Computes layout for all overlays (active + dismissing).
    ///
    /// For each overlay: measures the widget's intrinsic size via the layout
    /// solver, then computes the screen-space placement rectangle.
    pub fn layout_overlays(
        &mut self,
        measurer: &dyn crate::widgets::TextMeasurer,
        theme: &UiTheme,
    ) {
        let viewport = self.viewport;
        let layout_ctx = LayoutCtx { measurer, theme };

        for overlay in self.overlays.iter_mut().chain(self.dismissing.iter_mut()) {
            let layout_box = overlay.widget.layout(&layout_ctx);
            let unconstrained = Rect::new(0.0, 0.0, f32::INFINITY, f32::INFINITY);
            let node = compute_layout(&layout_box, unconstrained);
            let content_size = Size::new(node.rect.width(), node.rect.height());

            overlay.computed_rect =
                compute_overlay_rect(overlay.anchor, content_size, viewport, overlay.placement);
        }
    }

    /// Returns the total number of overlays to draw (active + dismissing).
    pub fn draw_count(&self) -> usize {
        self.overlays.len() + self.dismissing.len()
    }

    /// Draws a single overlay at `draw_idx` and returns its compositor opacity.
    ///
    /// Indices `0..active_count` draw active overlays; the rest draw dismissing
    /// overlays (still visible during fade-out).
    ///
    /// Modal overlays emit a dimming rectangle (with opacity-adjusted alpha)
    /// before the content. The returned opacity should be passed to the GPU
    /// draw-list converter so all content colors are alpha-multiplied correctly.
    ///
    /// # Panics
    ///
    /// Panics if `draw_idx >= draw_count()`.
    pub fn draw_overlay_at(&self, draw_idx: usize, ctx: &mut DrawCtx<'_>, tree: &LayerTree) -> f32 {
        let overlay = if draw_idx < self.overlays.len() {
            &self.overlays[draw_idx]
        } else {
            &self.dismissing[draw_idx - self.overlays.len()]
        };

        let opacity = tree
            .get(overlay.layer_id)
            .map_or(1.0, |l| l.properties().opacity);

        // Modal dim — apply dim layer's own opacity to the color alpha.
        if overlay.kind == OverlayKind::Modal {
            let dim_opacity = overlay
                .dim_layer_id
                .and_then(|id| tree.get(id))
                .map_or(1.0, |l| l.properties().opacity);
            let dim_color = Color::rgba(
                MODAL_DIM_COLOR.r,
                MODAL_DIM_COLOR.g,
                MODAL_DIM_COLOR.b,
                MODAL_DIM_COLOR.a * dim_opacity,
            );
            ctx.draw_list
                .push_rect(self.viewport, RectStyle::filled(dim_color));
        }

        // Content widget draws at full alpha — the returned opacity is
        // applied by the GPU converter to all emitted instances.
        let mut overlay_ctx = DrawCtx {
            measurer: ctx.measurer,
            draw_list: ctx.draw_list,
            bounds: overlay.computed_rect,
            focused_widget: ctx.focused_widget,
            now: ctx.now,
            animations_running: ctx.animations_running,
            theme: ctx.theme,
        };
        overlay.widget.draw(&mut overlay_ctx);

        opacity
    }

    /// Returns focusable widget IDs from the topmost modal overlay.
    ///
    /// The application layer can use this with `FocusManager::set_focus_order()`
    /// to trap focus within the modal. Returns `None` if there is no modal.
    pub fn modal_focus_order(&self) -> Option<Vec<WidgetId>> {
        let topmost = self.overlays.last()?;
        if topmost.kind != OverlayKind::Modal {
            return None;
        }
        Some(topmost.widget.focusable_children())
    }

    // Private helpers

    /// Starts fade-out animations on an overlay's compositor layers.
    fn start_fade_out(
        overlay: &Overlay,
        tree: &LayerTree,
        animator: &mut LayerAnimator,
        now: Instant,
    ) {
        let params = AnimationParams {
            duration: FADE_DURATION,
            easing: Easing::EaseOut,
            tree,
            now,
        };
        animator.animate_opacity(overlay.layer_id, 0.0, &params);
        if let Some(dim_id) = overlay.dim_layer_id {
            animator.animate_opacity(dim_id, 0.0, &params);
        }
    }
}
