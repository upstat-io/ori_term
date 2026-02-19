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
    create_fg_pipeline, create_subpixel_fg_pipeline, create_uniform_bind_group_layout,
};
use super::prepare::{self, AtlasLookup};
use super::prepared_frame::PreparedFrame;
use super::state::GpuState;
use crate::font::{CellMetrics, FontCollection, GlyphFormat, RasterKey};
use crate::gpu::frame_input::ViewportSize;
use helpers::{
    ShapingScratch, ensure_shaped_glyphs_cached, pre_cache_atlas, record_draw, shape_frame,
    upload_buffer,
};

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

/// Bridges all atlases (mono, subpixel, color) into the [`AtlasLookup`] trait.
///
/// During the Prepare phase, glyph lookups check the color atlas first (for
/// color emoji), then the subpixel atlas, then the monochrome atlas. Each
/// entry carries an [`AtlasKind`](super::atlas::AtlasKind) that the prepare
/// phase uses to route glyphs to the correct instance buffer.
struct CombinedAtlasLookup<'a> {
    mono: &'a GlyphAtlas,
    subpixel: &'a GlyphAtlas,
    color: &'a GlyphAtlas,
}

impl AtlasLookup for CombinedAtlasLookup<'_> {
    fn lookup_key(&self, key: RasterKey) -> Option<&super::atlas::AtlasEntry> {
        self.color
            .lookup(key)
            .or_else(|| self.subpixel.lookup(key))
            .or_else(|| self.mono.lookup(key))
    }
}

// ── GpuRenderer ──

/// Owns all GPU rendering resources and executes the Render phase.
///
/// Created once at startup, reused every frame. Holds the render pipelines,
/// glyph atlases (monochrome + subpixel + color), font collection, bind
/// groups, and per-frame GPU buffers.
pub struct GpuRenderer {
    // Pipelines
    bg_pipeline: RenderPipeline,
    fg_pipeline: RenderPipeline,
    subpixel_fg_pipeline: RenderPipeline,
    color_fg_pipeline: RenderPipeline,

    // Bind groups + layouts
    uniform_buffer: UniformBuffer,
    atlas_bind_group: AtlasBindGroup,
    subpixel_atlas_bind_group: AtlasBindGroup,
    color_atlas_bind_group: AtlasBindGroup,
    #[allow(dead_code, reason = "retained for atlas rebuild on font change")]
    atlas_layout: BindGroupLayout,

    // Atlases + fonts
    atlas: GlyphAtlas,
    subpixel_atlas: GlyphAtlas,
    color_atlas: GlyphAtlas,
    font_collection: FontCollection,

    // Per-frame reusable scratch buffers.
    shaping: ShapingScratch,
    prepared: PreparedFrame,

    // Per-frame GPU instance buffers (grow-only, never shrink).
    bg_buffer: Option<Buffer>,
    fg_buffer: Option<Buffer>,
    subpixel_fg_buffer: Option<Buffer>,
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
        let subpixel_fg_pipeline = create_subpixel_fg_pipeline(gpu, &uniform_layout, &atlas_layout);
        let color_fg_pipeline = create_color_fg_pipeline(gpu, &uniform_layout, &atlas_layout);
        let t_pipelines = t0.elapsed();

        // Uniform buffer.
        let uniform_buffer = UniformBuffer::new(device, &uniform_layout);

        // Monochrome atlas + pre-cache printable ASCII (0x20–0x7E).
        // When subpixel is enabled, ASCII goes into the subpixel atlas instead.
        let format = font_collection.format();
        let (atlas, subpixel_atlas) = if format.is_subpixel() {
            let atlas = GlyphAtlas::new(device, GlyphFormat::Alpha);
            let mut sp_atlas = GlyphAtlas::new(device, format);
            pre_cache_atlas(&mut sp_atlas, &mut font_collection, queue);
            (atlas, sp_atlas)
        } else {
            let mut atlas = GlyphAtlas::new(device, GlyphFormat::Alpha);
            let sp_atlas = GlyphAtlas::new(device, GlyphFormat::SubpixelRgb);
            pre_cache_atlas(&mut atlas, &mut font_collection, queue);
            (atlas, sp_atlas)
        };
        let t_precache = t0.elapsed();

        // Color atlas (starts empty — emoji cached on first use).
        let color_atlas = GlyphAtlas::new(device, GlyphFormat::Color);

        // Bind groups.
        let atlas_bind_group = AtlasBindGroup::new(device, &atlas_layout, atlas.view());
        let subpixel_atlas_bind_group =
            AtlasBindGroup::new(device, &atlas_layout, subpixel_atlas.view());
        let color_atlas_bind_group = AtlasBindGroup::new(device, &atlas_layout, color_atlas.view());

        log::info!(
            "renderer init: pipelines={t_pipelines:?} precache={t_precache:?} total={:?}",
            t0.elapsed(),
        );

        Self {
            bg_pipeline,
            fg_pipeline,
            subpixel_fg_pipeline,
            color_fg_pipeline,
            uniform_buffer,
            atlas_bind_group,
            subpixel_atlas_bind_group,
            color_atlas_bind_group,
            atlas_layout,
            atlas,
            subpixel_atlas,
            color_atlas,
            font_collection,
            shaping: ShapingScratch::new(),
            prepared: PreparedFrame::new(ViewportSize::new(1, 1), Rgb { r: 0, g: 0, b: 0 }, 1.0),
            bg_buffer: None,
            fg_buffer: None,
            subpixel_fg_buffer: None,
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
        self.subpixel_atlas.begin_frame();
        self.color_atlas.begin_frame();

        // Phase A: Shape all rows.
        shape_frame(input, &self.font_collection, &mut self.shaping);

        // Phase B: Ensure shaped glyphs cached (routes to mono, subpixel, or color atlas).
        ensure_shaped_glyphs_cached(
            &self.shaping.frame,
            &mut self.atlas,
            &mut self.subpixel_atlas,
            &mut self.color_atlas,
            &mut self.font_collection,
            &gpu.queue,
        );

        // Phase B2: Ensure built-in geometric glyphs + decoration patterns cached.
        // Built-ins always go to the mono atlas (alpha-only bitmaps).
        super::builtin_glyphs::ensure_builtins_cached(
            input,
            self.shaping.frame.size_q6(),
            &mut self.atlas,
            &gpu.queue,
        );

        // Phase C: Fill prepared frame via combined atlas lookup bridge.
        let bridge = CombinedAtlasLookup {
            mono: &self.atlas,
            subpixel: &self.subpixel_atlas,
            color: &self.color_atlas,
        };
        prepare::prepare_frame_shaped_into(input, &bridge, &self.shaping.frame, &mut self.prepared);
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
        self.upload_instance_buffers(device, queue);

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

            self.record_draw_passes(&mut pass);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Upload all instance buffers to the GPU.
    fn upload_instance_buffers(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        upload_buffer(
            device,
            queue,
            &mut self.bg_buffer,
            self.prepared.backgrounds.as_bytes(),
            "bg_instance_buffer",
        );
        upload_buffer(
            device,
            queue,
            &mut self.fg_buffer,
            self.prepared.glyphs.as_bytes(),
            "fg_instance_buffer",
        );
        upload_buffer(
            device,
            queue,
            &mut self.subpixel_fg_buffer,
            self.prepared.subpixel_glyphs.as_bytes(),
            "subpixel_fg_instance_buffer",
        );
        upload_buffer(
            device,
            queue,
            &mut self.color_fg_buffer,
            self.prepared.color_glyphs.as_bytes(),
            "color_fg_instance_buffer",
        );
        upload_buffer(
            device,
            queue,
            &mut self.cursor_buffer,
            self.prepared.cursors.as_bytes(),
            "cursor_instance_buffer",
        );
    }

    /// Record the five draw passes into the render pass.
    fn record_draw_passes<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        let uniform_bg = self.uniform_buffer.bind_group();
        let mono_atlas = Some(self.atlas_bind_group.bind_group());
        let subpixel_atlas = Some(self.subpixel_atlas_bind_group.bind_group());
        let color_atlas = Some(self.color_atlas_bind_group.bind_group());

        // Draw 1: Backgrounds (solid-color cell rects).
        record_draw(
            pass,
            &self.bg_pipeline,
            uniform_bg,
            None,
            self.bg_buffer.as_ref(),
            self.prepared.backgrounds.len() as u32,
        );
        // Draw 2: Monochrome glyphs (R8Unorm atlas, tinted by `fg_color`).
        record_draw(
            pass,
            &self.fg_pipeline,
            uniform_bg,
            mono_atlas,
            self.fg_buffer.as_ref(),
            self.prepared.glyphs.len() as u32,
        );
        // Draw 3: Subpixel glyphs (Rgba8Unorm atlas, per-channel blend).
        record_draw(
            pass,
            &self.subpixel_fg_pipeline,
            uniform_bg,
            subpixel_atlas,
            self.subpixel_fg_buffer.as_ref(),
            self.prepared.subpixel_glyphs.len() as u32,
        );
        // Draw 4: Color glyphs (Rgba8Unorm atlas, rendered as-is).
        record_draw(
            pass,
            &self.color_fg_pipeline,
            uniform_bg,
            color_atlas,
            self.color_fg_buffer.as_ref(),
            self.prepared.color_glyphs.len() as u32,
        );
        // Draw 5: Cursors (solid-color rects via bg pipeline).
        record_draw(
            pass,
            &self.bg_pipeline,
            uniform_bg,
            None,
            self.cursor_buffer.as_ref(),
            self.prepared.cursors.len() as u32,
        );
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

    // ── Font size change ──

    /// Change font size, recomputing metrics, clearing atlases, and re-caching.
    ///
    /// Delegates to [`FontCollection::set_size`] for metrics + glyph cache,
    /// then clears all GPU atlases, re-populates the appropriate atlas with
    /// ASCII glyphs, and rebuilds bind groups for the new texture state.
    #[allow(dead_code, reason = "font size change wired in later section")]
    pub fn set_font_size(&mut self, size_pt: f32, dpi: f32, gpu: &GpuState) {
        self.font_collection.set_size(size_pt, dpi);
        self.clear_and_recache(gpu);
    }

    /// Change hinting mode, clearing atlases and re-caching.
    ///
    /// No-ops if the mode is unchanged. Mirrors [`set_font_size`] but only
    /// invalidates the glyph cache and atlases — cell metrics are unaffected
    /// because swash's `Metrics` API (used for cell dimensions) is independent
    /// of the hint flag.
    pub fn set_hinting_mode(&mut self, mode: crate::font::HintingMode, gpu: &GpuState) {
        if !self.font_collection.set_hinting(mode) {
            return;
        }
        self.clear_and_recache(gpu);
    }

    /// Change rasterization format (e.g. `Alpha` → `SubpixelRgb`), clearing
    /// atlases and re-caching.
    ///
    /// No-ops if the format is unchanged. Typically called once at startup
    /// after the display scale factor is known to enable LCD subpixel
    /// rendering on non-high-DPI displays.
    pub fn set_glyph_format(&mut self, format: GlyphFormat, gpu: &GpuState) {
        if !self.font_collection.set_format(format) {
            return;
        }
        self.clear_and_recache(gpu);
    }

    /// Clear all atlases, re-cache ASCII, and rebuild bind groups.
    fn clear_and_recache(&mut self, gpu: &GpuState) {
        self.atlas.clear();
        self.subpixel_atlas.clear();
        self.color_atlas.clear();

        let format = self.font_collection.format();
        if format.is_subpixel() {
            pre_cache_atlas(
                &mut self.subpixel_atlas,
                &mut self.font_collection,
                &gpu.queue,
            );
        } else {
            pre_cache_atlas(&mut self.atlas, &mut self.font_collection, &gpu.queue);
        }

        self.atlas_bind_group
            .rebuild(&gpu.device, &self.atlas_layout, self.atlas.view());
        self.subpixel_atlas_bind_group.rebuild(
            &gpu.device,
            &self.atlas_layout,
            self.subpixel_atlas.view(),
        );
        self.color_atlas_bind_group.rebuild(
            &gpu.device,
            &self.atlas_layout,
            self.color_atlas.view(),
        );
    }
}

#[cfg(test)]
mod tests;
