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
use super::srgb_f32_to_linear;
use crate::font::{FaceIdx, FontRealm, RasterKey, SyntheticFlags, subpx_bin, subpx_offset};

/// Context for converting [`DrawCommand::Text`] into glyph instances.
///
/// Bundles atlas lookup, output writers, and font metrics needed for text
/// rendering. Pass to [`convert_draw_list`] to enable text command conversion.
/// When `None` is passed instead, text commands are logged as deferred.
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
/// The `scale` factor converts logical-pixel draw commands to physical-pixel
/// GPU coordinates. Pass `1.0` when draw list coordinates are already in
/// physical pixels (or at 1:1 scale).
///
/// The `opacity` parameter (0.0–1.0) multiplies all output color alphas.
/// Used by the compositor to fade overlays in and out. Pass `1.0` for no
/// opacity modification.
///
/// Shadow commands emit an expanded shadow rect before the main rect.
/// Line commands are converted to thin rectangles.
/// Image and clip commands are logged as no-ops.
pub fn convert_draw_list(
    draw_list: &DrawList,
    ui_writer: &mut InstanceWriter,
    text_ctx: Option<&mut TextContext<'_>>,
    scale: f32,
    opacity: f32,
) {
    // Reborrow text_ctx so we can use it across loop iterations.
    let mut text_ctx = text_ctx;

    for cmd in draw_list.commands() {
        match cmd {
            DrawCommand::Rect { rect, style } => {
                convert_rect(*rect, style, ui_writer, scale, opacity);
            }
            DrawCommand::Line {
                from,
                to,
                width,
                color,
            } => {
                convert_line(*from, *to, *width, *color, ui_writer, scale, opacity);
            }
            DrawCommand::Text {
                position,
                shaped,
                color,
                bg_hint,
            } => {
                if let Some(ctx) = text_ctx.as_deref_mut() {
                    convert_text(*position, shaped, *color, *bg_hint, ctx, scale, opacity);
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
            // Layer commands are structural — bg is already baked into Text.bg_hint.
            DrawCommand::PushLayer { .. } | DrawCommand::PopLayer => {}
        }
    }
}

/// Convert a styled rect command to one or two UI rect instances.
fn convert_rect(
    rect: Rect,
    style: &RectStyle,
    writer: &mut InstanceWriter,
    scale: f32,
    opacity: f32,
) {
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
            shadow_rect.scaled(scale),
            color_to_linear_with_opacity(shadow.color, opacity),
            [0.0; 4],
            (uniform_radius(&style.corner_radius) + expand) * scale,
            0.0,
        );
    }

    // Main rect instance.
    let screen = to_screen_rect(rect).scaled(scale);
    let (border_color, border_width) = style.border.map_or(([0.0; 4], 0.0), |b| {
        (color_to_linear_with_opacity(b.color, opacity), b.width)
    });

    writer.push_ui_rect(
        screen,
        color_to_linear_with_opacity(fill, opacity),
        border_color,
        uniform_radius(&style.corner_radius) * scale,
        border_width * scale,
    );
}

/// Convert a line segment to GPU rect instances.
///
/// Axis-aligned lines (horizontal or vertical) produce a single thin rect.
/// Diagonal lines are decomposed into pixel-stepping rects along the major
/// axis — one `width × width` rect per step — to avoid the AABB problem
/// where a single bounding box fills a solid square for 45° lines.
#[expect(
    clippy::too_many_arguments,
    reason = "line conversion: endpoints, thickness, color, output, scale, opacity"
)]
fn convert_line(
    from: Point,
    to: Point,
    width: f32,
    color: Color,
    writer: &mut InstanceWriter,
    scale: f32,
    opacity: f32,
) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = dx.hypot(dy);
    if len < f32::EPSILON {
        return;
    }

    let fill = color_to_linear_with_opacity(color, opacity);
    let hw = width * 0.5;

    // Axis-aligned fast paths: single rect.
    if dx.abs() < f32::EPSILON {
        // Vertical line.
        let (min_y, max_y) = if from.y < to.y {
            (from.y, to.y)
        } else {
            (to.y, from.y)
        };
        let rect = ScreenRect {
            x: from.x - hw,
            y: min_y,
            w: width,
            h: max_y - min_y,
        }
        .scaled(scale);
        writer.push_ui_rect(rect, fill, [0.0; 4], 0.0, 0.0);
        return;
    }
    if dy.abs() < f32::EPSILON {
        // Horizontal line.
        let (min_x, max_x) = if from.x < to.x {
            (from.x, to.x)
        } else {
            (to.x, from.x)
        };
        let rect = ScreenRect {
            x: min_x,
            y: from.y - hw,
            w: max_x - min_x,
            h: width,
        }
        .scaled(scale);
        writer.push_ui_rect(rect, fill, [0.0; 4], 0.0, 0.0);
        return;
    }

    // Diagonal line: step along the major axis and emit one rect per step.
    let steps = dx.abs().max(dy.abs()).ceil() as usize;
    if steps == 0 {
        return;
    }
    let sx = dx / steps as f32;
    let sy = dy / steps as f32;

    for i in 0..=steps {
        let x = from.x + sx * i as f32;
        let y = from.y + sy * i as f32;
        let rect = ScreenRect {
            x: x - hw,
            y: y - hw,
            w: width,
            h: width,
        }
        .scaled(scale);
        writer.push_ui_rect(rect, fill, [0.0; 4], 0.0, 0.0);
    }
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
/// Convert a text draw command into glyph instances.
///
/// The text position is in logical pixels (from widget layout). Glyph
/// advances, offsets, bearings, and bitmap dimensions are in physical pixels
/// (from the font collection loaded at physical DPI). We scale the position
/// to physical at the start, then work entirely in physical pixel space —
/// no scaling of glyph bitmap dimensions, which would cause blurriness.
#[expect(
    clippy::too_many_arguments,
    reason = "text conversion: position, shaped, color, bg_hint, text context, scale, opacity"
)]
fn convert_text(
    position: Point,
    shaped: &ShapedText,
    color: Color,
    bg_hint: Option<Color>,
    ctx: &mut TextContext<'_>,
    scale: f32,
    opacity: f32,
) {
    let fg = color_to_rgb(color);
    let subpixel_bg = bg_hint.map(color_to_rgb);
    let alpha = color.a * opacity;
    let baseline = shaped.baseline;

    // Convert logical position to physical. All subsequent values
    // (advances, offsets, bearings, bitmap dims) are already physical.
    //
    // Round base_y to an integer pixel boundary. baseline and bearing_y
    // are already integers, so rounding base_y ensures every glyph's
    // screen rect has integer Y coordinates. Without this, fractional
    // positions (e.g. from centering a dialog in the viewport) cause the
    // bilinear atlas sampler to interpolate the bottom row of glyph pixels
    // with transparent atlas padding, producing a "cut off at bottom" artifact.
    let mut cursor_x = position.x * scale;
    let base_y = (position.y * scale).round();

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
                cursor_x,
                base_y,
                baseline,
                glyph,
                entry,
                fg,
                subpixel_bg,
                alpha,
                subpx,
                ctx,
            );
        }

        cursor_x += advance;
    }
}

/// Emit a single text glyph instance, routing by atlas kind.
///
/// All coordinates are in physical pixels — no scale factor needed. The
/// glyph bitmap dimensions come directly from the atlas entry (rasterized
/// at the font's physical pixel size).
#[expect(
    clippy::too_many_arguments,
    reason = "text glyph instance: position components, glyph data, atlas entry, color, bg"
)]
fn emit_text_glyph(
    cursor_x: f32,
    base_y: f32,
    baseline: f32,
    glyph: &oriterm_ui::text::ShapedGlyph,
    entry: &AtlasEntry,
    fg: oriterm_core::Rgb,
    subpixel_bg: Option<oriterm_core::Rgb>,
    alpha: f32,
    subpx: u8,
    ctx: &mut TextContext<'_>,
) {
    let absorbed = subpx_offset(subpx);
    let gx = cursor_x + glyph.x_offset - absorbed + entry.bearing_x as f32;
    let gy = base_y + baseline - entry.bearing_y as f32 - glyph.y_offset;
    let uv = [entry.uv_x, entry.uv_y, entry.uv_w, entry.uv_h];
    // All values are physical pixels — no scaling needed.
    let rect = ScreenRect {
        x: gx,
        y: gy,
        w: entry.width as f32,
        h: entry.height as f32,
    };

    match entry.kind {
        AtlasKind::Subpixel => {
            if let Some(bg) = subpixel_bg {
                // Known background — per-channel compositing in the shader.
                ctx.subpixel_writer
                    .push_glyph_with_bg(rect, uv, fg, bg, alpha, entry.page);
            } else {
                // No background hint — fall back to alpha blending.
                ctx.subpixel_writer
                    .push_glyph(rect, uv, fg, alpha, entry.page);
            }
        }
        AtlasKind::Mono => ctx.mono_writer.push_glyph(rect, uv, fg, alpha, entry.page),
        AtlasKind::Color => ctx.color_writer.push_glyph(rect, uv, fg, alpha, entry.page),
    }
}

/// Convert an [`oriterm_ui::color::Color`] (f32 RGBA) to [`oriterm_core::Rgb`] (u8 RGB).
fn color_to_rgb(c: Color) -> oriterm_core::Rgb {
    oriterm_core::Rgb {
        r: (c.r * 255.0).round() as u8,
        g: (c.g * 255.0).round() as u8,
        b: (c.b * 255.0).round() as u8,
    }
}

/// Convert an sRGB [`Color`] to a linear-light `[f32; 4]` for the GPU,
/// multiplying alpha by the compositor `opacity`.
///
/// The `*Srgb` render target applies hardware sRGB encoding on output, so
/// all colors passed to shaders must be in linear space. UI `Color` values
/// are stored as sRGB; this decodes each RGB channel and applies the
/// compositor opacity to the alpha channel.
fn color_to_linear_with_opacity(c: Color, opacity: f32) -> [f32; 4] {
    [
        srgb_f32_to_linear(c.r),
        srgb_f32_to_linear(c.g),
        srgb_f32_to_linear(c.b),
        c.a * opacity,
    ]
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
