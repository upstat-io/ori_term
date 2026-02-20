//! Flexbox-inspired layout engine.
//!
//! Computes positions and sizes from a [`LayoutBox`] descriptor tree,
//! producing a [`LayoutNode`] output tree. Pure computation — no rendering.

mod constraints;
mod flex;
mod layout_box;
mod layout_node;
mod size_spec;
mod solver;

pub use constraints::LayoutConstraints;
pub use flex::{Align, Direction, Justify};
pub use layout_box::{BoxContent, LayoutBox};
pub use layout_node::LayoutNode;
pub use size_spec::SizeSpec;
pub use solver::compute_layout;

#[cfg(test)]
mod tests;
