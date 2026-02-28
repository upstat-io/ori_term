//! Pooled render target allocation with power-of-two bucketing.
//!
//! [`RenderTargetPool`] manages offscreen GPU textures used as layer render
//! targets. Textures are reused across frames to avoid per-frame allocation.
//! Dimensions are rounded up to power-of-two buckets (minimum 256) to maximize
//! reuse when layer sizes vary slightly.

#![allow(
    dead_code,
    reason = "compositor infrastructure; production consumers in later sections"
)]

use std::fmt;

/// Handle to a pooled render target.
///
/// Obtained from [`RenderTargetPool::acquire`], used to access the texture
/// view and release the target back to the pool when done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PooledTargetId(usize);

/// A pool of reusable offscreen render target textures.
///
/// Textures have `RENDER_ATTACHMENT | TEXTURE_BINDING` usage so they can be
/// rendered into during the paint phase and sampled during the composition
/// phase.
pub struct RenderTargetPool {
    entries: Vec<PoolEntry>,
}

/// Internal entry tracking a single pooled texture.
struct PoolEntry {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Allocated width (power-of-two bucketed).
    width: u32,
    /// Allocated height (power-of-two bucketed).
    height: u32,
    /// Whether this entry is currently in use by a layer.
    in_use: bool,
}

impl RenderTargetPool {
    /// Creates an empty pool.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Acquires a render target of at least `width x height` pixels.
    ///
    /// Dimensions are rounded up to power-of-two buckets (minimum 256).
    /// Reuses an existing unused target if one matches; otherwise allocates
    /// a new texture.
    pub fn acquire(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> PooledTargetId {
        let bw = round_up_to_bucket(width);
        let bh = round_up_to_bucket(height);

        // Look for an existing unused entry with matching bucket size.
        for (i, entry) in self.entries.iter_mut().enumerate() {
            if !entry.in_use && entry.width == bw && entry.height == bh {
                entry.in_use = true;
                return PooledTargetId(i);
            }
        }

        // No match — allocate a new texture.
        let (texture, view) = create_pool_texture(device, bw, bh, format);
        let id = PooledTargetId(self.entries.len());
        self.entries.push(PoolEntry {
            texture,
            view,
            width: bw,
            height: bh,
            in_use: true,
        });
        id
    }

    /// Returns the texture view for use as a render pass attachment or sampler source.
    pub fn view(&self, id: PooledTargetId) -> &wgpu::TextureView {
        &self.entries[id.0].view
    }

    /// Returns the raw texture for use in GPU operations.
    pub fn texture(&self, id: PooledTargetId) -> &wgpu::Texture {
        &self.entries[id.0].texture
    }

    /// Returns the allocated (bucketed) dimensions of the target.
    pub fn size(&self, id: PooledTargetId) -> (u32, u32) {
        let e = &self.entries[id.0];
        (e.width, e.height)
    }

    /// Releases a render target back to the pool for reuse.
    pub fn release(&mut self, id: PooledTargetId) {
        self.entries[id.0].in_use = false;
    }

    /// Reclaims all unused textures, freeing GPU memory.
    ///
    /// Must only be called when all targets have been released (i.e., after
    /// clearing `layer_textures` in the compositor). Calling while targets
    /// are in use would invalidate stored `PooledTargetId` indices because
    /// `retain` compacts the `Vec`.
    pub fn trim(&mut self) {
        debug_assert!(
            self.entries.iter().all(|e| !e.in_use),
            "trim() called with {} targets still in use — release all targets first",
            self.active_count()
        );
        self.entries.retain(|e| e.in_use);
    }

    /// Returns the number of currently in-use targets.
    pub fn active_count(&self) -> usize {
        self.entries.iter().filter(|e| e.in_use).count()
    }

    /// Returns the total number of targets (active + pooled).
    pub fn total_count(&self) -> usize {
        self.entries.len()
    }
}

impl fmt::Debug for RenderTargetPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderTargetPool")
            .field("total", &self.total_count())
            .field("active", &self.active_count())
            .finish()
    }
}

/// Round a dimension up to the next power-of-two bucket (minimum 256).
fn round_up_to_bucket(size: u32) -> u32 {
    size.max(256).next_power_of_two()
}

/// Create a pooled texture with `RENDER_ATTACHMENT | TEXTURE_BINDING` usage.
fn create_pool_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("compositor_pool_target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

#[cfg(test)]
mod tests;
