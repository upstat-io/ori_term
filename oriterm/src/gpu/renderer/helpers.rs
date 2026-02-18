//! Free functions for the shaping pipeline and GPU buffer management.
//!
//! These are free functions (not methods) so the borrow checker can see
//! that different fields of [`GpuRenderer`](super::GpuRenderer) are
//! borrowed independently — e.g. `font_collection` immutably while
//! `scratch` is borrowed mutably.

use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device};

use super::super::atlas::GlyphAtlas;
use super::super::frame_input::FrameInput;
use super::super::prepare::ShapedFrame;
use crate::font::collection::size_key;
use crate::font::shaper::{build_col_glyph_map, prepare_line, shape_prepared_runs};
use crate::font::{FontCollection, GlyphFormat, RasterKey};

/// Reusable per-frame scratch buffers for the shaping pipeline.
///
/// Stored on [`GpuRenderer`](super::GpuRenderer) and cleared each frame to
/// avoid per-frame allocation of the shaping intermediaries and output.
pub(super) struct ShapingScratch {
    /// Shaped frame output (glyph positions + col maps).
    pub(super) frame: ShapedFrame,
    /// Shaping run segments for the current row.
    runs: Vec<crate::font::shaper::ShapingRun>,
    /// Shaped glyphs for the current row.
    glyphs: Vec<crate::font::shaper::ShapedGlyph>,
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
    queue: &wgpu::Queue,
) {
    let size_q6 = shaped.size_q6();
    for glyph in shaped.all_glyphs() {
        let key = RasterKey {
            glyph_id: glyph.glyph_id,
            face_idx: glyph.face_idx,
            size_q6,
            synthetic: glyph.synthetic,
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

/// Ensure a GPU buffer exists and is large enough for `data`.
///
/// Returns `Some(&Buffer)` if data is non-empty (caller should write to it),
/// or `None` if data is empty (no upload needed).
pub(super) fn ensure_buffer<'a>(
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
