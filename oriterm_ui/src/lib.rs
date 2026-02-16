//! UI framework types for oriterm windowing, layout, and rendering.
//!
//! Provides platform-agnostic geometry primitives, DPI scaling, hit testing,
//! and window management. Platform-specific glue lives in `#[cfg]`-gated
//! submodules.

pub mod geometry;
pub mod hit_test;
pub mod scale;
