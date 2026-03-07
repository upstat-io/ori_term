//! Tab bar drawing implementation.
//!
//! Contains all visual rendering logic: tab backgrounds, title text, close
//! buttons, separators, new-tab/dropdown buttons, dragged tab overlay, and
//! the bell animation phase. The [`Widget`] trait impl routes into these
//! drawing helpers.

use std::time::Instant;

use crate::animation::Lerp;
use crate::color::Color;
use crate::draw::RectStyle;
use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, KeyEvent, MouseEvent};
use crate::layout::LayoutBox;
use crate::text::{TextOverflow, TextStyle};

use super::super::constants::{
    CLOSE_BUTTON_RIGHT_PAD, CLOSE_BUTTON_WIDTH, DROPDOWN_BUTTON_WIDTH, ICON_TEXT_GAP,
    NEW_TAB_BUTTON_WIDTH, TAB_BAR_HEIGHT, TAB_PADDING, TAB_TOP_MARGIN,
};
use super::super::hit::TabBarHit;
use super::{TabBarWidget, TabEntry, TabIcon};

use crate::widgets::{DrawCtx, EventCtx, LayoutCtx, Widget, WidgetResponse};

// Drawing constants (logical pixels).

/// Corner radius for the active tab's top-left and top-right corners.
pub(super) const ACTIVE_TAB_RADIUS: f32 = 8.0;

/// Corner radius for hover backgrounds on buttons and close targets.
const BUTTON_HOVER_RADIUS: f32 = 4.0;

/// Inset from close button edges to the × crosshair center.
pub(super) const CLOSE_ICON_INSET: f32 = 7.0;

/// Line thickness for icon strokes (× close, + new tab, ▾ dropdown).
pub(super) const ICON_STROKE_WIDTH: f32 = 1.5;

/// Half-arm length of the + icon in the new-tab button.
const PLUS_ARM: f32 = 6.0;

/// Half-width of the ▾ chevron in the dropdown button.
const CHEVRON_HALF_W: f32 = 4.0;

/// Half-height of the ▾ chevron in the dropdown button.
const CHEVRON_HALF_H: f32 = 3.0;

/// Vertical inset for separators from tab top/bottom edges.
const SEPARATOR_INSET: f32 = 8.0;

/// Duration of the bell pulse animation in seconds.
const BELL_DURATION_SECS: f32 = 3.0;

/// Frequency of the bell pulse sine wave in Hz.
const BELL_FREQUENCY_HZ: f32 = 2.0;

/// Tab strip geometry and per-tab draw state passed to drawing helpers.
pub(super) struct TabStrip {
    /// Y coordinate of the tab tops (after top margin).
    pub(super) y: f32,
    /// Height of each tab (bar height minus top margin).
    pub(super) h: f32,
    /// Whether the current tab being drawn is the active tab.
    pub(super) active: bool,
    /// Bell animation phase for the current tab (0.0 if none).
    pub(super) bell: f32,
    /// Title/icon text color for the current tab.
    pub(super) text_color: Color,
}

// Drawing helpers

impl TabBarWidget {
    /// Draws a single tab (background, title text, close button).
    ///
    /// Per-tab state (`active`, `bell` phase) is carried in `strip` to keep
    /// the argument count within clippy's limit.
    fn draw_tab(&self, ctx: &mut DrawCtx<'_>, index: usize, strip: &TabStrip) {
        let tab = &self.tabs[index];
        let x = self.layout.tab_x(index) + self.anim_offset(index);
        let tab_rect = Rect::new(x, strip.y, self.layout.tab_width_at(index), strip.h);
        let bg = self.tab_background_color(index, strip, ctx.now);

        // Active tab gets rounded top corners.
        let style = if strip.active {
            RectStyle::filled(bg).with_per_corner_radius(
                ACTIVE_TAB_RADIUS,
                ACTIVE_TAB_RADIUS,
                0.0,
                0.0,
            )
        } else {
            RectStyle::filled(bg)
        };

        // Width multiplier — content fades in faster than width expands.
        let width_t = self
            .width_multipliers
            .get(index)
            .map_or(1.0, |m| m.get(ctx.now));
        let content_opacity = (width_t * 2.0).min(1.0);

        // Clip tab content to its bounds — prevents overflow into adjacent tabs.
        ctx.draw_list.push_clip(tab_rect);
        ctx.draw_list.push_layer(bg);
        ctx.draw_list.push_rect(tab_rect, style);

        // Only draw content when somewhat visible.
        if content_opacity > 0.01 {
            self.draw_tab_label(ctx, tab, x, strip);

            // Close button: always visible on active, animated fade on inactive.
            if strip.active {
                self.draw_close_button(ctx, index, x, strip, content_opacity);
            } else {
                let opacity = self
                    .close_btn_opacity
                    .get(index)
                    .map_or(0.0, |o| o.get(ctx.now));
                let opacity = opacity * content_opacity;
                if opacity > 0.01 {
                    self.draw_close_button(ctx, index, x, strip, opacity);
                }
            }
        }

        ctx.draw_list.pop_layer();
        ctx.draw_list.pop_clip();
    }

    /// Resolves the background color for a tab.
    ///
    /// Priority: active bg > bell pulse > animated hover blend over inactive bg.
    fn tab_background_color(&self, index: usize, strip: &TabStrip, now: Instant) -> Color {
        if strip.active {
            self.colors.active_bg
        } else if strip.bell > 0.0 {
            self.colors.bell_pulse(strip.bell)
        } else {
            let hover_t = self.hover_progress.get(index).map_or(0.0, |p| p.get(now));
            Color::lerp(self.colors.inactive_bg, self.colors.tab_hover_bg, hover_t)
        }
    }

    /// Draws the icon (if any) and title text for a tab at the given X position.
    ///
    /// Shared between [`draw_tab`] (normal tabs) and [`draw_dragged_tab_overlay`]
    /// (floating drag overlay) — the only differences are position and color.
    /// Text color comes from `strip.text_color`.
    pub(super) fn draw_tab_label(
        &self,
        ctx: &mut DrawCtx<'_>,
        tab: &TabEntry,
        x: f32,
        strip: &TabStrip,
    ) {
        let color = strip.text_color;

        // Icon rendering: shape and draw emoji before the title.
        let text_offset = if let Some(TabIcon::Emoji(ref emoji)) = tab.icon {
            let icon_style = TextStyle::new(ctx.theme.font_size_small, color);
            let icon_shaped = ctx.measurer.shape(emoji, &icon_style, f32::INFINITY);
            let icon_x = x + TAB_PADDING;
            let icon_y = strip.y + (strip.h - icon_shaped.height) / 2.0;
            let icon_w = icon_shaped.width;
            ctx.draw_list
                .push_text(Point::new(icon_x, icon_y), icon_shaped, color);
            icon_w + ICON_TEXT_GAP
        } else {
            0.0
        };

        let title = if tab.title.is_empty() {
            "Terminal"
        } else {
            &tab.title
        };
        let max_w = (self.layout.max_text_width() - text_offset).max(0.0);
        let text_style =
            TextStyle::new(ctx.theme.font_size_small, color).with_overflow(TextOverflow::Ellipsis);
        let shaped = ctx.measurer.shape(title, &text_style, max_w);
        let text_x = x + TAB_PADDING + text_offset;
        let text_y = strip.y + (strip.h - shaped.height) / 2.0;
        ctx.draw_list
            .push_text(Point::new(text_x, text_y), shaped, color);
    }

    /// Draws the close (×) button for a tab with the given opacity.
    #[expect(
        clippy::too_many_arguments,
        reason = "close button draw: self + ctx + index + tab_x + strip + opacity"
    )]
    fn draw_close_button(
        &self,
        ctx: &mut DrawCtx<'_>,
        index: usize,
        tab_x: f32,
        strip: &TabStrip,
        opacity: f32,
    ) {
        let cx =
            tab_x + self.layout.tab_width_at(index) - CLOSE_BUTTON_RIGHT_PAD - CLOSE_BUTTON_WIDTH;
        let cy = strip.y + (strip.h - CLOSE_BUTTON_WIDTH) / 2.0;
        let btn = Rect::new(cx, cy, CLOSE_BUTTON_WIDTH, CLOSE_BUTTON_WIDTH);

        // Hover highlight on the close button.
        if self.hover_hit == TabBarHit::CloseTab(index) {
            let style = RectStyle::filled(self.colors.button_hover_bg.with_alpha(opacity))
                .with_radius(BUTTON_HOVER_RADIUS);
            ctx.draw_list.push_rect(btn, style);
        }

        // × lines.
        let x1 = cx + CLOSE_ICON_INSET;
        let y1 = cy + CLOSE_ICON_INSET;
        let x2 = cx + CLOSE_BUTTON_WIDTH - CLOSE_ICON_INSET;
        let y2 = cy + CLOSE_BUTTON_WIDTH - CLOSE_ICON_INSET;
        let fg = self.colors.close_fg.with_alpha(opacity);
        ctx.draw_list.push_line(
            Point::new(x1, y1),
            Point::new(x2, y2),
            ICON_STROKE_WIDTH,
            fg,
        );
        ctx.draw_list.push_line(
            Point::new(x1, y2),
            Point::new(x2, y1),
            ICON_STROKE_WIDTH,
            fg,
        );
    }

    /// Draws inactive tabs, active tab, and checks for running animations.
    ///
    /// Inactive tabs draw first (behind active). After all tabs, any running
    /// hover/close/width/bell animations request continued redraws.
    fn draw_all_tabs(&self, ctx: &mut DrawCtx<'_>, strip: &mut TabStrip) {
        // Inactive tabs (drawn first, behind active tab).
        for i in 0..self.tabs.len() {
            if i == self.active_index || self.is_dragged(i) {
                continue;
            }
            strip.active = false;
            strip.bell = bell_phase(&self.tabs[i], ctx.now);
            strip.text_color = self.colors.inactive_text;
            self.draw_tab(ctx, i, strip);

            if strip.bell > 0.0 {
                ctx.animations_running.set(true);
            }
        }

        // Active tab (drawn on top of inactive tabs).
        if self.active_index < self.tabs.len() && !self.is_dragged(self.active_index) {
            strip.active = true;
            strip.bell = 0.0;
            strip.text_color = self.colors.text_fg;
            self.draw_tab(ctx, self.active_index, strip);
        }

        // Request continued redraws if any animation is running.
        let hover_animating = self.hover_progress.iter().any(|p| p.is_animating(ctx.now));
        let close_animating = self
            .close_btn_opacity
            .iter()
            .any(|o| o.is_animating(ctx.now));
        let width_animating = self.has_width_animation(ctx.now);
        if hover_animating || close_animating || width_animating {
            ctx.animations_running.set(true);
        }
    }

    /// Draws separators between tabs with suppression rules.
    fn draw_separators(&self, ctx: &mut DrawCtx<'_>, strip: &TabStrip) {
        for i in 1..self.tabs.len() {
            // Suppress adjacent to active tab.
            if i == self.active_index || i == self.active_index + 1 {
                continue;
            }
            // Suppress adjacent to hovered tab.
            if let TabBarHit::Tab(h) | TabBarHit::CloseTab(h) = self.hover_hit {
                if i == h || i == h + 1 {
                    continue;
                }
            }
            // Suppress adjacent to dragged tab.
            if let Some((d, _)) = self.drag_visual {
                if i == d || i == d + 1 {
                    continue;
                }
            }

            let x = self.layout.tab_x(i);
            let y1 = strip.y + SEPARATOR_INSET;
            let y2 = strip.y + strip.h - SEPARATOR_INSET;
            ctx.draw_list.push_line(
                Point::new(x, y1),
                Point::new(x, y2),
                1.0,
                self.colors.separator,
            );
        }
    }

    /// Draws the new-tab (+) button.
    fn draw_new_tab_button(&self, ctx: &mut DrawCtx<'_>, strip: &TabStrip) {
        let bx = new_tab_button_x(self);
        let btn = Rect::new(bx, strip.y, NEW_TAB_BUTTON_WIDTH, strip.h);

        if self.hover_hit == TabBarHit::NewTab {
            let style =
                RectStyle::filled(self.colors.button_hover_bg).with_radius(BUTTON_HOVER_RADIUS);
            ctx.draw_list.push_rect(btn, style);
        }

        // + icon.
        let cx = bx + NEW_TAB_BUTTON_WIDTH / 2.0;
        let cy = strip.y + strip.h / 2.0;
        let fg = self.colors.close_fg;
        ctx.draw_list.push_line(
            Point::new(cx - PLUS_ARM, cy),
            Point::new(cx + PLUS_ARM, cy),
            ICON_STROKE_WIDTH,
            fg,
        );
        ctx.draw_list.push_line(
            Point::new(cx, cy - PLUS_ARM),
            Point::new(cx, cy + PLUS_ARM),
            ICON_STROKE_WIDTH,
            fg,
        );
    }

    /// Draws the dropdown (▾) button.
    fn draw_dropdown_button(&self, ctx: &mut DrawCtx<'_>, strip: &TabStrip) {
        let bx = dropdown_button_x(self);
        let btn = Rect::new(bx, strip.y, DROPDOWN_BUTTON_WIDTH, strip.h);

        if self.hover_hit == TabBarHit::Dropdown {
            let style =
                RectStyle::filled(self.colors.button_hover_bg).with_radius(BUTTON_HOVER_RADIUS);
            ctx.draw_list.push_rect(btn, style);
        }

        // ▾ chevron.
        let cx = bx + DROPDOWN_BUTTON_WIDTH / 2.0;
        let cy = strip.y + strip.h / 2.0;
        let fg = self.colors.close_fg;
        ctx.draw_list.push_line(
            Point::new(cx - CHEVRON_HALF_W, cy - CHEVRON_HALF_H),
            Point::new(cx, cy + CHEVRON_HALF_H),
            ICON_STROKE_WIDTH,
            fg,
        );
        ctx.draw_list.push_line(
            Point::new(cx, cy + CHEVRON_HALF_H),
            Point::new(cx + CHEVRON_HALF_W, cy - CHEVRON_HALF_H),
            ICON_STROKE_WIDTH,
            fg,
        );
    }
}

// Free functions used by both drawing and tests

/// Computes the bell animation phase for a tab.
///
/// Returns 0.0–1.0 for an active bell animation, 0.0 otherwise.
/// The phase follows a decaying sine wave that pulses for
/// [`BELL_DURATION_SECS`] seconds after the bell fires.
pub(super) fn bell_phase(tab: &TabEntry, now: Instant) -> f32 {
    let Some(start) = tab.bell_start else {
        return 0.0;
    };
    let elapsed = now.duration_since(start).as_secs_f32();
    if elapsed >= BELL_DURATION_SECS {
        return 0.0;
    }
    let fade = 1.0 - (elapsed / BELL_DURATION_SECS);
    let wave = (elapsed * BELL_FREQUENCY_HZ * std::f32::consts::TAU)
        .sin()
        .abs();
    wave * fade
}

/// X position of the new-tab button, adjusted for drag.
///
/// When dragging a tab past the end of the strip, the button moves
/// right to stay visible: `max(default_x, drag_x + tab_width)`.
pub(super) fn new_tab_button_x(widget: &TabBarWidget) -> f32 {
    let default_x = widget.layout.new_tab_x();
    if let Some((idx, drag_x)) = widget.drag_visual {
        default_x.max(drag_x + widget.layout.tab_width_at(idx))
    } else {
        default_x
    }
}

/// X position of the dropdown button, adjusted for drag.
pub(super) fn dropdown_button_x(widget: &TabBarWidget) -> f32 {
    let default_x = widget.layout.dropdown_x();
    if let Some((idx, drag_x)) = widget.drag_visual {
        default_x.max(drag_x + widget.layout.tab_width_at(idx) + NEW_TAB_BUTTON_WIDTH)
    } else {
        default_x
    }
}

// Widget impl

impl Widget for TabBarWidget {
    fn id(&self) -> crate::widget_id::WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        false
    }

    fn layout(&self, _ctx: &LayoutCtx<'_>) -> LayoutBox {
        LayoutBox::leaf(self.window_width, TAB_BAR_HEIGHT).with_widget_id(self.id)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        if self.tabs.is_empty() {
            return;
        }

        let y0 = ctx.bounds.y();
        let w = ctx.bounds.width();

        // 1. Tab bar background.
        let bar = Rect::new(0.0, y0, w, TAB_BAR_HEIGHT);
        ctx.draw_list
            .push_rect(bar, RectStyle::filled(self.colors.bar_bg));

        let mut strip = TabStrip {
            y: y0 + TAB_TOP_MARGIN,
            h: TAB_BAR_HEIGHT - TAB_TOP_MARGIN,
            active: false,
            bell: 0.0,
            text_color: self.colors.inactive_text,
        };

        // 2. Inactive tabs, 3. Active tab, 3.5. Animation checks.
        self.draw_all_tabs(ctx, &mut strip);

        // 4. Separators, 5. New tab, 6. Dropdown, 6.5. Window controls.
        self.draw_separators(ctx, &strip);
        self.draw_new_tab_button(ctx, &strip);
        self.draw_dropdown_button(ctx, &strip);
        self.draw_window_controls(ctx);

        // 7. Dragged tab overlay (floats above everything).
        strip.text_color = self.colors.text_fg;
        self.draw_dragged_tab_overlay(ctx, &strip);
    }

    fn handle_mouse(&mut self, _event: &MouseEvent, _ctx: &EventCtx<'_>) -> WidgetResponse {
        // Hit testing and click dispatch are Section 16.3.
        WidgetResponse::ignored()
    }

    fn handle_hover(&mut self, _event: HoverEvent, _ctx: &EventCtx<'_>) -> WidgetResponse {
        // Hover enter/leave routing is Section 16.3.
        WidgetResponse::ignored()
    }

    fn handle_key(&mut self, _event: KeyEvent, _ctx: &EventCtx<'_>) -> WidgetResponse {
        WidgetResponse::ignored()
    }
}
