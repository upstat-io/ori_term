//! Composition render pass: pipeline, bind groups, and draw logic.
//!
//! [`CompositionPass`] manages the GPU pipeline that composites layer textures
//! onto the screen. Each layer is drawn as a textured quad with per-layer
//! opacity and 2D affine transform. Layers are drawn back-to-front with
//! premultiplied alpha blending.

#![allow(
    dead_code,
    reason = "compositor infrastructure; production consumers in later sections"
)]

use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendComponent,
    BlendFactor, BlendOperation, BlendState, Buffer, BufferBindingType, BufferDescriptor,
    BufferUsages, ColorTargetState, ColorWrites, Device, FilterMode, FragmentState, FrontFace,
    MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology,
    Queue, RenderPass, RenderPipeline, RenderPipelineDescriptor, SamplerBindingType,
    SamplerDescriptor, ShaderModuleDescriptor, ShaderStages, TextureSampleType, TextureView,
    TextureViewDimension, VertexState,
};

use super::super::state::GpuState;

/// Embedded WGSL source for the composition shader.
const COMPOSITE_SHADER_SRC: &str = include_str!("../shaders/composite.wgsl");

/// Premultiplied alpha blend: `src * 1 + dst * (1 - src_alpha)`.
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

/// `TriangleStrip`, no culling — same as terminal pipelines.
const QUAD_PRIMITIVE: PrimitiveState = PrimitiveState {
    topology: PrimitiveTopology::TriangleStrip,
    strip_index_format: None,
    front_face: FrontFace::Ccw,
    cull_mode: None,
    unclipped_depth: false,
    polygon_mode: PolygonMode::Fill,
    conservative: false,
};

/// Byte size of the `LayerUniform` struct in GPU memory.
///
/// ```text
/// mat3x3<f32>  = 3 × vec3 padded to 16 bytes = 48 bytes
/// vec4<f32>    = 16 bytes  (bounds)
/// f32          =  4 bytes  (opacity)
/// vec3<f32>    = 12 bytes  (padding)
/// Total        = 80 bytes
/// ```
const LAYER_UNIFORM_SIZE: u64 = 80;

/// Maximum number of layers that can be composited in a single frame.
///
/// Determines the size of the dynamic uniform buffer. 64 layers is more
/// than sufficient for overlays, tab bars, dialogs, and terminal panes.
const MAX_LAYERS: u32 = 64;

/// Descriptor for a single layer to be composited.
pub struct CompositeLayerDesc<'a> {
    /// Texture view of the layer's rendered content.
    pub texture_view: &'a TextureView,
    /// Cached bind group for sampling this layer's texture (group 2).
    pub bind_group: &'a BindGroup,
    /// Layer bounds in screen pixels: `[x, y, width, height]`.
    pub bounds: [f32; 4],
    /// 2D affine transform as a column-major 3×3 matrix.
    ///
    /// Each column is `[a, b, 0]`, `[c, d, 0]`, `[tx, ty, 1]`.
    /// Stored as `[[col0], [col1], [col2]]`.
    pub transform: [[f32; 3]; 3],
    /// Layer opacity (0.0 = transparent, 1.0 = opaque).
    pub opacity: f32,
}

/// GPU resources for the layer composition pipeline.
///
/// Created once at startup. The `draw_layers` method records draw commands
/// into an active render pass.
pub struct CompositionPass {
    pipeline: RenderPipeline,
    layer_uniform_layout: BindGroupLayout,
    texture_layout: BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Dynamic uniform buffer for per-layer data.
    layer_uniform_buffer: Buffer,
    /// Bind group for the dynamic uniform buffer (group 1).
    layer_uniform_bind_group: BindGroup,
    /// Aligned size of one layer uniform slot in the dynamic buffer.
    uniform_alignment: u32,
    /// Reusable byte buffer for per-frame uniform data upload.
    scratch_uniform_data: Vec<u8>,
}

impl CompositionPass {
    /// Creates the composition pipeline, bind group layouts, and uniform buffer.
    ///
    /// The `screen_uniform_layout` is the existing group 0 layout shared with
    /// the terminal pipelines (contains `screen_size`).
    pub fn new(gpu: &GpuState, screen_uniform_layout: &BindGroupLayout) -> Self {
        let device = &gpu.device;

        let min_align = device.limits().min_uniform_buffer_offset_alignment;
        let uniform_alignment = align_up(LAYER_UNIFORM_SIZE as u32, min_align);

        let layer_uniform_layout = create_layer_uniform_layout(device);
        let texture_layout = create_texture_layout(device);

        let pipeline = create_pipeline(
            gpu,
            screen_uniform_layout,
            &layer_uniform_layout,
            &texture_layout,
        );

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("compositor_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let buffer_size = u64::from(uniform_alignment) * u64::from(MAX_LAYERS);
        let layer_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("compositor_layer_uniforms"),
            size: buffer_size,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let layer_uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("compositor_layer_uniform_bg"),
            layout: &layer_uniform_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &layer_uniform_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(LAYER_UNIFORM_SIZE),
                }),
            }],
        });

        Self {
            pipeline,
            layer_uniform_layout,
            texture_layout,
            sampler,
            layer_uniform_buffer,
            layer_uniform_bind_group,
            uniform_alignment,
            scratch_uniform_data: Vec::new(),
        }
    }

    /// Returns the bind group layout for layer textures (group 2).
    ///
    /// Used by [`super::GpuCompositor`] to create per-layer texture bind groups.
    pub fn texture_layout(&self) -> &BindGroupLayout {
        &self.texture_layout
    }

    /// Returns the shared sampler for layer texture sampling.
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// Uploads per-layer uniforms and records draw commands into the render pass.
    ///
    /// The `screen_uniform_bg` (group 0) must already be bound or will be set
    /// here. Layers are drawn in the order provided (caller ensures
    /// back-to-front).
    pub fn draw_layers<'a>(
        &'a mut self,
        queue: &Queue,
        pass: &mut RenderPass<'a>,
        screen_uniform_bg: &'a BindGroup,
        layers: &[CompositeLayerDesc<'a>],
    ) {
        debug_assert!(
            layers.len() <= MAX_LAYERS as usize,
            "too many layers: {} > {MAX_LAYERS}",
            layers.len()
        );

        if layers.is_empty() {
            return;
        }

        // Write all layer uniforms into the reusable scratch buffer.
        let aligned = self.uniform_alignment as usize;
        let required = layers.len() * aligned;
        self.scratch_uniform_data.clear();
        self.scratch_uniform_data.resize(required, 0);

        for (i, layer) in layers.iter().enumerate() {
            let offset = i * aligned;
            write_layer_uniform(&mut self.scratch_uniform_data[offset..], layer);
        }

        queue.write_buffer(&self.layer_uniform_buffer, 0, &self.scratch_uniform_data);

        // Record draw commands.
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, Some(screen_uniform_bg), &[]);

        for (i, layer) in layers.iter().enumerate() {
            let dynamic_offset = (i as u32) * self.uniform_alignment;
            pass.set_bind_group(1, Some(&self.layer_uniform_bind_group), &[dynamic_offset]);
            pass.set_bind_group(2, Some(layer.bind_group), &[]);
            pass.draw(0..4, 0..1);
        }
    }

    /// Creates a bind group for a layer's texture (group 2).
    pub fn create_texture_bind_group(
        &self,
        device: &Device,
        texture_view: &TextureView,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("compositor_layer_texture_bg"),
            layout: &self.texture_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

impl std::fmt::Debug for CompositionPass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositionPass")
            .field("uniform_alignment", &self.uniform_alignment)
            .finish_non_exhaustive()
    }
}

/// Creates the per-layer uniform bind group layout (group 1).
///
/// Uses a dynamic-offset uniform buffer so all layers share one bind group.
fn create_layer_uniform_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("compositor_layer_uniform_layout"),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(LAYER_UNIFORM_SIZE),
            },
            count: None,
        }],
    })
}

/// Creates the layer texture bind group layout (group 2).
///
/// Each layer gets its own bind group with a 2D texture and sampler.
fn create_texture_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("compositor_texture_layout"),
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

/// Creates the composition render pipeline.
fn create_pipeline(
    gpu: &GpuState,
    screen_uniform_layout: &BindGroupLayout,
    layer_uniform_layout: &BindGroupLayout,
    texture_layout: &BindGroupLayout,
) -> RenderPipeline {
    let device = &gpu.device;

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("composite_shader"),
        source: wgpu::ShaderSource::Wgsl(COMPOSITE_SHADER_SRC.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("composite_pipeline_layout"),
        bind_group_layouts: &[screen_uniform_layout, layer_uniform_layout, texture_layout],
        ..Default::default()
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("composite_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[], // No vertex/instance buffers — pure vertex pulling.
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

/// Write a `LayerUniform` into the buffer at the current offset.
///
/// Layout matches the WGSL struct:
/// ```text
/// mat3x3<f32>  @ 0   (3 × vec3 padded to vec4 = 48 bytes)
/// vec4<f32>    @ 48  (bounds)
/// f32          @ 64  (opacity)
/// vec3<f32>    @ 68  (padding, 12 bytes)
/// Total        = 80 bytes
/// ```
fn write_layer_uniform(buf: &mut [u8], layer: &CompositeLayerDesc<'_>) {
    write_layer_uniform_raw(buf, &layer.transform, &layer.bounds, layer.opacity);
}

/// Writes raw layer uniform fields into the buffer.
///
/// Separated from [`write_layer_uniform`] for testability (no GPU types needed).
fn write_layer_uniform_raw(
    buf: &mut [u8],
    transform: &[[f32; 3]; 3],
    bounds: &[f32; 4],
    opacity: f32,
) {
    // mat3x3: 3 columns, each vec3 padded to 16 bytes.
    for (col_idx, col) in transform.iter().enumerate() {
        let base = col_idx * 16;
        for (row_idx, &val) in col.iter().enumerate() {
            let off = base + row_idx * 4;
            buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
        }
        // Padding byte at base+12 is already zero.
    }

    // bounds: vec4<f32> at offset 48.
    for (i, &val) in bounds.iter().enumerate() {
        let off = 48 + i * 4;
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    }

    // opacity: f32 at offset 64.
    buf[64..68].copy_from_slice(&opacity.to_le_bytes());

    // Remaining bytes (68..80) are padding, already zero.
}

/// Round `value` up to the next multiple of `alignment`.
fn align_up(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_up_basic() {
        assert_eq!(align_up(80, 256), 256);
        assert_eq!(align_up(256, 256), 256);
        assert_eq!(align_up(257, 256), 512);
        assert_eq!(align_up(1, 1), 1);
        assert_eq!(align_up(0, 256), 0);
    }

    #[test]
    fn write_layer_uniform_identity_transform() {
        let transform = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let bounds = [10.0, 20.0, 100.0, 50.0];
        let opacity = 0.75;

        let mut buf = vec![0u8; 80];
        write_layer_uniform_raw(&mut buf, &transform, &bounds, opacity);

        // Column 0: [1.0, 0.0, 0.0, pad]
        assert_eq!(f32::from_le_bytes(buf[0..4].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(buf[4..8].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(buf[8..12].try_into().unwrap()), 0.0);
        // Padding at 12..16 should be zero.
        assert_eq!(&buf[12..16], &[0, 0, 0, 0]);

        // Column 1: [0.0, 1.0, 0.0, pad]
        assert_eq!(f32::from_le_bytes(buf[16..20].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(buf[20..24].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(buf[24..28].try_into().unwrap()), 0.0);

        // Column 2: [0.0, 0.0, 1.0, pad]
        assert_eq!(f32::from_le_bytes(buf[32..36].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(buf[36..40].try_into().unwrap()), 0.0);
        assert_eq!(f32::from_le_bytes(buf[40..44].try_into().unwrap()), 1.0);

        // Bounds at offset 48.
        assert_eq!(f32::from_le_bytes(buf[48..52].try_into().unwrap()), 10.0);
        assert_eq!(f32::from_le_bytes(buf[52..56].try_into().unwrap()), 20.0);
        assert_eq!(f32::from_le_bytes(buf[56..60].try_into().unwrap()), 100.0);
        assert_eq!(f32::from_le_bytes(buf[60..64].try_into().unwrap()), 50.0);

        // Opacity at offset 64.
        assert_eq!(f32::from_le_bytes(buf[64..68].try_into().unwrap()), 0.75);

        // Trailing padding should be zero.
        assert_eq!(&buf[68..80], &[0u8; 12]);
    }

    #[test]
    fn write_layer_uniform_translation() {
        // Transform with a 50px X, 30px Y translation.
        let transform = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [50.0, 30.0, 1.0]];
        let bounds = [0.0, 0.0, 200.0, 100.0];
        let opacity = 1.0;

        let mut buf = vec![0u8; 80];
        write_layer_uniform_raw(&mut buf, &transform, &bounds, opacity);

        // Column 2 should contain the translation.
        assert_eq!(f32::from_le_bytes(buf[32..36].try_into().unwrap()), 50.0);
        assert_eq!(f32::from_le_bytes(buf[36..40].try_into().unwrap()), 30.0);
        assert_eq!(f32::from_le_bytes(buf[40..44].try_into().unwrap()), 1.0);

        // Opacity.
        assert_eq!(f32::from_le_bytes(buf[64..68].try_into().unwrap()), 1.0);
    }
}
