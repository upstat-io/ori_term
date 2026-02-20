//! Toggle switch widget — a pill-shaped on/off switch.
//!
//! Emits `WidgetAction::Toggled` when clicked or activated via Space.
//! Includes a `toggle_progress` field (0.0 = off, 1.0 = on) ready for
//! the animation system (07.9) to interpolate.

use crate::color::Color;
use crate::draw::RectStyle;
use crate::geometry::Rect;
use crate::input::{HoverEvent, Key, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::LayoutBox;
use crate::widget_id::WidgetId;

use super::{
    DEFAULT_ACCENT, DEFAULT_BG, DEFAULT_DISABLED_BG, DEFAULT_DISABLED_FG, DEFAULT_FOCUS_RING,
    DEFAULT_HOVER_BG, DrawCtx, EventCtx, LayoutCtx, Widget, WidgetAction, WidgetResponse,
};

/// Visual style for a [`ToggleWidget`].
#[derive(Debug, Clone, PartialEq)]
pub struct ToggleStyle {
    /// Width of the pill track.
    pub width: f32,
    /// Height of the pill track.
    pub height: f32,
    /// Off-state track background.
    pub off_bg: Color,
    /// Off-state hovered track background.
    pub off_hover_bg: Color,
    /// On-state track background.
    pub on_bg: Color,
    /// Thumb color.
    pub thumb_color: Color,
    /// Padding between track edge and thumb.
    pub thumb_padding: f32,
    /// Disabled track background.
    pub disabled_bg: Color,
    /// Disabled thumb color.
    pub disabled_thumb: Color,
    /// Focus ring color.
    pub focus_ring_color: Color,
}

impl Default for ToggleStyle {
    fn default() -> Self {
        Self {
            width: 40.0,
            height: 22.0,
            off_bg: DEFAULT_BG,
            off_hover_bg: DEFAULT_HOVER_BG,
            on_bg: DEFAULT_ACCENT,
            thumb_color: Color::WHITE,
            thumb_padding: 2.0,
            disabled_bg: DEFAULT_DISABLED_BG,
            disabled_thumb: DEFAULT_DISABLED_FG,
            focus_ring_color: DEFAULT_FOCUS_RING,
        }
    }
}

/// A pill-shaped toggle switch.
///
/// Snaps between on (1.0) and off (0.0). The `toggle_progress` field
/// is exposed for the animation system to interpolate; without animation,
/// it snaps immediately.
#[derive(Debug, Clone)]
pub struct ToggleWidget {
    id: WidgetId,
    on: bool,
    disabled: bool,
    hovered: bool,
    /// Animation progress: 0.0 = off, 1.0 = on.
    /// Snaps immediately; animation system (07.9) will interpolate.
    toggle_progress: f32,
    style: ToggleStyle,
}

impl Default for ToggleWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl ToggleWidget {
    /// Creates a toggle in the off state.
    pub fn new() -> Self {
        Self {
            id: WidgetId::next(),
            on: false,
            disabled: false,
            hovered: false,
            toggle_progress: 0.0,
            style: ToggleStyle::default(),
        }
    }

    /// Returns whether the toggle is on.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Sets the on/off state.
    pub fn set_on(&mut self, on: bool) {
        self.on = on;
        self.toggle_progress = if on { 1.0 } else { 0.0 };
    }

    /// Returns the current animation progress (0.0..1.0).
    pub fn toggle_progress(&self) -> f32 {
        self.toggle_progress
    }

    /// Sets the animation progress directly (for animation system).
    pub fn set_toggle_progress(&mut self, progress: f32) {
        self.toggle_progress = progress.clamp(0.0, 1.0);
    }

    /// Returns whether the toggle is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the toggle is hovered.
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Sets the disabled state.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
        if disabled {
            self.hovered = false;
        }
    }

    /// Sets the style.
    #[must_use]
    pub fn with_style(mut self, style: ToggleStyle) -> Self {
        self.style = style;
        self
    }

    /// Sets initial on state via builder.
    #[must_use]
    pub fn with_on(mut self, on: bool) -> Self {
        self.on = on;
        self.toggle_progress = if on { 1.0 } else { 0.0 };
        self
    }

    /// Sets disabled state via builder.
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Toggles state and returns the action.
    fn toggle(&mut self) -> WidgetAction {
        self.on = !self.on;
        self.toggle_progress = if self.on { 1.0 } else { 0.0 };
        WidgetAction::Toggled {
            id: self.id,
            value: self.on,
        }
    }

    /// Returns the track background based on state.
    fn track_bg(&self) -> Color {
        if self.disabled {
            return self.style.disabled_bg;
        }
        if self.on {
            return self.style.on_bg;
        }
        if self.hovered {
            return self.style.off_hover_bg;
        }
        self.style.off_bg
    }

    /// Returns the thumb color based on state.
    fn thumb_color(&self) -> Color {
        if self.disabled {
            self.style.disabled_thumb
        } else {
            self.style.thumb_color
        }
    }
}

impl Widget for ToggleWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        !self.disabled
    }

    fn layout(&self, _ctx: &LayoutCtx<'_>) -> LayoutBox {
        LayoutBox::leaf(self.style.width, self.style.height).with_widget_id(self.id)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        let focused = ctx.focused_widget == Some(self.id);
        let s = &self.style;
        let radius = s.height / 2.0;

        // Focus ring.
        if focused {
            let ring = ctx.bounds.inset(crate::geometry::Insets::all(-2.0));
            let ring_style = RectStyle::filled(Color::TRANSPARENT)
                .with_border(2.0, s.focus_ring_color)
                .with_radius(radius + 2.0);
            ctx.draw_list.push_rect(ring, ring_style);
        }

        // Track.
        let track_style = RectStyle::filled(self.track_bg()).with_radius(radius);
        ctx.draw_list.push_rect(ctx.bounds, track_style);

        // Thumb — a circle within the track.
        let thumb_diameter = s.height - s.thumb_padding * 2.0;
        let thumb_radius = thumb_diameter / 2.0;
        let travel = s.width - s.thumb_padding * 2.0 - thumb_diameter;
        let thumb_x = ctx.bounds.x() + s.thumb_padding + travel * self.toggle_progress;
        let thumb_y = ctx.bounds.y() + s.thumb_padding;
        let thumb_rect = Rect::new(thumb_x, thumb_y, thumb_diameter, thumb_diameter);
        let thumb_style = RectStyle::filled(self.thumb_color()).with_radius(thumb_radius);
        ctx.draw_list.push_rect(thumb_rect, thumb_style);
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

    fn handle_key(&mut self, event: KeyEvent, _ctx: &EventCtx<'_>) -> WidgetResponse {
        if self.disabled {
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
