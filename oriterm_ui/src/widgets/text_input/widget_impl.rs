//! `Widget` trait implementation for `TextInputWidget`.
//!
//! Separated from `mod.rs` to keep files under 500 lines.

use crate::draw::RectStyle;
use crate::geometry::{Point, Rect};
use crate::input::{HoverEvent, Key, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::layout::LayoutBox;
use crate::widget_id::WidgetId;

use super::super::{DrawCtx, EventCtx, LayoutCtx, Widget, WidgetAction, WidgetResponse};
use super::TextInputWidget;

impl Widget for TextInputWidget {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn is_focusable(&self) -> bool {
        !self.disabled
    }

    fn layout(&self, ctx: &LayoutCtx<'_>) -> LayoutBox {
        let style = self.text_style();
        let metrics = ctx.measurer.measure(
            if self.text.is_empty() {
                &self.placeholder
            } else {
                &self.text
            },
            &style,
            f32::INFINITY,
        );
        let w = (metrics.width + self.style.padding.width()).max(self.style.min_width);
        let h = metrics.height + self.style.padding.height();
        LayoutBox::leaf(w, h).with_widget_id(self.id)
    }

    #[expect(
        clippy::string_slice,
        reason = "selection bounds always on char boundaries"
    )]
    fn draw(&self, ctx: &mut DrawCtx<'_>) {
        let focused = ctx.focused_widget == Some(self.id);
        let bounds = ctx.bounds;
        let s = &self.style;

        // Background + border.
        let bg = if self.disabled { s.disabled_bg } else { s.bg };
        let border_color = if focused {
            s.focus_border_color
        } else {
            s.border_color
        };
        let bg_style = RectStyle::filled(bg)
            .with_border(s.border_width, border_color)
            .with_radius(s.corner_radius);
        ctx.draw_list.push_rect(bounds, bg_style);

        // Clip to inner area.
        let inner = bounds.inset(s.padding);
        ctx.draw_list.push_clip(inner);

        let style = self.text_style();

        if self.text.is_empty() {
            // Placeholder.
            if !self.placeholder.is_empty() {
                let shaped = ctx.measurer.shape(&self.placeholder, &style, inner.width());
                let y = inner.y() + (inner.height() - shaped.height) / 2.0;
                ctx.draw_list
                    .push_text(Point::new(inner.x(), y), shaped, s.placeholder_color);
            }
        } else {
            // Selection highlight.
            if let Some((sel_start, sel_end)) = self.selection_range() {
                if sel_start != sel_end {
                    let prefix_w = ctx
                        .measurer
                        .measure(&self.text[..sel_start], &style, f32::INFINITY)
                        .width;
                    let sel_w = ctx
                        .measurer
                        .measure(&self.text[sel_start..sel_end], &style, f32::INFINITY)
                        .width;
                    let sel_rect =
                        Rect::new(inner.x() + prefix_w, inner.y(), sel_w, inner.height());
                    ctx.draw_list
                        .push_rect(sel_rect, RectStyle::filled(s.selection_color));
                }
            }

            // Text.
            let shaped = ctx.measurer.shape(&self.text, &style, f32::INFINITY);
            let fg = if self.disabled { s.disabled_fg } else { s.fg };
            let y = inner.y() + (inner.height() - shaped.height) / 2.0;
            ctx.draw_list
                .push_text(Point::new(inner.x(), y), shaped, fg);
        }

        // Cursor (only when focused).
        if focused && !self.disabled {
            let cursor_x = inner.x() + self.cursor_x(ctx.measurer);
            let cursor_rect = Rect::new(cursor_x, inner.y(), s.cursor_width, inner.height());
            ctx.draw_list
                .push_rect(cursor_rect, RectStyle::filled(s.cursor_color));
        }

        ctx.draw_list.pop_clip();
    }

    #[expect(clippy::string_slice, reason = "cursor always on char boundary")]
    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx<'_>) -> WidgetResponse {
        if self.disabled {
            return WidgetResponse::ignored();
        }
        if event.kind == MouseEventKind::Down(MouseButton::Left) {
            let inner = ctx.bounds.inset(self.style.padding);
            let rel_x = (event.pos.x - inner.x()).max(0.0);
            let style = self.text_style();

            // Walk char boundaries; pick the one closest to click X.
            let mut best_pos = 0;
            let mut best_dist = rel_x;
            for (i, _) in self.text.char_indices() {
                let w = ctx
                    .measurer
                    .measure(&self.text[..i], &style, f32::INFINITY)
                    .width;
                let dist = (w - rel_x).abs();
                if dist < best_dist {
                    best_dist = dist;
                    best_pos = i;
                }
            }
            // Check end position.
            let end_w = ctx
                .measurer
                .measure(&self.text, &style, f32::INFINITY)
                .width;
            if (end_w - rel_x).abs() < best_dist {
                best_pos = self.text.len();
            }

            self.cursor = best_pos;
            self.selection_anchor = None;
            return WidgetResponse::focus();
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
        let shift = event.modifiers.shift();
        let ctrl = event.modifiers.ctrl();

        match event.key {
            Key::Character(ch) => self.handle_character(ch, ctrl),
            Key::Backspace => self.handle_backspace(),
            Key::Delete => self.handle_delete(),
            Key::ArrowLeft => {
                self.move_left(shift);
                WidgetResponse::redraw()
            }
            Key::ArrowRight => {
                self.move_right(shift);
                WidgetResponse::redraw()
            }
            Key::Home => self.handle_home_end(0, shift),
            Key::End => self.handle_home_end(self.text.len(), shift),
            _ => WidgetResponse::ignored(),
        }
    }
}

impl TextInputWidget {
    /// Handles a character insertion (or Ctrl+A).
    fn handle_character(&mut self, ch: char, ctrl: bool) -> WidgetResponse {
        if ctrl {
            if ch == 'a' {
                self.selection_anchor = Some(0);
                self.cursor = self.text.len();
                return WidgetResponse::redraw();
            }
            return WidgetResponse::ignored();
        }
        self.delete_selection();
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        WidgetResponse::redraw().with_action(WidgetAction::TextChanged {
            id: self.id,
            text: self.text.clone(),
        })
    }

    /// Handles Backspace: delete selection or previous character.
    fn handle_backspace(&mut self) -> WidgetResponse {
        if self.delete_selection() {
            return self.text_changed_response();
        }
        if self.cursor > 0 {
            let prev = self.prev_char_boundary(self.cursor);
            self.text.drain(prev..self.cursor);
            self.cursor = prev;
            return self.text_changed_response();
        }
        WidgetResponse::handled()
    }

    /// Handles Delete: delete selection or next character.
    fn handle_delete(&mut self) -> WidgetResponse {
        if self.delete_selection() {
            return self.text_changed_response();
        }
        if self.cursor < self.text.len() {
            let next = self.next_char_boundary(self.cursor);
            self.text.drain(self.cursor..next);
            return self.text_changed_response();
        }
        WidgetResponse::handled()
    }

    /// Handles Home/End with optional Shift selection.
    fn handle_home_end(&mut self, target: usize, shift: bool) -> WidgetResponse {
        if shift && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }
        self.cursor = target;
        if !shift {
            self.selection_anchor = None;
        }
        WidgetResponse::redraw()
    }

    /// Returns a redraw response with `TextChanged` action.
    fn text_changed_response(&self) -> WidgetResponse {
        WidgetResponse::redraw().with_action(WidgetAction::TextChanged {
            id: self.id,
            text: self.text.clone(),
        })
    }
}
