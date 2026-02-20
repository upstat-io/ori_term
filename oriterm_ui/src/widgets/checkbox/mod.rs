//! Checkbox widget — a toggleable check box with label.
//!
//! Emits `WidgetAction::Toggled` when clicked or activated via Space.
//! The check box and label are laid out horizontally with a configurable gap.

use crate::color::Color;
use crate::draw::RectStyle;
use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, Key, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::LayoutBox;
use crate::text::TextStyle;
use crate::widget_id::WidgetId;

use super::{
    DEFAULT_ACCENT, DEFAULT_BG, DEFAULT_BORDER, DEFAULT_DISABLED_BG, DEFAULT_DISABLED_FG,
    DEFAULT_FG, DEFAULT_FOCUS_RING, DEFAULT_HOVER_BG, DrawCtx, EventCtx, LayoutCtx, Widget,
    WidgetAction, WidgetResponse,
};

/// Visual style for a [`CheckboxWidget`].
#[derive(Debug, Clone, PartialEq)]
pub struct CheckboxStyle {
    /// Size of the check box square.
    pub box_size: f32,
    /// Gap between the box and label text.
    pub gap: f32,
    /// Unchecked box background.
    pub bg: Color,
    /// Unchecked box background when hovered.
    pub hover_bg: Color,
    /// Checked box background (accent fill).
    pub checked_bg: Color,
    /// Box border color.
    pub border_color: Color,
    /// Border width.
    pub border_width: f32,
    /// Corner radius.
    pub corner_radius: f32,
    /// Check mark color.
    pub check_color: Color,
    /// Label text color.
    pub label_color: Color,
    /// Font size for the label.
    pub font_size: f32,
    /// Disabled text and box color.
    pub disabled_fg: Color,
    /// Disabled background.
    pub disabled_bg: Color,
    /// Focus ring color.
    pub focus_ring_color: Color,
}

impl Default for CheckboxStyle {
    fn default() -> Self {
        Self {
            box_size: 16.0,
            gap: 8.0,
            bg: DEFAULT_BG,
            hover_bg: DEFAULT_HOVER_BG,
            checked_bg: DEFAULT_ACCENT,
            border_color: DEFAULT_BORDER,
            border_width: 1.0,
            corner_radius: 3.0,
            check_color: Color::WHITE,
            label_color: DEFAULT_FG,
            font_size: 13.0,
            disabled_fg: DEFAULT_DISABLED_FG,
            disabled_bg: DEFAULT_DISABLED_BG,
            focus_ring_color: DEFAULT_FOCUS_RING,
        }
    }
}

/// A checkbox with label text.
///
/// Toggles between checked and unchecked on click or Space.
/// Emits `WidgetAction::Toggled { id, value }`.
#[derive(Debug, Clone)]
pub struct CheckboxWidget {
    id: WidgetId,
    label: String,
    checked: bool,
    disabled: bool,
    hovered: bool,
    style: CheckboxStyle,
}

impl CheckboxWidget {
    /// Creates an unchecked checkbox with the given label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: WidgetId::next(),
            label: label.into(),
            checked: false,
            disabled: false,
            hovered: false,
            style: CheckboxStyle::default(),
        }
    }

    /// Returns whether the checkbox is checked.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Sets the checked state.
    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Returns whether the checkbox is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Sets the disabled state.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
        if disabled {
            self.hovered = false;
        }
    }

    /// Returns whether the checkbox is hovered.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Sets the style.
    #[must_use]
    pub fn with_style(mut self, style: CheckboxStyle) -> Self {
        self.style = style;
        self
    }

    /// Sets the initial checked state via builder.
    #[must_use]
    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Sets the disabled state via builder.
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Toggles the check state and returns the resulting action.
    fn toggle(&mut self) -> WidgetAction {
        self.checked = !self.checked;
        WidgetAction::Toggled {
            id: self.id,
            value: self.checked,
        }
    }

    /// Returns the box background color based on state.
    fn box_bg(&self) -> Color {
        if self.disabled {
            return self.style.disabled_bg;
        }
        if self.checked {
            return self.style.checked_bg;
        }
        if self.hovered {
            return self.style.hover_bg;
        }
        self.style.bg
    }

    /// Returns the label text color based on state.
    fn label_fg(&self) -> Color {
        if self.disabled {
            self.style.disabled_fg
        } else {
            self.style.label_color
        }
    }

    /// Builds the label `TextStyle`.
    fn text_style(&self) -> TextStyle {
        TextStyle::new(self.style.font_size, self.label_fg())
    }
}

impl Widget for CheckboxWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        !self.disabled
    }

    fn layout(&self, ctx: &LayoutCtx<'_>) -> LayoutBox {
        let style = self.text_style();
        let metrics = ctx.measurer.measure(&self.label, &style, f32::INFINITY);
        let w = self.style.box_size + self.style.gap + metrics.width;
        let h = self.style.box_size.max(metrics.height);
        LayoutBox::leaf(w, h).with_widget_id(self.id)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        let focused = ctx.focused_widget == Some(self.id);
        let bounds = ctx.bounds;
        let s = &self.style;

        // Check box rect — vertically centered.
        let box_y = bounds.y() + (bounds.height() - s.box_size) / 2.0;
        let box_rect = Rect::new(bounds.x(), box_y, s.box_size, s.box_size);

        // Focus ring around the box.
        if focused {
            let ring = box_rect.inset(crate::geometry::Insets::all(-2.0));
            let ring_style = RectStyle::filled(Color::TRANSPARENT)
                .with_border(2.0, s.focus_ring_color)
                .with_radius(s.corner_radius + 2.0);
            ctx.draw_list.push_rect(ring, ring_style);
        }

        // Box background + border.
        let box_style = RectStyle::filled(self.box_bg())
            .with_border(s.border_width, s.border_color)
            .with_radius(s.corner_radius);
        ctx.draw_list.push_rect(box_rect, box_style);

        // Check mark — simple diagonal lines forming a check.
        if self.checked {
            let color = if self.disabled {
                s.disabled_fg
            } else {
                s.check_color
            };
            let inset = s.box_size * 0.25;
            let x0 = box_rect.x() + inset;
            let y0 = box_rect.y() + s.box_size * 0.5;
            let x1 = box_rect.x() + s.box_size * 0.4;
            let y1 = box_rect.bottom() - inset;
            let x2 = box_rect.right() - inset;
            let y2 = box_rect.y() + inset;

            ctx.draw_list
                .push_line(Point::new(x0, y0), Point::new(x1, y1), 2.0, color);
            ctx.draw_list
                .push_line(Point::new(x1, y1), Point::new(x2, y2), 2.0, color);
        }

        // Label text.
        if !self.label.is_empty() {
            let style = self.text_style();
            let text_x = bounds.x() + s.box_size + s.gap;
            let text_w = bounds.width() - s.box_size - s.gap;
            let shaped = ctx.measurer.shape(&self.label, &style, text_w);
            let text_y = bounds.y() + (bounds.height() - shaped.height) / 2.0;
            ctx.draw_list
                .push_text(Point::new(text_x, text_y), shaped, self.label_fg());
        }
    }

    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        if self.disabled {
            return WidgetResponse::ignored();
        }
        if event.kind == MouseEventKind::Up(MouseButton::Left) && ctx.bounds.contains(event.pos) {
            let action = self.toggle();
            return WidgetResponse::focus().with_action(action);
        }
        WidgetResponse::ignored()
    }

    fn handle_hover(&mut self, event: HoverEvent, _ctx: &EventCtx<'_>) -> WidgetResponse {
        if self.disabled {
            return WidgetResponse::ignored();
        }
        match event {
            HoverEvent::Enter => {
                self.hovered = true;
                WidgetResponse::redraw()
            }
            HoverEvent::Leave => {
                self.hovered = false;
                WidgetResponse::redraw()
            }
        }
    }

    fn handle_key(&mut self, event: KeyEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        if self.disabled || !ctx.is_focused {
            return WidgetResponse::ignored();
        }
        if event.key == Key::Space {
            let action = self.toggle();
            return WidgetResponse::redraw().with_action(action);
        }
        WidgetResponse::ignored()
    }
}

#[cfg(test)]
mod tests;
