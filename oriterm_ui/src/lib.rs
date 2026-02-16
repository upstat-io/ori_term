//! UI framework types for oriterm windowing, layout, and rendering.
//!
//! Provides platform-agnostic geometry primitives, DPI scaling, hit testing,
//! and window management. Platform-specific glue lives in `#[cfg]`-gated
//! submodules.

pub mod geometry;
pub mod hit_test;
pub mod scale;
pub mod window;

#[cfg(target_os = "windows")]
pub mod platform_windows;
#[cfg(target_os = "macos")]
pub mod platform_macos;
#[cfg(target_os = "linux")]
pub mod platform_linux;
