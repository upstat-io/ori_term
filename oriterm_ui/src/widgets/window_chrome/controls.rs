//! Window control button widget (minimize, maximize/restore, close).
//!
//! Each button renders its symbol geometrically (lines for ─, rect outline
//! for □, X lines for ×) — no font glyphs needed. Hover state uses
//! [`AnimatedValue`] for smooth 100ms color transitions, matching the
//! [`ButtonWidget`](super::super::button::ButtonWidget) pattern.

use std::time::{Duration, Instant};

use crate::animation::{AnimatedValue, Easing, Lerp};
use crate::color::Color;
use crate::draw::RectStyle;
use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::LayoutBox;
use crate::widget_id::WidgetId;

use super::super::{DrawCtx, EventCtx, LayoutCtx, Widget, WidgetAction, WidgetResponse};
use super::constants::{CONTROL_BUTTON_WIDTH, SYMBOL_SIZE, SYMBOL_STROKE_WIDTH};
use super::layout::ControlKind;

/// Duration of the hover color transition.
const HOVER_DURATION: Duration = Duration::from_millis(100);

/// Colors for a window control button.
///
/// Bundled into a struct to avoid excessive constructor parameters.
#[derive(Debug, Clone, Copy)]
pub struct ControlButtonColors {
    /// Normal foreground (symbol stroke).
    pub fg: Color,
    /// Normal background (transparent when unhovered).
    pub bg: Color,
    /// Hover background for non-close buttons.
    pub hover_bg: Color,
    /// Close button hover background (platform-standard red).
    pub close_hover_bg: Color,
    /// Close button pressed background (darker red).
    pub close_pressed_bg: Color,
}

/// A window control button: minimize, maximize/restore, or close.
///
/// Renders geometric symbols (no font dependency) with animated hover
/// transitions. Emits `WidgetAction::WindowMinimize`, `WindowMaximize`,
/// or `WindowClose` when clicked.
#[derive(Debug, Clone)]
pub struct WindowControlButton {
    id: WidgetId,
    kind: ControlKind,
    /// Whether the window is currently maximized (affects the maximize
    /// button symbol: □ vs ⧉).
    is_maximized: bool,
    hovered: bool,
    pressed: bool,
    hover_progress: AnimatedValue<f32>,
    /// Normal button colors (derived from theme).
    fg: Color,
    bg: Color,
    hover_bg: Color,
    pressed_bg: Color,
    /// Caption background color, set by the chrome widget before drawing.
    /// Used by the restore symbol to occlude the back window outline.
    caption_bg: Color,
    /// Close button hover background (from theme).
    close_hover_bg: Color,
    /// Close button pressed background (from theme).
    close_pressed_bg: Color,
}

impl WindowControlButton {
    /// Creates a new control button of the given kind.
    pub fn new(kind: ControlKind, colors: ControlButtonColors) -> Self {
        Self {
            id: WidgetId::next(),
            kind,
            is_maximized: false,
            hovered: false,
            pressed: false,
            hover_progress: AnimatedValue::new(0.0, HOVER_DURATION, Easing::EaseOut),
            fg: colors.fg,
            bg: colors.bg,
            hover_bg: colors.hover_bg,
            pressed_bg: colors.bg,
            caption_bg: colors.bg,
            close_hover_bg: colors.close_hover_bg,
            close_pressed_bg: colors.close_pressed_bg,
        }
    }

    /// Returns this button's kind.
    pub fn kind(&self) -> ControlKind {
        self.kind
    }

    /// Returns whether this button is currently pressed.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Updates the maximized state (affects maximize/restore symbol).
    pub fn set_maximized(&mut self, maximized: bool) {
        self.is_maximized = maximized;
    }

    /// Sets the caption background color for restore symbol rendering.
    pub fn set_caption_bg(&mut self, color: Color) {
        self.caption_bg = color;
    }

    /// Returns the current background color with hover interpolation.
    fn current_bg(&self, now: Instant) -> Color {
        if self.pressed {
            return self.pressed_color();
        }
        let t = self.hover_progress.get(now);
        Color::lerp(self.bg, self.hover_color(), t)
    }

    /// Returns the foreground (symbol) color — white on close hover.
    fn current_fg(&self, now: Instant) -> Color {
        if self.kind == ControlKind::Close {
            let t = self.hover_progress.get(now);
            Color::lerp(self.fg, Color::WHITE, t)
        } else {
            self.fg
        }
    }

    /// The hover background for this button kind.
    fn hover_color(&self) -> Color {
        if self.kind == ControlKind::Close {
            self.close_hover_bg
        } else {
            self.hover_bg
        }
    }

    /// The pressed background for this button kind.
    fn pressed_color(&self) -> Color {
        if self.kind == ControlKind::Close {
            self.close_pressed_bg
        } else {
            self.pressed_bg
        }
    }

    /// Maps this button kind to the corresponding `WidgetAction`.
    fn action(&self) -> WidgetAction {
        match self.kind {
            ControlKind::Minimize => WidgetAction::WindowMinimize,
            ControlKind::MaximizeRestore => WidgetAction::WindowMaximize,
            ControlKind::Close => WidgetAction::WindowClose,
        }
    }
}

/// Draw the minimize symbol: a horizontal dash centered in the button.
fn draw_minimize(ctx: &mut DrawCtx<'_>, bounds: Rect, fg: Color) {
    let cx = bounds.x() + bounds.width() / 2.0;
    let cy = bounds.y() + bounds.height() / 2.0;
    let half = SYMBOL_SIZE / 2.0;

    ctx.draw_list.push_line(
        Point::new(cx - half, cy),
        Point::new(cx + half, cy),
        SYMBOL_STROKE_WIDTH,
        fg,
    );
}

/// Draw the maximize symbol: a square outline centered in the button.
fn draw_maximize(ctx: &mut DrawCtx<'_>, bounds: Rect, fg: Color) {
    let cx = bounds.x() + bounds.width() / 2.0;
    let cy = bounds.y() + bounds.height() / 2.0;
    let half = SYMBOL_SIZE / 2.0;

    let rect = Rect::new(cx - half, cy - half, SYMBOL_SIZE, SYMBOL_SIZE);
    let style = RectStyle::filled(Color::TRANSPARENT).with_border(SYMBOL_STROKE_WIDTH, fg);
    ctx.draw_list.push_rect(rect, style);
}

/// Draw the restore symbol: two overlapping square outlines.
fn draw_restore(ctx: &mut DrawCtx<'_>, bounds: Rect, fg: Color, caption_bg: Color) {
    let cx = bounds.x() + bounds.width() / 2.0;
    let cy = bounds.y() + bounds.height() / 2.0;
    let size = SYMBOL_SIZE - 2.0;
    let offset = 2.0;

    // Back window (offset up-right).
    let back = Rect::new(
        cx - size / 2.0 + offset,
        cy - size / 2.0 - offset,
        size,
        size,
    );
    let style = RectStyle::filled(Color::TRANSPARENT).with_border(SYMBOL_STROKE_WIDTH, fg);
    ctx.draw_list.push_rect(back, style);

    // Front window (offset down-left) with filled bg to occlude back window.
    let front = Rect::new(cx - size / 2.0, cy - size / 2.0, size, size);
    let bg_style = RectStyle::filled(caption_bg).with_border(SYMBOL_STROKE_WIDTH, fg);
    ctx.draw_list.push_rect(front, bg_style);
}

/// Draw the close symbol: an X centered in the button.
fn draw_close(ctx: &mut DrawCtx<'_>, bounds: Rect, fg: Color) {
    let cx = bounds.x() + bounds.width() / 2.0;
    let cy = bounds.y() + bounds.height() / 2.0;
    let half = SYMBOL_SIZE / 2.0;

    // Top-left to bottom-right diagonal.
    ctx.draw_list.push_line(
        Point::new(cx - half, cy - half),
        Point::new(cx + half, cy + half),
        SYMBOL_STROKE_WIDTH,
        fg,
    );
    // Top-right to bottom-left diagonal.
    ctx.draw_list.push_line(
        Point::new(cx + half, cy - half),
        Point::new(cx - half, cy + half),
        SYMBOL_STROKE_WIDTH,
        fg,
    );
}

impl Widget for WindowControlButton {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        false
    }

    fn layout(&self, _ctx: &LayoutCtx<'_>) -> LayoutBox {
        LayoutBox::leaf(CONTROL_BUTTON_WIDTH, 0.0).with_widget_id(self.id)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        let bg = self.current_bg(ctx.now);
        let fg = self.current_fg(ctx.now);

        // Button background (only visible on hover/press).
        if self.hovered || self.pressed || self.hover_progress.is_animating(ctx.now) {
            let style = RectStyle::filled(bg);
            ctx.draw_list.push_rect(ctx.bounds, style);
        }

        // Symbol glyph.
        match self.kind {
            ControlKind::Minimize => draw_minimize(ctx, ctx.bounds, fg),
            ControlKind::MaximizeRestore => {
                if self.is_maximized {
                    draw_restore(ctx, ctx.bounds, fg, self.caption_bg);
                } else {
                    draw_maximize(ctx, ctx.bounds, fg);
                }
            }
            ControlKind::Close => draw_close(ctx, ctx.bounds, fg),
        }

        // Request continued redraws during animation.
        if self.hover_progress.is_animating(ctx.now) {
            ctx.animations_running.set(true);
        }
    }

    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.pressed = true;
                WidgetResponse::redraw()
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let was_pressed = self.pressed;
                self.pressed = false;
                if was_pressed && ctx.bounds.contains(event.pos) {
                    WidgetResponse::redraw().with_action(self.action())
                } else {
                    WidgetResponse::redraw()
                }
            }
            _ => WidgetResponse::ignored(),
        }
    }

    fn handle_hover(&mut self, event: HoverEvent, _ctx: &EventCtx<'_>) -> WidgetResponse {
        let now = Instant::now();
        match event {
            HoverEvent::Enter => {
                self.hovered = true;
                self.hover_progress.set(1.0, now);
                WidgetResponse::redraw()
            }
            HoverEvent::Leave => {
                self.hovered = false;
                self.pressed = false;
                self.hover_progress.set(0.0, now);
                WidgetResponse::redraw()
            }
        }
    }

    fn handle_key(&mut self, _event: KeyEvent, _ctx: &EventCtx<'_>) -> WidgetResponse {
        WidgetResponse::ignored()
    }
}
