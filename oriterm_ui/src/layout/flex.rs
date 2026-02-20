//! Flex layout enums and axis-abstraction helpers.
//!
//! `Direction` provides axis helpers that let the solver operate generically
//! on main/cross axes without Row/Column branching in every operation.

use crate::geometry::Insets;

/// Main axis direction for a flex container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Children laid out left-to-right.
    Row,
    /// Children laid out top-to-bottom.
    Column,
}

/// Cross-axis alignment for children within a flex container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// Align to the start of the cross axis.
    Start,
    /// Center along the cross axis.
    Center,
    /// Align to the end of the cross axis.
    End,
    /// Stretch to fill the cross axis.
    Stretch,
}

/// Main-axis distribution of remaining space in a flex container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify {
    /// Pack children to the start.
    Start,
    /// Center children along the main axis.
    Center,
    /// Pack children to the end.
    End,
    /// Equal spacing between children; no space before first or after last.
    SpaceBetween,
    /// Equal spacing around each child (half-space at edges).
    SpaceAround,
}

impl Direction {
    /// Extracts the main-axis dimension from `(width, height)`.
    pub fn main(self, width: f32, height: f32) -> f32 {
        match self {
            Self::Row => width,
            Self::Column => height,
        }
    }

    /// Extracts the cross-axis dimension from `(width, height)`.
    pub fn cross(self, width: f32, height: f32) -> f32 {
        match self {
            Self::Row => height,
            Self::Column => width,
        }
    }

    /// Recomposes main/cross values back into `(width, height)`.
    pub fn compose(self, main: f32, cross: f32) -> (f32, f32) {
        match self {
            Self::Row => (main, cross),
            Self::Column => (cross, main),
        }
    }

    /// Total insets along the main axis.
    pub fn main_insets(self, insets: Insets) -> f32 {
        match self {
            Self::Row => insets.width(),
            Self::Column => insets.height(),
        }
    }

    /// Total insets along the cross axis.
    pub fn cross_insets(self, insets: Insets) -> f32 {
        match self {
            Self::Row => insets.height(),
            Self::Column => insets.width(),
        }
    }

    /// Start inset on the main axis (left for Row, top for Column).
    pub fn main_start(self, insets: Insets) -> f32 {
        match self {
            Self::Row => insets.left,
            Self::Column => insets.top,
        }
    }

    /// Start inset on the cross axis (top for Row, left for Column).
    pub fn cross_start(self, insets: Insets) -> f32 {
        match self {
            Self::Row => insets.top,
            Self::Column => insets.left,
        }
    }
}
