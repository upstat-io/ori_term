//! Layout data structures for the multiplexing system.
//!
//! This module contains the immutable split tree, floating pane layer, and
//! layout computation that converts abstract trees into concrete pixel rects.

pub mod compute;
pub mod floating;
pub mod split_tree;

pub use compute::{DividerLayout, LayoutDescriptor, PaneLayout, compute_dividers, compute_layout};
pub use floating::{FloatingLayer, FloatingPane, Rect};
pub use split_tree::{SplitDirection, SplitTree};
