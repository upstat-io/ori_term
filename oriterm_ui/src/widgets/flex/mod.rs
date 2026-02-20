//! Flex container widget — Row and Column layouts.
//!
//! The primary layout container for the UI framework. Arranges children
//! along a main axis (horizontal for Row, vertical for Column) with
//! configurable gap, alignment, and justification. Delegates to the
//! flex layout solver from Section 07.3.

use crate::geometry::Rect;
use crate::input::{HoverEvent, KeyEvent, MouseEvent, layout_hit_test};
use crate::layout::{Align, Direction, Justify, LayoutBox, LayoutNode, compute_layout};
use crate::widget_id::WidgetId;

use super::{DrawCtx, EventCtx, LayoutCtx, Widget, WidgetResponse};

/// A flex container that arranges children along a main axis.
///
/// Row (horizontal) and Column (vertical) are the two modes. This is the
/// primary layout building block — all complex layouts are nested Rows and
/// Columns. Children are stored as `Box<dyn Widget>` to support
/// heterogeneous widget types.
pub struct FlexWidget {
    id: WidgetId,
    direction: Direction,
    children: Vec<Box<dyn Widget>>,
    gap: f32,
    align: Align,
    justify: Justify,
}

impl FlexWidget {
    /// Creates a horizontal (Row) flex container.
    pub fn row(children: Vec<Box<dyn Widget>>) -> Self {
        Self {
            id: WidgetId::next(),
            direction: Direction::Row,
            children,
            gap: 0.0,
            align: Align::Start,
            justify: Justify::Start,
        }
    }

    /// Creates a vertical (Column) flex container.
    pub fn column(children: Vec<Box<dyn Widget>>) -> Self {
        Self {
            id: WidgetId::next(),
            direction: Direction::Column,
            children,
            gap: 0.0,
            align: Align::Start,
            justify: Justify::Start,
        }
    }

    /// Sets the gap between children along the main axis.
    #[must_use]
    pub fn with_gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Sets cross-axis alignment.
    #[must_use]
    pub fn with_align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Sets main-axis justification.
    #[must_use]
    pub fn with_justify(mut self, justify: Justify) -> Self {
        self.justify = justify;
        self
    }

    /// Returns the number of children.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Computes layout for all children within the given bounds.
    fn compute_child_layout(&self, ctx: &LayoutCtx<'_>, bounds: Rect) -> LayoutNode {
        let layout_box = self.build_layout_box(ctx);
        compute_layout(&layout_box, bounds)
    }

    /// Builds the `LayoutBox` descriptor tree.
    fn build_layout_box(&self, ctx: &LayoutCtx<'_>) -> LayoutBox {
        let child_boxes: Vec<LayoutBox> = self.children.iter().map(|c| c.layout(ctx)).collect();
        LayoutBox::flex(self.direction, child_boxes)
            .with_gap(self.gap)
            .with_align(self.align)
            .with_justify(self.justify)
            .with_widget_id(self.id)
    }

    /// Finds which child widget matches a `WidgetId` from hit testing.
    fn find_child_index(&self, target: WidgetId) -> Option<usize> {
        self.children.iter().position(|c| c.id() == target)
    }

    /// Finds the deepest child widget under a point via hit testing.
    fn hit_test_children(&self, layout: &LayoutNode, pos: crate::geometry::Point) -> Option<usize> {
        let target_id = layout_hit_test(layout, pos)?;
        if target_id == self.id {
            return None;
        }
        // Check direct children first.
        if let Some(idx) = self.find_child_index(target_id) {
            return Some(idx);
        }
        // The target is nested inside a child — find which child contains it.
        for (idx, child_node) in layout.children.iter().enumerate() {
            if child_node.rect.contains(pos) {
                return Some(idx);
            }
        }
        None
    }
}

impl Widget for FlexWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        false
    }

    fn layout(&self, ctx: &LayoutCtx<'_>) -> LayoutBox {
        self.build_layout_box(ctx)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        let layout_ctx = LayoutCtx {
            measurer: ctx.measurer,
        };
        let layout = self.compute_child_layout(&layout_ctx, ctx.bounds);

        for (idx, child) in self.children.iter().enumerate() {
            if let Some(child_node) = layout.children.get(idx) {
                let mut child_ctx = DrawCtx {
                    measurer: ctx.measurer,
                    draw_list: ctx.draw_list,
                    bounds: child_node.content_rect,
                    focused_widget: ctx.focused_widget,
                    now: ctx.now,
                    animations_running: ctx.animations_running,
                };
                child.draw(&mut child_ctx);
            }
        }
    }

    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        let layout_ctx = LayoutCtx {
            measurer: ctx.measurer,
        };
        let layout = self.compute_child_layout(&layout_ctx, ctx.bounds);

        if let Some(idx) = self.hit_test_children(&layout, event.pos) {
            if let (Some(child), Some(child_node)) =
                (self.children.get_mut(idx), layout.children.get(idx))
            {
                let child_ctx = EventCtx {
                    measurer: ctx.measurer,
                    bounds: child_node.content_rect,
                    is_focused: ctx.is_focused,
                };
                return child.handle_mouse(event, &child_ctx);
            }
        }
        WidgetResponse::ignored()
    }

    fn handle_hover(&mut self, event: HoverEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        // Hover events are already targeted — delegate to all children.
        // The routing layer manages hover enter/leave per widget.
        for child in &mut self.children {
            let resp = child.handle_hover(event, ctx);
            if resp.response.is_handled() {
                return resp;
            }
        }
        WidgetResponse::ignored()
    }

    fn handle_key(&mut self, event: KeyEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        // Key events go to the focused child. Since we don't track focus
        // here, delegate to all children; the focused one will handle it.
        for child in &mut self.children {
            let resp = child.handle_key(event, ctx);
            if resp.response.is_handled() {
                return resp;
            }
        }
        WidgetResponse::ignored()
    }
}

#[cfg(test)]
mod tests;
