//! GPU rendering: wgpu state management, render pipeline types, and platform transparency.

pub(crate) mod atlas;
pub(crate) mod bind_groups;
pub(crate) mod frame_input;
pub(crate) mod instance_writer;
pub(crate) mod pipeline;
pub(crate) mod prepared_frame;
pub(crate) mod render_target;
pub(crate) mod state;
pub(crate) mod transparency;

// Re-exports consumed starting in Section 5.10.
#[expect(
    unused_imports,
    reason = "atlas types used starting in Section 5.10"
)]
pub(crate) use atlas::{AtlasEntry, GlyphAtlas};
#[expect(
    unused_imports,
    reason = "bind group types used starting in Section 5.10"
)]
pub(crate) use bind_groups::{AtlasBindGroup, UniformBuffer, create_placeholder_atlas_texture};

// Re-exports consumed starting in Section 5.8.
#[expect(
    unused_imports,
    reason = "render pipeline types used starting in Section 5.8"
)]
pub(crate) use frame_input::{FrameInput, FramePalette, ViewportSize};
#[expect(
    unused_imports,
    reason = "render pipeline types used starting in Section 5.8"
)]
pub(crate) use instance_writer::{InstanceKind, InstanceWriter};
#[expect(
    unused_imports,
    reason = "render pipeline types used starting in Section 5.8"
)]
pub(crate) use prepared_frame::PreparedFrame;
#[expect(
    unused_imports,
    reason = "render targets used starting in Section 5.13"
)]
pub(crate) use render_target::{ReadbackError, RenderTarget};
pub(crate) use state::validate_gpu;
#[expect(
    unused_imports,
    reason = "GpuState used once event loop is wired in Section 05"
)]
pub(crate) use state::{GpuInitError, GpuState};
