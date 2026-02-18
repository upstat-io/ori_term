//! GPU renderer: ties pipelines, atlas, fonts, and buffers into a single
//! draw-frame entry point.
//!
//! [`GpuRenderer`] owns all GPU resources needed to render a terminal frame.
//! The caller runs Extract → Prepare on the CPU, then hands the resulting
//! [`PreparedFrame`] to [`GpuRenderer::render_frame`] for GPU submission.

mod helpers;

use std::fmt;

use wgpu::{
    BindGroupLayout, Buffer, Color, CommandEncoderDescriptor, LoadOp, Operations,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, StoreOp, TextureView,
    TextureViewDescriptor,
};

use oriterm_core::Rgb;

use super::atlas::GlyphAtlas;
use super::bind_groups::{AtlasBindGroup, UniformBuffer};
use super::frame_input::FrameInput;
use super::pipeline::{
    create_atlas_bind_group_layout, create_bg_pipeline, create_color_fg_pipeline,
    create_fg_pipeline, create_uniform_bind_group_layout,
};
use super::prepare::{self, AtlasLookup};
use super::prepared_frame::PreparedFrame;
use super::state::GpuState;
use crate::font::collection::size_key;
use crate::font::{CellMetrics, FontCollection, GlyphFormat, GlyphStyle, RasterKey};
use crate::gpu::frame_input::ViewportSize;
use helpers::{ensure_buffer, ensure_shaped_glyphs_cached, shape_frame, ShapingScratch};

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

/// Bridges both monochrome and color atlases into the [`AtlasLookup`] trait.
///
/// During the Prepare phase, glyph lookups check the color atlas first (for
/// color emoji), then the monochrome atlas. Entries from the color atlas
/// have `is_color = true`, enabling the prepare phase to route them to the
/// correct instance buffer.
struct CombinedAtlasLookup<'a> {
    mono: &'a GlyphAtlas,
    color: &'a GlyphAtlas,
}

impl AtlasLookup for CombinedAtlasLookup<'_> {
    fn lookup_key(&self, key: RasterKey) -> Option<&super::atlas::AtlasEntry> {
        self.color.lookup(key).or_else(|| self.mono.lookup(key))
    }
}

// ── GpuRenderer ──

/// Owns all GPU rendering resources and executes the Render phase.
///
/// Created once at startup, reused every frame. Holds the render pipelines,
/// glyph atlases (monochrome + color), font collection, bind groups, and
/// per-frame GPU buffers.
pub struct GpuRenderer {
    // Pipelines
    bg_pipeline: RenderPipeline,
    fg_pipeline: RenderPipeline,
    color_fg_pipeline: RenderPipeline,

    // Bind groups + layouts
    uniform_buffer: UniformBuffer,
    atlas_bind_group: AtlasBindGroup,
    color_atlas_bind_group: AtlasBindGroup,
    #[allow(dead_code, reason = "retained for atlas rebuild on font change")]
    atlas_layout: BindGroupLayout,

    // Atlases + fonts
    atlas: GlyphAtlas,
    color_atlas: GlyphAtlas,
    font_collection: FontCollection,

    // Per-frame reusable scratch buffers.
    shaping: ShapingScratch,
    prepared: PreparedFrame,

    // Per-frame GPU instance buffers (grow-only, never shrink).
    bg_buffer: Option<Buffer>,
    fg_buffer: Option<Buffer>,
    color_fg_buffer: Option<Buffer>,
    cursor_buffer: Option<Buffer>,
}

impl GpuRenderer {
    /// Create a new renderer with pipelines, atlases, and pre-cached ASCII glyphs.
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
        let color_fg_pipeline = create_color_fg_pipeline(gpu, &uniform_layout, &atlas_layout);
        let t_pipelines = t0.elapsed();

        // Uniform buffer.
        let uniform_buffer = UniformBuffer::new(device, &uniform_layout);

        // Monochrome atlas + pre-cache printable ASCII (0x20–0x7E).
        let mut atlas = GlyphAtlas::new(device, GlyphFormat::Alpha);
        let size_q6 = size_key(font_collection.size_px());
        for ch in ' '..='~' {
            let resolved = font_collection.resolve(ch, GlyphStyle::Regular);
            let key = RasterKey::from_resolved(resolved, size_q6);
            if let Some(glyph) = font_collection.rasterize(key) {
                atlas.insert(key, glyph, queue);
            }
        }
        let t_precache = t0.elapsed();

        // Color atlas (starts empty — emoji cached on first use).
        let color_atlas = GlyphAtlas::new(device, GlyphFormat::Color);

        // Bind groups.
        let atlas_bind_group = AtlasBindGroup::new(device, &atlas_layout, atlas.view());
        let color_atlas_bind_group =
            AtlasBindGroup::new(device, &atlas_layout, color_atlas.view());

        log::info!(
            "renderer init: pipelines={t_pipelines:?} precache={t_precache:?} total={:?}",
            t0.elapsed(),
        );

        Self {
            bg_pipeline,
            fg_pipeline,
            color_fg_pipeline,
            uniform_buffer,
            atlas_bind_group,
            color_atlas_bind_group,
            atlas_layout,
            atlas,
            color_atlas,
            font_collection,
            shaping: ShapingScratch::new(),
            prepared: PreparedFrame::new(
                ViewportSize::new(1, 1),
                Rgb { r: 0, g: 0, b: 0 },
                1.0,
            ),
            bg_buffer: None,
            fg_buffer: None,
            color_fg_buffer: None,
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

    /// Run the Prepare phase: shape text and build GPU instance buffers.
    ///
    /// Fills `self.prepared` via buffer reuse (no per-frame allocation after
    /// the first frame). Access the result via [`prepared()`](Self::prepared).
    ///
    /// Three phases:
    /// 1. **Shape** — segment rows into runs and shape via rustybuzz.
    /// 2. **Cache** — rasterize and upload any missing shaped glyphs.
    /// 3. **Prepare** — emit GPU instances from shaped glyph positions.
    pub fn prepare(&mut self, input: &FrameInput, gpu: &GpuState) {
        self.atlas.begin_frame();
        self.color_atlas.begin_frame();

        // Phase A: Shape all rows.
        shape_frame(input, &self.font_collection, &mut self.shaping);

        // Phase B: Ensure shaped glyphs cached (routes to mono or color atlas).
        ensure_shaped_glyphs_cached(
            &self.shaping.frame,
            &mut self.atlas,
            &mut self.color_atlas,
            &mut self.font_collection,
            &gpu.queue,
        );

        // Phase B2: Ensure built-in geometric glyphs cached.
        super::builtin_glyphs::ensure_cached(
            input,
            self.shaping.frame.size_q6(),
            &mut self.atlas,
            &gpu.queue,
        );

        // Phase B3: Ensure patterned decoration glyphs cached.
        super::builtin_glyphs::ensure_decorations_cached(
            input,
            self.shaping.frame.size_q6(),
            &mut self.atlas,
            &gpu.queue,
        );

        // Phase C: Fill prepared frame via combined atlas lookup bridge.
        let bridge = CombinedAtlasLookup {
            mono: &self.atlas,
            color: &self.color_atlas,
        };
        prepare::prepare_frame_shaped_into(
            input,
            &bridge,
            &self.shaping.frame,
            &mut self.prepared,
        );
    }

    /// The most recently prepared frame.
    pub fn prepared(&self) -> &PreparedFrame {
        &self.prepared
    }

    // ── Render phase ──

    /// Upload the stored prepared frame to the GPU and execute draw calls.
    ///
    /// Reads from `self.prepared` (filled by [`prepare`](Self::prepare)).
    /// Accepts any `TextureView` as target — works for both surfaces and
    /// offscreen render targets (tab previews, headless testing).
    pub fn render_frame(&mut self, gpu: &GpuState, target: &TextureView) {
        let device = &gpu.device;
        let queue = &gpu.queue;
        let vp = self.prepared.viewport;

        // Update screen_size uniform.
        self.uniform_buffer
            .write_screen_size(queue, vp.width as f32, vp.height as f32);

        // Upload instance data to GPU buffers.
        let bg_buf = ensure_buffer(
            device,
            &mut self.bg_buffer,
            self.prepared.backgrounds.as_bytes(),
            "bg_instance_buffer",
        );
        let fg_buf = ensure_buffer(
            device,
            &mut self.fg_buffer,
            self.prepared.glyphs.as_bytes(),
            "fg_instance_buffer",
        );
        let color_fg_buf = ensure_buffer(
            device,
            &mut self.color_fg_buffer,
            self.prepared.color_glyphs.as_bytes(),
            "color_fg_instance_buffer",
        );
        let cur_buf = ensure_buffer(
            device,
            &mut self.cursor_buffer,
            self.prepared.cursors.as_bytes(),
            "cursor_instance_buffer",
        );

        if let Some(buf) = bg_buf {
            queue.write_buffer(buf, 0, self.prepared.backgrounds.as_bytes());
        }
        if let Some(buf) = fg_buf {
            queue.write_buffer(buf, 0, self.prepared.glyphs.as_bytes());
        }
        if let Some(buf) = color_fg_buf {
            queue.write_buffer(buf, 0, self.prepared.color_glyphs.as_bytes());
        }
        if let Some(buf) = cur_buf {
            queue.write_buffer(buf, 0, self.prepared.cursors.as_bytes());
        }

        // Encode render commands.
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("frame_encoder"),
        });

        let clear = Color {
            r: self.prepared.clear_color[0],
            g: self.prepared.clear_color[1],
            b: self.prepared.clear_color[2],
            a: self.prepared.clear_color[3],
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
            if !self.prepared.backgrounds.is_empty() {
                if let Some(buf) = &self.bg_buffer {
                    pass.set_pipeline(&self.bg_pipeline);
                    pass.set_bind_group(0, uniform_bg, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..4, 0..self.prepared.backgrounds.len() as u32);
                }
            }

            // Draw 2: Monochrome glyphs (R8Unorm atlas, tinted by fg_color).
            if !self.prepared.glyphs.is_empty() {
                if let Some(buf) = &self.fg_buffer {
                    pass.set_pipeline(&self.fg_pipeline);
                    pass.set_bind_group(0, uniform_bg, &[]);
                    pass.set_bind_group(1, self.atlas_bind_group.bind_group(), &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..4, 0..self.prepared.glyphs.len() as u32);
                }
            }

            // Draw 3: Color glyphs (Rgba8Unorm atlas, rendered as-is).
            if !self.prepared.color_glyphs.is_empty() {
                if let Some(buf) = &self.color_fg_buffer {
                    pass.set_pipeline(&self.color_fg_pipeline);
                    pass.set_bind_group(0, uniform_bg, &[]);
                    pass.set_bind_group(
                        1,
                        self.color_atlas_bind_group.bind_group(),
                        &[],
                    );
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..4, 0..self.prepared.color_glyphs.len() as u32);
                }
            }

            // Draw 4: Cursors (solid-color rects via bg pipeline).
            if !self.prepared.cursors.is_empty() {
                if let Some(buf) = &self.cursor_buffer {
                    pass.set_pipeline(&self.bg_pipeline);
                    pass.set_bind_group(0, uniform_bg, &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..4, 0..self.prepared.cursors.len() as u32);
                }
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Acquire a surface texture, render the stored prepared frame, and present.
    ///
    /// Handles surface errors: `Lost`/`Outdated` → caller should reconfigure,
    /// `OutOfMemory` → propagated, `Timeout` → propagated.
    pub fn render_to_surface(
        &mut self,
        gpu: &GpuState,
        surface: &wgpu::Surface<'_>,
    ) -> Result<(), SurfaceError> {
        let output = surface.get_current_texture().map_err(|e| match e {
            wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => SurfaceError::Lost,
            wgpu::SurfaceError::OutOfMemory => SurfaceError::OutOfMemory,
            wgpu::SurfaceError::Timeout => SurfaceError::Timeout,
            wgpu::SurfaceError::Other => SurfaceError::Other,
        })?;

        let view = output.texture.create_view(&TextureViewDescriptor {
            format: Some(gpu.render_format()),
            ..Default::default()
        });

        self.render_frame(gpu, &view);
        output.present();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
