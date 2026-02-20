//! Dropdown widget — a trigger button showing the selected item.
//!
//! Displays the currently selected item and a dropdown indicator.
//! The popup/overlay is deferred to section 07.8 — this widget only
//! handles the closed (trigger) state. Emits `WidgetAction::Selected`
//! when the selection changes programmatically.

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

/// Visual style for a [`DropdownWidget`].
#[derive(Debug, Clone, PartialEq)]
pub struct DropdownStyle {
    /// Text color.
    pub fg: Color,
    /// Background color.
    pub bg: Color,
    /// Hovered background.
    pub hover_bg: Color,
    /// Pressed background.
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
    /// Width reserved for the dropdown indicator arrow.
    pub indicator_width: f32,
    /// Indicator color.
    pub indicator_color: Color,
    /// Disabled text color.
    pub disabled_fg: Color,
    /// Disabled background.
    pub disabled_bg: Color,
    /// Focus ring color.
    pub focus_ring_color: Color,
}

impl Default for DropdownStyle {
    fn default() -> Self {
        Self {
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            hover_bg: DEFAULT_HOVER_BG,
            pressed_bg: DEFAULT_PRESSED_BG,
            border_color: DEFAULT_BORDER,
            border_width: 1.0,
            corner_radius: 4.0,
            padding: Insets::vh(6.0, 10.0),
            font_size: 13.0,
            indicator_width: 20.0,
            indicator_color: DEFAULT_FG,
            disabled_fg: DEFAULT_DISABLED_FG,
            disabled_bg: DEFAULT_DISABLED_BG,
            focus_ring_color: DEFAULT_FOCUS_RING,
        }
    }
}

/// A dropdown trigger button showing the selected item.
///
/// The popup list is deferred to the overlay system (07.8). For now,
/// the widget tracks selection state and renders the trigger button.
/// Arrow Up/Down keys cycle through items; Enter/Space would open the
/// popup (emitted as `WidgetAction::Clicked` for the app layer).
#[derive(Debug, Clone)]
pub struct DropdownWidget {
    id: WidgetId,
    items: Vec<String>,
    selected: usize,
    disabled: bool,
    hovered: bool,
    pressed: bool,
    style: DropdownStyle,
}

impl DropdownWidget {
    /// Creates a dropdown with the given items, selecting the first.
    ///
    /// Panics if `items` is empty.
    pub fn new(items: Vec<String>) -> Self {
        assert!(!items.is_empty(), "dropdown requires at least one item");
        Self {
            id: WidgetId::next(),
            items,
            selected: 0,
            disabled: false,
            hovered: false,
            pressed: false,
            style: DropdownStyle::default(),
        }
    }

    /// Returns the currently selected index.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Returns the currently selected item text.
    pub fn selected_text(&self) -> &str {
        &self.items[self.selected]
    }

    /// Returns the items list.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// Sets the selected index, clamping to valid range.
    pub fn set_selected(&mut self, index: usize) {
        self.selected = index.min(self.items.len() - 1);
    }

    /// Returns whether the dropdown is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the dropdown is hovered.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Sets the disabled state.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
        if disabled {
            self.hovered = false;
            self.pressed = false;
        }
    }

    /// Sets the selected index via builder.
    #[must_use]
    pub fn with_selected(mut self, index: usize) -> Self {
        self.selected = index.min(self.items.len() - 1);
        self
    }

    /// Sets the disabled state via builder.
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the style.
    #[must_use]
    pub fn with_style(mut self, style: DropdownStyle) -> Self {
        self.style = style;
        self
    }

    /// Returns the current background color.
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

    /// Returns the current text color.
    fn current_fg(&self) -> Color {
        if self.disabled {
            self.style.disabled_fg
        } else {
            self.style.fg
        }
    }

    /// Builds the `TextStyle` for measurement.
    fn text_style(&self) -> TextStyle {
        TextStyle::new(self.style.font_size, self.current_fg())
    }

    /// Selects the next item, wrapping at the end.
    fn select_next(&mut self) -> WidgetAction {
        self.selected = (self.selected + 1) % self.items.len();
        WidgetAction::Selected {
            id: self.id,
            index: self.selected,
        }
    }

    /// Selects the previous item, wrapping at the start.
    fn select_prev(&mut self) -> WidgetAction {
        self.selected = if self.selected == 0 {
            self.items.len() - 1
        } else {
            self.selected - 1
        };
        WidgetAction::Selected {
            id: self.id,
            index: self.selected,
        }
    }
}

impl Widget for DropdownWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        !self.disabled
    }

    fn layout(&self, ctx: &LayoutCtx<'_>) -> LayoutBox {
        // Width accommodates the widest item + padding + indicator.
        let style = self.text_style();
        let max_text_w = self
            .items
            .iter()
            .map(|item| ctx.measurer.measure(item, &style, f32::INFINITY).width)
            .fold(0.0_f32, f32::max);
        let w = max_text_w + self.style.padding.width() + self.style.indicator_width;
        let metrics = ctx.measurer.measure(&self.items[0], &style, f32::INFINITY);
        let h = metrics.height + self.style.padding.height();
        LayoutBox::leaf(w, h).with_widget_id(self.id)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        let focused = ctx.focused_widget == Some(self.id);
        let bounds = ctx.bounds;
        let s = &self.style;

        // Focus ring.
        if focused {
            let ring = bounds.inset(Insets::all(-2.0));
            let ring_style = RectStyle::filled(Color::TRANSPARENT)
                .with_border(2.0, s.focus_ring_color)
                .with_radius(s.corner_radius + 2.0);
            ctx.draw_list.push_rect(ring, ring_style);
        }

        // Background.
        let bg_style = RectStyle::filled(self.current_bg())
            .with_border(s.border_width, s.border_color)
            .with_radius(s.corner_radius);
        ctx.draw_list.push_rect(bounds, bg_style);

        // Selected item text.
        let inner = bounds.inset(s.padding);
        let text_w = inner.width() - s.indicator_width;
        let style = self.text_style();
        let shaped = ctx.measurer.shape(self.selected_text(), &style, text_w);
        let y = inner.y() + (inner.height() - shaped.height) / 2.0;
        ctx.draw_list
            .push_text(Point::new(inner.x(), y), shaped, self.current_fg());

        // Dropdown indicator (simple downward-pointing chevron as two lines).
        let ind_x = bounds.right() - s.indicator_width;
        let ind_center_x = ind_x + s.indicator_width / 2.0;
        let ind_center_y = bounds.y() + bounds.height() / 2.0;
        let arrow_half = 4.0;
        let ind_color = if self.disabled {
            s.disabled_fg
        } else {
            s.indicator_color
        };

        ctx.draw_list.push_line(
            Point::new(ind_center_x - arrow_half, ind_center_y - arrow_half / 2.0),
            Point::new(ind_center_x, ind_center_y + arrow_half / 2.0),
            1.5,
            ind_color,
        );
        ctx.draw_list.push_line(
            Point::new(ind_center_x, ind_center_y + arrow_half / 2.0),
            Point::new(ind_center_x + arrow_half, ind_center_y - arrow_half / 2.0),
            1.5,
            ind_color,
        );
    }

    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
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
                    // Emit Clicked — the app layer would open the popup (07.8).
                    WidgetResponse::redraw().with_action(WidgetAction::Clicked(self.id))
                } else {
                    WidgetResponse::redraw()
                }
            }
            _ => WidgetResponse::ignored(),
        }
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
                self.pressed = false;
                WidgetResponse::redraw()
            }
        }
    }

    fn handle_key(&mut self, event: KeyEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        if self.disabled || !ctx.is_focused {
            return WidgetResponse::ignored();
        }
        match event.key {
            Key::ArrowDown => {
                let action = self.select_next();
                WidgetResponse::redraw().with_action(action)
            }
            Key::ArrowUp => {
                let action = self.select_prev();
                WidgetResponse::redraw().with_action(action)
            }
            Key::Enter | Key::Space => {
                // Would open popup — emit Clicked for app layer.
                WidgetResponse::redraw().with_action(WidgetAction::Clicked(self.id))
            }
            _ => WidgetResponse::ignored(),
        }
    }
}

#[cfg(test)]
mod tests;
