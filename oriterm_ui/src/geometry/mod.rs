//! Geometry primitives for layout and rendering.
//!
//! Modeled after Chromium's `ui/gfx/geometry/`. All values are `f32` logical
//! pixels. Pure data types with no platform dependencies, fully testable.

mod insets;
mod point;
mod rect;
mod size;

pub use insets::Insets;
pub use point::Point;
pub use rect::Rect;
pub use size::Size;

#[cfg(test)]
mod tests;
