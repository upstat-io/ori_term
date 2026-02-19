//! GPU render pipelines: WGSL shaders, vertex layout, and pipeline factories.
//!
//! Four pipelines share a single 80-byte instance buffer layout:
//! - **Background** ([`create_bg_pipeline`]): solid-color quads, no texture.
//! - **Foreground** ([`create_fg_pipeline`]): `R8Unorm` atlas-sampled glyph quads.
//! - **Subpixel foreground** ([`create_subpixel_fg_pipeline`]): `Rgba8Unorm`
//!   atlas-sampled LCD subpixel quads (per-channel `mix(bg, fg, mask)`).
//! - **Color foreground** ([`create_color_fg_pipeline`]): `Rgba8Unorm` atlas-sampled
//!   color emoji quads (no `fg_color` tinting).
//!
//! All use `TriangleStrip` topology with vertex pulling (`@builtin(vertex_index)`).

use wgpu::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendComponent,
    BlendFactor, BlendOperation, BlendState, BufferBindingType, ColorTargetState, ColorWrites,
    Device, FragmentState, FrontFace, MultisampleState, PipelineLayoutDescriptor, PolygonMode,
    PrimitiveState, PrimitiveTopology, RenderPipeline, RenderPipelineDescriptor,
    SamplerBindingType, ShaderModuleDescriptor, ShaderStages, TextureSampleType,
    TextureViewDimension, VertexAttribute, VertexBufferLayout, VertexFormat, VertexState,
    VertexStepMode,
};

use super::instance_writer::INSTANCE_SIZE;
use super::state::GpuState;

/// Embedded WGSL source for the background shader.
const BG_SHADER_SRC: &str = include_str!("../shaders/bg.wgsl");

/// Embedded WGSL source for the foreground shader.
const FG_SHADER_SRC: &str = include_str!("../shaders/fg.wgsl");

/// Embedded WGSL source for the subpixel foreground shader.
const SUBPIXEL_FG_SHADER_SRC: &str = include_str!("../shaders/subpixel_fg.wgsl");

/// Embedded WGSL source for the color foreground shader.
const COLOR_FG_SHADER_SRC: &str = include_str!("../shaders/color_fg.wgsl");

/// Instance buffer stride in bytes.
pub const INSTANCE_STRIDE: u64 = INSTANCE_SIZE as u64;

/// Vertex attributes for the 80-byte instance record.
///
/// Maps to the `InstanceInput` struct in the WGSL shaders.
pub const INSTANCE_ATTRS: [VertexAttribute; 7] = [
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
    // location 2: uv (vec4<f32>) at offset 16.
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 16,
        shader_location: 2,
    },
    // location 3: fg_color (vec4<f32>) at offset 32.
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 32,
        shader_location: 3,
    },
    // location 4: bg_color (vec4<f32>) at offset 48.
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 48,
        shader_location: 4,
    },
    // location 5: kind (u32) at offset 64.
    VertexAttribute {
        format: VertexFormat::Uint32,
        offset: 64,
        shader_location: 5,
    },
    // location 6: atlas_page (u32) at offset 68.
    VertexAttribute {
        format: VertexFormat::Uint32,
        offset: 68,
        shader_location: 6,
    },
];

/// Premultiplied alpha blend state: `src * 1 + dst * (1 - src_alpha)`.
///
/// Used for both pipelines to support transparent windows.
const PREMUL_ALPHA_BLEND: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    },
};

/// Quad primitive state: `TriangleStrip`, no culling.
const QUAD_PRIMITIVE: PrimitiveState = PrimitiveState {
    topology: PrimitiveTopology::TriangleStrip,
    strip_index_format: None,
    front_face: FrontFace::Ccw,
    cull_mode: None,
    unclipped_depth: false,
    polygon_mode: PolygonMode::Fill,
    conservative: false,
};

/// Returns the instance buffer layout shared by both pipelines.
pub fn instance_buffer_layout() -> VertexBufferLayout<'static> {
    VertexBufferLayout {
        array_stride: INSTANCE_STRIDE,
        step_mode: VertexStepMode::Instance,
        attributes: &INSTANCE_ATTRS,
    }
}

/// Create the bind group layout for the uniform buffer (group 0).
///
/// Contains `screen_size: vec2<f32>` (16 bytes with padding).
/// Visible to vertex shaders for pixel-to-NDC conversion.
pub fn create_uniform_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("uniform_bind_group_layout"),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(16),
            },
            count: None,
        }],
    })
}

/// Create the bind group layout for the glyph atlas texture (group 1).
///
/// Contains an `R8Unorm` texture and linear sampler for glyph alpha lookup.
/// Visible to fragment shaders only.
pub fn create_atlas_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("atlas_bind_group_layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2Array,
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

/// Create the background render pipeline.
///
/// Uses only bind group 0 (uniforms). Renders solid-color quads — no texture
/// sampling. Premultiplied alpha blend for transparent window support.
pub fn create_bg_pipeline(gpu: &GpuState, uniform_layout: &BindGroupLayout) -> RenderPipeline {
    let shader = gpu.device.create_shader_module(ShaderModuleDescriptor {
        label: Some("bg_shader"),
        source: wgpu::ShaderSource::Wgsl(BG_SHADER_SRC.into()),
    });

    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("bg_pipeline_layout"),
            bind_group_layouts: &[uniform_layout],
            ..Default::default()
        });

    gpu.device
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("bg_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[instance_buffer_layout()],
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

/// Create the foreground render pipeline.
///
/// Uses bind groups 0 (uniforms) and 1 (atlas texture + sampler). Samples
/// glyph alpha from the atlas and tints with `fg_color`. Premultiplied alpha
/// blend for correct compositing over the background pass.
pub fn create_fg_pipeline(
    gpu: &GpuState,
    uniform_layout: &BindGroupLayout,
    atlas_layout: &BindGroupLayout,
) -> RenderPipeline {
    let shader = gpu.device.create_shader_module(ShaderModuleDescriptor {
        label: Some("fg_shader"),
        source: wgpu::ShaderSource::Wgsl(FG_SHADER_SRC.into()),
    });

    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("fg_pipeline_layout"),
            bind_group_layouts: &[uniform_layout, atlas_layout],
            ..Default::default()
        });

    gpu.device
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("fg_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[instance_buffer_layout()],
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

/// Create the subpixel foreground render pipeline.
///
/// Uses bind groups 0 (uniforms) and 1 (subpixel atlas texture + sampler).
/// Samples per-channel coverage from the RGBA atlas and composites with
/// `mix(bg, fg, mask)` for LCD subpixel rendering. Premultiplied alpha.
pub fn create_subpixel_fg_pipeline(
    gpu: &GpuState,
    uniform_layout: &BindGroupLayout,
    atlas_layout: &BindGroupLayout,
) -> RenderPipeline {
    let shader = gpu.device.create_shader_module(ShaderModuleDescriptor {
        label: Some("subpixel_fg_shader"),
        source: wgpu::ShaderSource::Wgsl(SUBPIXEL_FG_SHADER_SRC.into()),
    });

    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("subpixel_fg_pipeline_layout"),
            bind_group_layouts: &[uniform_layout, atlas_layout],
            ..Default::default()
        });

    gpu.device
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("subpixel_fg_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[instance_buffer_layout()],
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

/// Create the color foreground render pipeline.
///
/// Uses bind groups 0 (uniforms) and 1 (color atlas texture + sampler).
/// Samples RGBA color data directly from the atlas without `fg_color` tinting.
/// Used for color emoji rendering.
pub fn create_color_fg_pipeline(
    gpu: &GpuState,
    uniform_layout: &BindGroupLayout,
    atlas_layout: &BindGroupLayout,
) -> RenderPipeline {
    let shader = gpu.device.create_shader_module(ShaderModuleDescriptor {
        label: Some("color_fg_shader"),
        source: wgpu::ShaderSource::Wgsl(COLOR_FG_SHADER_SRC.into()),
    });

    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("color_fg_pipeline_layout"),
            bind_group_layouts: &[uniform_layout, atlas_layout],
            ..Default::default()
        });

    gpu.device
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("color_fg_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[instance_buffer_layout()],
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

#[cfg(test)]
mod tests;
