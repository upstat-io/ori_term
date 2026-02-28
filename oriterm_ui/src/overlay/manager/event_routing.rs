//! Event routing through the overlay stack.
//!
//! Mouse events, key events, and hover events are routed through the overlay
//! stack before reaching the main widget tree. Dismissals trigger compositor
//! fade-out animations.

use std::time::Instant;

use crate::compositor::layer_animator::LayerAnimator;
use crate::compositor::layer_tree::LayerTree;
use crate::geometry::Point;
use crate::input::{HoverEvent, Key, KeyEvent, MouseEvent, MouseEventKind};
use crate::theme::UiTheme;
use crate::widget_id::WidgetId;
use crate::widgets::{EventCtx, WidgetResponse};

use super::{OverlayEventResult, OverlayKind, OverlayManager};

impl OverlayManager {
    /// Routes a mouse event through the overlay stack.
    ///
    /// Hit-tests overlays back-to-front (topmost first). The `focused_widget`
    /// parameter indicates which widget currently has keyboard focus (from the
    /// app layer's `FocusManager`). See [`OverlayEventResult`] for routing rules.
    ///
    /// Click-outside dismissals start a fade-out animation via the compositor.
    #[expect(
        clippy::too_many_arguments,
        reason = "event routing: event, measurer, theme, focus, tree, animator, now"
    )]
    pub fn process_mouse_event(
        &mut self,
        event: &MouseEvent,
        measurer: &dyn crate::widgets::TextMeasurer,
        theme: &UiTheme,
        focused_widget: Option<WidgetId>,
        tree: &LayerTree,
        animator: &mut LayerAnimator,
        now: Instant,
    ) -> OverlayEventResult {
        if self.overlays.is_empty() {
            return OverlayEventResult::PassThrough;
        }

        // Hit test from topmost to bottom.
        for i in (0..self.overlays.len()).rev() {
            if self.overlays[i].computed_rect.contains(event.pos) {
                let overlay = &mut self.overlays[i];
                let id = overlay.id;
                let root_id = overlay.widget.id();
                let ctx = EventCtx {
                    measurer,
                    bounds: overlay.computed_rect,
                    is_focused: focused_widget == Some(root_id),
                    focused_widget,
                    theme,
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
                    self.begin_dismiss_topmost(tree, animator, now);
                    OverlayEventResult::Dismissed(topmost_id)
                } else {
                    OverlayEventResult::PassThrough
                }
            }
        }
    }

    /// Routes a key event through the overlay stack.
    ///
    /// Escape dismisses the topmost overlay with a fade-out animation.
    /// Modal overlays never pass through. The `focused_widget` parameter
    /// indicates which widget currently has keyboard focus (from the app
    /// layer's `FocusManager`).
    #[expect(
        clippy::too_many_arguments,
        reason = "event routing: event, measurer, theme, focus, tree, animator, now"
    )]
    pub fn process_key_event(
        &mut self,
        event: KeyEvent,
        measurer: &dyn crate::widgets::TextMeasurer,
        theme: &UiTheme,
        focused_widget: Option<WidgetId>,
        tree: &LayerTree,
        animator: &mut LayerAnimator,
        now: Instant,
    ) -> OverlayEventResult {
        if self.overlays.is_empty() {
            return OverlayEventResult::PassThrough;
        }

        // Escape always dismisses topmost (with fade-out).
        if event.key == Key::Escape {
            let id = self
                .begin_dismiss_topmost(tree, animator, now)
                .expect("checked non-empty above");
            return OverlayEventResult::Dismissed(id);
        }

        let topmost = self.overlays.last_mut().expect("checked non-empty above");
        let id = topmost.id;
        let is_modal = topmost.kind == OverlayKind::Modal;
        let root_id = topmost.widget.id();
        let ctx = EventCtx {
            measurer,
            bounds: topmost.computed_rect,
            is_focused: focused_widget == Some(root_id),
            focused_widget,
            theme,
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
    ///
    /// Tracks which overlay was previously hovered. When the cursor moves
    /// between overlays, sends `HoverEvent::Leave` to the old overlay and
    /// `HoverEvent::Enter` to the new one.
    pub fn process_hover_event(
        &mut self,
        point: Point,
        measurer: &dyn crate::widgets::TextMeasurer,
        theme: &UiTheme,
        focused_widget: Option<WidgetId>,
    ) -> OverlayEventResult {
        if self.overlays.is_empty() {
            self.hovered_overlay = None;
            return OverlayEventResult::PassThrough;
        }

        // Find topmost overlay containing the point.
        let new_hover = (0..self.overlays.len())
            .rev()
            .find(|&i| self.overlays[i].computed_rect.contains(point));

        let hover_changed = self.hovered_overlay != new_hover;

        if hover_changed {
            // Send Leave to old overlay.
            if let Some(old_idx) = self.hovered_overlay {
                if let Some(old_overlay) = self.overlays.get_mut(old_idx) {
                    let root_id = old_overlay.widget.id();
                    let ctx = EventCtx {
                        measurer,
                        bounds: old_overlay.computed_rect,
                        is_focused: focused_widget == Some(root_id),
                        focused_widget,
                        theme,
                    };
                    old_overlay.widget.handle_hover(HoverEvent::Leave, &ctx);
                }
            }
            self.hovered_overlay = new_hover;
        }

        match new_hover {
            Some(idx) if hover_changed => {
                // Send Enter to newly hovered overlay.
                let overlay = &mut self.overlays[idx];
                let id = overlay.id;
                let root_id = overlay.widget.id();
                let ctx = EventCtx {
                    measurer,
                    bounds: overlay.computed_rect,
                    is_focused: focused_widget == Some(root_id),
                    focused_widget,
                    theme,
                };
                let response = overlay.widget.handle_hover(HoverEvent::Enter, &ctx);
                OverlayEventResult::Delivered {
                    overlay_id: id,
                    response,
                }
            }
            Some(idx) => {
                // Hover unchanged, still over this overlay — no re-enter.
                OverlayEventResult::Delivered {
                    overlay_id: self.overlays[idx].id,
                    response: WidgetResponse::handled(),
                }
            }
            None => {
                // Point is outside all overlays.
                if self.has_modal() {
                    OverlayEventResult::Blocked
                } else {
                    OverlayEventResult::PassThrough
                }
            }
        }
    }
}
