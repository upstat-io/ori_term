//! Retained draw command list for UI rendering.
//!
//! [`DrawList`] accumulates [`DrawCommand`]s in painter's order. The GPU
//! converter in oriterm walks the list to emit instance buffer records.

use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::text::ShapedText;

use super::rect_style::RectStyle;

/// A single draw operation in painter's order.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    /// A styled rectangle.
    Rect {
        /// Bounding rectangle in logical pixels.
        rect: Rect,
        /// Visual style (fill, border, radius, shadow).
        style: RectStyle,
    },
    /// A line segment.
    Line {
        /// Start point in logical pixels.
        from: Point,
        /// End point in logical pixels.
        to: Point,
        /// Line thickness in logical pixels.
        width: f32,
        /// Line color.
        color: Color,
    },
    /// A textured image quad (deferred — logged as no-op by converter).
    Image {
        /// Bounding rectangle in logical pixels.
        rect: Rect,
        /// GPU texture identifier.
        texture_id: u32,
        /// UV coordinates `[u_left, v_top, u_right, v_bottom]`.
        uv: [f32; 4],
    },
    /// A pre-shaped text block.
    Text {
        /// Top-left position of the text block in logical pixels.
        position: Point,
        /// Shaped glyphs with layout metrics.
        shaped: ShapedText,
        /// Text color (overrides the color in the original [`TextStyle`]).
        color: Color,
    },
    /// Push a clip rectangle onto the clip stack.
    PushClip {
        /// Clip bounds in logical pixels.
        rect: Rect,
    },
    /// Pop the most recent clip rectangle from the stack.
    PopClip,
}

/// An ordered list of draw commands for a single frame.
///
/// Commands are drawn in push order (painter's algorithm). Clip state is
/// tracked via `push_clip` / `pop_clip` pairs — the converter enforces
/// balanced stacks.
pub struct DrawList {
    commands: Vec<DrawCommand>,
    /// Tracks push/pop balance for debug assertions.
    clip_stack_depth: u32,
}

impl DrawList {
    /// Creates an empty draw list.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            clip_stack_depth: 0,
        }
    }

    /// Appends a styled rectangle.
    pub fn push_rect(&mut self, rect: Rect, style: RectStyle) {
        self.commands.push(DrawCommand::Rect { rect, style });
    }

    /// Appends a line segment.
    pub fn push_line(&mut self, from: Point, to: Point, width: f32, color: Color) {
        self.commands.push(DrawCommand::Line {
            from,
            to,
            width,
            color,
        });
    }

    /// Appends a pre-shaped text block.
    pub fn push_text(&mut self, position: Point, shaped: ShapedText, color: Color) {
        self.commands.push(DrawCommand::Text {
            position,
            shaped,
            color,
        });
    }

    /// Appends a textured image quad.
    pub fn push_image(&mut self, rect: Rect, texture_id: u32, uv: [f32; 4]) {
        self.commands.push(DrawCommand::Image {
            rect,
            texture_id,
            uv,
        });
    }

    /// Pushes a clip rectangle. Must be paired with [`pop_clip`](Self::pop_clip).
    pub fn push_clip(&mut self, rect: Rect) {
        self.clip_stack_depth += 1;
        self.commands.push(DrawCommand::PushClip { rect });
    }

    /// Pops the most recent clip rectangle.
    ///
    /// # Panics
    ///
    /// Panics if the clip stack is already empty.
    pub fn pop_clip(&mut self) {
        assert!(
            self.clip_stack_depth > 0,
            "pop_clip called with empty clip stack",
        );
        self.clip_stack_depth -= 1;
        self.commands.push(DrawCommand::PopClip);
    }

    /// Returns the commands in draw order.
    pub fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }

    /// Whether the list contains no commands.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Number of commands in the list.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Removes all commands and resets the clip stack, retaining allocated memory.
    pub fn clear(&mut self) {
        self.commands.clear();
        self.clip_stack_depth = 0;
    }
}

impl Default for DrawList {
    fn default() -> Self {
        Self::new()
    }
}
