//! Overlay manager — lifecycle, event routing, and drawing for floating layers.
//!
//! Sits alongside the widget tree (not inside it). The application layer calls
//! into the manager at specific frame-loop points: events before the main tree,
//! layout after the main tree, drawing after the main tree.

use crate::color::Color;
use crate::draw::{DrawList, RectStyle};
use crate::geometry::{Point, Rect, Size};
use crate::input::{HoverEvent, Key, KeyEvent, MouseEvent, MouseEventKind};
use crate::layout::compute_layout;
use crate::widget_id::WidgetId;
use crate::widgets::{DrawCtx, EventCtx, LayoutCtx, Widget, WidgetResponse};

use super::overlay_id::OverlayId;
use super::placement::{Placement, compute_overlay_rect};

/// Semi-transparent black for modal dimming.
const MODAL_DIM_COLOR: Color = Color::rgba(0.0, 0.0, 0.0, 0.5);

/// Discriminates overlay behavior: popup vs. modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OverlayKind {
    /// Non-modal popup — dismissed on click outside.
    Popup,
    /// Modal dialog — blocks interaction below, not dismissable by click outside.
    Modal,
}

/// A floating overlay containing a widget.
pub(super) struct Overlay {
    /// Unique identifier for this overlay.
    pub(super) id: OverlayId,
    /// The widget displayed in this overlay.
    pub(super) widget: Box<dyn Widget>,
    /// Anchor rectangle for placement computation.
    pub(super) anchor: Rect,
    /// Placement strategy relative to anchor.
    pub(super) placement: Placement,
    /// Popup vs. modal behavior.
    pub(super) kind: OverlayKind,
    /// Computed screen-space rectangle (set by `layout_overlays`).
    pub(super) computed_rect: Rect,
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
    overlays: Vec<Overlay>,
    viewport: Rect,
}

impl OverlayManager {
    /// Creates a new overlay manager with the given viewport bounds.
    pub fn new(viewport: Rect) -> Self {
        Self {
            overlays: Vec::new(),
            viewport,
        }
    }

    /// Updates the viewport bounds (e.g. on window resize).
    pub fn set_viewport(&mut self, viewport: Rect) {
        self.viewport = viewport;
    }

    /// Returns the current viewport.
    pub fn viewport(&self) -> Rect {
        self.viewport
    }

    /// Returns `true` if no overlays are active.
    pub fn is_empty(&self) -> bool {
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
    pub fn push_overlay(
        &mut self,
        widget: Box<dyn Widget>,
        anchor: Rect,
        placement: Placement,
    ) -> OverlayId {
        let id = OverlayId::next();
        self.overlays.push(Overlay {
            id,
            widget,
            anchor,
            placement,
            kind: OverlayKind::Popup,
            computed_rect: Rect::default(),
        });
        id
    }

    /// Pushes a modal overlay (blocks interaction below, no click-outside dismiss).
    pub fn push_modal(
        &mut self,
        widget: Box<dyn Widget>,
        anchor: Rect,
        placement: Placement,
    ) -> OverlayId {
        let id = OverlayId::next();
        self.overlays.push(Overlay {
            id,
            widget,
            anchor,
            placement,
            kind: OverlayKind::Modal,
            computed_rect: Rect::default(),
        });
        id
    }

    /// Removes a specific overlay by ID. Returns `true` if found.
    pub fn pop_overlay(&mut self, id: OverlayId) -> bool {
        if let Some(idx) = self.overlays.iter().position(|o| o.id == id) {
            self.overlays.remove(idx);
            true
        } else {
            false
        }
    }

    /// Removes the topmost overlay and returns its ID.
    pub fn pop_topmost(&mut self) -> Option<OverlayId> {
        self.overlays.pop().map(|o| o.id)
    }

    /// Removes all overlays.
    pub fn clear_all(&mut self) {
        self.overlays.clear();
    }

    // Frame-loop API

    /// Computes layout for all overlays.
    ///
    /// For each overlay: measures the widget's intrinsic size via the layout
    /// solver, then computes the screen-space placement rectangle.
    pub fn layout_overlays(&mut self, measurer: &dyn crate::widgets::TextMeasurer) {
        let viewport = self.viewport;
        let layout_ctx = LayoutCtx { measurer };

        for overlay in &mut self.overlays {
            let layout_box = overlay.widget.layout(&layout_ctx);
            let unconstrained = Rect::new(0.0, 0.0, f32::INFINITY, f32::INFINITY);
            let node = compute_layout(&layout_box, unconstrained);
            let content_size = Size::new(node.rect.width(), node.rect.height());

            overlay.computed_rect =
                compute_overlay_rect(overlay.anchor, content_size, viewport, overlay.placement);
        }
    }

    /// Draws all overlays in back-to-front order.
    ///
    /// Modal overlays emit a dimming rectangle covering the viewport before
    /// drawing the overlay content.
    pub fn draw_overlays(
        &self,
        draw_list: &mut DrawList,
        measurer: &dyn crate::widgets::TextMeasurer,
        focused_widget: Option<WidgetId>,
    ) {
        for overlay in &self.overlays {
            if overlay.kind == OverlayKind::Modal {
                draw_list.push_rect(self.viewport, RectStyle::filled(MODAL_DIM_COLOR));
            }

            let mut ctx = DrawCtx {
                measurer,
                draw_list,
                bounds: overlay.computed_rect,
                focused_widget,
            };
            overlay.widget.draw(&mut ctx);
        }
    }

    /// Routes a mouse event through the overlay stack.
    ///
    /// Hit-tests overlays back-to-front (topmost first). See
    /// [`OverlayEventResult`] for the routing rules.
    pub fn process_mouse_event(
        &mut self,
        event: &MouseEvent,
        measurer: &dyn crate::widgets::TextMeasurer,
    ) -> OverlayEventResult {
        if self.overlays.is_empty() {
            return OverlayEventResult::PassThrough;
        }

        // Hit test from topmost to bottom.
        for i in (0..self.overlays.len()).rev() {
            if self.overlays[i].computed_rect.contains(event.pos) {
                let overlay = &mut self.overlays[i];
                let id = overlay.id;
                let ctx = EventCtx {
                    measurer,
                    bounds: overlay.computed_rect,
                    is_focused: false,
                };
                let response = overlay.widget.handle_mouse(event, &ctx);
                return OverlayEventResult::Delivered {
                    overlay_id: id,
                    response,
                };
            }
        }

        // Click is outside all overlays — check topmost overlay's policy.
        let topmost = self.overlays.last().expect("checked non-empty above");
        let topmost_id = topmost.id;

        match topmost.kind {
            OverlayKind::Modal => OverlayEventResult::Blocked,
            OverlayKind::Popup => {
                // Only dismiss on actual clicks (Down), not moves/scrolls.
                if matches!(event.kind, MouseEventKind::Down(_)) {
                    self.overlays.pop();
                    OverlayEventResult::Dismissed(topmost_id)
                } else {
                    OverlayEventResult::PassThrough
                }
            }
        }
    }

    /// Routes a key event through the overlay stack.
    ///
    /// Escape dismisses the topmost overlay. Modal overlays never pass through.
    pub fn process_key_event(
        &mut self,
        event: KeyEvent,
        measurer: &dyn crate::widgets::TextMeasurer,
    ) -> OverlayEventResult {
        if self.overlays.is_empty() {
            return OverlayEventResult::PassThrough;
        }

        // Escape always pops topmost.
        if event.key == Key::Escape {
            let id = self.overlays.pop().expect("checked non-empty above").id;
            return OverlayEventResult::Dismissed(id);
        }

        let topmost = self.overlays.last_mut().expect("checked non-empty above");
        let id = topmost.id;
        let is_modal = topmost.kind == OverlayKind::Modal;
        let ctx = EventCtx {
            measurer,
            bounds: topmost.computed_rect,
            is_focused: true,
        };
        let response = topmost.widget.handle_key(event, &ctx);

        if response.response.is_handled() || is_modal {
            OverlayEventResult::Delivered {
                overlay_id: id,
                response,
            }
        } else {
            OverlayEventResult::PassThrough
        }
    }

    /// Routes a hover event through the overlay stack.
    pub fn process_hover_event(
        &mut self,
        point: Point,
        event: HoverEvent,
        measurer: &dyn crate::widgets::TextMeasurer,
    ) -> OverlayEventResult {
        if self.overlays.is_empty() {
            return OverlayEventResult::PassThrough;
        }

        // Find topmost overlay containing the point.
        for i in (0..self.overlays.len()).rev() {
            if self.overlays[i].computed_rect.contains(point) {
                let overlay = &mut self.overlays[i];
                let id = overlay.id;
                let ctx = EventCtx {
                    measurer,
                    bounds: overlay.computed_rect,
                    is_focused: false,
                };
                let response = overlay.widget.handle_hover(event, &ctx);
                return OverlayEventResult::Delivered {
                    overlay_id: id,
                    response,
                };
            }
        }

        // Point is outside all overlays.
        if self.has_modal() {
            OverlayEventResult::Blocked
        } else {
            OverlayEventResult::PassThrough
        }
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
        let mut ids = Vec::new();
        collect_focusable(&*topmost.widget, &mut ids);
        Some(ids)
    }
}

/// Recursively collects focusable widget IDs.
///
/// For the modal focus order, we only need the top-level widget's focusability.
/// Container widgets would need their own traversal — for now we check the
/// root widget only.
fn collect_focusable(widget: &dyn Widget, ids: &mut Vec<WidgetId>) {
    if widget.is_focusable() {
        ids.push(widget.id());
    }
}
