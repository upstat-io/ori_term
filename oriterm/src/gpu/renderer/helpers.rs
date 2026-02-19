//! Free functions for the shaping pipeline and GPU buffer management.
//!
//! These are free functions (not methods) so the borrow checker can see
//! that different fields of [`GpuRenderer`](super::GpuRenderer) are
//! borrowed independently — e.g. `font_collection` immutably while
//! `scratch` is borrowed mutably.

use wgpu::{
    BindGroup, Buffer, BufferDescriptor, BufferUsages, Device, Queue, RenderPass, RenderPipeline,
};

use super::super::atlas::GlyphAtlas;
use super::super::frame_input::FrameInput;
use super::super::prepare::ShapedFrame;
use crate::font::{
    FontCollection, GlyphFormat, RasterKey, build_col_glyph_map, prepare_line, shape_prepared_runs,
    size_key,
};

/// Reusable per-frame scratch buffers for the shaping pipeline.
///
/// Stored on [`GpuRenderer`](super::GpuRenderer) and cleared each frame to
/// avoid per-frame allocation of the shaping intermediaries and output.
pub(super) struct ShapingScratch {
    /// Shaped frame output (glyph positions + col maps).
    pub(super) frame: ShapedFrame,
    /// Shaping run segments for the current row.
    runs: Vec<crate::font::ShapingRun>,
    /// Shaped glyphs for the current row.
    glyphs: Vec<crate::font::ShapedGlyph>,
    /// Column-to-glyph map for the current row.
    col_map: Vec<Option<usize>>,
    /// Rustybuzz buffer reused across frames to avoid per-frame allocation.
    unicode_buffer: Option<rustybuzz::UnicodeBuffer>,
}

impl ShapingScratch {
    pub(super) fn new() -> Self {
        Self {
            frame: ShapedFrame::new(0, 0),
            runs: Vec::new(),
            glyphs: Vec::new(),
            col_map: Vec::new(),
            unicode_buffer: None,
        }
    }
}

/// Shape all visible rows into the scratch `ShapedFrame`.
pub(super) fn shape_frame(
    input: &FrameInput,
    fonts: &FontCollection,
    scratch: &mut ShapingScratch,
) {
    let cols = input.columns();
    let size_q6 = size_key(fonts.size_px());
    let hinted = fonts.hinting_mode().hint_flag();
    scratch.frame.clear(cols, size_q6, hinted);
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
        shape_prepared_runs(
            &scratch.runs,
            &faces,
            fonts,
            &mut scratch.glyphs,
            &mut scratch.unicode_buffer,
        );
        build_col_glyph_map(&scratch.glyphs, cols, &mut scratch.col_map);
        scratch.frame.push_row(&scratch.glyphs, &scratch.col_map);
    }
}

/// Ensure all shaped glyphs are cached in the appropriate atlas.
///
/// Routes color glyphs ([`GlyphFormat::Color`]) to `color_atlas` and all
/// others to `mono_atlas`.
pub(super) fn ensure_shaped_glyphs_cached(
    shaped: &ShapedFrame,
    mono_atlas: &mut GlyphAtlas,
    color_atlas: &mut GlyphAtlas,
    fonts: &mut FontCollection,
    queue: &Queue,
) {
    let size_q6 = shaped.size_q6();
    let hinted = fonts.hinting_mode().hint_flag();
    for glyph in shaped.all_glyphs() {
        let key = RasterKey {
            glyph_id: glyph.glyph_id,
            face_idx: glyph.face_idx,
            size_q6,
            synthetic: glyph.synthetic,
            hinted,
        };
        // Check both atlases for cache hit.
        if mono_atlas.lookup_touch(key).is_some() || color_atlas.lookup_touch(key).is_some() {
            continue;
        }
        if mono_atlas.is_known_empty(key) {
            continue;
        }
        if let Some(rasterized) = fonts.rasterize(key) {
            if rasterized.format == GlyphFormat::Color {
                color_atlas.insert(key, rasterized, queue);
            } else {
                mono_atlas.insert(key, rasterized, queue);
            }
        } else {
            mono_atlas.mark_empty(key);
        }
    }
}

/// Ensure a GPU buffer exists, is large enough, and upload `data` to it.
///
/// No-ops when `data` is empty. Grows the buffer (power-of-2 amortized) when
/// needed, then writes the data in a single `write_buffer` call.
pub(super) fn upload_buffer(
    device: &Device,
    queue: &Queue,
    slot: &mut Option<Buffer>,
    data: &[u8],
    label: &'static str,
) {
    if data.is_empty() {
        return;
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

    if let Some(buf) = slot.as_ref() {
        queue.write_buffer(buf, 0, data);
    }
}

/// Record a single instanced draw call into the render pass.
///
/// Sets the pipeline, bind groups, and vertex buffer, then issues an
/// instanced `draw(0..4, 0..instance_count)`. No-ops when `instance_count`
/// is zero or the buffer slot is empty.
#[expect(
    clippy::too_many_arguments,
    reason = "GPU render pass recording: pipeline, bind groups, buffer, count"
)]
pub(super) fn record_draw(
    pass: &mut RenderPass<'_>,
    pipeline: &RenderPipeline,
    uniform_bg: &BindGroup,
    atlas_bg: Option<&BindGroup>,
    buffer: Option<&Buffer>,
    instance_count: u32,
) {
    if instance_count == 0 {
        return;
    }
    let Some(buf) = buffer else { return };
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, uniform_bg, &[]);
    if let Some(atlas) = atlas_bg {
        pass.set_bind_group(1, atlas, &[]);
    }
    pass.set_vertex_buffer(0, buf.slice(..));
    pass.draw(0..4, 0..instance_count);
}
