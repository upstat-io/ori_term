//! GPU texture cache and rendering support for inline terminal images.
//!
//! Each image gets its own `wgpu::Texture` (not atlased — images vary
//! wildly in size). Textures are uploaded lazily when images enter the
//! viewport and evicted via LRU when GPU memory exceeds the limit.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use oriterm_core::image::ImageId;
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource,
    Device, Extent3d, FilterMode, Queue, SamplerDescriptor, Texture, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
};

/// Default GPU memory limit for image textures (512 MiB).
const DEFAULT_GPU_MEMORY_LIMIT: usize = 512 * 1024 * 1024;

/// An uploaded image texture on the GPU.
pub(crate) struct GpuImageTexture {
    _texture: Texture,
    _view: TextureView,
    bind_group: BindGroup,
    size_bytes: usize,
    last_frame: u64,
}

/// GPU-side image texture cache.
///
/// Manages per-image `wgpu::Texture` resources. Uploads are lazy (on first
/// viewport appearance). LRU eviction keeps GPU memory bounded.
pub(crate) struct ImageTextureCache {
    textures: HashMap<ImageId, GpuImageTexture>,
    gpu_memory_used: usize,
    gpu_memory_limit: usize,
    frame_counter: u64,
    sampler: wgpu::Sampler,
}

impl ImageTextureCache {
    /// Create a new empty cache with a shared sampler.
    pub(crate) fn new(device: &Device) -> Self {
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("image_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        Self {
            textures: HashMap::new(),
            gpu_memory_used: 0,
            gpu_memory_limit: DEFAULT_GPU_MEMORY_LIMIT,
            frame_counter: 0,
            sampler,
        }
    }

    /// Advance the frame counter. Call once per frame before `ensure_uploaded`.
    pub(crate) fn begin_frame(&mut self) {
        self.frame_counter += 1;
    }

    /// Ensure the image is uploaded to the GPU, returning its bind group.
    ///
    /// If already uploaded, touches the LRU counter. If not, creates a new
    /// texture and uploads the RGBA data.
    #[expect(
        clippy::too_many_arguments,
        reason = "GPU upload needs device, queue, layout, id, data, and dimensions"
    )]
    pub(crate) fn ensure_uploaded(
        &mut self,
        device: &Device,
        queue: &Queue,
        layout: &BindGroupLayout,
        id: ImageId,
        data: &[u8],
        width: u32,
        height: u32,
    ) -> &BindGroup {
        let frame = self.frame_counter;

        match self.textures.entry(id) {
            Entry::Occupied(e) => {
                let entry = e.into_mut();
                entry.last_frame = frame;
                &entry.bind_group
            }
            Entry::Vacant(e) => {
                let size = Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                };

                let texture = device.create_texture(&TextureDescriptor {
                    label: Some("image_texture"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba8UnormSrgb,
                    usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                    view_formats: &[],
                });

                queue.write_texture(
                    texture.as_image_copy(),
                    data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * width),
                        rows_per_image: Some(height),
                    },
                    size,
                );

                let view = texture.create_view(&TextureViewDescriptor::default());

                let bind_group = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("image_bind_group"),
                    layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::TextureView(&view),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::Sampler(&self.sampler),
                        },
                    ],
                });

                let size_bytes = (width as usize) * (height as usize) * 4;
                self.gpu_memory_used += size_bytes;

                let entry = e.insert(GpuImageTexture {
                    _texture: texture,
                    _view: view,
                    bind_group,
                    size_bytes,
                    last_frame: frame,
                });
                &entry.bind_group
            }
        }
    }

    /// Get the bind group for an already-uploaded image.
    pub(crate) fn get_bind_group(&self, id: ImageId) -> Option<&BindGroup> {
        self.textures.get(&id).map(|t| &t.bind_group)
    }

    /// Evict textures not used in the last `threshold` frames.
    pub(crate) fn evict_unused(&mut self, threshold: u64) {
        let cutoff = self.frame_counter.saturating_sub(threshold);
        let mut to_remove = Vec::new();

        for (&id, tex) in &self.textures {
            if tex.last_frame < cutoff {
                to_remove.push(id);
            }
        }

        for id in to_remove {
            if let Some(tex) = self.textures.remove(&id) {
                self.gpu_memory_used = self.gpu_memory_used.saturating_sub(tex.size_bytes);
            }
        }
    }

    /// Evict the oldest textures until GPU memory is under the limit.
    pub(crate) fn evict_over_limit(&mut self) {
        while self.gpu_memory_used > self.gpu_memory_limit && !self.textures.is_empty() {
            // Find the least recently used texture.
            let oldest = self
                .textures
                .iter()
                .min_by_key(|(_, t)| t.last_frame)
                .map(|(&id, _)| id);

            if let Some(id) = oldest {
                if let Some(tex) = self.textures.remove(&id) {
                    self.gpu_memory_used = self.gpu_memory_used.saturating_sub(tex.size_bytes);
                }
            }
        }
    }

    /// Remove a specific image texture.
    #[cfg(test)]
    pub(crate) fn remove(&mut self, id: ImageId) {
        if let Some(tex) = self.textures.remove(&id) {
            self.gpu_memory_used = self.gpu_memory_used.saturating_sub(tex.size_bytes);
        }
    }

    /// Update the GPU memory limit. Triggers eviction if currently over.
    pub(crate) fn set_gpu_memory_limit(&mut self, limit: usize) {
        self.gpu_memory_limit = limit;
        self.evict_over_limit();
    }

    /// Total GPU memory used by image textures.
    #[cfg(test)]
    pub(crate) fn gpu_memory_used(&self) -> usize {
        self.gpu_memory_used
    }

    /// Number of textures currently cached.
    #[cfg(test)]
    pub(crate) fn texture_count(&self) -> usize {
        self.textures.len()
    }
}

#[cfg(test)]
mod tests;
