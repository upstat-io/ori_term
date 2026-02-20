//! Slider widget — a horizontal track with draggable thumb.
//!
//! Emits `WidgetAction::ValueChanged` when the value changes via drag
//! or arrow keys. Supports configurable min/max/step and keyboard
//! adjustment (arrow keys, Home/End).

use crate::color::Color;
use crate::draw::RectStyle;
use crate::geometry::Rect;
use crate::input::{HoverEvent, Key, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::LayoutBox;
use crate::widget_id::WidgetId;

use super::{
    DEFAULT_ACCENT, DEFAULT_BG, DEFAULT_BORDER, DEFAULT_DISABLED_BG, DEFAULT_DISABLED_FG,
    DEFAULT_FOCUS_RING, DEFAULT_HOVER_BG, DrawCtx, EventCtx, LayoutCtx, Widget, WidgetAction,
    WidgetResponse,
};

/// Visual style for a [`SliderWidget`].
#[derive(Debug, Clone, PartialEq)]
pub struct SliderStyle {
    /// Total width of the slider (track + thumb).
    pub width: f32,
    /// Track height.
    pub track_height: f32,
    /// Track background color.
    pub track_bg: Color,
    /// Filled portion color (left of thumb).
    pub fill_color: Color,
    /// Track corner radius.
    pub track_radius: f32,
    /// Thumb diameter.
    pub thumb_size: f32,
    /// Thumb color.
    pub thumb_color: Color,
    /// Thumb color when hovered.
    pub thumb_hover_color: Color,
    /// Thumb border color.
    pub thumb_border_color: Color,
    /// Thumb border width.
    pub thumb_border_width: f32,
    /// Disabled track/thumb color.
    pub disabled_bg: Color,
    /// Disabled fill color.
    pub disabled_fill: Color,
    /// Focus ring color.
    pub focus_ring_color: Color,
}

impl Default for SliderStyle {
    fn default() -> Self {
        Self {
            width: 200.0,
            track_height: 4.0,
            track_bg: DEFAULT_BG,
            fill_color: DEFAULT_ACCENT,
            track_radius: 2.0,
            thumb_size: 16.0,
            thumb_color: Color::WHITE,
            thumb_hover_color: DEFAULT_HOVER_BG,
            thumb_border_color: DEFAULT_BORDER,
            thumb_border_width: 1.0,
            disabled_bg: DEFAULT_DISABLED_BG,
            disabled_fill: DEFAULT_DISABLED_FG,
            focus_ring_color: DEFAULT_FOCUS_RING,
        }
    }
}

/// A horizontal slider with track and draggable thumb.
///
/// Value ranges from `min` to `max` with optional `step` snapping.
/// Arrow keys adjust by `step`, Home/End jump to min/max.
#[derive(Debug, Clone)]
pub struct SliderWidget {
    id: WidgetId,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    disabled: bool,
    hovered: bool,
    dragging: bool,
    style: SliderStyle,
}

impl Default for SliderWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl SliderWidget {
    /// Creates a slider with value 0.0, range 0.0..1.0, step 0.01.
    pub fn new() -> Self {
        Self {
            id: WidgetId::next(),
            value: 0.0,
            min: 0.0,
            max: 1.0,
            step: 0.01,
            disabled: false,
            hovered: false,
            dragging: false,
            style: SliderStyle::default(),
        }
    }

    /// Returns the current value.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Sets the value, clamping to [min, max].
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(self.min, self.max);
    }

    /// Returns the minimum value.
    pub fn min(&self) -> f32 {
        self.min
    }

    /// Returns the maximum value.
    pub fn max(&self) -> f32 {
        self.max
    }

    /// Returns whether the slider is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the slider is hovered.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Returns whether the thumb is being dragged.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Sets the range.
    #[must_use]
    pub fn with_range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max;
        self.value = self.value.clamp(min, max);
        self
    }

    /// Sets the step increment.
    #[must_use]
    pub fn with_step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Sets the initial value.
    #[must_use]
    pub fn with_value(mut self, value: f32) -> Self {
        self.value = value.clamp(self.min, self.max);
        self
    }

    /// Sets the disabled state.
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the style.
    #[must_use]
    pub fn with_style(mut self, style: SliderStyle) -> Self {
        self.style = style;
        self
    }

    /// Returns the normalized position (0.0..1.0) of the current value.
    fn normalized(&self) -> f32 {
        if (self.max - self.min).abs() < f32::EPSILON {
            return 0.0;
        }
        (self.value - self.min) / (self.max - self.min)
    }

    /// Converts a pixel X position within bounds to a value.
    fn value_from_x(&self, x: f32, bounds: Rect) -> f32 {
        let half_thumb = self.style.thumb_size / 2.0;
        let track_left = bounds.x() + half_thumb;
        let track_width = bounds.width() - self.style.thumb_size;
        if track_width <= 0.0 {
            return self.min;
        }
        let t = ((x - track_left) / track_width).clamp(0.0, 1.0);
        let raw = self.min + t * (self.max - self.min);
        self.snap_to_step(raw)
    }

    /// Snaps a raw value to the nearest step.
    fn snap_to_step(&self, raw: f32) -> f32 {
        if self.step <= 0.0 {
            return raw.clamp(self.min, self.max);
        }
        let steps = ((raw - self.min) / self.step).round();
        (self.min + steps * self.step).clamp(self.min, self.max)
    }

    /// Sets value and returns action if it changed.
    fn set_value_action(&mut self, new_value: f32) -> Option<WidgetAction> {
        let clamped = new_value.clamp(self.min, self.max);
        if (clamped - self.value).abs() < f32::EPSILON {
            return None;
        }
        self.value = clamped;
        Some(WidgetAction::ValueChanged {
            id: self.id,
            value: self.value,
        })
    }
}

impl Widget for SliderWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        !self.disabled
    }

    fn layout(&self, _ctx: &LayoutCtx<'_>) -> LayoutBox {
        let height = self.style.thumb_size.max(self.style.track_height);
        LayoutBox::leaf(self.style.width, height).with_widget_id(self.id)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        let focused = ctx.focused_widget == Some(self.id);
        let s = &self.style;
        let bounds = ctx.bounds;

        // Focus ring.
        if focused {
            let ring = bounds.inset(crate::geometry::Insets::all(-2.0));
            let ring_style = RectStyle::filled(Color::TRANSPARENT)
                .with_border(2.0, s.focus_ring_color)
                .with_radius(s.track_radius + 2.0);
            ctx.draw_list.push_rect(ring, ring_style);
        }

        // Track background.
        let track_y = bounds.y() + (bounds.height() - s.track_height) / 2.0;
        let track_rect = Rect::new(bounds.x(), track_y, bounds.width(), s.track_height);
        let bg_color = if self.disabled {
            s.disabled_bg
        } else {
            s.track_bg
        };
        let track_style = RectStyle::filled(bg_color).with_radius(s.track_radius);
        ctx.draw_list.push_rect(track_rect, track_style);

        // Filled portion (left of thumb).
        let norm = self.normalized();
        let fill_width = norm * bounds.width();
        if fill_width > 0.0 {
            let fill_rect = Rect::new(bounds.x(), track_y, fill_width, s.track_height);
            let fill_color = if self.disabled {
                s.disabled_fill
            } else {
                s.fill_color
            };
            let fill_style = RectStyle::filled(fill_color).with_radius(s.track_radius);
            ctx.draw_list.push_rect(fill_rect, fill_style);
        }

        // Thumb.
        let half_thumb = s.thumb_size / 2.0;
        let travel = bounds.width() - s.thumb_size;
        let thumb_x = bounds.x() + travel * norm;
        let thumb_y = bounds.y() + (bounds.height() - s.thumb_size) / 2.0;
        let thumb_rect = Rect::new(thumb_x, thumb_y, s.thumb_size, s.thumb_size);
        let thumb_bg = if self.disabled {
            s.disabled_bg
        } else if self.hovered || self.dragging {
            s.thumb_hover_color
        } else {
            s.thumb_color
        };
        let thumb_style = RectStyle::filled(thumb_bg)
            .with_border(s.thumb_border_width, s.thumb_border_color)
            .with_radius(half_thumb);
        ctx.draw_list.push_rect(thumb_rect, thumb_style);
    }

    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        if self.disabled {
            return WidgetResponse::ignored();
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.dragging = true;
                let new_val = self.value_from_x(event.pos.x, ctx.bounds);
                let action = self.set_value_action(new_val);
                let mut r = WidgetResponse::focus();
                if let Some(a) = action {
                    r = r.with_action(a);
                }
                r
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.dragging = false;
                WidgetResponse::redraw()
            }
            MouseEventKind::Move if self.dragging => {
                let new_val = self.value_from_x(event.pos.x, ctx.bounds);
                let action = self.set_value_action(new_val);
                let mut r = WidgetResponse::redraw();
                if let Some(a) = action {
                    r = r.with_action(a);
                }
                r
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
                WidgetResponse::redraw()
            }
        }
    }

    fn handle_key(&mut self, event: KeyEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        if self.disabled || !ctx.is_focused {
            return WidgetResponse::ignored();
        }
        let delta = match event.key {
            Key::ArrowRight | Key::ArrowUp => self.step,
            Key::ArrowLeft | Key::ArrowDown => -self.step,
            Key::Home => {
                let action = self.set_value_action(self.min);
                let mut r = WidgetResponse::redraw();
                if let Some(a) = action {
                    r = r.with_action(a);
                }
                return r;
            }
            Key::End => {
                let action = self.set_value_action(self.max);
                let mut r = WidgetResponse::redraw();
                if let Some(a) = action {
                    r = r.with_action(a);
                }
                return r;
            }
            _ => return WidgetResponse::ignored(),
        };
        let new_val = self.snap_to_step(self.value + delta);
        let action = self.set_value_action(new_val);
        let mut r = WidgetResponse::redraw();
        if let Some(a) = action {
            r = r.with_action(a);
        }
        r
    }
}

#[cfg(test)]
mod tests;
