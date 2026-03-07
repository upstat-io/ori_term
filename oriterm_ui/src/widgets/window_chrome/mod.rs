//! Window chrome widget — title bar with minimize/maximize/close controls.
//!
//! [`WindowChromeWidget`] composes a title label area with three
//! [`WindowControlButton`]s in a horizontal row. It draws the caption
//! background, manages active/inactive state, and exposes
//! [`interactive_rects`](WindowChromeWidget::interactive_rects) for
//! platform hit testing.

pub mod constants;
pub mod controls;
pub mod layout;

use crate::animation::Lerp;
use crate::color::Color;
use crate::draw::RectStyle;
use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::LayoutBox;
use crate::theme::UiTheme;
use crate::widget_id::WidgetId;

use self::controls::{ControlButtonColors, WindowControlButton};
use self::layout::{ChromeLayout, ControlKind};
use super::{DrawCtx, EventCtx, LayoutCtx, Widget, WidgetResponse};

/// Window chrome widget: caption bar with title and window controls.
///
/// Draws a colored bar at the top of the window containing a title string
/// and three control buttons (minimize, maximize/restore, close). The caption
/// background is draggable (the platform layer uses [`interactive_rects`] to
/// exclude button areas from the drag zone).
pub struct WindowChromeWidget {
    id: WidgetId,
    title: String,
    /// Whether the window is currently active (focused).
    active: bool,
    /// Whether the window is currently maximized.
    is_maximized: bool,
    /// Whether the window is currently fullscreen.
    is_fullscreen: bool,
    /// Window width in logical pixels (updated on resize).
    window_width: f32,
    /// Cached layout (recomputed on state/size change).
    chrome_layout: ChromeLayout,
    /// Control buttons: [minimize, maximize, close].
    controls: [WindowControlButton; 3],
    /// Index of the currently hovered control button (None if no hover).
    hovered_control: Option<usize>,
    /// Caption background color (active).
    caption_bg: Color,
    /// Caption background color (inactive / unfocused).
    caption_bg_inactive: Color,
    /// Caption foreground (title text) color.
    caption_fg: Color,
}

impl WindowChromeWidget {
    /// Creates a new window chrome widget with default dark theme colors.
    pub fn new(title: impl Into<String>, window_width: f32) -> Self {
        Self::with_theme(title, window_width, &UiTheme::dark())
    }

    /// Creates a new window chrome widget with colors from the given theme.
    pub fn with_theme(title: impl Into<String>, window_width: f32, theme: &UiTheme) -> Self {
        let chrome_layout = ChromeLayout::compute(window_width, false, false);

        let caption_bg = theme.bg_secondary;

        let colors = ControlButtonColors {
            fg: theme.fg_primary,
            bg: Color::TRANSPARENT,
            hover_bg: theme.bg_hover,
            close_hover_bg: theme.close_hover_bg,
            close_pressed_bg: theme.close_pressed_bg,
        };

        let mut min_btn = WindowControlButton::new(ControlKind::Minimize, colors);
        min_btn.set_caption_bg(caption_bg);
        let mut max_btn = WindowControlButton::new(ControlKind::MaximizeRestore, colors);
        max_btn.set_caption_bg(caption_bg);
        let mut close_btn = WindowControlButton::new(ControlKind::Close, colors);
        close_btn.set_caption_bg(caption_bg);

        Self {
            id: WidgetId::next(),
            title: title.into(),
            active: true,
            is_maximized: false,
            is_fullscreen: false,
            window_width,
            chrome_layout,
            controls: [min_btn, max_btn, close_btn],
            hovered_control: None,
            caption_bg,
            caption_bg_inactive: darken(caption_bg, 0.3),
            caption_fg: theme.fg_secondary,
        }
    }

    // Accessors

    /// Returns the caption height in logical pixels (0 if fullscreen).
    pub fn caption_height(&self) -> f32 {
        self.chrome_layout.caption_height
    }

    /// Returns the interactive rects for hit test exclusion.
    ///
    /// These are the button rects within the caption area. Points inside
    /// these rects should be treated as `Client` hits (clickable), not
    /// `Caption` (draggable).
    pub fn interactive_rects(&self) -> &[Rect] {
        &self.chrome_layout.interactive_rects
    }

    /// Whether the chrome is visible (false in fullscreen).
    pub fn is_visible(&self) -> bool {
        self.chrome_layout.visible
    }

    // State updates

    /// Sets the window title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Sets the active/focused state.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        self.sync_caption_bg();
    }

    /// Sets the maximized state and recomputes layout.
    pub fn set_maximized(&mut self, maximized: bool) {
        self.is_maximized = maximized;
        for ctrl in &mut self.controls {
            ctrl.set_maximized(maximized);
        }
        self.recompute_layout();
    }

    /// Sets the fullscreen state and recomputes layout.
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.is_fullscreen = fullscreen;
        self.recompute_layout();
    }

    /// Updates the window width and recomputes layout.
    pub fn set_window_width(&mut self, width: f32) {
        self.window_width = width;
        self.recompute_layout();
    }

    /// Updates all theme-derived colors from a new [`UiTheme`].
    pub fn apply_theme(&mut self, theme: &UiTheme) {
        self.caption_bg = theme.bg_secondary;
        self.caption_bg_inactive = darken(theme.bg_secondary, 0.3);
        self.caption_fg = theme.fg_secondary;
        let colors = ControlButtonColors {
            fg: theme.fg_primary,
            bg: Color::TRANSPARENT,
            hover_bg: theme.bg_hover,
            close_hover_bg: theme.close_hover_bg,
            close_pressed_bg: theme.close_pressed_bg,
        };
        for ctrl in &mut self.controls {
            ctrl.set_colors(colors);
        }
        self.sync_caption_bg();
    }

    /// Recomputes the chrome layout from current state.
    fn recompute_layout(&mut self) {
        self.chrome_layout =
            ChromeLayout::compute(self.window_width, self.is_maximized, self.is_fullscreen);
    }

    /// Returns the current caption background color based on active state.
    fn current_caption_bg(&self) -> Color {
        if self.active {
            self.caption_bg
        } else {
            self.caption_bg_inactive
        }
    }

    /// Syncs the caption background color to all control buttons.
    fn sync_caption_bg(&mut self) {
        let bg = self.current_caption_bg();
        for ctrl in &mut self.controls {
            ctrl.set_caption_bg(bg);
        }
    }

    /// Finds which control button (if any) contains the given point.
    fn control_at_point(&self, point: Point) -> Option<usize> {
        self.chrome_layout
            .controls
            .iter()
            .position(|c| c.rect.contains(point))
    }
}

impl Widget for WindowChromeWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        false
    }

    fn layout(&self, _ctx: &LayoutCtx<'_>) -> LayoutBox {
        LayoutBox::leaf(self.window_width, self.chrome_layout.caption_height)
            .with_widget_id(self.id)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        if !self.chrome_layout.visible {
            return;
        }

        // Layer captures the caption bg for subpixel title text compositing.
        let bg = self.current_caption_bg();
        ctx.draw_list.push_layer(bg);

        // Caption background bar.
        let caption_rect = Rect::new(0.0, 0.0, ctx.bounds.width(), self.caption_height());
        ctx.draw_list.push_rect(caption_rect, RectStyle::filled(bg));

        // Title text (centered vertically in the title area).
        if !self.title.is_empty() {
            let title_rect = self.chrome_layout.title_rect;
            let style = crate::text::TextStyle::new(ctx.theme.font_size_small, self.caption_fg);
            let shaped = ctx.measurer.shape(&self.title, &style, title_rect.width());
            let x = title_rect.x() + 8.0;
            let y = title_rect.y() + (title_rect.height() - shaped.height) / 2.0;
            ctx.draw_list
                .push_text(Point::new(x, y), shaped, self.caption_fg);
        }

        ctx.draw_list.pop_layer();

        // Control buttons (outside the caption layer — each button has its own bg).
        for (i, ctrl) in self.controls.iter().enumerate() {
            let ctrl_rect = self.chrome_layout.controls[i].rect;
            let mut child_ctx = DrawCtx {
                measurer: ctx.measurer,
                draw_list: ctx.draw_list,
                bounds: ctrl_rect,
                focused_widget: ctx.focused_widget,
                now: ctx.now,
                animations_running: ctx.animations_running,
                theme: ctx.theme,
            };
            ctrl.draw(&mut child_ctx);
        }
    }

    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        if !self.chrome_layout.visible {
            return WidgetResponse::ignored();
        }

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(idx) = self.control_at_point(event.pos) {
                    let ctrl_rect = self.chrome_layout.controls[idx].rect;
                    let child_ctx = EventCtx {
                        measurer: ctx.measurer,
                        bounds: ctrl_rect,
                        is_focused: false,
                        focused_widget: ctx.focused_widget,
                        theme: ctx.theme,
                    };
                    return self.controls[idx].handle_mouse(event, &child_ctx);
                }
                WidgetResponse::ignored()
            }
            MouseEventKind::Up(MouseButton::Left) => {
                // Route release to the control that was pressed.
                for (i, ctrl) in self.controls.iter_mut().enumerate() {
                    if ctrl.is_pressed() {
                        let ctrl_rect = self.chrome_layout.controls[i].rect;
                        let child_ctx = EventCtx {
                            measurer: ctx.measurer,
                            bounds: ctrl_rect,
                            is_focused: false,
                            focused_widget: ctx.focused_widget,
                            theme: ctx.theme,
                        };
                        return ctrl.handle_mouse(event, &child_ctx);
                    }
                }
                WidgetResponse::ignored()
            }
            _ => WidgetResponse::ignored(),
        }
    }

    fn handle_hover(&mut self, event: HoverEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        if !self.chrome_layout.visible {
            return WidgetResponse::ignored();
        }

        match event {
            HoverEvent::Enter => WidgetResponse::ignored(),
            HoverEvent::Leave => {
                // Clear hover on all controls when leaving the chrome area.
                if let Some(idx) = self.hovered_control.take() {
                    let ctrl_rect = self.chrome_layout.controls[idx].rect;
                    let child_ctx = EventCtx {
                        measurer: ctx.measurer,
                        bounds: ctrl_rect,
                        is_focused: false,
                        focused_widget: ctx.focused_widget,
                        theme: ctx.theme,
                    };
                    self.controls[idx].handle_hover(HoverEvent::Leave, &child_ctx)
                } else {
                    WidgetResponse::ignored()
                }
            }
        }
    }

    fn handle_key(&mut self, _event: KeyEvent, _ctx: &EventCtx<'_>) -> WidgetResponse {
        WidgetResponse::ignored()
    }
}

impl WindowChromeWidget {
    /// Update hover state based on cursor position.
    ///
    /// Called by the app layer on cursor move. Routes hover enter/leave
    /// events to the appropriate control button.
    pub fn update_hover(&mut self, pos: Point, ctx: &EventCtx<'_>) -> WidgetResponse {
        if !self.chrome_layout.visible {
            return WidgetResponse::ignored();
        }

        let new_idx = self.control_at_point(pos);

        // No change — nothing to do.
        if new_idx == self.hovered_control {
            return WidgetResponse::ignored();
        }

        // Leave old control.
        let left = if let Some(old) = self.hovered_control {
            let ctrl_rect = self.chrome_layout.controls[old].rect;
            let child_ctx = EventCtx {
                measurer: ctx.measurer,
                bounds: ctrl_rect,
                is_focused: false,
                focused_widget: ctx.focused_widget,
                theme: ctx.theme,
            };
            self.controls[old].handle_hover(HoverEvent::Leave, &child_ctx);
            true
        } else {
            false
        };

        // Enter new control.
        let entered = if let Some(new) = new_idx {
            let ctrl_rect = self.chrome_layout.controls[new].rect;
            let child_ctx = EventCtx {
                measurer: ctx.measurer,
                bounds: ctrl_rect,
                is_focused: false,
                focused_widget: ctx.focused_widget,
                theme: ctx.theme,
            };
            self.controls[new].handle_hover(HoverEvent::Enter, &child_ctx);
            true
        } else {
            false
        };

        self.hovered_control = new_idx;

        if left || entered {
            WidgetResponse::redraw()
        } else {
            WidgetResponse::ignored()
        }
    }
}

// Test helpers

#[cfg(test)]
impl WindowChromeWidget {
    /// Test-only access to the active caption background.
    pub fn test_caption_bg(&self) -> Color {
        self.caption_bg
    }

    /// Test-only access to the inactive caption background.
    pub fn test_caption_bg_inactive(&self) -> Color {
        self.caption_bg_inactive
    }

    /// Test-only access to the caption foreground (title text).
    pub fn test_caption_fg(&self) -> Color {
        self.caption_fg
    }

    /// Test-only access to the hovered control index.
    pub fn test_hovered_control(&self) -> Option<usize> {
        self.hovered_control
    }

    /// Test-only access to the maximized flag.
    pub fn test_is_maximized(&self) -> bool {
        self.is_maximized
    }
}

/// Darken a color by blending toward black.
fn darken(color: Color, amount: f32) -> Color {
    Color::lerp(color, Color::BLACK, amount)
}

#[cfg(test)]
mod tests;
