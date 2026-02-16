//! DPI scale factor abstraction.
//!
//! Wraps the raw `f64` scale factor from the windowing system as a
//! clamped newtype, with methods to convert between logical and
//! physical coordinates.

mod scale_factor;

pub use scale_factor::ScaleFactor;

#[cfg(test)]
mod tests;
