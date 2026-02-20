//! Scroll container widget — clips content and supports scrolling.
//!
//! Wraps a single child widget that may be taller (or wider) than the
//! container's visible area. Provides mouse wheel scrolling, keyboard
//! navigation (PageUp/Down, Home/End), and an overlay scrollbar.

use crate::color::Color;
use crate::draw::RectStyle;
use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, Key, KeyEvent, Modifiers, MouseEvent, MouseEventKind, ScrollDelta};
use crate::layout::{LayoutBox, compute_layout};
use crate::widget_id::WidgetId;

use super::{DrawCtx, EventCtx, LayoutCtx, Widget, WidgetResponse};

/// Scroll direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    /// Vertical scrolling only (most common).
    Vertical,
    /// Horizontal scrolling only.
    Horizontal,
    /// Both axes scroll independently.
    Both,
}

/// Style for the overlay scrollbar.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollbarStyle {
    /// Scrollbar width (logical pixels).
    pub width: f32,
    /// Scrollbar thumb color.
    pub thumb_color: Color,
    /// Scrollbar track color (behind the thumb).
    pub track_color: Color,
    /// Corner radius of the thumb.
    pub thumb_radius: f32,
    /// Minimum thumb height (logical pixels).
    pub min_thumb_height: f32,
}

impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            width: 6.0,
            thumb_color: Color::WHITE.with_alpha(0.25),
            track_color: Color::TRANSPARENT,
            thumb_radius: 3.0,
            min_thumb_height: 20.0,
        }
    }
}

/// A scrollable container that clips its child to visible bounds.
///
/// Supports vertical, horizontal, or dual-axis scrolling. Renders a
/// thin overlay scrollbar when content overflows.
pub struct ScrollWidget {
    id: WidgetId,
    child: Box<dyn Widget>,
    direction: ScrollDirection,
    /// Current scroll offset (pixels scrolled from top/left).
    scroll_offset: f32,
    /// Horizontal scroll offset (only used with `Both` direction).
    scroll_offset_x: f32,
    scrollbar_style: ScrollbarStyle,
    /// Pixels per mouse wheel line.
    line_height: f32,
}

impl ScrollWidget {
    /// Creates a vertical scroll container wrapping the given child.
    pub fn vertical(child: Box<dyn Widget>) -> Self {
        Self {
            id: WidgetId::next(),
            child,
            direction: ScrollDirection::Vertical,
            scroll_offset: 0.0,
            scroll_offset_x: 0.0,
            scrollbar_style: ScrollbarStyle::default(),
            line_height: 20.0,
        }
    }

    /// Creates a scroll container with a specific direction.
    pub fn new(child: Box<dyn Widget>, direction: ScrollDirection) -> Self {
        Self {
            id: WidgetId::next(),
            child,
            direction,
            scroll_offset: 0.0,
            scroll_offset_x: 0.0,
            scrollbar_style: ScrollbarStyle::default(),
            line_height: 20.0,
        }
    }

    /// Sets the scrollbar style.
    #[must_use]
    pub fn with_scrollbar_style(mut self, style: ScrollbarStyle) -> Self {
        self.scrollbar_style = style;
        self
    }

    /// Returns the current vertical scroll offset.
    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    /// Sets the vertical scroll offset, clamping to valid range.
    pub fn set_scroll_offset(&mut self, offset: f32, content_height: f32, view_height: f32) {
        let max = (content_height - view_height).max(0.0);
        self.scroll_offset = offset.clamp(0.0, max);
    }

    /// Computes the child's natural (unconstrained) size.
    fn child_natural_size(&self, ctx: &LayoutCtx<'_>, viewport: Rect) -> (f32, f32) {
        let child_box = self.child.layout(ctx);
        // Give the child unlimited space in the scroll direction.
        let (w, h) = match self.direction {
            ScrollDirection::Vertical => (viewport.width(), f32::INFINITY),
            ScrollDirection::Horizontal => (f32::INFINITY, viewport.height()),
            ScrollDirection::Both => (f32::INFINITY, f32::INFINITY),
        };
        let unconstrained = Rect::new(0.0, 0.0, w, h);
        let node = compute_layout(&child_box, unconstrained);
        (node.rect.width(), node.rect.height())
    }

    /// Scrolls by a delta, clamping to valid range. Returns true if offset changed.
    fn scroll_by(&mut self, delta_y: f32, content_height: f32, view_height: f32) -> bool {
        let max = (content_height - view_height).max(0.0);
        let old = self.scroll_offset;
        self.scroll_offset = (self.scroll_offset - delta_y).clamp(0.0, max);
        (self.scroll_offset - old).abs() > f32::EPSILON
    }

    /// Draws the vertical scrollbar thumb.
    fn draw_scrollbar(&self, ctx: &mut DrawCtx<'_>, content_height: f32, view_height: f32) {
        if content_height <= view_height {
            return;
        }

        let s = &self.scrollbar_style;
        let track_x = ctx.bounds.right() - s.width;
        let track_h = view_height;

        // Thumb proportional to visible portion.
        let ratio = view_height / content_height;
        let thumb_h = (track_h * ratio).max(s.min_thumb_height).min(track_h);
        let scroll_range = content_height - view_height;
        let thumb_top = if scroll_range > 0.0 {
            (self.scroll_offset / scroll_range) * (track_h - thumb_h)
        } else {
            0.0
        };

        let thumb_rect = Rect::new(track_x, ctx.bounds.y() + thumb_top, s.width, thumb_h);
        let style = RectStyle::filled(s.thumb_color).with_radius(s.thumb_radius);
        ctx.draw_list.push_rect(thumb_rect, style);
    }
}

impl Widget for ScrollWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn layout(&self, ctx: &LayoutCtx<'_>) -> LayoutBox {
        // The scroll container itself uses the child's unconstrained width
        // but constrains height to Hug (the container's visible size is
        // determined by the parent, not the child's full height).
        let child_box = self.child.layout(ctx);
        let unconstrained = Rect::new(0.0, 0.0, f32::INFINITY, f32::INFINITY);
        let child_node = compute_layout(&child_box, unconstrained);
        LayoutBox::leaf(child_node.rect.width(), child_node.rect.height()).with_widget_id(self.id)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        let layout_ctx = LayoutCtx {
            measurer: ctx.measurer,
        };
        let (content_w, content_h) = self.child_natural_size(&layout_ctx, ctx.bounds);

        // Clip to visible area.
        ctx.draw_list.push_clip(ctx.bounds);

        // Offset the child by the scroll amount.
        let child_bounds = Rect::new(
            ctx.bounds.x() - self.scroll_offset_x,
            ctx.bounds.y() - self.scroll_offset,
            content_w,
            content_h,
        );
        let mut child_ctx = DrawCtx {
            measurer: ctx.measurer,
            draw_list: ctx.draw_list,
            bounds: child_bounds,
            focused_widget: ctx.focused_widget,
            now: ctx.now,
            animations_running: ctx.animations_running,
        };
        self.child.draw(&mut child_ctx);

        ctx.draw_list.pop_clip();

        // Draw scrollbar on top of content.
        self.draw_scrollbar(ctx, content_h, ctx.bounds.height());
    }

    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        let layout_ctx = LayoutCtx {
            measurer: ctx.measurer,
        };
        let (content_w, content_h) = self.child_natural_size(&layout_ctx, ctx.bounds);
        let view_h = ctx.bounds.height();

        // Handle scroll events.
        if let MouseEventKind::Scroll(delta) = event.kind {
            let delta_y = match delta {
                ScrollDelta::Pixels { y, .. } => y,
                ScrollDelta::Lines { y, .. } => y * self.line_height,
            };
            if self.scroll_by(delta_y, content_h, view_h) {
                return WidgetResponse::redraw();
            }
            return WidgetResponse::handled();
        }

        // Translate event position for the child (account for scroll offset).
        let child_pos = Point::new(
            event.pos.x + self.scroll_offset_x,
            event.pos.y + self.scroll_offset,
        );
        let child_event = MouseEvent {
            kind: event.kind,
            pos: child_pos,
            modifiers: event.modifiers,
        };
        let child_bounds = Rect::new(0.0, 0.0, content_w, content_h);
        let child_ctx = EventCtx {
            measurer: ctx.measurer,
            bounds: child_bounds,
            is_focused: ctx.is_focused,
        };
        self.child.handle_mouse(&child_event, &child_ctx)
    }

    fn handle_hover(&mut self, event: HoverEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        self.child.handle_hover(event, ctx)
    }

    fn handle_key(&mut self, event: KeyEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        let layout_ctx = LayoutCtx {
            measurer: ctx.measurer,
        };
        let (_, content_h) = self.child_natural_size(&layout_ctx, ctx.bounds);
        let view_h = ctx.bounds.height();

        // Handle scroll keys.
        if event.modifiers == Modifiers::NONE {
            match event.key {
                Key::ArrowUp => {
                    if self.scroll_by(self.line_height, content_h, view_h) {
                        return WidgetResponse::redraw();
                    }
                    return WidgetResponse::handled();
                }
                Key::ArrowDown => {
                    if self.scroll_by(-self.line_height, content_h, view_h) {
                        return WidgetResponse::redraw();
                    }
                    return WidgetResponse::handled();
                }
                Key::Home => {
                    let changed = self.scroll_offset > f32::EPSILON;
                    self.scroll_offset = 0.0;
                    return if changed {
                        WidgetResponse::redraw()
                    } else {
                        WidgetResponse::handled()
                    };
                }
                Key::End => {
                    let max = (content_h - view_h).max(0.0);
                    let changed = (self.scroll_offset - max).abs() > f32::EPSILON;
                    self.scroll_offset = max;
                    return if changed {
                        WidgetResponse::redraw()
                    } else {
                        WidgetResponse::handled()
                    };
                }
                _ => {}
            }
        }

        // Delegate to child for non-scroll keys.
        self.child.handle_key(event, ctx)
    }
}

#[cfg(test)]
mod tests;
