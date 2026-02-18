//! GPU renderer: ties pipelines, atlas, fonts, and buffers into a single
//! draw-frame entry point.
//!
//! [`GpuRenderer`] owns all GPU resources needed to render a terminal frame.
//! The caller runs Extract → Prepare on the CPU, then hands the resulting
//! [`PreparedFrame`] to [`GpuRenderer::render_frame`] for GPU submission.

use std::collections::HashMap;
use std::fmt;

use oriterm_core::CellFlags;
use wgpu::{
    BindGroupLayout, Buffer, BufferDescriptor, BufferUsages, Color, CommandEncoderDescriptor,
    Device, LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    StoreOp, TextureView, TextureViewDescriptor,
};

use super::atlas::GlyphAtlas;
use super::bind_groups::{AtlasBindGroup, UniformBuffer};
use super::frame_input::FrameInput;
use super::pipeline::{
    create_atlas_bind_group_layout, create_bg_pipeline, create_fg_pipeline,
    create_uniform_bind_group_layout,
};
use super::prepare::{self, AtlasLookup};
use super::prepared_frame::PreparedFrame;
use super::state::GpuState;
use crate::font::collection::size_key;
use crate::font::{CellMetrics, FontCollection, GlyphStyle, RasterKey};

// ── Error type ──

/// Error returned by [`GpuRenderer::render_to_surface`].
#[derive(Debug)]
pub enum SurfaceError {
    /// Surface is lost or outdated — caller should reconfigure.
    Lost,
    /// GPU is out of memory.
    OutOfMemory,
    /// Surface acquisition timed out.
    Timeout,
    /// Unspecified surface error.
    Other,
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lost => f.write_str("surface lost or outdated"),
            Self::OutOfMemory => f.write_str("GPU out of memory"),
            Self::Timeout => f.write_str("surface timeout"),
            Self::Other => f.write_str("surface error"),
        }
    }
}

impl std::error::Error for SurfaceError {}

// ── Atlas lookup bridge ──

/// Bridges [`FontCollection`] + [`GlyphAtlas`] into the [`AtlasLookup`] trait.
///
/// Used by the Prepare phase to look up cached glyphs without GPU types.
/// Reads from the resolve cache populated by [`GpuRenderer::ensure_glyphs_cached`]
/// to avoid redundant `FontCollection::resolve` calls.
struct RendererAtlas<'a> {
    resolve_cache: &'a HashMap<(char, GlyphStyle), RasterKey>,
    collection: &'a FontCollection,
    atlas: &'a GlyphAtlas,
    size_q6: u32,
}

impl AtlasLookup for RendererAtlas<'_> {
    fn lookup(&self, ch: char, style: GlyphStyle) -> Option<&super::atlas::AtlasEntry> {
        let key = if let Some(&cached) = self.resolve_cache.get(&(ch, style)) {
            cached
        } else {
            // Cache miss (should be rare — ensure_glyphs_cached covers all cells).
            let resolved = self.collection.resolve(ch, style);
            RasterKey::from_resolved(resolved, self.size_q6)
        };
        self.atlas.lookup(key)
    }
}

// ── GpuRenderer ──

/// Owns all GPU rendering resources and executes the Render phase.
///
/// Created once at startup, reused every frame. Holds the render pipelines,
/// glyph atlas, font collection, bind groups, and per-frame GPU buffers.
pub struct GpuRenderer {
    // Pipelines
    bg_pipeline: RenderPipeline,
    fg_pipeline: RenderPipeline,

    // Bind groups + layouts
    uniform_buffer: UniformBuffer,
    atlas_bind_group: AtlasBindGroup,
    #[allow(dead_code, reason = "retained for atlas rebuild on font change")]
    atlas_layout: BindGroupLayout,

    // Atlas + fonts
    atlas: GlyphAtlas,
    font_collection: FontCollection,

    // Resolve cache: (char, GlyphStyle) → RasterKey.
    // Stable across frames — only invalidated on font change. Populated by
    // `ensure_glyphs_cached`, consumed by `RendererAtlas::lookup`.
    resolve_cache: HashMap<(char, GlyphStyle), RasterKey>,

    // Per-frame GPU instance buffers (grow-only, never shrink).
    bg_buffer: Option<Buffer>,
    fg_buffer: Option<Buffer>,
    cursor_buffer: Option<Buffer>,
}

impl GpuRenderer {
    /// Create a new renderer with pipelines, atlas, and pre-cached ASCII glyphs.
    pub fn new(gpu: &GpuState, mut font_collection: FontCollection) -> Self {
        let t0 = std::time::Instant::now();
        let device = &gpu.device;
        let queue = &gpu.queue;

        // Layouts.
        let uniform_layout = create_uniform_bind_group_layout(device);
        let atlas_layout = create_atlas_bind_group_layout(device);

        // Pipelines.
        let bg_pipeline = create_bg_pipeline(gpu, &uniform_layout);
        let fg_pipeline = create_fg_pipeline(gpu, &uniform_layout, &atlas_layout);
        let t_pipelines = t0.elapsed();

        // Uniform buffer.
        let uniform_buffer = UniformBuffer::new(device, &uniform_layout);

        // Atlas + pre-cache printable ASCII (0x20–0x7E).
        let mut atlas = GlyphAtlas::new(device);
        let mut resolve_cache = HashMap::new();
        let size_q6 = size_key(font_collection.size_px());
        for ch in ' '..='~' {
            let resolved = font_collection.resolve(ch, GlyphStyle::Regular);
            let key = RasterKey::from_resolved(resolved, size_q6);
            resolve_cache.insert((ch, GlyphStyle::Regular), key);
            if let Some(glyph) = font_collection.rasterize(key) {
                atlas.insert(key, glyph, queue);
            }
        }
        let t_precache = t0.elapsed();

        // Atlas bind group (with real atlas texture array).
        let atlas_bind_group = AtlasBindGroup::new(device, &atlas_layout, atlas.view());

        log::info!(
            "renderer init: pipelines={t_pipelines:?} precache={t_precache:?} total={:?}",
            t0.elapsed(),
        );

        Self {
            bg_pipeline,
            fg_pipeline,
            uniform_buffer,
            atlas_bind_group,
            atlas_layout,
            atlas,
            font_collection,
            resolve_cache,
            bg_buffer: None,
            fg_buffer: None,
            cursor_buffer: None,
        }
    }

    // ── Accessors ──

    /// Cell dimensions derived from the current font metrics.
    pub fn cell_metrics(&self) -> CellMetrics {
        self.font_collection.cell_metrics()
    }

    /// Primary font family name.
    pub fn family_name(&self) -> &str {
        self.font_collection.family_name()
    }

    /// Glyph atlas for cache statistics.
    #[allow(dead_code, reason = "atlas access for diagnostics and Section 6")]
    pub fn atlas(&self) -> &GlyphAtlas {
        &self.atlas
    }

    // ── Frame preparation ──

    /// Ensure all glyphs in the frame are cached in the atlas.
    ///
    /// Rasterizes and uploads any missing glyphs. Must be called before
    /// [`prepare`](Self::prepare) so every atlas lookup hits the cache.
    pub fn ensure_glyphs_cached(&mut self, input: &FrameInput, gpu: &GpuState) {
        let size_q6 = size_key(self.font_collection.size_px());

        for cell in &input.content.cells {
            if cell.ch == ' ' || cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                continue;
            }

            let style = prepare::glyph_style(cell.flags);

            // Use the resolve cache to avoid redundant charmap + fallback
            // chain lookups. The cache persists across frames since the
            // mapping is stable until font configuration changes.
            let key = if let Some(&cached) = self.resolve_cache.get(&(cell.ch, style)) {
                cached
            } else {
                let resolved = self.font_collection.resolve(cell.ch, style);
                let k = RasterKey::from_resolved(resolved, size_q6);
                self.resolve_cache.insert((cell.ch, style), k);
                k
            };

            // Touch the page for LRU tracking if already cached.
            if self.atlas.lookup_touch(key).is_some() {
                continue;
            }

            if self.atlas.is_known_empty(key) {
                continue;
            }

            if let Some(glyph) = self.font_collection.rasterize(key) {
                self.atlas.insert(key, glyph, &gpu.queue);
            }
        }
    }

    /// Run the Prepare phase: convert a `FrameInput` into a `PreparedFrame`.
    ///
    /// Calls [`ensure_glyphs_cached`](Self::ensure_glyphs_cached) first, then
    /// builds the prepared frame via the atlas lookup bridge.
    pub fn prepare(&mut self, input: &FrameInput, gpu: &GpuState) -> PreparedFrame {
        self.atlas.begin_frame();
        self.ensure_glyphs_cached(input, gpu);
        let bridge = RendererAtlas {
            resolve_cache: &self.resolve_cache,
            collection: &self.font_collection,
            atlas: &self.atlas,
            size_q6: size_key(self.font_collection.size_px()),
        };
        prepare::prepare_frame(input, &bridge)
    }

    /// Run the Prepare phase into an existing `PreparedFrame`, reusing buffers.
    #[allow(dead_code, reason = "frame reuse optimization for later sections")]
    pub fn prepare_into(
        &mut self,
        input: &FrameInput,
        gpu: &GpuState,
        out: &mut PreparedFrame,
    ) {
        self.atlas.begin_frame();
        self.ensure_glyphs_cached(input, gpu);
        let bridge = RendererAtlas {
            resolve_cache: &self.resolve_cache,
            collection: &self.font_collection,
            atlas: &self.atlas,
            size_q6: size_key(self.font_collection.size_px()),
        };
        prepare::prepare_frame_into(input, &bridge, out);
    }

    // ── Render phase ──

    /// Upload prepared buffers to the GPU and execute draw calls.
    ///
    /// Accepts any `TextureView` as target — works for both surfaces and
    /// offscreen render targets (tab previews, headless testing).
    pub fn render_frame(
        &mut self,
        prepared: &PreparedFrame,
        gpu: &GpuState,
        target: &TextureView,
    ) {
        let device = &gpu.device;
        let queue = &gpu.queue;
        let vp = prepared.viewport;

        // Update screen_size uniform.
        self.uniform_buffer
            .write_screen_size(queue, vp.width as f32, vp.height as f32);

        // Upload instance data to GPU buffers.
        let bg_buf =
            ensure_buffer(device, &mut self.bg_buffer, prepared.backgrounds.as_bytes(), "bg_instance_buffer");
        let fg_buf =
            ensure_buffer(device, &mut self.fg_buffer, prepared.glyphs.as_bytes(), "fg_instance_buffer");
        let cur_buf =
            ensure_buffer(device, &mut self.cursor_buffer, prepared.cursors.as_bytes(), "cursor_instance_buffer");

        if let Some(buf) = bg_buf {
            queue.write_buffer(buf, 0, prepared.backgrounds.as_bytes());
        }
        if let Some(buf) = fg_buf {
            queue.write_buffer(buf, 0, prepared.glyphs.as_bytes());
        }
        if let Some(buf) = cur_buf {
            queue.write_buffer(buf, 0, prepared.cursors.as_bytes());
        }

        // Encode render commands.
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("frame_encoder"),
        });

        let clear = Color {
            r: prepared.clear_color[0],
            g: prepared.clear_color[1],
            b: prepared.clear_color[2],
            a: prepared.clear_color[3],
        };

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("terminal_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(clear),
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });

            let uniform_bg = self.uniform_buffer.bind_group();

            // Draw 1: Backgrounds (solid-color cell rects).
            if !prepared.backgrounds.is_empty() {
                if let Some(buf) = &self.bg_buffer {
                    pass.set_pipeline(&self.bg_pipeline);
                    pass.set_bind_group(0, uniform_bg, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..4, 0..prepared.backgrounds.len() as u32);
                }
            }

            // Draw 2: Glyphs (atlas-sampled text).
            if !prepared.glyphs.is_empty() {
                if let Some(buf) = &self.fg_buffer {
                    pass.set_pipeline(&self.fg_pipeline);
                    pass.set_bind_group(0, uniform_bg, &[]);
                    pass.set_bind_group(1, self.atlas_bind_group.bind_group(), &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..4, 0..prepared.glyphs.len() as u32);
                }
            }

            // Draw 3: Cursors (solid-color rects via bg pipeline).
            if !prepared.cursors.is_empty() {
                if let Some(buf) = &self.cursor_buffer {
                    pass.set_pipeline(&self.bg_pipeline);
                    pass.set_bind_group(0, uniform_bg, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..4, 0..prepared.cursors.len() as u32);
                }
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Acquire a surface texture, render, and present.
    ///
    /// Handles surface errors: `Lost`/`Outdated` → caller should reconfigure,
    /// `OutOfMemory` → propagated, `Timeout` → propagated.
    pub fn render_to_surface(
        &mut self,
        prepared: &PreparedFrame,
        gpu: &GpuState,
        surface: &wgpu::Surface<'_>,
    ) -> Result<(), SurfaceError> {
        let output = surface
            .get_current_texture()
            .map_err(|e| match e {
                wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => SurfaceError::Lost,
                wgpu::SurfaceError::OutOfMemory => SurfaceError::OutOfMemory,
                wgpu::SurfaceError::Timeout => SurfaceError::Timeout,
                wgpu::SurfaceError::Other => SurfaceError::Other,
            })?;

        // Create a view with the sRGB render format for correct gamma.
        let view = output.texture.create_view(&TextureViewDescriptor {
            format: Some(gpu.render_format()),
            ..Default::default()
        });

        self.render_frame(prepared, gpu, &view);
        output.present();
        Ok(())
    }
}

/// Ensure a GPU buffer exists and is large enough for `data`.
///
/// Returns `Some(&Buffer)` if data is non-empty (caller should write to it),
/// or `None` if data is empty (no upload needed).
fn ensure_buffer<'a>(
    device: &Device,
    slot: &'a mut Option<Buffer>,
    data: &[u8],
    label: &'static str,
) -> Option<&'a Buffer> {
    if data.is_empty() {
        return None;
    }

    let needed = data.len() as u64;
    let should_recreate = match slot {
        Some(buf) => buf.size() < needed,
        None => true,
    };

    if should_recreate {
        // Round up to next power of 2 for amortized growth.
        let size = needed.next_power_of_two().max(256);
        *slot = Some(device.create_buffer(&BufferDescriptor {
            label: Some(label),
            size,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }

    slot.as_ref()
}

#[cfg(test)]
mod tests;
