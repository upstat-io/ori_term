//! GPU rendering: wgpu state management, render pipeline types, and platform transparency.

pub(crate) mod atlas;
pub(crate) mod bind_groups;
pub(crate) mod extract;
pub(crate) mod frame_input;
pub(crate) mod instance_writer;
pub(crate) mod pipeline;
pub(crate) mod prepare;
pub(crate) mod prepared_frame;
pub(crate) mod render_target;
pub(crate) mod renderer;
pub(crate) mod state;
pub(crate) mod transparency;

// Re-exports consumed by App and Window.
pub(crate) use extract::extract_frame;
pub(crate) use frame_input::ViewportSize;
pub(crate) use renderer::{GpuRenderer, SurfaceError};
pub(crate) use state::GpuState;
pub(crate) use transparency::apply_transparency;

#[cfg(test)]
mod pipeline_tests;
#[cfg(test)]
mod visual_regression;
