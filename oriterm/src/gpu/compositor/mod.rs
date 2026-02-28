//! GPU compositor: offscreen layer rendering and composition.
//!
//! The [`GpuCompositor`] manages the full composition pipeline:
//!
//! 1. **Paint phase**: Dirty layers are rendered to pooled offscreen textures
//!    via [`RenderTargetPool`].
//! 2. **Composition phase**: All visible layers are drawn back-to-front via
//!    [`CompositionPass`], applying per-layer opacity and 2D transforms.
//!
//! Layers with default properties (opacity 1.0, identity transform) can use
//! a direct-render fast path that skips the intermediate texture.

#![allow(
    dead_code,
    reason = "compositor infrastructure; production consumers in later sections"
)]

mod composition_pass;
pub mod render_target_pool;

use std::collections::HashMap;

use wgpu::{BindGroup, BindGroupLayout, Device};

use composition_pass::{CompositeLayerDesc, CompositionPass};
use render_target_pool::{PooledTargetId, RenderTargetPool};

use oriterm_ui::compositor::layer::LayerId;

use super::state::GpuState;

/// Tracks the GPU texture assigned to a layer.
#[derive(Debug)]
struct TextureAssignment {
    /// Pool target for this layer's offscreen texture.
    target_id: PooledTargetId,
    /// Cached bind group for this layer's texture (group 2).
    bind_group: BindGroup,
}

/// GPU compositor for layer-based rendering.
///
/// Owns the render target pool and composition pipeline. Integrates with
/// the `oriterm_ui` layer tree to determine which layers need painting
/// and how to composite them.
pub struct GpuCompositor {
    pool: RenderTargetPool,
    pass: CompositionPass,
    /// Per-layer texture assignments (layer → pool target + bind group).
    layer_textures: HashMap<LayerId, TextureAssignment>,
}

impl GpuCompositor {
    /// Creates a new compositor with the given screen uniform layout.
    ///
    /// The `screen_uniform_layout` is the existing group 0 layout shared
    /// with the terminal pipelines (contains `screen_size: vec2<f32>`).
    pub fn new(gpu: &GpuState, screen_uniform_layout: &BindGroupLayout) -> Self {
        Self {
            pool: RenderTargetPool::new(),
            pass: CompositionPass::new(gpu, screen_uniform_layout),
            layer_textures: HashMap::new(),
        }
    }

    /// Ensures a layer has a pooled render target of the given size.
    ///
    /// If the layer already has a target of sufficient size, it is reused.
    /// Otherwise, the old target is released and a new one acquired.
    pub fn ensure_layer_target(&mut self, device: &Device, req: &LayerTargetRequest) {
        // Check if existing assignment matches.
        if let Some(assignment) = self.layer_textures.get(&req.layer_id) {
            let (ew, eh) = self.pool.size(assignment.target_id);
            if ew >= req.width && eh >= req.height {
                return; // Existing target is large enough.
            }
            // Release the old target.
            let old_id = assignment.target_id;
            self.pool.release(old_id);
        }

        let target_id = self.pool.acquire(device, req.width, req.height, req.format);
        let view = self.pool.view(target_id);
        let bind_group = self.pass.create_texture_bind_group(device, view);

        self.layer_textures.insert(
            req.layer_id,
            TextureAssignment {
                target_id,
                bind_group,
            },
        );
    }

    /// Returns the texture view for a layer's render target.
    ///
    /// The layer must have a target assigned via [`ensure_layer_target`].
    pub fn layer_target_view(&self, layer_id: LayerId) -> Option<&wgpu::TextureView> {
        self.layer_textures
            .get(&layer_id)
            .map(|a| self.pool.view(a.target_id))
    }

    /// Composites the given layers onto the active render pass.
    ///
    /// Layers must be provided in back-to-front order. Each layer descriptor
    /// references a previously assigned render target.
    ///
    /// `screen_uniform_bg` is the bind group for group 0 (`screen_size` uniform).
    pub fn compose<'a>(
        &'a mut self,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'a>,
        screen_uniform_bg: &'a BindGroup,
        layer_descs: &[LayerCompositeInfo],
    ) {
        if layer_descs.is_empty() {
            return;
        }

        // Build CompositeLayerDesc from layer info + texture assignments.
        // This Vec holds references into `self.pool` and `self.layer_textures`,
        // so it cannot be stored as a reusable field (self-referential borrow).
        let mut descs = Vec::with_capacity(layer_descs.len());

        for info in layer_descs {
            let assignment = match self.layer_textures.get(&info.layer_id) {
                Some(a) => a,
                None => continue, // Skip layers without assigned textures.
            };

            descs.push(CompositeLayerDesc {
                texture_view: self.pool.view(assignment.target_id),
                bind_group: &assignment.bind_group,
                bounds: info.bounds,
                transform: info.transform,
                opacity: info.opacity,
            });
        }

        self.pass
            .draw_layers(queue, pass, screen_uniform_bg, &descs);
    }

    /// Releases the render target for a layer and removes its texture assignment.
    pub fn release_layer_target(&mut self, layer_id: LayerId) {
        if let Some(assignment) = self.layer_textures.remove(&layer_id) {
            self.pool.release(assignment.target_id);
        }
    }

    /// Reclaims all pool textures, freeing GPU memory.
    ///
    /// All layer targets must be released first via [`release_layer_target`]
    /// or [`clear_layer_targets`]. Calling with active targets will panic in
    /// debug builds.
    pub fn trim_pool(&mut self) {
        self.pool.trim();
    }

    /// Releases all layer texture assignments and their pool targets.
    ///
    /// Call before [`trim_pool`] to ensure no stale `PooledTargetId` handles
    /// remain (e.g., on resize when all layers will be re-acquired).
    pub fn clear_layer_targets(&mut self) {
        for assignment in self.layer_textures.values() {
            self.pool.release(assignment.target_id);
        }
        self.layer_textures.clear();
    }

    /// Returns whether a layer has a direct-render fast path available.
    ///
    /// A layer can be rendered directly (without an intermediate texture)
    /// when it has identity transform and full opacity. The caller should
    /// check this before assigning a render target.
    pub fn is_direct_render_eligible(opacity: f32, transform: &[[f32; 3]; 3]) -> bool {
        const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        (opacity - 1.0).abs() < f32::EPSILON && *transform == IDENTITY
    }

    /// Returns the render target pool (for diagnostics/testing).
    pub fn pool(&self) -> &RenderTargetPool {
        &self.pool
    }
}

impl std::fmt::Debug for GpuCompositor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuCompositor")
            .field("pool", &self.pool)
            .field("layers", &self.layer_textures.len())
            .finish_non_exhaustive()
    }
}

/// Information needed to composite a single layer.
#[derive(Debug, Clone)]
pub struct LayerCompositeInfo {
    /// Layer identifier.
    pub layer_id: LayerId,
    /// Layer bounds in screen pixels: `[x, y, width, height]`.
    pub bounds: [f32; 4],
    /// 2D affine transform as column-major 3×3 matrix.
    pub transform: [[f32; 3]; 3],
    /// Layer opacity (0.0–1.0).
    pub opacity: f32,
}

/// Parameters for allocating a layer render target.
#[derive(Debug, Clone, Copy)]
pub struct LayerTargetRequest {
    /// Layer identifier.
    pub layer_id: LayerId,
    /// Required width in pixels.
    pub width: u32,
    /// Required height in pixels.
    pub height: u32,
    /// Texture format.
    pub format: wgpu::TextureFormat,
}

#[cfg(test)]
mod tests;
