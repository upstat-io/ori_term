//! Panel widget — a visual container with background, border, and shadow.
//!
//! Wraps a single child widget with configurable styling. Used for card-style
//! layouts, dialog backgrounds, and settings panels.

use crate::color::Color;
use crate::draw::{RectStyle, Shadow};
use crate::geometry::Insets;
use crate::input::{HoverEvent, KeyEvent, MouseEvent};
use crate::layout::{LayoutBox, LayoutNode, SizeSpec, compute_layout};
use crate::widget_id::WidgetId;

use super::{DEFAULT_BG, DEFAULT_BORDER, DrawCtx, EventCtx, LayoutCtx, Widget, WidgetResponse};

/// Visual style for a [`PanelWidget`].
#[derive(Debug, Clone, PartialEq)]
pub struct PanelStyle {
    /// Background fill color.
    pub bg: Color,
    /// Border color.
    pub border_color: Color,
    /// Border width in logical pixels.
    pub border_width: f32,
    /// Uniform corner radius.
    pub corner_radius: f32,
    /// Inner padding between panel edge and child.
    pub padding: Insets,
    /// Optional drop shadow.
    pub shadow: Option<Shadow>,
}

impl Default for PanelStyle {
    fn default() -> Self {
        Self {
            bg: DEFAULT_BG,
            border_color: DEFAULT_BORDER,
            border_width: 1.0,
            corner_radius: 8.0,
            padding: Insets::all(12.0),
            shadow: None,
        }
    }
}

/// A styled container wrapping a single child widget.
///
/// Draws a background rectangle (with optional border, radius, and shadow)
/// behind the child. The child is positioned within the panel's padding area.
pub struct PanelWidget {
    id: WidgetId,
    child: Box<dyn Widget>,
    style: PanelStyle,
}

impl PanelWidget {
    /// Creates a panel wrapping the given child widget.
    pub fn new(child: Box<dyn Widget>) -> Self {
        Self {
            id: WidgetId::next(),
            child,
            style: PanelStyle::default(),
        }
    }

    /// Sets the panel style.
    #[must_use]
    pub fn with_style(mut self, style: PanelStyle) -> Self {
        self.style = style;
        self
    }

    /// Sets the background color.
    #[must_use]
    pub fn with_bg(mut self, bg: Color) -> Self {
        self.style.bg = bg;
        self
    }

    /// Sets the corner radius.
    #[must_use]
    pub fn with_corner_radius(mut self, radius: f32) -> Self {
        self.style.corner_radius = radius;
        self
    }

    /// Sets the inner padding.
    #[must_use]
    pub fn with_padding(mut self, padding: Insets) -> Self {
        self.style.padding = padding;
        self
    }

    /// Sets the drop shadow.
    #[must_use]
    pub fn with_shadow(mut self, shadow: Shadow) -> Self {
        self.style.shadow = Some(shadow);
        self
    }

    /// Computes child layout within the given panel bounds.
    fn child_layout(&self, ctx: &LayoutCtx<'_>, bounds: crate::geometry::Rect) -> LayoutNode {
        let child_box = self.child.layout(ctx);
        let wrapper = LayoutBox::flex(crate::layout::Direction::Column, vec![child_box])
            .with_padding(self.style.padding)
            .with_width(SizeSpec::Fill)
            .with_height(SizeSpec::Fill)
            .with_widget_id(self.id);
        compute_layout(&wrapper, bounds)
    }
}

impl Widget for PanelWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        false
    }

    fn layout(&self, ctx: &LayoutCtx<'_>) -> LayoutBox {
        let child_box = self.child.layout(ctx);
        LayoutBox::flex(crate::layout::Direction::Column, vec![child_box])
            .with_padding(self.style.padding)
            .with_widget_id(self.id)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        // Draw panel background.
        let mut rect_style = RectStyle::filled(self.style.bg).with_radius(self.style.corner_radius);
        if self.style.border_width > 0.0 {
            rect_style = rect_style.with_border(self.style.border_width, self.style.border_color);
        }
        if let Some(shadow) = self.style.shadow {
            rect_style = rect_style.with_shadow(shadow);
        }
        ctx.draw_list.push_rect(ctx.bounds, rect_style);

        // Compute child layout and draw child.
        let layout_ctx = LayoutCtx {
            measurer: ctx.measurer,
        };
        let layout = self.child_layout(&layout_ctx, ctx.bounds);
        if let Some(child_node) = layout.children.first() {
            let mut child_ctx = DrawCtx {
                measurer: ctx.measurer,
                draw_list: ctx.draw_list,
                bounds: child_node.content_rect,
                focused_widget: ctx.focused_widget,
                now: ctx.now,
                animations_running: ctx.animations_running,
            };
            self.child.draw(&mut child_ctx);
        }
    }

    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        let layout_ctx = LayoutCtx {
            measurer: ctx.measurer,
        };
        let layout = self.child_layout(&layout_ctx, ctx.bounds);
        if let Some(child_node) = layout.children.first() {
            if child_node.rect.contains(event.pos) {
                let child_ctx = EventCtx {
                    measurer: ctx.measurer,
                    bounds: child_node.content_rect,
                    is_focused: ctx.is_focused,
                };
                return self.child.handle_mouse(event, &child_ctx);
            }
        }
        WidgetResponse::ignored()
    }

    fn handle_hover(&mut self, event: HoverEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        let layout_ctx = LayoutCtx {
            measurer: ctx.measurer,
        };
        let layout = self.child_layout(&layout_ctx, ctx.bounds);
        if let Some(child_node) = layout.children.first() {
            let child_ctx = EventCtx {
                measurer: ctx.measurer,
                bounds: child_node.content_rect,
                is_focused: ctx.is_focused,
            };
            return self.child.handle_hover(event, &child_ctx);
        }
        WidgetResponse::ignored()
    }

    fn handle_key(&mut self, event: KeyEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        let layout_ctx = LayoutCtx {
            measurer: ctx.measurer,
        };
        let layout = self.child_layout(&layout_ctx, ctx.bounds);
        if let Some(child_node) = layout.children.first() {
            let child_ctx = EventCtx {
                measurer: ctx.measurer,
                bounds: child_node.content_rect,
                is_focused: ctx.is_focused,
            };
            return self.child.handle_key(event, &child_ctx);
        }
        WidgetResponse::ignored()
    }
}

#[cfg(test)]
mod tests;
