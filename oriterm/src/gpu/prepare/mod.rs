//! Prepare phase: convert a [`FrameInput`] into GPU-ready instance buffers.
//!
//! [`prepare_frame`] is a pure CPU function — no wgpu types, no device, no
//! queue. Given a terminal snapshot and an atlas lookup, it produces a
//! [`PreparedFrame`] containing three [`InstanceWriter`] buffers (backgrounds,
//! glyphs, cursors) ready for GPU upload.
//!
//! The [`AtlasLookup`] trait abstracts glyph lookup for testability: production
//! wraps `FontCollection::resolve` + `GlyphAtlas::lookup`; tests use a simple
//! `HashMap`.

mod decorations;
pub(crate) mod shaped_frame;

use oriterm_core::{CellFlags, Column, CursorShape, RenderableCell, RenderableCursor, Rgb};

use super::atlas::{AtlasEntry, AtlasKind};
use oriterm_core::search::MatchType;

use super::frame_input::{
    FrameInput, FramePalette, FrameSearch, FrameSelection, MarkCursorOverride,
};
use super::prepared_frame::PreparedFrame;
use crate::font::{FontRealm, GlyphStyle, RasterKey, ShapedGlyph, subpx_bin, subpx_offset};
use crate::gpu::instance_writer::ScreenRect;

pub(crate) use shaped_frame::ShapedFrame;

/// Match highlight background: yellow-tinted for visibility.
const SEARCH_MATCH_BG: Rgb = Rgb {
    r: 100,
    g: 100,
    b: 30,
};

/// Focused match highlight: brighter yellow.
const SEARCH_FOCUSED_BG: Rgb = Rgb {
    r: 200,
    g: 170,
    b: 40,
};

/// Focused match foreground: dark for contrast.
const SEARCH_FOCUSED_FG: Rgb = Rgb { r: 0, g: 0, b: 0 };

/// Abstracts glyph atlas lookup for testability.
///
/// Production: the shaped path uses [`lookup_key`](Self::lookup_key) for
/// direct `RasterKey` → `AtlasEntry` lookups. Tests may override `lookup`
/// for the per-cell unshaped path.
pub trait AtlasLookup {
    /// Look up a cached glyph entry by character and style.
    ///
    /// Used by the unshaped [`prepare_frame`] test path. Default returns
    /// `None` — production implementations only need [`lookup_key`](Self::lookup_key).
    #[allow(dead_code, reason = "used by test-only unshaped prepare_frame path")]
    fn lookup(&self, _ch: char, _style: GlyphStyle) -> Option<&AtlasEntry> {
        None
    }

    /// Look up a cached glyph entry by [`RasterKey`] (shaped path).
    fn lookup_key(&self, key: RasterKey) -> Option<&AtlasEntry>;
}

/// Resolve the effective cursor for rendering.
///
/// When mark mode is active (`mark_cursor` is `Some`), the override replaces
/// the terminal cursor's position and shape. Otherwise the extracted terminal
/// cursor is used as-is.
fn resolve_cursor(
    content_cursor: &RenderableCursor,
    mark: Option<&MarkCursorOverride>,
) -> RenderableCursor {
    match mark {
        Some(mc) => RenderableCursor {
            line: mc.line,
            column: mc.column,
            shape: mc.shape,
            visible: true,
        },
        None => *content_cursor,
    }
}

/// Convert cell flags to the corresponding glyph style.
#[cfg(test)]
fn glyph_style(flags: CellFlags) -> GlyphStyle {
    GlyphStyle::from_cell_flags(flags)
}

/// Resolve per-cell colors with selection highlighting applied.
///
/// Returns `(fg, bg)` for the cell, accounting for:
/// - **Selection inversion**: selected cells swap fg/bg.
/// - **Block cursor exclusion**: the cell under a visible block cursor is not
///   inverted — the cursor overlay handles its own visual.
/// - **INVERSE flag**: cells already inverted by SGR 7 would look identical
///   to unselected normal cells after a naive swap. Falls back to palette
///   defaults to ensure the selection is visible.
/// - **fg==bg reveal**: if inversion produces matching fg/bg (invisible text),
///   falls back to palette defaults — unless the cell has HIDDEN set (SGR 8
///   intentionally hides text, and selection should not reveal it).
#[expect(
    clippy::too_many_arguments,
    reason = "cell, selection, search, cursor, blink, palette are all distinct concerns"
)]
fn resolve_cell_colors(
    cell: &RenderableCell,
    sel: Option<&FrameSelection>,
    search: Option<&FrameSearch>,
    cursor: &RenderableCursor,
    cursor_blink_visible: bool,
    palette: &FramePalette,
) -> (Rgb, Rgb) {
    let col = cell.column.0;
    let row = cell.line;
    let is_wide = cell.flags.contains(CellFlags::WIDE_CHAR);

    // Block cursor cell: skip selection/search inversion so cursor overlay dominates.
    let is_block_cursor_cell = cursor_blink_visible
        && cursor.visible
        && cursor.shape == CursorShape::Block
        && cursor.line == row
        && cursor.column == Column(col);

    // Selection takes priority over search highlighting.
    let selected = !is_block_cursor_cell
        && sel.is_some_and(|s| s.contains(row, col) || (is_wide && s.contains(row, col + 1)));

    if selected {
        // When explicit selection colors are configured, use them directly.
        if let (Some(sfg), Some(sbg)) = (palette.selection_fg, palette.selection_bg) {
            return (sfg, sbg);
        }
        // Fallback: swap fg/bg with INVERSE and visibility guards.
        if cell.flags.contains(CellFlags::INVERSE) {
            return (palette.background, palette.foreground);
        }
        let (sel_fg, sel_bg) = (cell.bg, cell.fg);
        if sel_fg == sel_bg && !cell.flags.contains(CellFlags::HIDDEN) {
            return (palette.background, palette.foreground);
        }
        return (sel_fg, sel_bg);
    }

    // Search match highlighting (below selection in priority).
    if !is_block_cursor_cell {
        if let Some(search) = search {
            match search.cell_match_type(row, col) {
                MatchType::FocusedMatch => return (SEARCH_FOCUSED_FG, SEARCH_FOCUSED_BG),
                MatchType::Match => return (cell.fg, SEARCH_MATCH_BG),
                MatchType::None => {}
            }
        }
    }

    (cell.fg, cell.bg)
}

/// Convert a [`FrameInput`] into a GPU-ready [`PreparedFrame`] using per-cell
/// character lookups (unshaped path).
///
/// Used by tests to verify prepare logic without shaping complexity. Production
/// rendering uses [`prepare_frame_shaped`] instead.
#[cfg(test)]
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
/// Used by tests. Production rendering uses [`prepare_frame_shaped`] instead.
#[cfg(test)]
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

/// Convert a [`FrameInput`] into a GPU-ready [`PreparedFrame`] using shaped
/// glyph data.
///
/// Like [`prepare_frame`] but uses pre-shaped glyph positions from a
/// [`ShapedFrame`] instead of per-cell character lookups. This enables
/// ligatures, combining marks, and shaper-driven positioning.
///
/// Used by tests to get a fresh frame. Production uses
/// [`prepare_frame_shaped_into`] for buffer reuse.
#[cfg(test)]
pub fn prepare_frame_shaped(
    input: &FrameInput,
    atlas: &dyn AtlasLookup,
    shaped: &ShapedFrame,
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
    fill_frame_shaped(input, atlas, shaped, &mut frame, origin, true);
    frame
}

/// Convert a [`FrameInput`] into a pre-existing [`PreparedFrame`], reusing
/// its buffer allocations (shaped path).
///
/// Like [`prepare_frame_shaped`] but clears and refills `out` instead of
/// allocating a new frame. The `origin` offset shifts all cell positions
/// (from layout), and `cursor_blink_visible` gates cursor emission (from
/// application-level blink state).
#[expect(
    clippy::too_many_arguments,
    reason = "origin + cursor blink are pipeline context, not FrameInput concerns"
)]
pub fn prepare_frame_shaped_into(
    input: &FrameInput,
    atlas: &dyn AtlasLookup,
    shaped: &ShapedFrame,
    out: &mut PreparedFrame,
    origin: (f32, f32),
    cursor_blink_visible: bool,
) {
    out.clear();
    out.viewport = input.viewport;
    out.set_clear_color(input.palette.background, f64::from(input.palette.opacity));
    fill_frame_shaped(input, atlas, shaped, out, origin, cursor_blink_visible);
}

/// Unshaped per-cell rendering: emit instances into `frame`.
///
/// Iterates every visible cell, emits background and glyph instances via
/// character lookup, then builds cursor instances. Used by tests; production
/// rendering uses [`fill_frame_shaped`].
#[cfg(test)]
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
                frame.glyphs.push_glyph(rect, uv, fg, 1.0, entry.page);
            }
        }
    }

    // Implicit URL hover: one continuous underline rect per segment.
    draw_url_hover_underline(input, frame, ox, oy);

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
}

/// Shaped rendering: emit background, glyph, and cursor instances from shaped data.
///
/// Backgrounds and cursors use the same per-cell logic as [`fill_frame`].
/// Glyphs are driven by the [`ShapedFrame`] col-to-glyph map instead of
/// per-cell character lookups, enabling ligatures and combining marks.
#[expect(
    clippy::too_many_arguments,
    reason = "origin + cursor blink are pipeline context passed from renderer"
)]
#[expect(
    clippy::too_many_lines,
    reason = "linear pipeline: bg → decorations → builtins → shaped glyphs → cursors"
)]
fn fill_frame_shaped(
    input: &FrameInput,
    atlas: &dyn AtlasLookup,
    shaped: &ShapedFrame,
    frame: &mut PreparedFrame,
    origin: (f32, f32),
    cursor_blink_visible: bool,
) {
    let cw = input.cell_size.width;
    let ch = input.cell_size.height;
    let baseline = input.cell_size.baseline;
    let (ox, oy) = origin;
    let sel = input.selection.as_ref();
    let search = input.search.as_ref();
    let cursor = resolve_cursor(&input.content.cursor, input.mark_cursor.as_ref());

    for cell in &input.content.cells {
        if cell
            .flags
            .intersects(CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }

        let col = cell.column.0;
        let row = cell.line;
        let x = ox + col as f32 * cw;
        let y = oy + row as f32 * ch;

        let (fg, bg) = resolve_cell_colors(
            cell,
            sel,
            search,
            &cursor,
            cursor_blink_visible,
            &input.palette,
        );

        // Background (identical to unshaped path).
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

        let is_hovered = input.hovered_cell == Some((row, col));
        decorations::DecorationContext {
            backgrounds: &mut frame.backgrounds,
            glyphs: &mut frame.glyphs,
            atlas,
            size_q6: shaped.size_q6(),
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

        // Built-in geometric glyphs: bypass shaping, render from atlas.
        if crate::font::is_builtin(cell.ch) {
            let key = super::builtin_glyphs::raster_key(cell.ch, shaped.size_q6());
            if let Some(entry) = atlas.lookup_key(key) {
                let uv = [entry.uv_x, entry.uv_y, entry.uv_w, entry.uv_h];
                let rect = ScreenRect {
                    x,
                    y,
                    w: entry.width as f32,
                    h: entry.height as f32,
                };
                frame.glyphs.push_glyph(rect, uv, fg, 1.0, entry.page);
            }
            continue;
        }

        // Foreground: emit shaped glyphs via col-to-glyph map.
        // Guard: viewport cells may exceed shaped frame during async resize.
        if row >= shaped.rows() || col >= shaped.cols() {
            continue;
        }
        if let Some(start_idx) = shaped.col_map(row, col) {
            let row_glyphs = shaped.row_glyphs(row);
            GlyphEmitter {
                baseline,
                size_q6: shaped.size_q6(),
                hinted: shaped.hinted(),
                atlas,
                frame,
            }
            .emit(row_glyphs, start_idx, col, x, y, fg, bg);
        }
    }

    // Implicit URL hover: one continuous underline rect per segment.
    draw_url_hover_underline(input, frame, ox, oy);

    // Cursor (gated by terminal visibility AND application blink state).
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
}

/// Frame-level context for shaped glyph emission.
///
/// Bundles the atlas, size key, baseline, and output frame that are invariant
/// across cells. Per-cell parameters (row glyphs, column, position, color)
/// are passed to [`emit`](Self::emit).
struct GlyphEmitter<'a> {
    baseline: f32,
    size_q6: u32,
    hinted: bool,
    atlas: &'a dyn AtlasLookup,
    frame: &'a mut PreparedFrame,
}

impl GlyphEmitter<'_> {
    /// Emit glyph instances for a shaped cell: base glyph + any combining marks.
    ///
    /// Starts at `start_idx` in `row_glyphs` (the base glyph from the col map),
    /// then iterates forward while subsequent glyphs share the same `col_start`
    /// (combining marks are contiguous in the shaper output).
    ///
    /// Routing by [`AtlasKind`]:
    /// - `Mono` → `frame.glyphs` (R8 atlas, tinted by `fg_color`).
    /// - `Subpixel` → `frame.subpixel_glyphs` (RGBA atlas, per-channel blend).
    /// - `Color` → `frame.color_glyphs` (RGBA atlas, rendered as-is).
    #[expect(
        clippy::too_many_arguments,
        reason = "per-cell shaped glyph params: glyph source, grid column, screen position, color"
    )]
    fn emit(
        &mut self,
        row_glyphs: &[ShapedGlyph],
        start_idx: usize,
        col: usize,
        x: f32,
        y: f32,
        fg: Rgb,
        bg: Rgb,
    ) {
        let mut is_first = true;
        for sg in row_glyphs.get(start_idx..).unwrap_or_default() {
            // Stop at the first glyph in a different column (combining marks are contiguous).
            if !is_first && sg.col_start != col {
                break;
            }
            is_first = false;

            let subpx = subpx_bin(sg.x_offset);
            let key = RasterKey {
                glyph_id: sg.glyph_id,
                face_idx: sg.face_idx,
                size_q6: self.size_q6,
                synthetic: sg.synthetic,
                hinted: self.hinted,
                subpx_x: subpx,
                font_realm: FontRealm::Terminal,
            };
            if let Some(entry) = self.atlas.lookup_key(key) {
                // Apply shaper offsets: x_offset shifts horizontally,
                // y_offset shifts vertically (positive = up in font coords = subtract in screen).
                // Subtract the absorbed subpixel offset to avoid double-counting
                // (once in the rasterized bitmap, once in positioning).
                let absorbed = subpx_offset(subpx);
                let gx = x + entry.bearing_x as f32 + sg.x_offset - absorbed;
                let gy = y + self.baseline - entry.bearing_y as f32 - sg.y_offset;
                let uv = [entry.uv_x, entry.uv_y, entry.uv_w, entry.uv_h];
                let rect = ScreenRect {
                    x: gx,
                    y: gy,
                    w: entry.width as f32,
                    h: entry.height as f32,
                };
                let writer = match entry.kind {
                    AtlasKind::Color => &mut self.frame.color_glyphs,
                    AtlasKind::Subpixel => &mut self.frame.subpixel_glyphs,
                    AtlasKind::Mono => &mut self.frame.glyphs,
                };
                writer.push_glyph_with_bg(rect, uv, fg, bg, 1.0, entry.page);
            }
        }
    }
}

/// Emit cursor instances into the prepared frame.
///
/// The cursor shape determines the geometry:
/// - `Block` — full cell rectangle.
/// - `Bar` — 2px-wide vertical line at the left edge.
/// - `Underline` — 2px-tall horizontal line at the bottom.
/// - `HollowBlock` — 4 thin outline rectangles (top, bottom, left, right).
/// - `Hidden` — no instances.
#[expect(
    clippy::too_many_arguments,
    reason = "cursor geometry: frame, shape, grid position, cell size, origin offset, color"
)]
fn build_cursor(
    frame: &mut PreparedFrame,
    shape: CursorShape,
    col: usize,
    row: usize,
    cw: f32,
    ch: f32,
    ox: f32,
    oy: f32,
    color: Rgb,
) {
    let x = ox + col as f32 * cw;
    let y = oy + row as f32 * ch;
    let t = 2.0_f32;

    match shape {
        CursorShape::Block => {
            frame
                .cursors
                .push_cursor(ScreenRect { x, y, w: cw, h: ch }, color, 1.0);
        }
        CursorShape::Bar => {
            frame
                .cursors
                .push_cursor(ScreenRect { x, y, w: t, h: ch }, color, 1.0);
        }
        CursorShape::Underline => {
            let rect = ScreenRect {
                x,
                y: y + ch - t,
                w: cw,
                h: t,
            };
            frame.cursors.push_cursor(rect, color, 1.0);
        }
        CursorShape::HollowBlock => {
            // Top edge.
            frame
                .cursors
                .push_cursor(ScreenRect { x, y, w: cw, h: t }, color, 1.0);
            // Bottom edge.
            let rect = ScreenRect {
                x,
                y: y + ch - t,
                w: cw,
                h: t,
            };
            frame.cursors.push_cursor(rect, color, 1.0);
            // Left edge.
            frame
                .cursors
                .push_cursor(ScreenRect { x, y, w: t, h: ch }, color, 1.0);
            // Right edge.
            let rect = ScreenRect {
                x: x + cw - t,
                y,
                w: t,
                h: ch,
            };
            frame.cursors.push_cursor(rect, color, 1.0);
        }
        CursorShape::Hidden => {}
    }
}

/// Draw implicit URL hover underlines as continuous rects per segment.
///
/// Renders into the cursor layer (on top of glyphs) so the underline is
/// not obscured by character pixels that extend into the underline zone
/// (e.g. `/` descenders in `https://`).
fn draw_url_hover_underline(input: &FrameInput, frame: &mut PreparedFrame, ox: f32, oy: f32) {
    if input.hovered_url_segments.is_empty() {
        return;
    }
    let cw = input.cell_size.width;
    let ch = input.cell_size.height;
    let underline_y_offset = input.cell_size.baseline + input.cell_size.underline_offset;
    let t = input.cell_size.stroke_size;
    let fg = input.palette.foreground;

    for &(line, start_col, end_col) in &input.hovered_url_segments {
        let x = ox + start_col as f32 * cw;
        let y = oy + line as f32 * ch + underline_y_offset;
        let w = (end_col - start_col + 1) as f32 * cw;
        frame
            .cursors
            .push_cursor(ScreenRect { x, y, w, h: t }, fg, 1.0);
    }
}

#[cfg(test)]
mod tests;
