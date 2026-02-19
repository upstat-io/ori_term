//! Converts a [`DrawList`] into GPU instance buffer records.
//!
//! This module bridges `oriterm_ui`'s GPU-agnostic [`DrawCommand`]s and
//! oriterm's [`InstanceWriter`]. Each command becomes one or more instance
//! buffer records:
//! - Rect/line → [`push_ui_rect`](super::instance_writer::InstanceWriter::push_ui_rect)
//! - Text → [`push_glyph`](super::instance_writer::InstanceWriter::push_glyph) per shaped glyph
//!
//! Image and clip commands are deferred (logged as no-ops).

use oriterm_ui::color::Color;
use oriterm_ui::draw::{DrawCommand, DrawList, RectStyle};
use oriterm_ui::geometry::{Point, Rect};
use oriterm_ui::text::ShapedText;

use super::atlas::{AtlasEntry, AtlasKind};
use super::instance_writer::{InstanceWriter, ScreenRect};
use super::prepare::AtlasLookup;
use crate::font::{FaceIdx, FontRealm, RasterKey, SyntheticFlags, subpx_bin, subpx_offset};

/// Context for converting [`DrawCommand::Text`] into glyph instances.
///
/// Bundles atlas lookup, output writers, and font metrics needed for text
/// rendering. Pass to [`convert_draw_list`] to enable text command conversion.
/// When `None` is passed instead, text commands are logged as deferred.
#[allow(dead_code, reason = "wired when widgets produce DrawLists with text")]
pub struct TextContext<'a> {
    /// Glyph atlas lookup (shared with the terminal prepare phase).
    pub atlas: &'a dyn AtlasLookup,
    /// Output writer for monochrome atlas glyphs.
    pub mono_writer: &'a mut InstanceWriter,
    /// Output writer for subpixel atlas glyphs.
    pub subpixel_writer: &'a mut InstanceWriter,
    /// Output writer for color atlas glyphs (emoji, bitmap).
    pub color_writer: &'a mut InstanceWriter,
    /// Font size in 26.6 fixed-point for [`RasterKey`] construction.
    pub size_q6: u32,
    /// Whether hinting is enabled for [`RasterKey`] construction.
    pub hinted: bool,
}

/// Convert all commands in a [`DrawList`] to GPU instance buffer records.
///
/// Rect and line commands go to `ui_writer`. Text commands go to the writers
/// in `text_ctx` (routed by atlas kind). Pass `None` for `text_ctx` to defer
/// text rendering.
///
/// Shadow commands emit an expanded shadow rect before the main rect.
/// Line commands are converted to thin rectangles.
/// Image and clip commands are logged as no-ops.
#[allow(
    dead_code,
    reason = "public API for Section 07 — not yet wired into render loop"
)]
pub fn convert_draw_list(
    draw_list: &DrawList,
    ui_writer: &mut InstanceWriter,
    text_ctx: Option<&mut TextContext<'_>>,
) {
    // Reborrow text_ctx so we can use it across loop iterations.
    let mut text_ctx = text_ctx;

    for cmd in draw_list.commands() {
        match cmd {
            DrawCommand::Rect { rect, style } => convert_rect(*rect, style, ui_writer),
            DrawCommand::Line {
                from,
                to,
                width,
                color,
            } => {
                convert_line(*from, *to, *width, *color, ui_writer);
            }
            DrawCommand::Text {
                position,
                shaped,
                color,
            } => {
                if let Some(ctx) = text_ctx.as_deref_mut() {
                    convert_text(*position, shaped, *color, ctx);
                } else {
                    log::trace!("DrawCommand::Text deferred — no TextContext provided");
                }
            }
            DrawCommand::Image { .. } => {
                log::trace!("DrawCommand::Image deferred — not yet implemented");
            }
            DrawCommand::PushClip { .. } => {
                log::trace!("DrawCommand::PushClip deferred — not yet implemented");
            }
            DrawCommand::PopClip => {
                log::trace!("DrawCommand::PopClip deferred — not yet implemented");
            }
        }
    }
}

/// Convert a styled rect command to one or two UI rect instances.
fn convert_rect(rect: Rect, style: &RectStyle, writer: &mut InstanceWriter) {
    // Resolve fill color: prefer gradient first stop, then solid fill.
    let fill = style
        .gradient
        .as_ref()
        .and_then(|g| g.stops.first().map(|s| s.color))
        .or(style.fill)
        .unwrap_or(Color::TRANSPARENT);

    // Shadow instance (if present): expanded rect behind the main rect.
    if let Some(shadow) = &style.shadow {
        let expand = shadow.spread + shadow.blur_radius;
        let shadow_rect = ScreenRect {
            x: rect.x() + shadow.offset_x - expand,
            y: rect.y() + shadow.offset_y - expand,
            w: rect.width() + expand * 2.0,
            h: rect.height() + expand * 2.0,
        };
        writer.push_ui_rect(
            shadow_rect,
            shadow.color.to_array(),
            [0.0; 4],
            uniform_radius(&style.corner_radius) + expand,
            0.0,
        );
    }

    // Main rect instance.
    let screen = to_screen_rect(rect);
    let (border_color, border_width) = style
        .border
        .map_or(([0.0; 4], 0.0), |b| (b.color.to_array(), b.width));

    writer.push_ui_rect(
        screen,
        fill.to_array(),
        border_color,
        uniform_radius(&style.corner_radius),
        border_width,
    );
}

/// Convert a line segment to a thin axis-aligned rect instance.
fn convert_line(from: Point, to: Point, width: f32, color: Color, writer: &mut InstanceWriter) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = dx.hypot(dy);
    if len < f32::EPSILON {
        return;
    }

    // Perpendicular offset for line thickness.
    let nx = -dy / len * width * 0.5;
    let ny = dx / len * width * 0.5;

    // Axis-aligned bounding box of the thick line.
    let corners_x = [from.x + nx, from.x - nx, to.x + nx, to.x - nx];
    let corners_y = [from.y + ny, from.y - ny, to.y + ny, to.y - ny];

    let min_x = corners_x[0]
        .min(corners_x[1])
        .min(corners_x[2])
        .min(corners_x[3]);
    let min_y = corners_y[0]
        .min(corners_y[1])
        .min(corners_y[2])
        .min(corners_y[3]);
    let max_x = corners_x[0]
        .max(corners_x[1])
        .max(corners_x[2])
        .max(corners_x[3]);
    let max_y = corners_y[0]
        .max(corners_y[1])
        .max(corners_y[2])
        .max(corners_y[3]);

    let screen = ScreenRect {
        x: min_x,
        y: min_y,
        w: max_x - min_x,
        h: max_y - min_y,
    };

    writer.push_ui_rect(screen, color.to_array(), [0.0; 4], 0.0, 0.0);
}

/// Convert a geometry [`Rect`] to a [`ScreenRect`] for the instance writer.
fn to_screen_rect(rect: Rect) -> ScreenRect {
    ScreenRect {
        x: rect.x(),
        y: rect.y(),
        w: rect.width(),
        h: rect.height(),
    }
}

/// Convert a text draw command into glyph instances.
///
/// Iterates shaped glyphs, looks up each in the atlas by [`RasterKey`],
/// and emits positioned glyph instances. Glyphs not found in the atlas are
/// silently skipped (they should have been pre-cached by the caller).
///
/// Position computation follows the same pattern as the terminal
/// [`GlyphEmitter`](super::prepare::GlyphEmitter): bearing offsets place the
/// glyph bitmap relative to the text origin, and subpixel phase is absorbed.
fn convert_text(position: Point, shaped: &ShapedText, color: Color, ctx: &mut TextContext<'_>) {
    let fg = color_to_rgb(color);
    let alpha = color.a;
    let baseline = shaped.baseline;

    let mut cursor_x = position.x;

    for glyph in &shaped.glyphs {
        let advance = glyph.x_advance;

        // Skip advance-only glyphs (spaces: glyph_id=0).
        if glyph.glyph_id == 0 {
            cursor_x += advance;
            continue;
        }

        let subpx = subpx_bin(cursor_x + glyph.x_offset);
        let key = RasterKey {
            glyph_id: glyph.glyph_id,
            face_idx: FaceIdx(glyph.face_index),
            size_q6: ctx.size_q6,
            synthetic: SyntheticFlags::NONE,
            hinted: ctx.hinted,
            subpx_x: subpx,
            font_realm: FontRealm::Ui,
        };

        if let Some(entry) = ctx.atlas.lookup_key(key) {
            emit_text_glyph(
                cursor_x, position.y, baseline, glyph, entry, fg, alpha, subpx, ctx,
            );
        }

        cursor_x += advance;
    }
}

/// Emit a single text glyph instance, routing by atlas kind.
#[expect(
    clippy::too_many_arguments,
    reason = "text glyph instance: position components, glyph data, atlas entry, color"
)]
fn emit_text_glyph(
    cursor_x: f32,
    text_y: f32,
    baseline: f32,
    glyph: &oriterm_ui::text::ShapedGlyph,
    entry: &AtlasEntry,
    fg: oriterm_core::Rgb,
    alpha: f32,
    subpx: u8,
    ctx: &mut TextContext<'_>,
) {
    let absorbed = subpx_offset(subpx);
    let gx = cursor_x + glyph.x_offset - absorbed + entry.bearing_x as f32;
    let gy = text_y + baseline - entry.bearing_y as f32 - glyph.y_offset;
    let uv = [entry.uv_x, entry.uv_y, entry.uv_w, entry.uv_h];
    let rect = ScreenRect {
        x: gx,
        y: gy,
        w: entry.width as f32,
        h: entry.height as f32,
    };

    let writer = match entry.kind {
        AtlasKind::Mono => &mut ctx.mono_writer,
        AtlasKind::Subpixel => &mut ctx.subpixel_writer,
        AtlasKind::Color => &mut ctx.color_writer,
    };
    writer.push_glyph(rect, uv, fg, alpha, entry.page);
}

/// Convert an [`oriterm_ui::color::Color`] (f32 RGBA) to [`oriterm_core::Rgb`] (u8 RGB).
fn color_to_rgb(c: Color) -> oriterm_core::Rgb {
    oriterm_core::Rgb {
        r: (c.r * 255.0).round() as u8,
        g: (c.g * 255.0).round() as u8,
        b: (c.b * 255.0).round() as u8,
    }
}

/// Pick a uniform radius from the per-corner array.
///
/// The SDF shader currently supports a single radius value. When per-corner
/// radii differ, use the maximum (visually reasonable until a 4-corner SDF
/// is implemented).
fn uniform_radius(radii: &[f32; 4]) -> f32 {
    radii[0].max(radii[1]).max(radii[2]).max(radii[3])
}

#[cfg(test)]
mod tests;
