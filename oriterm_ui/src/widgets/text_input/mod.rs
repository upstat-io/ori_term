//! Text input widget — single-line text field with cursor and selection.
//!
//! Handles keyboard editing (character input, backspace, delete, arrow
//! navigation, Home/End, Ctrl+A select all). Emits `WidgetAction::TextChanged`
//! on content changes.
//!
//! Clipboard operations are deferred — the widget emits actions that the
//! app layer interprets for actual clipboard I/O.

mod widget_impl;

use crate::color::Color;
use crate::geometry::Insets;
use crate::text::TextStyle;
use crate::widget_id::WidgetId;

use super::{
    DEFAULT_BG, DEFAULT_BORDER, DEFAULT_DISABLED_BG, DEFAULT_DISABLED_FG, DEFAULT_FG,
    DEFAULT_FOCUS_RING,
};

/// Visual style for a [`TextInputWidget`].
#[derive(Debug, Clone, PartialEq)]
pub struct TextInputStyle {
    /// Text color.
    pub fg: Color,
    /// Background color.
    pub bg: Color,
    /// Border color.
    pub border_color: Color,
    /// Border color when focused.
    pub focus_border_color: Color,
    /// Border width.
    pub border_width: f32,
    /// Corner radius.
    pub corner_radius: f32,
    /// Inner padding.
    pub padding: Insets,
    /// Font size in points.
    pub font_size: f32,
    /// Placeholder text color.
    pub placeholder_color: Color,
    /// Cursor color.
    pub cursor_color: Color,
    /// Cursor width in pixels.
    pub cursor_width: f32,
    /// Selection highlight color.
    pub selection_color: Color,
    /// Minimum width.
    pub min_width: f32,
    /// Disabled text color.
    pub disabled_fg: Color,
    /// Disabled background.
    pub disabled_bg: Color,
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self {
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            border_color: DEFAULT_BORDER,
            focus_border_color: DEFAULT_FOCUS_RING,
            border_width: 1.0,
            corner_radius: 4.0,
            padding: Insets::vh(6.0, 8.0),
            font_size: 13.0,
            placeholder_color: Color::from_rgb_u8(0x80, 0x80, 0x80),
            cursor_color: Color::WHITE,
            cursor_width: 1.5,
            selection_color: Color::from_rgb_u8(0x26, 0x4F, 0x78),
            min_width: 120.0,
            disabled_fg: DEFAULT_DISABLED_FG,
            disabled_bg: DEFAULT_DISABLED_BG,
        }
    }
}

/// A single-line text input field.
///
/// Manages text content, cursor position, and selection. Keyboard
/// editing is handled internally; `WidgetAction::TextChanged` is
/// emitted when content changes.
#[derive(Debug, Clone)]
pub struct TextInputWidget {
    pub(super) id: WidgetId,
    pub(super) text: String,
    pub(super) placeholder: String,
    pub(super) cursor: usize,
    pub(super) selection_anchor: Option<usize>,
    pub(super) disabled: bool,
    pub(super) hovered: bool,
    pub(super) style: TextInputStyle,
}

impl Default for TextInputWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInputWidget {
    /// Creates an empty text input.
    pub fn new() -> Self {
        Self {
            id: WidgetId::next(),
            text: String::new(),
            placeholder: String::new(),
            cursor: 0,
            selection_anchor: None,
            disabled: false,
            hovered: false,
            style: TextInputStyle::default(),
        }
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the text content programmatically.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.len();
        self.selection_anchor = None;
    }

    /// Returns the cursor byte position.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Returns the selection anchor (start of selection), if any.
    pub fn selection_anchor(&self) -> Option<usize> {
        self.selection_anchor
    }

    /// Returns the selected text range as `(start, end)`, if any.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            let start = anchor.min(self.cursor);
            let end = anchor.max(self.cursor);
            (start, end)
        })
    }

    /// Returns whether the input is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether the input is hovered.
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

    /// Sets placeholder text.
    #[must_use]
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
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
    pub fn with_style(mut self, style: TextInputStyle) -> Self {
        self.style = style;
        self
    }

    /// Builds the `TextStyle` for measurement.
    pub(super) fn text_style(&self) -> TextStyle {
        let color = if self.disabled {
            self.style.disabled_fg
        } else {
            self.style.fg
        };
        TextStyle::new(self.style.font_size, color)
    }

    /// Deletes the currently selected text, placing cursor at selection start.
    /// Returns `true` if text was deleted.
    pub(super) fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            if start != end {
                self.text.drain(start..end);
                self.cursor = start;
                self.selection_anchor = None;
                return true;
            }
        }
        self.selection_anchor = None;
        false
    }

    /// Returns the byte offset of the next char boundary after `pos`.
    pub(super) fn next_char_boundary(&self, pos: usize) -> usize {
        let mut idx = pos + 1;
        while idx < self.text.len() && !self.text.is_char_boundary(idx) {
            idx += 1;
        }
        idx.min(self.text.len())
    }

    /// Returns the byte offset of the previous char boundary before `pos`.
    pub(super) fn prev_char_boundary(&self, pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }
        let mut idx = pos - 1;
        while idx > 0 && !self.text.is_char_boundary(idx) {
            idx -= 1;
        }
        idx
    }

    /// Moves cursor left, handling shift for selection.
    pub(super) fn move_left(&mut self, shift: bool) {
        if shift {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
            self.cursor = self.prev_char_boundary(self.cursor);
        } else {
            match self.selection_range() {
                Some((start, end)) if start != end => self.cursor = start,
                _ => self.cursor = self.prev_char_boundary(self.cursor),
            }
            self.selection_anchor = None;
        }
    }

    /// Moves cursor right, handling shift for selection.
    pub(super) fn move_right(&mut self, shift: bool) {
        if shift {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
            if self.cursor < self.text.len() {
                self.cursor = self.next_char_boundary(self.cursor);
            }
        } else {
            match self.selection_range() {
                Some((start, end)) if start != end => self.cursor = end,
                _ => {
                    if self.cursor < self.text.len() {
                        self.cursor = self.next_char_boundary(self.cursor);
                    }
                }
            }
            self.selection_anchor = None;
        }
    }

    /// Computes cursor X position in pixels using the measurer.
    #[expect(clippy::string_slice, reason = "cursor always on char boundary")]
    pub(super) fn cursor_x(&self, measurer: &dyn super::TextMeasurer) -> f32 {
        let prefix = &self.text[..self.cursor];
        let style = self.text_style();
        let metrics = measurer.measure(prefix, &style, f32::INFINITY);
        metrics.width
    }
}

#[cfg(test)]
mod tests;
