//! UI framework types for oriterm windowing, layout, and rendering.
//!
//! Provides platform-agnostic geometry primitives, DPI scaling, hit testing,
//! and window management. Platform-specific glue lives in `#[cfg]`-gated
//! submodules.

pub mod color;
pub mod draw;
pub mod focus;
pub mod geometry;
pub mod hit_test;
pub mod input;
pub mod layout;
pub mod overlay;
pub mod scale;
pub mod text;
pub mod widget_id;
pub mod widgets;
pub mod window;

#[cfg(target_os = "linux")]
pub mod platform_linux;
#[cfg(target_os = "macos")]
pub mod platform_macos;
#[cfg(target_os = "windows")]
pub mod platform_windows;
