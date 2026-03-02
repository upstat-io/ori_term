//! GPU renderer: ties pipelines, atlas, fonts, and buffers into a single
//! draw-frame entry point.
//!
//! [`GpuRenderer`] owns all GPU resources needed to render a terminal frame.
//! The caller runs Extract → Prepare on the CPU, then hands the resulting
//! [`PreparedFrame`] to [`GpuRenderer::render_frame`] for GPU submission.

mod font_config;
mod helpers;
mod multi_pane;

use std::collections::HashSet;
use std::fmt;

use wgpu::{
    Buffer, Color, CommandEncoderDescriptor, LoadOp, Operations, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipeline, StoreOp, TextureView, TextureViewDescriptor,
};

use oriterm_core::Rgb;

use super::atlas::GlyphAtlas;
use super::bind_groups::{AtlasBindGroup, UniformBuffer};
use super::frame_input::FrameInput;
use super::pipeline::{
    create_atlas_bind_group_layout, create_bg_pipeline, create_color_fg_pipeline,
    create_fg_pipeline, create_subpixel_fg_pipeline, create_ui_rect_pipeline,
    create_uniform_bind_group_layout,
};
use super::prepare::{self, AtlasLookup};
use super::prepared_frame::PreparedFrame;
use super::state::GpuState;
use crate::font::{CellMetrics, FontCollection, GlyphFormat, RasterKey};
use crate::gpu::frame_input::ViewportSize;
use helpers::{
    ShapingScratch, ensure_glyphs_cached, grid_raster_keys, pre_cache_atlas, record_draw,
    shape_frame, ui_text_raster_keys, upload_buffer,
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
/// During the Prepare phase, glyph lookups probe the monochrome atlas first
/// (most glyphs are mono text), then the subpixel atlas, then the color atlas.
/// Each entry carries an [`AtlasKind`](super::atlas::AtlasKind) that the
/// prepare phase uses to route glyphs to the correct instance buffer.
struct CombinedAtlasLookup<'a> {
    mono: &'a GlyphAtlas,
    subpixel: &'a GlyphAtlas,
    color: &'a GlyphAtlas,
}

impl AtlasLookup for CombinedAtlasLookup<'_> {
    fn lookup_key(&self, key: RasterKey) -> Option<&super::atlas::AtlasEntry> {
        self.mono
            .lookup(key)
            .or_else(|| self.subpixel.lookup(key))
            .or_else(|| self.color.lookup(key))
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
    ui_rect_pipeline: RenderPipeline,

    // Bind groups + layouts
    uniform_buffer: UniformBuffer,
    atlas_bind_group: AtlasBindGroup,
    subpixel_atlas_bind_group: AtlasBindGroup,
    color_atlas_bind_group: AtlasBindGroup,
    // Atlases + fonts
    atlas: GlyphAtlas,
    subpixel_atlas: GlyphAtlas,
    color_atlas: GlyphAtlas,
    /// Keys known to produce zero-size glyphs (spaces, non-printing chars).
    ///
    /// Cross-atlas: a glyph that fails rasterization produces no bitmap
    /// regardless of target atlas. Owned here rather than per-atlas so
    /// all three atlases share a single authoritative set.
    empty_keys: HashSet<RasterKey>,
    font_collection: FontCollection,
    /// UI font collection (proportional sans-serif) for tab bar, labels, and overlays.
    ///
    /// `None` if no UI font was found — falls back to terminal font.
    ui_font_collection: Option<FontCollection>,

    // Per-frame reusable scratch buffers.
    shaping: ShapingScratch,
    /// GPU-ready instances for the current frame.
    ///
    /// Exposed to `app::redraw` so the pane render cache can merge cached
    /// per-pane instances into the aggregate frame.
    pub(crate) prepared: PreparedFrame,

    // Per-frame GPU instance buffers (grow-only, never shrink).
    bg_buffer: Option<Buffer>,
    fg_buffer: Option<Buffer>,
    subpixel_fg_buffer: Option<Buffer>,
    color_fg_buffer: Option<Buffer>,
    cursor_buffer: Option<Buffer>,
    ui_rect_buffer: Option<Buffer>,
    ui_fg_buffer: Option<Buffer>,
    ui_subpixel_fg_buffer: Option<Buffer>,
    ui_color_fg_buffer: Option<Buffer>,
    overlay_rect_buffer: Option<Buffer>,
    overlay_fg_buffer: Option<Buffer>,
    overlay_subpixel_fg_buffer: Option<Buffer>,
    overlay_color_fg_buffer: Option<Buffer>,
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
        let ui_rect_pipeline = create_ui_rect_pipeline(gpu, &uniform_layout);
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

        // UI font collection (proportional sans-serif for overlays).
        let ui_font_collection = load_ui_font_collection(&font_collection);
        let t_uifont = t0.elapsed();

        log::info!(
            "renderer init: pipelines={t_pipelines:?} precache={t_precache:?} uifont={t_uifont:?} total={:?}",
            t0.elapsed(),
        );

        Self {
            bg_pipeline,
            fg_pipeline,
            subpixel_fg_pipeline,
            color_fg_pipeline,
            ui_rect_pipeline,
            uniform_buffer,
            atlas_bind_group,
            subpixel_atlas_bind_group,
            color_atlas_bind_group,
            atlas,
            subpixel_atlas,
            color_atlas,
            empty_keys: HashSet::new(),
            font_collection,
            ui_font_collection,
            shaping: ShapingScratch::new(),
            prepared: PreparedFrame::new(ViewportSize::new(1, 1), Rgb { r: 0, g: 0, b: 0 }, 1.0),
            bg_buffer: None,
            fg_buffer: None,
            subpixel_fg_buffer: None,
            color_fg_buffer: None,
            cursor_buffer: None,
            ui_rect_buffer: None,
            ui_fg_buffer: None,
            ui_subpixel_fg_buffer: None,
            ui_color_fg_buffer: None,
            overlay_rect_buffer: None,
            overlay_fg_buffer: None,
            overlay_subpixel_fg_buffer: None,
            overlay_color_fg_buffer: None,
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

    /// Active UI font collection (proportional sans-serif, or terminal font fallback).
    pub fn active_ui_collection(&self) -> &FontCollection {
        self.ui_font_collection
            .as_ref()
            .unwrap_or(&self.font_collection)
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
    /// the first frame).
    ///
    /// The `origin` offset positions the grid on screen (from layout). The
    /// `cursor_blink_visible` flag gates cursor emission (from application
    /// blink state) — when `false`, no cursor instances are emitted even
    /// if the terminal reports the cursor as visible.
    ///
    /// Three phases:
    /// 1. **Shape** — segment rows into runs and shape via rustybuzz.
    /// 2. **Cache** — rasterize and upload any missing shaped glyphs.
    /// 3. **Prepare** — emit GPU instances from shaped glyph positions.
    pub fn prepare(
        &mut self,
        input: &FrameInput,
        gpu: &GpuState,
        origin: (f32, f32),
        cursor_blink_visible: bool,
    ) {
        self.atlas.begin_frame();
        self.subpixel_atlas.begin_frame();
        self.color_atlas.begin_frame();

        // Phase A: Shape all rows.
        shape_frame(input, &self.font_collection, &mut self.shaping);

        // Phase B: Ensure shaped glyphs cached (routes to mono, subpixel, or color atlas).
        ensure_glyphs_cached(
            grid_raster_keys(
                &self.shaping.frame,
                self.font_collection.hinting_mode().hint_flag(),
            ),
            &mut self.atlas,
            &mut self.subpixel_atlas,
            &mut self.color_atlas,
            &mut self.empty_keys,
            &mut self.font_collection,
            &gpu.queue,
        );

        // Phase B2: Ensure built-in geometric glyphs + decoration patterns cached.
        // Built-ins always go to the mono atlas (alpha-only bitmaps).
        super::builtin_glyphs::ensure_builtins_cached(
            input,
            self.shaping.frame.size_q6(),
            &mut self.atlas,
            &mut self.empty_keys,
            &gpu.queue,
        );

        // Phase C: Fill prepared frame via combined atlas lookup bridge.
        let bridge = CombinedAtlasLookup {
            mono: &self.atlas,
            subpixel: &self.subpixel_atlas,
            color: &self.color_atlas,
        };
        prepare::prepare_frame_shaped_into(
            input,
            &bridge,
            &self.shaping.frame,
            &mut self.prepared,
            origin,
            cursor_blink_visible,
        );

        log::trace!(
            "frame: cells={} bg_inst={} glyph_inst={} cursor_inst={}",
            input.content.cells.len(),
            self.prepared.backgrounds.len(),
            self.prepared.glyphs.len(),
            self.prepared.cursors.len(),
        );
    }

    /// Append UI rect draw commands from a [`DrawList`] into the prepared frame.
    ///
    /// Converts rect and line commands to GPU instances via
    /// [`convert_draw_list`]. Text commands are deferred (no text context
    /// provided) — chrome uses geometric symbols, not font glyphs.
    ///
    /// Call this after [`prepare`](Self::prepare) and before
    /// [`render_frame`](Self::render_frame).
    pub fn append_ui_draw_list(
        &mut self,
        draw_list: &oriterm_ui::draw::DrawList,
        scale: f32,
        opacity: f32,
    ) {
        super::draw_list_convert::convert_draw_list(
            draw_list,
            &mut self.prepared.ui_rects,
            None,
            scale,
            opacity,
        );
    }

    /// Append UI draw commands **with text** from a [`DrawList`].
    ///
    /// Unlike [`append_ui_draw_list`](Self::append_ui_draw_list) which defers
    /// text commands, this method:
    /// 1. Rasterizes uncached UI text glyphs into atlases.
    /// 2. Converts text commands with a real [`TextContext`] so glyph
    ///    instances are emitted into the mono/subpixel/color writers.
    ///
    /// Use this for overlays containing visible text (dialog title, message,
    /// button labels). Call after [`prepare`](Self::prepare) and before
    /// [`render_frame`](Self::render_frame).
    pub fn append_ui_draw_list_with_text(
        &mut self,
        draw_list: &oriterm_ui::draw::DrawList,
        scale: f32,
        opacity: f32,
        gpu: &GpuState,
    ) {
        let ui_fc = self
            .ui_font_collection
            .as_mut()
            .unwrap_or(&mut self.font_collection);
        let size_q6 = crate::font::size_key(ui_fc.size_px());
        let hinted = ui_fc.hinting_mode().hint_flag();

        let keys = ui_text_raster_keys(draw_list, size_q6, hinted, scale);
        ensure_glyphs_cached(
            keys.into_iter(),
            &mut self.atlas,
            &mut self.subpixel_atlas,
            &mut self.color_atlas,
            &mut self.empty_keys,
            ui_fc,
            &gpu.queue,
        );

        let bridge = CombinedAtlasLookup {
            mono: &self.atlas,
            subpixel: &self.subpixel_atlas,
            color: &self.color_atlas,
        };

        // Use UI-specific glyph writers so text renders AFTER UI rect
        // backgrounds (draws 7–9) instead of behind them (draws 2–4).
        // Per-text bg_hint is baked into each Text command by the layer stack.
        let mut text_ctx = super::draw_list_convert::TextContext {
            atlas: &bridge,
            mono_writer: &mut self.prepared.ui_glyphs,
            subpixel_writer: &mut self.prepared.ui_subpixel_glyphs,
            color_writer: &mut self.prepared.ui_color_glyphs,
            size_q6,
            hinted,
        };
        super::draw_list_convert::convert_draw_list(
            draw_list,
            &mut self.prepared.ui_rects,
            Some(&mut text_ctx),
            scale,
            opacity,
        );
    }

    /// Append overlay draw commands **with text** into the overlay tier.
    ///
    /// Identical to [`append_ui_draw_list_with_text`](Self::append_ui_draw_list_with_text)
    /// but writes to the overlay buffers (draws 10–13) instead of the chrome
    /// buffers (draws 6–9). This ensures overlay content renders ON TOP of
    /// all chrome text (tab bar titles), not behind it.
    pub fn append_overlay_draw_list_with_text(
        &mut self,
        draw_list: &oriterm_ui::draw::DrawList,
        scale: f32,
        opacity: f32,
        gpu: &GpuState,
    ) {
        let ui_fc = self
            .ui_font_collection
            .as_mut()
            .unwrap_or(&mut self.font_collection);
        let size_q6 = crate::font::size_key(ui_fc.size_px());
        let hinted = ui_fc.hinting_mode().hint_flag();

        let keys = ui_text_raster_keys(draw_list, size_q6, hinted, scale);
        ensure_glyphs_cached(
            keys.into_iter(),
            &mut self.atlas,
            &mut self.subpixel_atlas,
            &mut self.color_atlas,
            &mut self.empty_keys,
            ui_fc,
            &gpu.queue,
        );

        let bridge = CombinedAtlasLookup {
            mono: &self.atlas,
            subpixel: &self.subpixel_atlas,
            color: &self.color_atlas,
        };

        let mut text_ctx = super::draw_list_convert::TextContext {
            atlas: &bridge,
            mono_writer: &mut self.prepared.overlay_glyphs,
            subpixel_writer: &mut self.prepared.overlay_subpixel_glyphs,
            color_writer: &mut self.prepared.overlay_color_glyphs,
            size_q6,
            hinted,
        };
        super::draw_list_convert::convert_draw_list(
            draw_list,
            &mut self.prepared.overlay_rects,
            Some(&mut text_ctx),
            scale,
            opacity,
        );
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
        upload_buffer(
            device,
            queue,
            &mut self.ui_rect_buffer,
            self.prepared.ui_rects.as_bytes(),
            "ui_rect_instance_buffer",
        );
        upload_buffer(
            device,
            queue,
            &mut self.ui_fg_buffer,
            self.prepared.ui_glyphs.as_bytes(),
            "ui_fg_instance_buffer",
        );
        upload_buffer(
            device,
            queue,
            &mut self.ui_subpixel_fg_buffer,
            self.prepared.ui_subpixel_glyphs.as_bytes(),
            "ui_subpixel_fg_instance_buffer",
        );
        upload_buffer(
            device,
            queue,
            &mut self.ui_color_fg_buffer,
            self.prepared.ui_color_glyphs.as_bytes(),
            "ui_color_fg_instance_buffer",
        );
        upload_buffer(
            device,
            queue,
            &mut self.overlay_rect_buffer,
            self.prepared.overlay_rects.as_bytes(),
            "overlay_rect_instance_buffer",
        );
        upload_buffer(
            device,
            queue,
            &mut self.overlay_fg_buffer,
            self.prepared.overlay_glyphs.as_bytes(),
            "overlay_fg_instance_buffer",
        );
        upload_buffer(
            device,
            queue,
            &mut self.overlay_subpixel_fg_buffer,
            self.prepared.overlay_subpixel_glyphs.as_bytes(),
            "overlay_subpixel_fg_instance_buffer",
        );
        upload_buffer(
            device,
            queue,
            &mut self.overlay_color_fg_buffer,
            self.prepared.overlay_color_glyphs.as_bytes(),
            "overlay_color_fg_instance_buffer",
        );
    }

    /// Record the thirteen draw passes into the render pass.
    ///
    /// Three tiers in painter's order:
    /// - Terminal (draws 1–5): cell backgrounds, glyphs, cursors
    /// - Chrome (draws 6–9): UI rects + chrome text (tab bar, search bar)
    /// - Overlay (draws 10–13): overlay rects + overlay text (context menus)
    #[expect(
        clippy::too_many_lines,
        reason = "GPU draw dispatch table: 13 sequential record_draw calls across 3 tiers"
    )]
    fn record_draw_passes<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        let bg = self.uniform_buffer.bind_group();
        let mono = Some(self.atlas_bind_group.bind_group());
        let sub = Some(self.subpixel_atlas_bind_group.bind_group());
        let color = Some(self.color_atlas_bind_group.bind_group());
        let p = &self.prepared;

        // Terminal tier (draws 1–5).
        record_draw(
            pass,
            &self.bg_pipeline,
            bg,
            None,
            self.bg_buffer.as_ref(),
            p.backgrounds.len() as u32,
        );
        record_draw(
            pass,
            &self.fg_pipeline,
            bg,
            mono,
            self.fg_buffer.as_ref(),
            p.glyphs.len() as u32,
        );
        record_draw(
            pass,
            &self.subpixel_fg_pipeline,
            bg,
            sub,
            self.subpixel_fg_buffer.as_ref(),
            p.subpixel_glyphs.len() as u32,
        );
        record_draw(
            pass,
            &self.color_fg_pipeline,
            bg,
            color,
            self.color_fg_buffer.as_ref(),
            p.color_glyphs.len() as u32,
        );
        record_draw(
            pass,
            &self.bg_pipeline,
            bg,
            None,
            self.cursor_buffer.as_ref(),
            p.cursors.len() as u32,
        );

        // Chrome tier (draws 6–9).
        record_draw(
            pass,
            &self.ui_rect_pipeline,
            bg,
            None,
            self.ui_rect_buffer.as_ref(),
            p.ui_rects.len() as u32,
        );
        record_draw(
            pass,
            &self.fg_pipeline,
            bg,
            mono,
            self.ui_fg_buffer.as_ref(),
            p.ui_glyphs.len() as u32,
        );
        record_draw(
            pass,
            &self.subpixel_fg_pipeline,
            bg,
            sub,
            self.ui_subpixel_fg_buffer.as_ref(),
            p.ui_subpixel_glyphs.len() as u32,
        );
        record_draw(
            pass,
            &self.color_fg_pipeline,
            bg,
            color,
            self.ui_color_fg_buffer.as_ref(),
            p.ui_color_glyphs.len() as u32,
        );

        // Overlay tier (draws 10–13).
        record_draw(
            pass,
            &self.ui_rect_pipeline,
            bg,
            None,
            self.overlay_rect_buffer.as_ref(),
            p.overlay_rects.len() as u32,
        );
        record_draw(
            pass,
            &self.fg_pipeline,
            bg,
            mono,
            self.overlay_fg_buffer.as_ref(),
            p.overlay_glyphs.len() as u32,
        );
        record_draw(
            pass,
            &self.subpixel_fg_pipeline,
            bg,
            sub,
            self.overlay_subpixel_fg_buffer.as_ref(),
            p.overlay_subpixel_glyphs.len() as u32,
        );
        record_draw(
            pass,
            &self.color_fg_pipeline,
            bg,
            color,
            self.overlay_color_fg_buffer.as_ref(),
            p.overlay_color_glyphs.len() as u32,
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
}

/// Try to load a UI font collection for overlays and labels.
///
/// Uses [`discover_ui_fonts`](crate::font::discovery::discover_ui_fonts) to
/// find a proportional sans-serif font, then builds a [`FontCollection`] at
/// the same format and hinting settings as the terminal font. Returns `None`
/// if loading fails (the renderer falls back to the terminal font).
fn load_ui_font_collection(terminal_fc: &FontCollection) -> Option<FontCollection> {
    use crate::font::FontSet;

    let discovery = crate::font::discovery::discover_ui_fonts();
    let font_set = FontSet::from_discovery(&discovery).ok()?;
    // Use the terminal font's physical DPI so UI glyphs are rasterized
    // at the correct pixel size for the current display scale factor.
    let fc = FontCollection::new(
        font_set,
        11.0,
        terminal_fc.dpi(),
        terminal_fc.format(),
        400,
        terminal_fc.hinting_mode(),
    )
    .ok();
    if fc.is_some() {
        log::info!("UI font loaded: {:?}", discovery.primary.family_name,);
    }
    fc
}

#[cfg(test)]
mod tests;
