//! Stack widget — Z-axis overlay container.
//!
//! Layers children on top of each other. All children share the parent's
//! bounds. The last child in the list is frontmost (drawn last, receives
//! events first). Used for absolute positioning within a relative container.

use crate::geometry::Rect;
use crate::input::{HoverEvent, KeyEvent, MouseEvent};
use crate::layout::{LayoutBox, compute_layout};
use crate::widget_id::WidgetId;

use super::{DrawCtx, EventCtx, LayoutCtx, Widget, WidgetResponse};

/// A Z-axis container that overlays children on top of each other.
///
/// All children share the same bounds (the stack's bounds). Children
/// are drawn in order — the last child is frontmost. Events are routed
/// back-to-front: the frontmost child that handles the event wins.
pub struct StackWidget {
    id: WidgetId,
    children: Vec<Box<dyn Widget>>,
    /// Index of the child currently hovered (set by hover Enter).
    hovered_child: Option<usize>,
}

impl StackWidget {
    /// Creates a stack with the given children (last = frontmost).
    pub fn new(children: Vec<Box<dyn Widget>>) -> Self {
        Self {
            id: WidgetId::next(),
            children,
            hovered_child: None,
        }
    }

    /// Returns the number of children.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Finds the largest resolved size among children to size the stack.
    ///
    /// Resolves each child through the layout solver with unconstrained bounds
    /// so both `Leaf` and `Flex` children contribute their natural size.
    fn max_child_size(&self, ctx: &LayoutCtx<'_>) -> (f32, f32) {
        let mut max_w: f32 = 0.0;
        let mut max_h: f32 = 0.0;
        let unconstrained = Rect::new(0.0, 0.0, f32::INFINITY, f32::INFINITY);
        for child in &self.children {
            let child_box = child.layout(ctx);
            let node = compute_layout(&child_box, unconstrained);
            max_w = max_w.max(node.rect.width());
            max_h = max_h.max(node.rect.height());
        }
        (max_w, max_h)
    }
}

impl Widget for StackWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        false
    }

    fn layout(&self, ctx: &LayoutCtx<'_>) -> LayoutBox {
        // Stack sizes to the largest child. All children share the
        // stack's full bounds (positioned manually in draw/events).
        let (max_w, max_h) = self.max_child_size(ctx);
        LayoutBox::leaf(max_w, max_h).with_widget_id(self.id)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        // Draw children in order: first = backmost, last = frontmost.
        for child in &self.children {
            let mut child_ctx = DrawCtx {
                measurer: ctx.measurer,
                draw_list: ctx.draw_list,
                bounds: ctx.bounds,
                focused_widget: ctx.focused_widget,
                now: ctx.now,
                animations_running: ctx.animations_running,
            };
            child.draw(&mut child_ctx);
        }
    }

    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        if !ctx.bounds.contains(event.pos) {
            return WidgetResponse::ignored();
        }
        // Route back-to-front: frontmost child that handles it wins.
        for child in self.children.iter_mut().rev() {
            let resp = child.handle_mouse(event, ctx);
            if resp.response.is_handled() {
                return resp;
            }
        }
        WidgetResponse::ignored()
    }

    fn handle_hover(&mut self, event: HoverEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        match event {
            HoverEvent::Enter => {
                // Enter the frontmost child that accepts hover.
                for (idx, child) in self.children.iter_mut().enumerate().rev() {
                    let resp = child.handle_hover(HoverEvent::Enter, ctx);
                    if resp.response.is_handled() {
                        self.hovered_child = Some(idx);
                        return resp;
                    }
                }
                WidgetResponse::ignored()
            }
            HoverEvent::Leave => {
                // Leave only the tracked hovered child.
                if let Some(idx) = self.hovered_child.take() {
                    if let Some(child) = self.children.get_mut(idx) {
                        return child.handle_hover(HoverEvent::Leave, ctx);
                    }
                }
                WidgetResponse::handled()
            }
        }
    }

    fn handle_key(&mut self, event: KeyEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        // Key events go to frontmost child that handles them,
        // with per-child focus discrimination.
        for child in self.children.iter_mut().rev() {
            let child_ctx = EventCtx {
                measurer: ctx.measurer,
                bounds: ctx.bounds,
                is_focused: ctx.focused_widget == Some(child.id()),
                focused_widget: ctx.focused_widget,
            };
            let resp = child.handle_key(event, &child_ctx);
            if resp.response.is_handled() {
                return resp;
            }
        }
        WidgetResponse::ignored()
    }

    fn focusable_children(&self) -> Vec<WidgetId> {
        self.children
            .iter()
            .flat_map(|c| c.focusable_children())
            .collect()
    }
}

#[cfg(test)]
mod tests;
