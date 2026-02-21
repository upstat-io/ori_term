//! Flex container widget — Row and Column layouts.
//!
//! The primary layout container for the UI framework. Arranges children
//! along a main axis (horizontal for Row, vertical for Column) with
//! configurable gap, alignment, and justification. Delegates to the
//! flex layout solver from Section 07.3.

use std::cell::RefCell;
use std::rc::Rc;

use crate::geometry::Rect;
use crate::input::{HoverEvent, KeyEvent, MouseEvent, MouseEventKind, layout_hit_test};
use crate::layout::{Align, Direction, Justify, LayoutBox, LayoutNode, compute_layout};
use crate::widget_id::WidgetId;

use crate::theme::UiTheme;

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
    /// Index of the child currently under the cursor (set by mouse Move).
    hovered_child: Option<usize>,
    /// Cached layout result, keyed by bounds. Avoids re-solving the layout
    /// solver when bounds haven't changed between draw/event calls.
    cached_layout: RefCell<Option<(Rect, Rc<LayoutNode>)>>,
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
            hovered_child: None,
            cached_layout: RefCell::new(None),
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
            hovered_child: None,
            cached_layout: RefCell::new(None),
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

    /// Returns cached layout if bounds match, otherwise recomputes.
    fn get_or_compute_layout(
        &self,
        measurer: &dyn super::TextMeasurer,
        theme: &UiTheme,
        bounds: Rect,
    ) -> Rc<LayoutNode> {
        {
            let cached = self.cached_layout.borrow();
            if let Some((ref cb, ref node)) = *cached {
                if *cb == bounds {
                    return Rc::clone(node);
                }
            }
        }
        let ctx = LayoutCtx { measurer, theme };
        let layout_box = self.build_layout_box(&ctx);
        let node = Rc::new(compute_layout(&layout_box, bounds));
        *self.cached_layout.borrow_mut() = Some((bounds, Rc::clone(&node)));
        node
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

    /// Updates hover tracking when the cursor moves. Sends Enter/Leave to
    /// the correct child based on hit testing.
    fn update_hover(
        &mut self,
        layout: &LayoutNode,
        pos: crate::geometry::Point,
        ctx: &EventCtx<'_>,
    ) -> WidgetResponse {
        let new_hover = self.hit_test_children(layout, pos);
        if new_hover == self.hovered_child {
            return WidgetResponse::ignored();
        }
        // Leave old child.
        if let Some(old_idx) = self.hovered_child {
            if let (Some(child), Some(child_node)) =
                (self.children.get_mut(old_idx), layout.children.get(old_idx))
            {
                let child_ctx = EventCtx {
                    measurer: ctx.measurer,
                    bounds: child_node.content_rect,
                    is_focused: ctx.focused_widget == Some(child.id()),
                    focused_widget: ctx.focused_widget,
                    theme: ctx.theme,
                };
                child.handle_hover(HoverEvent::Leave, &child_ctx);
            }
        }
        // Enter new child.
        if let Some(new_idx) = new_hover {
            if let (Some(child), Some(child_node)) =
                (self.children.get_mut(new_idx), layout.children.get(new_idx))
            {
                let child_ctx = EventCtx {
                    measurer: ctx.measurer,
                    bounds: child_node.content_rect,
                    is_focused: ctx.focused_widget == Some(child.id()),
                    focused_widget: ctx.focused_widget,
                    theme: ctx.theme,
                };
                child.handle_hover(HoverEvent::Enter, &child_ctx);
            }
        }
        self.hovered_child = new_hover;
        WidgetResponse::redraw()
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
        // Invalidate cache each frame so children with changed intrinsic sizes
        // get fresh layout. The cache still prevents redundant recomputation
        // within a single frame (draw + event handling share the same bounds).
        *self.cached_layout.borrow_mut() = None;
        let layout = self.get_or_compute_layout(ctx.measurer, ctx.theme, ctx.bounds);

        for (idx, child) in self.children.iter().enumerate() {
            if let Some(child_node) = layout.children.get(idx) {
                let mut child_ctx = DrawCtx {
                    measurer: ctx.measurer,
                    draw_list: ctx.draw_list,
                    bounds: child_node.content_rect,
                    focused_widget: ctx.focused_widget,
                    now: ctx.now,
                    animations_running: ctx.animations_running,
                    theme: ctx.theme,
                };
                child.draw(&mut child_ctx);
            }
        }
    }

    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        let layout = self.get_or_compute_layout(ctx.measurer, ctx.theme, ctx.bounds);

        // Track hover state on cursor moves.
        if matches!(event.kind, MouseEventKind::Move) {
            return self.update_hover(&layout, event.pos, ctx);
        }

        if let Some(idx) = self.hit_test_children(&layout, event.pos) {
            if let (Some(child), Some(child_node)) =
                (self.children.get_mut(idx), layout.children.get(idx))
            {
                let child_ctx = EventCtx {
                    measurer: ctx.measurer,
                    bounds: child_node.content_rect,
                    is_focused: ctx.focused_widget == Some(child.id()),
                    focused_widget: ctx.focused_widget,
                    theme: ctx.theme,
                };
                return child.handle_mouse(event, &child_ctx);
            }
        }
        WidgetResponse::ignored()
    }

    fn handle_hover(&mut self, event: HoverEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        match event {
            HoverEvent::Enter => {
                // Position unknown — defer to next mouse Move for child targeting.
                WidgetResponse::handled()
            }
            HoverEvent::Leave => {
                // Clear tracked hover child with correct bounds.
                if let Some(idx) = self.hovered_child.take() {
                    let layout = self.get_or_compute_layout(ctx.measurer, ctx.theme, ctx.bounds);
                    if let (Some(child), Some(child_node)) =
                        (self.children.get_mut(idx), layout.children.get(idx))
                    {
                        let child_ctx = EventCtx {
                            measurer: ctx.measurer,
                            bounds: child_node.content_rect,
                            is_focused: ctx.focused_widget == Some(child.id()),
                            focused_widget: ctx.focused_widget,
                            theme: ctx.theme,
                        };
                        child.handle_hover(HoverEvent::Leave, &child_ctx);
                    }
                }
                WidgetResponse::handled()
            }
        }
    }

    fn handle_key(&mut self, event: KeyEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        let layout = self.get_or_compute_layout(ctx.measurer, ctx.theme, ctx.bounds);

        // Delegate to children with per-child focus discrimination.
        for (idx, child) in self.children.iter_mut().enumerate() {
            if let Some(child_node) = layout.children.get(idx) {
                let child_ctx = EventCtx {
                    measurer: ctx.measurer,
                    bounds: child_node.content_rect,
                    is_focused: ctx.focused_widget == Some(child.id()),
                    focused_widget: ctx.focused_widget,
                    theme: ctx.theme,
                };
                let resp = child.handle_key(event, &child_ctx);
                if resp.response.is_handled() {
                    return resp;
                }
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
