//! Layout box descriptor — the input to the layout solver.
//!
//! A tree of `LayoutBox` nodes describes the desired sizing and arrangement.
//! Widgets will construct `LayoutBox` trees; the solver produces `LayoutNode`
//! trees as output.

use crate::geometry::Insets;

use super::flex::{Align, Direction, Justify};
use super::size_spec::SizeSpec;

/// Content of a layout box — either a leaf with intrinsic size or a
/// flex container with children.
#[derive(Debug, Clone, PartialEq)]
pub enum BoxContent {
    /// A leaf node with intrinsic dimensions.
    Leaf {
        /// Natural width of the content.
        intrinsic_width: f32,
        /// Natural height of the content.
        intrinsic_height: f32,
    },
    /// A flex container that arranges children along an axis.
    Flex {
        /// Layout direction.
        direction: Direction,
        /// Cross-axis alignment.
        align: Align,
        /// Main-axis justification.
        justify: Justify,
        /// Spacing between children along the main axis.
        gap: f32,
        /// Child layout boxes.
        children: Vec<LayoutBox>,
    },
}

/// A layout box describing desired size, spacing, and content.
///
/// This is a pure data descriptor — no rendering, no trait objects.
/// The layout solver reads the tree and produces [`super::LayoutNode`] output.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutBox {
    /// How width is determined.
    pub width: SizeSpec,
    /// How height is determined.
    pub height: SizeSpec,
    /// Inner padding (shrinks content area).
    pub padding: Insets,
    /// Outer margin (offsets position, consumes parent space).
    pub margin: Insets,
    /// Minimum width constraint (`0.0` = no minimum).
    pub min_width: f32,
    /// Maximum width constraint (`f32::INFINITY` = no maximum).
    pub max_width: f32,
    /// Minimum height constraint (`0.0` = no minimum).
    pub min_height: f32,
    /// Maximum height constraint (`f32::INFINITY` = no maximum).
    pub max_height: f32,
    /// What this box contains.
    pub content: BoxContent,
}

impl LayoutBox {
    /// Creates a leaf box with intrinsic dimensions and `Hug` sizing.
    pub fn leaf(intrinsic_width: f32, intrinsic_height: f32) -> Self {
        Self {
            width: SizeSpec::Hug,
            height: SizeSpec::Hug,
            padding: Insets::default(),
            margin: Insets::default(),
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
            content: BoxContent::Leaf {
                intrinsic_width,
                intrinsic_height,
            },
        }
    }

    /// Creates a flex container with default alignment and justification.
    pub fn flex(direction: Direction, children: Vec<Self>) -> Self {
        Self {
            width: SizeSpec::Hug,
            height: SizeSpec::Hug,
            padding: Insets::default(),
            margin: Insets::default(),
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
            content: BoxContent::Flex {
                direction,
                align: Align::Start,
                justify: Justify::Start,
                gap: 0.0,
                children,
            },
        }
    }

    /// Sets the width spec.
    #[must_use]
    pub fn with_width(mut self, spec: SizeSpec) -> Self {
        self.width = spec;
        self
    }

    /// Sets the height spec.
    #[must_use]
    pub fn with_height(mut self, spec: SizeSpec) -> Self {
        self.height = spec;
        self
    }

    /// Sets padding on all sides.
    #[must_use]
    pub fn with_padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    /// Sets margin on all sides.
    #[must_use]
    pub fn with_margin(mut self, margin: Insets) -> Self {
        self.margin = margin;
        self
    }

    /// Sets the minimum width.
    #[must_use]
    pub fn with_min_width(mut self, v: f32) -> Self {
        self.min_width = v;
        self
    }

    /// Sets the maximum width.
    #[must_use]
    pub fn with_max_width(mut self, v: f32) -> Self {
        self.max_width = v;
        self
    }

    /// Sets the minimum height.
    #[must_use]
    pub fn with_min_height(mut self, v: f32) -> Self {
        self.min_height = v;
        self
    }

    /// Sets the maximum height.
    #[must_use]
    pub fn with_max_height(mut self, v: f32) -> Self {
        self.max_height = v;
        self
    }

    /// Sets cross-axis alignment (only meaningful for flex containers).
    #[must_use]
    pub fn with_align(mut self, align: Align) -> Self {
        if let BoxContent::Flex {
            align: ref mut a, ..
        } = self.content
        {
            *a = align;
        }
        self
    }

    /// Sets main-axis justification (only meaningful for flex containers).
    #[must_use]
    pub fn with_justify(mut self, justify: Justify) -> Self {
        if let BoxContent::Flex {
            justify: ref mut j, ..
        } = self.content
        {
            *j = justify;
        }
        self
    }

    /// Sets the gap between children (only meaningful for flex containers).
    #[must_use]
    pub fn with_gap(mut self, gap: f32) -> Self {
        if let BoxContent::Flex { gap: ref mut g, .. } = self.content {
            *g = gap;
        }
        self
    }
}
