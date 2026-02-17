//! Color types and palette for terminal emulation.
//!
//! Re-exports `Rgb` from `vte::ansi` and provides the 270-entry `Palette`
//! that maps indexed, named, and direct-RGB colors.

pub mod palette;

pub(crate) use palette::dim_rgb;
pub use palette::{Palette, Rgb};
