//! Tests for GPU render pipelines.

use wgpu::VertexStepMode;

use super::{
    INSTANCE_ATTRS, INSTANCE_STRIDE, UI_RECT_ATTRS, instance_buffer_layout, ui_rect_buffer_layout,
};
use crate::gpu::instance_writer::INSTANCE_SIZE;

// --- Unit tests (no GPU) ---

#[test]
fn stride_matches_instance_size() {
    assert_eq!(INSTANCE_STRIDE, INSTANCE_SIZE as u64);
}

#[test]
fn seven_attributes() {
    assert_eq!(INSTANCE_ATTRS.len(), 7);
}

#[test]
fn attribute_offsets_and_locations() {
    let expected: [(u64, u32); 7] = [
        (0, 0),  // pos
        (8, 1),  // size
        (16, 2), // uv
        (32, 3), // fg_color
        (48, 4), // bg_color
        (64, 5), // kind
        (68, 6), // atlas_page
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

#[test]
fn instance_attributes_are_contiguous() {
    for pair in INSTANCE_ATTRS.windows(2) {
        let end = pair[0].offset + pair[0].format.size();
        assert_eq!(
            end, pair[1].offset,
            "gap between locations {} and {}",
            pair[0].shader_location, pair[1].shader_location,
        );
    }
}

// --- UI rect attribute tests (no GPU) ---

#[test]
fn ui_rect_attrs_share_first_seven_with_instance_attrs() {
    for (i, (ui, inst)) in UI_RECT_ATTRS[..7]
        .iter()
        .zip(INSTANCE_ATTRS.iter())
        .enumerate()
    {
        assert_eq!(ui.format, inst.format, "format mismatch at index {i}",);
        assert_eq!(ui.offset, inst.offset, "offset mismatch at index {i}",);
        assert_eq!(
            ui.shader_location, inst.shader_location,
            "shader_location mismatch at index {i}",
        );
    }
}

#[test]
fn ui_rect_nine_attributes() {
    assert_eq!(UI_RECT_ATTRS.len(), 9);
}

#[test]
fn ui_rect_attribute_offsets_and_locations() {
    let expected: [(u64, u32); 9] = [
        (0, 0),  // pos
        (8, 1),  // size
        (16, 2), // uv
        (32, 3), // fg_color (border color)
        (48, 4), // bg_color (fill color)
        (64, 5), // kind
        (68, 6), // atlas_page
        (72, 7), // corner_radius
        (76, 8), // border_width
    ];

    for (attr, (offset, location)) in UI_RECT_ATTRS.iter().zip(expected.iter()) {
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
fn ui_rect_last_attribute_fits_within_stride() {
    let last = &UI_RECT_ATTRS[UI_RECT_ATTRS.len() - 1];
    let end = last.offset + last.format.size();
    assert!(
        end <= INSTANCE_STRIDE,
        "last UI rect attribute ends at byte {end}, but stride is {INSTANCE_STRIDE}",
    );
}

#[test]
fn ui_rect_buffer_layout_uses_instance_step_mode() {
    let layout = ui_rect_buffer_layout();
    assert_eq!(layout.step_mode, VertexStepMode::Instance);
    assert_eq!(layout.array_stride, INSTANCE_STRIDE);
}

#[test]
fn ui_rect_attributes_are_contiguous() {
    for pair in UI_RECT_ATTRS.windows(2) {
        let end = pair[0].offset + pair[0].format.size();
        assert_eq!(
            end, pair[1].offset,
            "gap between locations {} and {}",
            pair[0].shader_location, pair[1].shader_location,
        );
    }
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

#[test]
fn gpu_ui_rect_pipeline_succeeds() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let uniform_layout = super::create_uniform_bind_group_layout(&gpu.device);
    let _pipeline = super::create_ui_rect_pipeline(&gpu, &uniform_layout);
}
