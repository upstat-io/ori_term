//! GPU rendering: wgpu state management, render pipeline types, and platform transparency.

pub(crate) mod frame_input;
pub(crate) mod instance_writer;
pub(crate) mod pipeline;
pub(crate) mod prepared_frame;
pub(crate) mod render_target;
pub(crate) mod state;
pub(crate) mod transparency;

// Re-exports consumed starting in Section 5.8.
#[expect(
    unused_imports,
    reason = "render pipeline types used starting in Section 5.8"
)]
pub(crate) use frame_input::{CellMetrics, FrameInput, FramePalette, ViewportSize};
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
pub(crate) use render_target::RenderTarget;
pub(crate) use state::validate_gpu;
