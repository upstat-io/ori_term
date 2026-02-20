//! Button widget with hover, pressed, and disabled states.
//!
//! Emits `WidgetAction::Clicked` on mouse click or keyboard activation
//! (Enter/Space when focused). Supports configurable padding, border
//! radius, and color states.

use crate::color::Color;
use crate::draw::RectStyle;
use crate::geometry::{Insets, Point};
use crate::input::{HoverEvent, Key, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::LayoutBox;
use crate::text::TextStyle;
use crate::widget_id::WidgetId;

use super::{
    DEFAULT_BG, DEFAULT_BORDER, DEFAULT_DISABLED_BG, DEFAULT_DISABLED_FG, DEFAULT_FG,
    DEFAULT_FOCUS_RING, DEFAULT_HOVER_BG, DEFAULT_PRESSED_BG, DrawCtx, EventCtx, LayoutCtx, Widget,
    WidgetAction, WidgetResponse,
};

/// Visual style for a [`ButtonWidget`].
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonStyle {
    /// Text color.
    pub fg: Color,
    /// Background color (normal state).
    pub bg: Color,
    /// Background when hovered.
    pub hover_bg: Color,
    /// Background when pressed.
    pub pressed_bg: Color,
    /// Border color.
    pub border_color: Color,
    /// Border width.
    pub border_width: f32,
    /// Corner radius.
    pub corner_radius: f32,
    /// Inner padding.
    pub padding: Insets,
    /// Font size in points.
    pub font_size: f32,
    /// Disabled text color.
    pub disabled_fg: Color,
    /// Disabled background color.
    pub disabled_bg: Color,
    /// Focus ring color.
    pub focus_ring_color: Color,
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self {
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            hover_bg: DEFAULT_HOVER_BG,
            pressed_bg: DEFAULT_PRESSED_BG,
            border_color: DEFAULT_BORDER,
            border_width: 1.0,
            corner_radius: 4.0,
            padding: Insets::vh(6.0, 12.0),
            font_size: 13.0,
            disabled_fg: DEFAULT_DISABLED_FG,
            disabled_bg: DEFAULT_DISABLED_BG,
            focus_ring_color: DEFAULT_FOCUS_RING,
        }
    }
}

/// Interactive button widget.
///
/// Emits `WidgetAction::Clicked(id)` when clicked or keyboard-activated.
#[derive(Debug, Clone)]
pub struct ButtonWidget {
    id: WidgetId,
    label: String,
    disabled: bool,
    hovered: bool,
    pressed: bool,
    style: ButtonStyle,
}

impl ButtonWidget {
    /// Creates a button with the given label text.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: WidgetId::next(),
            label: label.into(),
            disabled: false,
            hovered: false,
            pressed: false,
            style: ButtonStyle::default(),
        }
    }

    /// Returns the button label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns whether the button is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the button is currently hovered.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Returns whether the button is currently pressed.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Sets the disabled state.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
        if disabled {
            self.hovered = false;
            self.pressed = false;
        }
    }

    /// Sets the button style.
    #[must_use]
    pub fn with_style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Sets the disabled state via builder.
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Returns the current background color based on state.
    fn current_bg(&self) -> Color {
        if self.disabled {
            return self.style.disabled_bg;
        }
        if self.pressed {
            return self.style.pressed_bg;
        }
        if self.hovered {
            return self.style.hover_bg;
        }
        self.style.bg
    }

    /// Returns the current text color based on state.
    fn current_fg(&self) -> Color {
        if self.disabled {
            self.style.disabled_fg
        } else {
            self.style.fg
        }
    }

    /// Builds the `TextStyle` for measurement and shaping.
    fn text_style(&self) -> TextStyle {
        TextStyle::new(self.style.font_size, self.current_fg())
    }
}

impl Widget for ButtonWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        !self.disabled
    }

    fn layout(&self, ctx: &LayoutCtx<'_>) -> LayoutBox {
        let style = self.text_style();
        let metrics = ctx.measurer.measure(&self.label, &style, f32::INFINITY);
        let w = metrics.width + self.style.padding.width();
        let h = metrics.height + self.style.padding.height();
        LayoutBox::leaf(w, h).with_widget_id(self.id)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        let focused = ctx.focused_widget == Some(self.id);

        // Focus ring (drawn behind the button).
        if focused {
            let ring_rect = ctx.bounds.inset(Insets::all(-2.0));
            let ring_style = RectStyle::filled(Color::TRANSPARENT)
                .with_border(2.0, self.style.focus_ring_color)
                .with_radius(self.style.corner_radius + 2.0);
            ctx.draw_list.push_rect(ring_rect, ring_style);
        }

        // Button background.
        let bg_style = RectStyle::filled(self.current_bg())
            .with_border(self.style.border_width, self.style.border_color)
            .with_radius(self.style.corner_radius);
        ctx.draw_list.push_rect(ctx.bounds, bg_style);

        // Label text, centered in the padded area.
        if !self.label.is_empty() {
            let style = self.text_style();
            let inner = ctx.bounds.inset(self.style.padding);
            let shaped = ctx.measurer.shape(&self.label, &style, inner.width());
            let x = inner.x() + (inner.width() - shaped.width) / 2.0;
            let y = inner.y() + (inner.height() - shaped.height) / 2.0;
            ctx.draw_list
                .push_text(Point::new(x, y), shaped, self.current_fg());
        }
    }

    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx) -> WidgetResponse {
        if self.disabled {
            return WidgetResponse::ignored();
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.pressed = true;
                WidgetResponse::focus()
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let was_pressed = self.pressed;
                self.pressed = false;
                if was_pressed && ctx.bounds.contains(event.pos) {
                    WidgetResponse::redraw().with_action(WidgetAction::Clicked(self.id))
                } else {
                    WidgetResponse::redraw()
                }
            }
            _ => WidgetResponse::ignored(),
        }
    }

    fn handle_hover(&mut self, event: HoverEvent, _ctx: &EventCtx) -> WidgetResponse {
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
                self.pressed = false;
                WidgetResponse::redraw()
            }
        }
    }

    fn handle_key(&mut self, event: KeyEvent, _ctx: &EventCtx) -> WidgetResponse {
        if self.disabled {
            return WidgetResponse::ignored();
        }
        match event.key {
            Key::Enter | Key::Space => {
                WidgetResponse::redraw().with_action(WidgetAction::Clicked(self.id))
            }
            _ => WidgetResponse::ignored(),
        }
    }
}

#[cfg(test)]
mod tests;
