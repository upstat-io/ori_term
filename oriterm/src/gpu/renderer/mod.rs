//! GPU renderer: ties pipelines, atlas, fonts, and buffers into a single
//! draw-frame entry point.
//!
//! [`GpuRenderer`] owns all GPU resources needed to render a terminal frame.
//! The caller runs Extract → Prepare on the CPU, then hands the resulting
//! [`PreparedFrame`] to [`GpuRenderer::render_frame`] for GPU submission.

use std::fmt;

use wgpu::{
    BindGroupLayout, Buffer, BufferDescriptor, BufferUsages, Color, CommandEncoderDescriptor,
    Device, LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    StoreOp, TextureView, TextureViewDescriptor,
};

use oriterm_core::Rgb;

use super::atlas::GlyphAtlas;
use super::bind_groups::{AtlasBindGroup, UniformBuffer};
use super::frame_input::FrameInput;
use super::pipeline::{
    create_atlas_bind_group_layout, create_bg_pipeline, create_fg_pipeline,
    create_uniform_bind_group_layout,
};
use super::prepare::{self, AtlasLookup, ShapedFrame};
use super::prepared_frame::PreparedFrame;
use super::state::GpuState;
use crate::gpu::frame_input::ViewportSize;
use crate::font::collection::size_key;
use crate::font::shaper::{build_col_glyph_map, prepare_line, shape_prepared_runs};
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

/// Bridges [`GlyphAtlas`] into the [`AtlasLookup`] trait.
///
/// Used by the Prepare phase to look up cached glyphs by [`RasterKey`]
/// without exposing GPU types.
struct RendererAtlas<'a> {
    atlas: &'a GlyphAtlas,
}

impl AtlasLookup for RendererAtlas<'_> {
    fn lookup_key(&self, key: RasterKey) -> Option<&super::atlas::AtlasEntry> {
        self.atlas.lookup(key)
    }
}

// ── Shaping scratch ──

/// Reusable per-frame scratch buffers for the shaping pipeline.
///
/// Stored on [`GpuRenderer`] and cleared each frame to avoid per-frame
/// allocation of the shaping intermediaries and output.
struct ShapingScratch {
    /// Shaped frame output (glyph positions + col maps).
    frame: ShapedFrame,
    /// Shaping run segments for the current row.
    runs: Vec<crate::font::shaper::ShapingRun>,
    /// Shaped glyphs for the current row.
    glyphs: Vec<crate::font::shaper::ShapedGlyph>,
    /// Column-to-glyph map for the current row.
    col_map: Vec<Option<usize>>,
}

impl ShapingScratch {
    fn new() -> Self {
        Self {
            frame: ShapedFrame::new(0, 0),
            runs: Vec::new(),
            glyphs: Vec::new(),
            col_map: Vec::new(),
        }
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

    // Per-frame reusable scratch buffers.
    shaping: ShapingScratch,
    prepared: PreparedFrame,

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
        let size_q6 = size_key(font_collection.size_px());
        for ch in ' '..='~' {
            let resolved = font_collection.resolve(ch, GlyphStyle::Regular);
            let key = RasterKey::from_resolved(resolved, size_q6);
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
            shaping: ShapingScratch::new(),
            prepared: PreparedFrame::new(
                ViewportSize::new(1, 1),
                Rgb { r: 0, g: 0, b: 0 },
                1.0,
            ),
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

        // Phase A: Shape all rows. Free function for split-borrow:
        // borrows font_collection immutably, shaping scratch mutably.
        shape_frame(input, &self.font_collection, &mut self.shaping);

        // Phase B: Ensure shaped glyphs cached. Free function for split-borrow:
        // borrows shaping.frame immutably, atlas + font_collection mutably.
        ensure_shaped_glyphs_cached(
            &self.shaping.frame,
            &mut self.atlas,
            &mut self.font_collection,
            &gpu.queue,
        );

        // Phase B2: Ensure built-in geometric glyphs cached.
        super::builtin_glyphs::ensure_cached(input, self.shaping.frame.size_q6(), &mut self.atlas, &gpu.queue);

        // Phase C: Fill prepared frame via atlas lookup bridge (reuses allocations).
        let bridge = RendererAtlas { atlas: &self.atlas };
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

            // Draw 2: Glyphs (atlas-sampled text).
            if !self.prepared.glyphs.is_empty() {
                if let Some(buf) = &self.fg_buffer {
                    pass.set_pipeline(&self.fg_pipeline);
                    pass.set_bind_group(0, uniform_bg, &[]);
                    pass.set_bind_group(1, self.atlas_bind_group.bind_group(), &[]);
                    pass.set_vertex_buffer(0, buf.slice(..));
                    pass.draw(0..4, 0..self.prepared.glyphs.len() as u32);
                }
            }

            // Draw 3: Cursors (solid-color rects via bg pipeline).
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

        self.render_frame(gpu, &view);
        output.present();
        Ok(())
    }
}

// ── Free functions for split-borrow shaping pipeline ──

/// Shape all visible rows into the scratch `ShapedFrame`.
///
/// Free function (not a method) so the borrow checker can see that
/// `font_collection` is borrowed immutably while `scratch` is borrowed
/// mutably — both are distinct fields of `GpuRenderer`.
fn shape_frame(input: &FrameInput, fonts: &FontCollection, scratch: &mut ShapingScratch) {
    let cols = input.columns();
    let size_q6 = size_key(fonts.size_px());
    scratch.frame.clear(cols, size_q6);
    if cols == 0 {
        return;
    }
    // Clamp rows to actual cell data — viewport dimensions may race ahead
    // of the terminal grid during async resize.
    let rows = input.rows().min(input.content.cells.len() / cols);
    let faces = fonts.create_shaping_faces();

    for row_idx in 0..rows {
        let start = row_idx * cols;
        let end = start + cols;
        let row_cells = &input.content.cells[start..end];

        prepare_line(row_cells, cols, fonts, &mut scratch.runs);
        shape_prepared_runs(&scratch.runs, &faces, fonts, &mut scratch.glyphs);
        build_col_glyph_map(&scratch.glyphs, cols, &mut scratch.col_map);
        scratch.frame.push_row(&scratch.glyphs, &scratch.col_map);
    }
}

/// Ensure all shaped glyphs are cached in the atlas.
///
/// Free function for split-borrow: reads `shaped` immutably while mutating
/// `atlas` and `fonts` — both are distinct fields of `GpuRenderer`.
fn ensure_shaped_glyphs_cached(
    shaped: &ShapedFrame,
    atlas: &mut GlyphAtlas,
    fonts: &mut FontCollection,
    queue: &wgpu::Queue,
) {
    let size_q6 = shaped.size_q6();
    for glyph in shaped.all_glyphs() {
        let key = RasterKey {
            glyph_id: glyph.glyph_id,
            face_idx: glyph.face_idx,
            size_q6,
        };
        if atlas.lookup_touch(key).is_some() {
            continue;
        }
        if atlas.is_known_empty(key) {
            continue;
        }
        if let Some(rasterized) = fonts.rasterize(key) {
            atlas.insert(key, rasterized, queue);
        }
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
