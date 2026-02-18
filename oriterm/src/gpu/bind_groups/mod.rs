//! GPU bind group resources: uniform buffer and atlas texture binding.
//!
//! Two bind groups wire shader resources into the render pipelines:
//! - **Group 0** ([`UniformBuffer`]): screen dimensions for pixel-to-NDC conversion.
//! - **Group 1** ([`AtlasBindGroup`]): glyph atlas texture + sampler for text rendering.
//!
//! Bind group *layouts* live in [`super::pipeline`] (created in Section 5.4).
//! This module creates the actual GPU resources and bind groups from those layouts.

use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource,
    Buffer, BufferDescriptor, BufferUsages, Device, Extent3d, FilterMode, Queue, SamplerDescriptor,
    Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension,
};

/// Uniform buffer size in bytes: `vec2<f32> screen_size` + `vec2<f32> _pad` = 16 bytes.
const UNIFORM_BUFFER_SIZE: u64 = 16;

/// Uniform buffer for the `screen_size` shader uniform (group 0, binding 0).
///
/// Contains two `f32` values (width, height) padded to 16 bytes. Updated on
/// window resize via [`write_screen_size`](Self::write_screen_size).
pub struct UniformBuffer {
    buffer: Buffer,
    bind_group: BindGroup,
}

impl UniformBuffer {
    /// Create a new uniform buffer and its bind group.
    pub fn new(device: &Device, layout: &BindGroupLayout) -> Self {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("uniform_buffer"),
            size: UNIFORM_BUFFER_SIZE,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("uniform_bind_group"),
            layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self { buffer, bind_group }
    }

    /// Update the screen size uniform. Call on window resize.
    ///
    /// Writes `[width, height, 0.0, 0.0]` (16 bytes) to match the WGSL
    /// `Uniform { screen_size: vec2<f32>, _pad: vec2<f32> }` layout.
    pub fn write_screen_size(&self, queue: &Queue, width: f32, height: f32) {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&width.to_le_bytes());
        bytes[4..8].copy_from_slice(&height.to_le_bytes());
        // bytes[8..16] remain zero (_pad).
        queue.write_buffer(&self.buffer, 0, &bytes);
    }

    /// Returns the bind group for use in render passes (group 0).
    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }
}

/// Atlas bind group (group 1): glyph texture view + linear sampler.
///
/// Recreated when the atlas texture grows (new pages allocated). The sampler
/// is created once and reused across rebuilds.
pub struct AtlasBindGroup {
    bind_group: BindGroup,
    #[allow(dead_code, reason = "retained for rebuild on font/atlas change")]
    sampler: wgpu::Sampler,
}

impl AtlasBindGroup {
    /// Create a new atlas bind group with the given texture view.
    ///
    /// The sampler uses `ClampToEdge` addressing and `Linear` filtering,
    /// matching the standard glyph atlas sampling pattern.
    pub fn new(device: &Device, layout: &BindGroupLayout, view: &TextureView) -> Self {
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("atlas_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = create_atlas_bind_group(device, layout, view, &sampler);

        Self {
            bind_group,
            sampler,
        }
    }

    /// Recreate the bind group with a new texture view.
    ///
    /// Called when the atlas texture changes (e.g. font size change + clear).
    /// Reuses the existing sampler.
    #[allow(dead_code, reason = "used on atlas clear/font change")]
    pub fn rebuild(&mut self, device: &Device, layout: &BindGroupLayout, view: &TextureView) {
        self.bind_group = create_atlas_bind_group(device, layout, view, &self.sampler);
    }

    /// Returns the bind group for use in render passes (group 1).
    pub fn bind_group(&self) -> &BindGroup {
        &self.bind_group
    }
}

/// Create a 1×1 `R8Unorm` placeholder `D2Array` texture (white pixel).
///
/// Returns both the texture and its `D2Array` view so the atlas bind group
/// can be created before the real glyph atlas exists. A 1-layer `D2Array`
/// view satisfies the bind group layout.
#[allow(dead_code, reason = "placeholder texture for early pipeline init")]
pub fn create_placeholder_atlas_texture(device: &Device, queue: &Queue) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("placeholder_atlas"),
        size: Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::R8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Write a single white pixel (0xFF = 1.0 alpha).
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[0xFF],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(1),
            rows_per_image: None,
        },
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..Default::default()
    });
    (texture, view)
}

/// Create an atlas bind group from a texture view and sampler.
fn create_atlas_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    view: &TextureView,
    sampler: &wgpu::Sampler,
) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: Some("atlas_bind_group"),
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(sampler),
            },
        ],
    })
}

#[cfg(test)]
mod tests;
