//! Per-window GPU renderer: owns fonts, atlases, shaping caches, and instance buffers.
//!
//! [`WindowRenderer`] holds all GPU resources specific to a single window.
//! Each window gets its own renderer so DPI scaling, atlas caches, and
//! shaping state are fully isolated — no cross-window contamination.

mod draw_list;
mod font_config;
mod helpers;
mod multi_pane;
mod render;

use std::collections::HashSet;
use std::fmt;

use wgpu::Buffer;

use oriterm_core::Rgb;

use super::atlas::GlyphAtlas;
use super::bind_groups::{AtlasBindGroup, UniformBuffer};
use super::frame_input::FrameInput;
use super::image_render::ImageTextureCache;
use super::pipelines::GpuPipelines;
use super::prepare::{self, AtlasLookup};
use super::prepared_frame::PreparedFrame;
use super::state::GpuState;
use crate::font::{CellMetrics, FontCollection, GlyphFormat, RasterKey};
use crate::gpu::frame_input::ViewportSize;
use helpers::{
    ShapingScratch, ensure_glyphs_cached, grid_raster_keys, pre_cache_atlas, shape_frame,
};

// ── Error type ──

/// Error returned by [`WindowRenderer::render_to_surface`].
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

// ── WindowRenderer ──

/// Per-window GPU renderer: owns fonts, atlases, and instance buffers.
///
/// Created per-window at window creation time. Holds the bind groups,
/// glyph atlases (monochrome + subpixel + color), font collection, and
/// per-frame GPU buffers. Pipelines are shared via [`GpuPipelines`].
pub struct WindowRenderer {
    // Bind groups (per-window, created with layouts from GpuPipelines).
    uniform_buffer: UniformBuffer,
    atlas_bind_group: AtlasBindGroup,
    subpixel_atlas_bind_group: AtlasBindGroup,
    color_atlas_bind_group: AtlasBindGroup,

    // Atlases + fonts (per-window, own DPI/rasterization).
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
    ui_raster_keys: Vec<RasterKey>,
    /// Reusable clip stack for `convert_draw_list` (avoids per-frame allocation).
    clip_stack: Vec<oriterm_ui::geometry::Rect>,
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

    // Image rendering.
    image_texture_cache: ImageTextureCache,
    image_instance_buffer: Option<Buffer>,
    /// Reusable scratch buffer for image quad instance data (avoids per-frame allocation).
    image_instance_data: Vec<u8>,
}

impl WindowRenderer {
    /// Create a per-window renderer using shared layouts from [`GpuPipelines`].
    pub fn new(
        gpu: &GpuState,
        pipelines: &GpuPipelines,
        mut font_collection: FontCollection,
        ui_font_collection: Option<FontCollection>,
    ) -> Self {
        let t0 = std::time::Instant::now();
        let device = &gpu.device;
        let queue = &gpu.queue;

        // Uniform buffer.
        let uniform_buffer = UniformBuffer::new(device, &pipelines.uniform_layout);

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

        // Color atlas (starts empty — emoji cached on first use).
        let color_atlas = GlyphAtlas::new(device, GlyphFormat::Color);

        // Bind groups.
        let atlas_bind_group = AtlasBindGroup::new(device, &pipelines.atlas_layout, atlas.view());
        let subpixel_atlas_bind_group =
            AtlasBindGroup::new(device, &pipelines.atlas_layout, subpixel_atlas.view());
        let color_atlas_bind_group =
            AtlasBindGroup::new(device, &pipelines.atlas_layout, color_atlas.view());

        log::info!("window renderer init: total={:?}", t0.elapsed(),);

        Self {
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
            ui_raster_keys: Vec::new(),
            clip_stack: Vec::new(),
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
            image_texture_cache: ImageTextureCache::new(device),
            image_instance_buffer: None,
            image_instance_data: Vec::new(),
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
    /// When `content_changed` is false the shaping phase is skipped entirely,
    /// reusing the previous frame's [`ShapedFrame`]. Decorations (cursor,
    /// selection, URL hover) only affect the prepare phase, so they work
    /// correctly with cached shaping data.
    ///
    /// Three phases:
    /// 1. **Shape** — segment rows into runs and shape via rustybuzz.
    /// 2. **Cache** — rasterize and upload any missing shaped glyphs.
    /// 3. **Prepare** — emit GPU instances from shaped glyph positions.
    #[expect(
        clippy::too_many_arguments,
        reason = "origin + cursor blink + content_changed are pipeline context"
    )]
    pub fn prepare(
        &mut self,
        input: &FrameInput,
        gpu: &GpuState,
        pipelines: &GpuPipelines,
        origin: (f32, f32),
        cursor_blink_visible: bool,
        content_changed: bool,
    ) {
        self.atlas.begin_frame();
        self.subpixel_atlas.begin_frame();
        self.color_atlas.begin_frame();

        // Phase A: Shape all rows, or reuse cached shaping when content
        // hasn't changed (mouse hover, cursor blink, selection changes
        // only affect the prepare phase).
        let cols = input.columns();
        let cached_valid = self.shaping.frame.rows() > 0 && self.shaping.frame.cols() == cols;
        if content_changed || !cached_valid {
            shape_frame(input, &self.font_collection, &mut self.shaping);
        }

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

        // Phase D: Ensure image textures uploaded.
        self.upload_image_textures(input, gpu, pipelines);

        log::trace!(
            "frame: cells={} bg_inst={} glyph_inst={} cursor_inst={} images={}",
            input.content.cells.len(),
            self.prepared.backgrounds.len(),
            self.prepared.glyphs.len(),
            self.prepared.cursors.len(),
            self.prepared.image_quads_below.len() + self.prepared.image_quads_above.len(),
        );
    }

    /// Upload image textures for the current frame.
    ///
    /// Ensures all images referenced by the prepared frame have GPU textures.
    /// Evicts textures that haven't been used recently.
    fn upload_image_textures(
        &mut self,
        input: &FrameInput,
        gpu: &GpuState,
        pipelines: &GpuPipelines,
    ) {
        self.image_texture_cache.begin_frame();

        // Upload textures for all visible images.
        for img_data in &input.content.image_data {
            self.image_texture_cache.ensure_uploaded(
                &gpu.device,
                &gpu.queue,
                &pipelines.image_texture_layout,
                img_data.id,
                &img_data.data,
                img_data.width,
                img_data.height,
            );
        }

        // Evict textures not used in the last 60 frames (~1 second at 60fps).
        self.image_texture_cache.evict_unused(60);
        self.image_texture_cache.evict_over_limit();
    }

    /// Update the GPU memory limit for image textures.
    ///
    /// Triggers immediate eviction if current usage exceeds the new limit.
    pub fn set_image_gpu_memory_limit(&mut self, limit: usize) {
        self.image_texture_cache.set_gpu_memory_limit(limit);
    }
}

#[cfg(test)]
mod tests;
