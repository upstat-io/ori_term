//! Tests for GPU render pipelines.

use wgpu::VertexStepMode;

use super::{INSTANCE_ATTRS, INSTANCE_STRIDE, instance_buffer_layout};
use crate::gpu::instance_writer::INSTANCE_SIZE;

// --- Unit tests (no GPU) ---

#[test]
fn stride_matches_instance_size() {
    assert_eq!(INSTANCE_STRIDE, INSTANCE_SIZE as u64);
}

#[test]
fn six_attributes() {
    assert_eq!(INSTANCE_ATTRS.len(), 6);
}

#[test]
fn attribute_offsets_and_locations() {
    let expected: [(u64, u32); 6] = [
        (0, 0),  // pos
        (8, 1),  // size
        (16, 2), // uv
        (32, 3), // fg_color
        (48, 4), // bg_color
        (64, 5), // kind
    ];

    for (attr, (offset, location)) in INSTANCE_ATTRS.iter().zip(expected.iter()) {
        assert_eq!(
            attr.offset, *offset,
            "offset mismatch for location {location}",
        );
        assert_eq!(
            attr.shader_location, *location,
            "location mismatch at offset {offset}",
        );
    }
}

#[test]
fn last_attribute_fits_within_stride() {
    let last = &INSTANCE_ATTRS[INSTANCE_ATTRS.len() - 1];
    let end = last.offset + last.format.size();
    assert!(
        end <= INSTANCE_STRIDE,
        "last attribute ends at byte {end}, but stride is {INSTANCE_STRIDE}",
    );
}

#[test]
fn instance_buffer_layout_uses_instance_step_mode() {
    let layout = instance_buffer_layout();
    assert_eq!(layout.step_mode, VertexStepMode::Instance);
    assert_eq!(layout.array_stride, INSTANCE_STRIDE);
}

// --- GPU integration tests (require adapter) ---

use crate::gpu::state::GpuState;

#[test]
fn gpu_uniform_bind_group_layout_succeeds() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let _layout = super::create_uniform_bind_group_layout(&gpu.device);
}

#[test]
fn gpu_atlas_bind_group_layout_succeeds() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let _layout = super::create_atlas_bind_group_layout(&gpu.device);
}

#[test]
fn gpu_bg_pipeline_succeeds() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let uniform_layout = super::create_uniform_bind_group_layout(&gpu.device);
    let _pipeline = super::create_bg_pipeline(&gpu, &uniform_layout);
}

#[test]
fn gpu_fg_pipeline_succeeds() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let uniform_layout = super::create_uniform_bind_group_layout(&gpu.device);
    let atlas_layout = super::create_atlas_bind_group_layout(&gpu.device);
    let _pipeline = super::create_fg_pipeline(&gpu, &uniform_layout, &atlas_layout);
}

#[test]
fn gpu_both_pipelines_share_instance_layout() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let uniform_layout = super::create_uniform_bind_group_layout(&gpu.device);
    let atlas_layout = super::create_atlas_bind_group_layout(&gpu.device);

    // Both pipelines are created with the same instance_buffer_layout().
    // If either fails, the shader doesn't match the layout.
    let _bg = super::create_bg_pipeline(&gpu, &uniform_layout);
    let _fg = super::create_fg_pipeline(&gpu, &uniform_layout, &atlas_layout);
}
