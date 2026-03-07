//! Unshaped (per-cell) prepare path — test-only.
//!
//! These functions use per-cell character lookups instead of shaped glyph
//! positions. Production rendering uses the shaped path in `mod.rs`.

use oriterm_core::CellFlags;

use super::super::frame_input::FrameInput;
use super::super::prepared_frame::PreparedFrame;
use super::emit::{build_cursor, draw_prompt_markers, draw_url_hover_underline};
use super::{AtlasLookup, decorations, resolve_cell_colors, resolve_cursor};
use crate::font::GlyphStyle;
use crate::gpu::instance_writer::ScreenRect;

/// Convert cell flags to the corresponding glyph style.
fn glyph_style(flags: CellFlags) -> GlyphStyle {
    GlyphStyle::from_cell_flags(flags)
}

/// Convert a [`FrameInput`] into a GPU-ready [`PreparedFrame`] using per-cell
/// character lookups (unshaped path).
///
/// Used by tests to verify prepare logic without shaping complexity. Production
/// rendering uses [`prepare_frame_shaped`](super::prepare_frame_shaped) instead.
pub fn prepare_frame(
    input: &FrameInput,
    atlas: &dyn AtlasLookup,
    origin: (f32, f32),
) -> PreparedFrame {
    let cols = input.columns();
    let rows = input.rows();
    let opacity = f64::from(input.palette.opacity);
    let mut frame = PreparedFrame::with_capacity(
        input.viewport,
        cols,
        rows,
        input.palette.background,
        opacity,
    );
    fill_frame(input, atlas, &mut frame, origin, true);
    frame
}

/// Convert a [`FrameInput`] into a pre-existing [`PreparedFrame`], reusing
/// its buffer allocations (unshaped path).
///
/// Used by tests. Production rendering uses
/// [`prepare_frame_shaped_into`](super::prepare_frame_shaped_into) instead.
pub fn prepare_frame_into(
    input: &FrameInput,
    atlas: &dyn AtlasLookup,
    out: &mut PreparedFrame,
    origin: (f32, f32),
) {
    out.clear();
    out.viewport = input.viewport;
    out.set_clear_color(input.palette.background, f64::from(input.palette.opacity));
    fill_frame(input, atlas, out, origin, true);
}

/// Unshaped per-cell rendering: emit instances into `frame`.
///
/// Iterates every visible cell, emits background and glyph instances via
/// character lookup, then builds cursor instances. Used by tests; production
/// rendering uses the shaped path.
fn fill_frame(
    input: &FrameInput,
    atlas: &dyn AtlasLookup,
    frame: &mut PreparedFrame,
    origin: (f32, f32),
    cursor_blink_visible: bool,
) {
    let cw = input.cell_size.width;
    let ch = input.cell_size.height;
    let baseline = input.cell_size.baseline;
    let fg_dim = input.fg_dim;
    let (ox, oy) = origin;
    let sel = input.selection.as_ref();
    let search = input.search.as_ref();
    let cursor = resolve_cursor(&input.content.cursor, input.mark_cursor.as_ref());

    for cell in &input.content.cells {
        // Spacer cells are handled by their primary cell (or are padding).
        if cell
            .flags
            .intersects(CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }

        let col = cell.column.0;
        let x = ox + col as f32 * cw;
        let y = oy + cell.line as f32 * ch;

        let (fg, bg) = resolve_cell_colors(
            cell,
            sel,
            search,
            &cursor,
            cursor_blink_visible,
            &input.palette,
        );

        // Background: wide chars span 2 cell widths.
        let bg_w = if cell.flags.contains(CellFlags::WIDE_CHAR) {
            2.0 * cw
        } else {
            cw
        };
        frame.backgrounds.push_rect(
            ScreenRect {
                x,
                y,
                w: bg_w,
                h: ch,
            },
            bg,
            1.0,
        );

        let is_hovered = input.hovered_cell == Some((cell.line, col));
        decorations::DecorationContext {
            backgrounds: &mut frame.backgrounds,
            glyphs: &mut frame.glyphs,
            atlas,
            size_q6: 0,
            metrics: &input.cell_size,
        }
        .draw(
            cell.flags,
            cell.underline_color,
            fg,
            x,
            y,
            bg_w,
            cell.has_hyperlink,
            is_hovered,
        );

        // Foreground glyph (skip spaces).
        if cell.ch != ' ' {
            let style = glyph_style(cell.flags);
            if let Some(entry) = atlas.lookup(cell.ch, style) {
                let glyph_x = x + entry.bearing_x as f32;
                let glyph_y = y + baseline - entry.bearing_y as f32;
                let uv = [entry.uv_x, entry.uv_y, entry.uv_w, entry.uv_h];
                let rect = ScreenRect {
                    x: glyph_x,
                    y: glyph_y,
                    w: entry.width as f32,
                    h: entry.height as f32,
                };
                frame.glyphs.push_glyph(rect, uv, fg, fg_dim, entry.page);
            }
        }
    }

    // Implicit URL hover: one continuous underline rect per segment.
    draw_url_hover_underline(input, frame, ox, oy);

    // Visual prompt markers: thin colored bar at left margin of prompt rows.
    draw_prompt_markers(input, frame, ox, oy);

    // Cursor instances (gated by terminal visibility AND application blink state).
    if cursor.visible && cursor_blink_visible {
        build_cursor(
            frame,
            cursor.shape,
            cursor.column.0,
            cursor.line,
            cw,
            ch,
            ox,
            oy,
            input.palette.cursor_color,
        );
    }

    // Emit image quads from RenderableContent, split by z-index.
    super::emit::emit_image_quads(input, frame, ox, oy);
}
