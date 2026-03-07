//! Image overlay pipeline: per-image textured quads for inline terminal images.

use wgpu::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType,
    ColorTargetState, ColorWrites, Device, FragmentState, MultisampleState,
    PipelineLayoutDescriptor, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType,
    ShaderModuleDescriptor, ShaderStages, TextureSampleType, TextureViewDimension, VertexAttribute,
    VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
};

use super::super::state::GpuState;
use super::{PREMUL_ALPHA_BLEND, QUAD_PRIMITIVE};

/// Embedded WGSL source for the image overlay shader.
const IMAGE_SHADER_SRC: &str = include_str!("../shaders/image.wgsl");

/// Instance stride for image quads (36 bytes).
///
/// Layout: `pos(2f) + size(2f) + uv_pos(2f) + uv_size(2f) + opacity(1f)`.
pub const IMAGE_INSTANCE_STRIDE: u64 = 36;

/// Vertex attributes for the 36-byte image instance record.
const IMAGE_INSTANCE_ATTRS: [VertexAttribute; 5] = [
    // location 0: pos (vec2<f32>) at offset 0.
    VertexAttribute {
        format: VertexFormat::Float32x2,
        offset: 0,
        shader_location: 0,
    },
    // location 1: size (vec2<f32>) at offset 8.
    VertexAttribute {
        format: VertexFormat::Float32x2,
        offset: 8,
        shader_location: 1,
    },
    // location 2: uv_pos (vec2<f32>) at offset 16.
    VertexAttribute {
        format: VertexFormat::Float32x2,
        offset: 16,
        shader_location: 2,
    },
    // location 3: uv_size (vec2<f32>) at offset 24.
    VertexAttribute {
        format: VertexFormat::Float32x2,
        offset: 24,
        shader_location: 3,
    },
    // location 4: opacity (f32) at offset 32.
    VertexAttribute {
        format: VertexFormat::Float32,
        offset: 32,
        shader_location: 4,
    },
];

/// Returns the instance buffer layout for image quads.
fn image_instance_buffer_layout() -> VertexBufferLayout<'static> {
    VertexBufferLayout {
        array_stride: IMAGE_INSTANCE_STRIDE,
        step_mode: VertexStepMode::Instance,
        attributes: &IMAGE_INSTANCE_ATTRS,
    }
}

/// Create the bind group layout for per-image textures (group 1).
///
/// Contains a `texture_2d` and a filtering sampler. Each image gets its
/// own bind group from this layout.
pub fn create_image_texture_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("image_texture_bind_group_layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Create the image overlay render pipeline.
///
/// Uses bind groups 0 (uniforms) and 1 (per-image texture + sampler).
/// Renders textured quads for inline terminal images. Premultiplied alpha blend.
pub fn create_image_pipeline(
    gpu: &GpuState,
    uniform_layout: &BindGroupLayout,
    image_texture_layout: &BindGroupLayout,
) -> RenderPipeline {
    let shader = gpu.device.create_shader_module(ShaderModuleDescriptor {
        label: Some("image_shader"),
        source: wgpu::ShaderSource::Wgsl(IMAGE_SHADER_SRC.into()),
    });

    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("image_pipeline_layout"),
            bind_group_layouts: &[uniform_layout, image_texture_layout],
            ..Default::default()
        });

    gpu.device
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("image_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[image_instance_buffer_layout()],
            },
            primitive: QUAD_PRIMITIVE,
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format: gpu.render_format(),
                    blend: Some(PREMUL_ALPHA_BLEND),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: gpu.pipeline_cache.as_ref(),
        })
}
