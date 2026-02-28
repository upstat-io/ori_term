//! Unit tests for DrawList → InstanceWriter conversion.

use std::collections::HashMap;

use oriterm_ui::color::Color;
use oriterm_ui::draw::{DrawList, RectStyle, Shadow};
use oriterm_ui::geometry::Logical;
use oriterm_ui::text::{ShapedGlyph, ShapedText};

use crate::font::{FaceIdx, FontRealm, GlyphStyle, RasterKey, SyntheticFlags};
use crate::gpu::atlas::{AtlasEntry, AtlasKind};
use crate::gpu::instance_writer::{INSTANCE_SIZE, InstanceWriter};
use crate::gpu::prepare::AtlasLookup;

use super::{TextContext, convert_draw_list};

type Rect = oriterm_ui::geometry::Rect<Logical>;
type Point = oriterm_ui::geometry::Point<Logical>;

/// Read a little-endian `f32` from the given byte offset.
fn read_f32(buf: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
}

/// Read a little-endian `u32` from the given byte offset.
fn read_u32(buf: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
}

// --- Basic rect conversion ---

#[test]
fn empty_draw_list_produces_no_instances() {
    let dl = DrawList::new();
    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);
    assert!(writer.is_empty());
}

#[test]
fn filled_rect_produces_one_instance() {
    let mut dl = DrawList::new();
    dl.push_rect(
        Rect::new(10.0, 20.0, 100.0, 50.0),
        RectStyle::filled(Color::WHITE),
    );

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    assert_eq!(writer.len(), 1);

    let rec = writer.as_bytes();
    // Position.
    assert_eq!(read_f32(rec, 0), 10.0);
    assert_eq!(read_f32(rec, 4), 20.0);
    assert_eq!(read_f32(rec, 8), 100.0);
    assert_eq!(read_f32(rec, 12), 50.0);

    // Fill (bg_color) = WHITE.
    assert_eq!(read_f32(rec, 48), 1.0);
    assert_eq!(read_f32(rec, 52), 1.0);
    assert_eq!(read_f32(rec, 56), 1.0);
    assert_eq!(read_f32(rec, 60), 1.0);

    // Kind = UiRect (3).
    assert_eq!(read_u32(rec, 64), 3);

    // No corner radius or border.
    assert_eq!(read_f32(rec, 72), 0.0);
    assert_eq!(read_f32(rec, 76), 0.0);
}

#[test]
fn rect_with_border_writes_border_fields() {
    let mut dl = DrawList::new();
    let style = RectStyle::filled(Color::BLACK)
        .with_border(2.0, Color::WHITE)
        .with_radius(8.0);
    dl.push_rect(Rect::new(0.0, 0.0, 200.0, 100.0), style);

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    assert_eq!(writer.len(), 1);

    let rec = writer.as_bytes();
    // Border color (fg_color) = WHITE.
    assert_eq!(read_f32(rec, 32), 1.0);
    assert_eq!(read_f32(rec, 36), 1.0);
    assert_eq!(read_f32(rec, 40), 1.0);
    assert_eq!(read_f32(rec, 44), 1.0);

    // Corner radius and border width.
    assert_eq!(read_f32(rec, 72), 8.0);
    assert_eq!(read_f32(rec, 76), 2.0);
}

// --- Shadow ---

#[test]
fn rect_with_shadow_produces_two_instances() {
    let mut dl = DrawList::new();
    let style = RectStyle::filled(Color::WHITE).with_shadow(Shadow {
        offset_x: 0.0,
        offset_y: 4.0,
        blur_radius: 8.0,
        spread: 2.0,
        color: Color::rgba(0.0, 0.0, 0.0, 0.5),
    });
    dl.push_rect(Rect::new(100.0, 100.0, 200.0, 150.0), style);

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    // Shadow + main rect.
    assert_eq!(writer.len(), 2);

    let bytes = writer.as_bytes();

    // First instance is the shadow (expanded).
    let shadow_rec = &bytes[..80];
    let expand = 2.0 + 8.0; // spread + blur
    assert_eq!(read_f32(shadow_rec, 0), 100.0 - expand); // x
    assert_eq!(read_f32(shadow_rec, 4), 100.0 + 4.0 - expand); // y + offset_y
    assert_eq!(read_f32(shadow_rec, 8), 200.0 + expand * 2.0); // w
    assert_eq!(read_f32(shadow_rec, 12), 150.0 + expand * 2.0); // h

    // Second instance is the main rect.
    let main_rec = &bytes[80..160];
    assert_eq!(read_f32(main_rec, 0), 100.0);
    assert_eq!(read_f32(main_rec, 4), 100.0);
}

// --- Line conversion ---

#[test]
fn horizontal_line_converts_to_rect() {
    let mut dl = DrawList::new();
    dl.push_line(
        Point::new(10.0, 50.0),
        Point::new(110.0, 50.0),
        2.0,
        Color::BLACK,
    );

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    assert_eq!(writer.len(), 1);

    let rec = writer.as_bytes();
    // Width should be ~100px, height ~2px.
    let w = read_f32(rec, 8);
    let h = read_f32(rec, 12);
    assert!((w - 100.0).abs() < 0.01);
    assert!((h - 2.0).abs() < 0.01);
}

#[test]
fn zero_length_line_produces_nothing() {
    let mut dl = DrawList::new();
    dl.push_line(
        Point::new(50.0, 50.0),
        Point::new(50.0, 50.0),
        2.0,
        Color::BLACK,
    );

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    assert!(writer.is_empty());
}

// --- Deferred commands ---

#[test]
fn image_command_is_noop() {
    let mut dl = DrawList::new();
    dl.push_image(Rect::new(0.0, 0.0, 64.0, 64.0), 1, [0.0, 0.0, 1.0, 1.0]);

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    assert!(writer.is_empty());
}

#[test]
fn clip_commands_are_noop() {
    let mut dl = DrawList::new();
    dl.push_clip(Rect::new(0.0, 0.0, 100.0, 100.0));
    dl.push_rect(
        Rect::new(10.0, 10.0, 50.0, 50.0),
        RectStyle::filled(Color::WHITE),
    );
    dl.pop_clip();

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    // Only the rect should produce an instance; clips are no-ops.
    assert_eq!(writer.len(), 1);
}

// --- Multiple commands ---

#[test]
fn multiple_rects_accumulate() {
    let mut dl = DrawList::new();
    dl.push_rect(
        Rect::new(0.0, 0.0, 50.0, 50.0),
        RectStyle::filled(Color::BLACK),
    );
    dl.push_rect(
        Rect::new(60.0, 0.0, 50.0, 50.0),
        RectStyle::filled(Color::WHITE),
    );
    dl.push_rect(
        Rect::new(120.0, 0.0, 50.0, 50.0),
        RectStyle::filled(Color::BLACK),
    );

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    assert_eq!(writer.len(), 3);
}

// --- Invisible rect ---

#[test]
fn invisible_rect_still_writes_instance() {
    let mut dl = DrawList::new();
    dl.push_rect(Rect::new(0.0, 0.0, 50.0, 50.0), RectStyle::default());

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    // An unstyled rect writes a transparent instance (the GPU will discard it).
    assert_eq!(writer.len(), 1);
    let rec = writer.as_bytes();
    assert_eq!(read_f32(rec, 48), 0.0); // fill alpha = 0 (transparent)
}

// --- Text conversion ---

/// Test atlas keyed by [`RasterKey`] for text glyph lookup.
struct KeyTestAtlas(HashMap<RasterKey, AtlasEntry>);

impl AtlasLookup for KeyTestAtlas {
    fn lookup(&self, _ch: char, _style: GlyphStyle) -> Option<&AtlasEntry> {
        None
    }

    fn lookup_key(&self, key: RasterKey) -> Option<&AtlasEntry> {
        self.0.get(&key)
    }
}

const TEST_SIZE_Q6: u32 = 896; // ~14px

/// Create a deterministic atlas entry for a glyph ID.
fn text_entry(glyph_id: u16) -> AtlasEntry {
    AtlasEntry {
        page: 0,
        uv_x: (glyph_id % 16) as f32 / 16.0,
        uv_y: (glyph_id / 16) as f32 / 16.0,
        uv_w: 7.0 / 1024.0,
        uv_h: 14.0 / 1024.0,
        width: 7,
        height: 14,
        bearing_x: 1,
        bearing_y: 12,
        kind: AtlasKind::Mono,
    }
}

/// Build a `KeyTestAtlas` with entries for the given glyph IDs using `FontRealm::Ui`.
fn text_atlas_with(glyph_ids: &[u16]) -> KeyTestAtlas {
    let mut map = HashMap::new();
    for &gid in glyph_ids {
        let key = RasterKey {
            glyph_id: gid,
            face_idx: FaceIdx::REGULAR,
            size_q6: TEST_SIZE_Q6,
            synthetic: SyntheticFlags::NONE,
            hinted: true,
            subpx_x: 0,
            font_realm: FontRealm::Ui,
        };
        map.insert(key, text_entry(gid));
    }
    KeyTestAtlas(map)
}

/// Build a ShapedText with the given glyphs and a 14px line height / 12px baseline.
fn shaped_text(glyphs: Vec<ShapedGlyph>) -> ShapedText {
    let width: f32 = glyphs.iter().map(|g| g.x_advance).sum();
    ShapedText::new(glyphs, width, 14.0, 12.0)
}

#[test]
fn text_without_context_is_noop() {
    let mut dl = DrawList::new();
    let st = shaped_text(vec![ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        x_advance: 7.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }]);
    dl.push_text(Point::new(10.0, 20.0), st, Color::WHITE);

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    // No text context → text is deferred, no instances.
    assert!(writer.is_empty());
}

#[test]
fn text_single_glyph_produces_one_instance() {
    let atlas = text_atlas_with(&[42]);
    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color = InstanceWriter::new();

    let st = shaped_text(vec![ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        x_advance: 7.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }]);

    let mut dl = DrawList::new();
    dl.push_text(Point::new(10.0, 20.0), st, Color::WHITE);

    let mut ui_writer = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui_writer, Some(&mut ctx), 1.0, 1.0);

    // No UI rect instances (only text).
    assert!(ui_writer.is_empty());
    // One mono glyph instance.
    assert_eq!(mono.len(), 1);
    assert!(subpx.is_empty());
    assert!(color.is_empty());

    // Verify position: x = 10.0 + bearing_x(1), y = 20.0 + baseline(12) - bearing_y(12).
    let rec = mono.as_bytes();
    let gx = read_f32(rec, 0);
    let gy = read_f32(rec, 4);
    assert!(
        (gx - 11.0).abs() < 0.5,
        "glyph x = 10 + bearing_x(1) = 11, got {gx}"
    );
    assert!(
        (gy - 20.0).abs() < 0.5,
        "glyph y = 20 + 12 - 12 = 20, got {gy}"
    );

    // Verify glyph instance kind.
    assert_eq!(read_u32(rec, 64), 1); // InstanceKind::Glyph
}

#[test]
fn text_spaces_are_advance_only() {
    let atlas = text_atlas_with(&[65, 66]); // 'A' and 'B' glyphs
    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color = InstanceWriter::new();

    // "A B" — glyph 65, space (glyph_id=0), glyph 66.
    let st = shaped_text(vec![
        ShapedGlyph {
            glyph_id: 65,
            face_index: 0,
            x_advance: 7.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 0,
            face_index: 0,
            x_advance: 7.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 66,
            face_index: 0,
            x_advance: 7.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ]);

    let mut dl = DrawList::new();
    dl.push_text(Point::new(0.0, 0.0), st, Color::WHITE);

    let mut ui = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui, Some(&mut ctx), 1.0, 1.0);

    // Only 2 glyph instances (space skipped).
    assert_eq!(mono.len(), 2);

    // Second glyph should be positioned after the space advance.
    let bytes = mono.as_bytes();
    let first_x = read_f32(bytes, 0);
    let second_x = read_f32(&bytes[INSTANCE_SIZE..], 0);
    assert!(
        second_x > first_x + 7.0,
        "second glyph should be after space: first_x={first_x}, second_x={second_x}",
    );
}

#[test]
fn text_mixed_with_rects() {
    let atlas = text_atlas_with(&[42]);
    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color_w = InstanceWriter::new();

    let st = shaped_text(vec![ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        x_advance: 7.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }]);

    let mut dl = DrawList::new();
    dl.push_rect(
        Rect::new(0.0, 0.0, 50.0, 50.0),
        RectStyle::filled(Color::BLACK),
    );
    dl.push_text(Point::new(60.0, 10.0), st, Color::WHITE);
    dl.push_rect(
        Rect::new(80.0, 0.0, 50.0, 50.0),
        RectStyle::filled(Color::WHITE),
    );

    let mut ui = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color_w,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui, Some(&mut ctx), 1.0, 1.0);

    // 2 UI rect instances, 1 glyph instance.
    assert_eq!(ui.len(), 2);
    assert_eq!(mono.len(), 1);
}

#[test]
fn text_color_conversion() {
    // Verify color_to_rgb converts f32 RGBA to u8 RGB correctly.
    let rgb = super::color_to_rgb(Color::rgba(1.0, 0.5, 0.0, 0.8));
    assert_eq!(rgb.r, 255);
    assert_eq!(rgb.g, 128);
    assert_eq!(rgb.b, 0);
}

#[test]
fn text_empty_shaped_produces_nothing() {
    let atlas = text_atlas_with(&[]);
    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color_w = InstanceWriter::new();

    let st = ShapedText::new(Vec::new(), 0.0, 14.0, 12.0);

    let mut dl = DrawList::new();
    dl.push_text(Point::new(0.0, 0.0), st, Color::WHITE);

    let mut ui = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color_w,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui, Some(&mut ctx), 1.0, 1.0);

    assert!(mono.is_empty());
}

#[test]
fn text_atlas_miss_skips_glyph() {
    // Atlas has no entry for glyph 99.
    let atlas = text_atlas_with(&[42]);
    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color_w = InstanceWriter::new();

    let st = shaped_text(vec![
        ShapedGlyph {
            glyph_id: 42,
            face_index: 0,
            x_advance: 7.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 99,
            face_index: 0,
            x_advance: 7.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ]);

    let mut dl = DrawList::new();
    dl.push_text(Point::new(0.0, 0.0), st, Color::WHITE);

    let mut ui = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color_w,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui, Some(&mut ctx), 1.0, 1.0);

    // Only 1 instance (glyph 99 missed in atlas → skipped).
    assert_eq!(mono.len(), 1);
}

#[test]
fn text_color_glyph_routes_to_color_writer() {
    // Create a color atlas entry.
    let mut map = HashMap::new();
    let key = RasterKey {
        glyph_id: 50,
        face_idx: FaceIdx::REGULAR,
        size_q6: TEST_SIZE_Q6,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
        subpx_x: 0,
        font_realm: FontRealm::Ui,
    };
    map.insert(
        key,
        AtlasEntry {
            page: 1,
            uv_x: 0.0,
            uv_y: 0.0,
            uv_w: 16.0 / 1024.0,
            uv_h: 16.0 / 1024.0,
            width: 16,
            height: 16,
            bearing_x: 0,
            bearing_y: 14,
            kind: AtlasKind::Color,
        },
    );
    let atlas = KeyTestAtlas(map);

    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color_w = InstanceWriter::new();

    let st = shaped_text(vec![ShapedGlyph {
        glyph_id: 50,
        face_index: 0,
        x_advance: 16.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }]);

    let mut dl = DrawList::new();
    dl.push_text(Point::new(0.0, 0.0), st, Color::WHITE);

    let mut ui = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color_w,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui, Some(&mut ctx), 1.0, 1.0);

    // Color glyph routes to color writer.
    assert!(mono.is_empty());
    assert!(subpx.is_empty());
    assert_eq!(color_w.len(), 1);
}

// --- Corner radius edge cases (from Chromium rrect_f_unittest) ---

#[test]
fn uniform_radius_picks_max_of_four_corners() {
    let mut dl = DrawList::new();
    let style = RectStyle::filled(Color::WHITE).with_per_corner_radius(2.0, 8.0, 4.0, 6.0);
    dl.push_rect(Rect::new(0.0, 0.0, 200.0, 100.0), style);

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    let rec = writer.as_bytes();
    assert_eq!(read_f32(rec, 72), 8.0, "should pick max(2, 8, 4, 6) = 8");
}

#[test]
fn all_corners_zero_is_sharp_rect() {
    let mut dl = DrawList::new();
    let style = RectStyle::filled(Color::WHITE).with_per_corner_radius(0.0, 0.0, 0.0, 0.0);
    dl.push_rect(Rect::new(0.0, 0.0, 50.0, 50.0), style);

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    let rec = writer.as_bytes();
    assert_eq!(read_f32(rec, 72), 0.0, "all-zero radii → sharp rect");
}

#[test]
fn radius_larger_than_half_dimension_passes_through() {
    // Chromium clamps, but our SDF shader handles this — verify it passes.
    let mut dl = DrawList::new();
    let style = RectStyle::filled(Color::WHITE).with_radius(30.0);
    dl.push_rect(Rect::new(0.0, 0.0, 20.0, 10.0), style);

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    let rec = writer.as_bytes();
    // The converter passes the radius as-is; the SDF shader clamps internally.
    assert_eq!(read_f32(rec, 72), 30.0);
}

// --- Zero-size and degenerate rect edge cases ---

#[test]
fn zero_width_rect_produces_instance() {
    let mut dl = DrawList::new();
    dl.push_rect(
        Rect::new(10.0, 20.0, 0.0, 50.0),
        RectStyle::filled(Color::WHITE),
    );

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    assert_eq!(writer.len(), 1);
    let rec = writer.as_bytes();
    assert_eq!(read_f32(rec, 8), 0.0, "zero width preserved");
    assert_eq!(read_f32(rec, 12), 50.0);
}

#[test]
fn zero_height_rect_produces_instance() {
    let mut dl = DrawList::new();
    dl.push_rect(
        Rect::new(10.0, 20.0, 100.0, 0.0),
        RectStyle::filled(Color::WHITE),
    );

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    assert_eq!(writer.len(), 1);
    let rec = writer.as_bytes();
    assert_eq!(read_f32(rec, 8), 100.0);
    assert_eq!(read_f32(rec, 12), 0.0, "zero height preserved");
}

// --- Shadow edge cases ---

#[test]
fn shadow_with_zero_blur_and_spread() {
    let mut dl = DrawList::new();
    let style = RectStyle::filled(Color::WHITE).with_shadow(Shadow {
        offset_x: 5.0,
        offset_y: 5.0,
        blur_radius: 0.0,
        spread: 0.0,
        color: Color::BLACK,
    });
    dl.push_rect(Rect::new(100.0, 100.0, 200.0, 150.0), style);

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    assert_eq!(writer.len(), 2);
    let bytes = writer.as_bytes();

    // Shadow rect: expand=0, so same size as main, just offset.
    let sx = read_f32(bytes, 0);
    let sy = read_f32(bytes, 4);
    let sw = read_f32(bytes, 8);
    let sh = read_f32(bytes, 12);
    assert_eq!(sx, 100.0 + 5.0); // offset_x only
    assert_eq!(sy, 100.0 + 5.0); // offset_y only
    assert_eq!(sw, 200.0); // no expansion
    assert_eq!(sh, 150.0); // no expansion
}

#[test]
fn shadow_with_negative_offset() {
    let mut dl = DrawList::new();
    let style = RectStyle::filled(Color::WHITE).with_shadow(Shadow {
        offset_x: -10.0,
        offset_y: -10.0,
        blur_radius: 0.0,
        spread: 0.0,
        color: Color::BLACK,
    });
    dl.push_rect(Rect::new(100.0, 100.0, 50.0, 50.0), style);

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    let bytes = writer.as_bytes();
    let sx = read_f32(bytes, 0);
    let sy = read_f32(bytes, 4);
    assert_eq!(sx, 90.0, "shadow shifted left");
    assert_eq!(sy, 90.0, "shadow shifted up");
}

#[test]
fn shadow_radius_inherits_rect_corner_radius() {
    let mut dl = DrawList::new();
    let style = RectStyle::filled(Color::WHITE)
        .with_radius(10.0)
        .with_shadow(Shadow {
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 4.0,
            spread: 2.0,
            color: Color::BLACK,
        });
    dl.push_rect(Rect::new(0.0, 0.0, 100.0, 100.0), style);

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    let bytes = writer.as_bytes();
    // Shadow corner_radius = original(10) + expand(4+2) = 16.
    let shadow_radius = read_f32(bytes, 72);
    assert_eq!(shadow_radius, 16.0);

    // Main rect corner_radius stays at 10.
    let main_radius = read_f32(&bytes[INSTANCE_SIZE..], 72);
    assert_eq!(main_radius, 10.0);
}

// --- Line edge cases (from Chromium line_f_unittest) ---

#[test]
fn vertical_line_converts_to_rect() {
    let mut dl = DrawList::new();
    dl.push_line(
        Point::new(50.0, 10.0),
        Point::new(50.0, 110.0),
        2.0,
        Color::BLACK,
    );

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    assert_eq!(writer.len(), 1);
    let rec = writer.as_bytes();
    let w = read_f32(rec, 8);
    let h = read_f32(rec, 12);
    assert!((w - 2.0).abs() < 0.01, "vertical line width ~2px, got {w}");
    assert!(
        (h - 100.0).abs() < 0.01,
        "vertical line height ~100px, got {h}"
    );
}

#[test]
fn diagonal_line_produces_stepping_rects() {
    let mut dl = DrawList::new();
    dl.push_line(
        Point::new(0.0, 0.0),
        Point::new(10.0, 10.0),
        1.0,
        Color::BLACK,
    );

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    // 10px diagonal → 11 stepping rects (0..=10).
    assert_eq!(writer.len(), 11);

    let bytes = writer.as_bytes();
    // Each rect should be 1×1 (stroke_width × stroke_width).
    for i in 0..11 {
        let offset = i * INSTANCE_SIZE;
        let w = read_f32(bytes, offset + 8);
        let h = read_f32(bytes, offset + 12);
        assert!(
            (w - 1.0).abs() < 0.01,
            "step {i}: width should be 1.0, got {w}",
        );
        assert!(
            (h - 1.0).abs() < 0.01,
            "step {i}: height should be 1.0, got {h}",
        );
    }

    // First rect centered at (0, 0): position = (-0.5, -0.5).
    let first_x = read_f32(bytes, 0);
    let first_y = read_f32(bytes, 4);
    assert!(
        (first_x - (-0.5)).abs() < 0.01,
        "first x = -0.5, got {first_x}"
    );
    assert!(
        (first_y - (-0.5)).abs() < 0.01,
        "first y = -0.5, got {first_y}"
    );

    // Last rect centered at (10, 10): position = (9.5, 9.5).
    let last_offset = 10 * INSTANCE_SIZE;
    let last_x = read_f32(bytes, last_offset);
    let last_y = read_f32(bytes, last_offset + 4);
    assert!((last_x - 9.5).abs() < 0.01, "last x = 9.5, got {last_x}");
    assert!((last_y - 9.5).abs() < 0.01, "last y = 9.5, got {last_y}");
}

#[test]
fn diagonal_line_x_pattern_no_overlap() {
    // Two crossed diagonals (the close button X pattern) should produce
    // separate stepping rects, not a single filled square.
    let mut dl = DrawList::new();
    // Top-left to bottom-right.
    dl.push_line(
        Point::new(0.0, 0.0),
        Point::new(10.0, 10.0),
        1.0,
        Color::BLACK,
    );
    // Top-right to bottom-left.
    dl.push_line(
        Point::new(10.0, 0.0),
        Point::new(0.0, 10.0),
        1.0,
        Color::BLACK,
    );

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    // 11 rects per diagonal = 22 total.
    assert_eq!(writer.len(), 22);
}

// --- Text subpixel routing ---

#[test]
fn text_subpixel_glyph_routes_to_subpixel_writer() {
    let mut map = HashMap::new();
    let key = RasterKey {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        size_q6: TEST_SIZE_Q6,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
        subpx_x: 0,
        font_realm: FontRealm::Ui,
    };
    map.insert(
        key,
        AtlasEntry {
            kind: AtlasKind::Subpixel,
            ..text_entry(42)
        },
    );
    let atlas = KeyTestAtlas(map);

    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color_w = InstanceWriter::new();

    let st = shaped_text(vec![ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        x_advance: 7.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }]);

    let mut dl = DrawList::new();
    dl.push_text(Point::new(0.0, 0.0), st, Color::WHITE);

    let mut ui = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color_w,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui, Some(&mut ctx), 1.0, 1.0);

    assert!(mono.is_empty(), "should not go to mono");
    assert_eq!(subpx.len(), 1, "should route to subpixel writer");
    assert!(color_w.is_empty(), "should not go to color");
}

// --- Text cursor accumulation and positioning ---

#[test]
fn text_many_glyphs_cursor_accumulates() {
    let ids: Vec<u16> = (1..=50).collect();
    let atlas = text_atlas_with(&ids);

    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color_w = InstanceWriter::new();

    let glyphs: Vec<ShapedGlyph> = ids
        .iter()
        .map(|&id| ShapedGlyph {
            glyph_id: id,
            face_index: 0,
            x_advance: 7.0,
            x_offset: 0.0,
            y_offset: 0.0,
        })
        .collect();
    let st = shaped_text(glyphs);

    let mut dl = DrawList::new();
    dl.push_text(Point::new(0.0, 0.0), st, Color::WHITE);

    let mut ui = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color_w,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui, Some(&mut ctx), 1.0, 1.0);

    assert_eq!(mono.len(), 50);

    // Verify last glyph x position: 49 * 7.0 + bearing_x(1) = 344.0.
    let bytes = mono.as_bytes();
    let last_x = read_f32(&bytes[49 * INSTANCE_SIZE..], 0);
    assert!(
        (last_x - 344.0).abs() < 0.5,
        "last glyph at x = 49*7 + 1 = 344, got {last_x}",
    );
}

#[test]
fn text_two_commands_independent_cursors() {
    let atlas = text_atlas_with(&[42, 43]);
    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color_w = InstanceWriter::new();

    let st1 = shaped_text(vec![ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        x_advance: 7.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }]);
    let st2 = shaped_text(vec![ShapedGlyph {
        glyph_id: 43,
        face_index: 0,
        x_advance: 7.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }]);

    let mut dl = DrawList::new();
    dl.push_text(Point::new(100.0, 0.0), st1, Color::WHITE);
    dl.push_text(Point::new(200.0, 0.0), st2, Color::WHITE);

    let mut ui = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color_w,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui, Some(&mut ctx), 1.0, 1.0);

    assert_eq!(mono.len(), 2);

    // Each text command starts its own cursor — positions should be independent.
    let bytes = mono.as_bytes();
    let x1 = read_f32(bytes, 0);
    let x2 = read_f32(&bytes[INSTANCE_SIZE..], 0);
    assert!(
        (x1 - 101.0).abs() < 0.5,
        "first text at 100 + bearing(1) = 101, got {x1}",
    );
    assert!(
        (x2 - 201.0).abs() < 0.5,
        "second text at 200 + bearing(1) = 201, got {x2}",
    );
}

#[test]
fn text_negative_bearing_extends_left() {
    let mut map = HashMap::new();
    let key = RasterKey {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        size_q6: TEST_SIZE_Q6,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
        subpx_x: 0,
        font_realm: FontRealm::Ui,
    };
    map.insert(
        key,
        AtlasEntry {
            bearing_x: -3,
            ..text_entry(42)
        },
    );
    let atlas = KeyTestAtlas(map);

    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color_w = InstanceWriter::new();

    let st = shaped_text(vec![ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        x_advance: 7.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }]);

    let mut dl = DrawList::new();
    dl.push_text(Point::new(10.0, 20.0), st, Color::WHITE);

    let mut ui = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color_w,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui, Some(&mut ctx), 1.0, 1.0);

    assert_eq!(mono.len(), 1);
    let gx = read_f32(mono.as_bytes(), 0);
    // x = 10.0 + bearing_x(-3) = 7.0 (extends left of cursor).
    assert!(
        (gx - 7.0).abs() < 0.5,
        "negative bearing extends left: 10 + (-3) = 7, got {gx}",
    );
}

#[test]
fn text_all_spaces_produces_no_glyph_instances() {
    let atlas = text_atlas_with(&[]);
    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color_w = InstanceWriter::new();

    let glyphs = vec![
        ShapedGlyph {
            glyph_id: 0,
            face_index: 0,
            x_advance: 7.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 0,
            face_index: 0,
            x_advance: 7.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 0,
            face_index: 0,
            x_advance: 7.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];
    let st = shaped_text(glyphs);

    let mut dl = DrawList::new();
    dl.push_text(Point::new(0.0, 0.0), st, Color::WHITE);

    let mut ui = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color_w,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui, Some(&mut ctx), 1.0, 1.0);

    assert!(
        mono.is_empty(),
        "all-space text should produce zero instances"
    );
}

#[test]
fn text_fractional_position_applies_subpixel_phase() {
    // Position at x=10.5 should produce subpx_x=2 (phase 0.50).
    let mut map = HashMap::new();
    let key = RasterKey {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        size_q6: TEST_SIZE_Q6,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
        subpx_x: 2, // phase for 0.5 fractional
        font_realm: FontRealm::Ui,
    };
    map.insert(key, text_entry(42));
    let atlas = KeyTestAtlas(map);

    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color_w = InstanceWriter::new();

    let st = shaped_text(vec![ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        x_advance: 7.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }]);

    let mut dl = DrawList::new();
    dl.push_text(Point::new(10.5, 20.0), st, Color::WHITE);

    let mut ui = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color_w,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui, Some(&mut ctx), 1.0, 1.0);

    // If the subpixel phase key matches, the atlas lookup succeeds and
    // we get one glyph instance.
    assert_eq!(
        mono.len(),
        1,
        "fractional position should match subpx_x=2 phase",
    );
}

// --- Border without fill ---

#[test]
fn border_only_rect_has_transparent_fill() {
    let mut dl = DrawList::new();
    let style = RectStyle::default()
        .with_border(2.0, Color::WHITE)
        .with_radius(4.0);
    dl.push_rect(Rect::new(0.0, 0.0, 100.0, 50.0), style);

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    assert_eq!(writer.len(), 1);
    let rec = writer.as_bytes();
    // Fill (bg_color) = transparent (default when no fill set).
    assert_eq!(read_f32(rec, 48), 0.0);
    assert_eq!(read_f32(rec, 52), 0.0);
    assert_eq!(read_f32(rec, 56), 0.0);
    assert_eq!(read_f32(rec, 60), 0.0);
    // Border (fg_color) = white.
    assert_eq!(read_f32(rec, 32), 1.0);
    // Border width.
    assert_eq!(read_f32(rec, 76), 2.0);
}

// --- DPI scale factor ---

#[test]
fn scale_factor_applies_to_rect_position_and_size() {
    let mut dl = DrawList::new();
    dl.push_rect(
        Rect::new(100.0, 200.0, 300.0, 150.0),
        RectStyle::filled(Color::WHITE),
    );

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.25, 1.0);

    assert_eq!(writer.len(), 1);
    let rec = writer.as_bytes();
    assert_eq!(read_f32(rec, 0), 125.0); // 100 * 1.25
    assert_eq!(read_f32(rec, 4), 250.0); // 200 * 1.25
    assert_eq!(read_f32(rec, 8), 375.0); // 300 * 1.25
    assert_eq!(read_f32(rec, 12), 187.5); // 150 * 1.25
}

#[test]
fn scale_factor_applies_to_border_and_radius() {
    let mut dl = DrawList::new();
    let style = RectStyle::filled(Color::WHITE)
        .with_border(2.0, Color::BLACK)
        .with_radius(8.0);
    dl.push_rect(Rect::new(0.0, 0.0, 100.0, 50.0), style);

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 2.0, 1.0);

    let rec = writer.as_bytes();
    assert_eq!(read_f32(rec, 72), 16.0); // radius 8 * 2
    assert_eq!(read_f32(rec, 76), 4.0); // border 2 * 2
}

#[test]
fn scale_factor_one_is_identity() {
    let mut dl = DrawList::new();
    dl.push_rect(
        Rect::new(10.0, 20.0, 100.0, 50.0),
        RectStyle::filled(Color::WHITE),
    );

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    let rec = writer.as_bytes();
    assert_eq!(read_f32(rec, 0), 10.0);
    assert_eq!(read_f32(rec, 4), 20.0);
    assert_eq!(read_f32(rec, 8), 100.0);
    assert_eq!(read_f32(rec, 12), 50.0);
}

// --- Layer commands ---

#[test]
fn layer_commands_are_noop_in_converter() {
    let mut dl = DrawList::new();
    dl.push_layer(Color::WHITE);
    dl.push_rect(
        Rect::new(0.0, 0.0, 50.0, 50.0),
        RectStyle::filled(Color::WHITE),
    );
    dl.pop_layer();

    let mut writer = InstanceWriter::new();
    convert_draw_list(&dl, &mut writer, None, 1.0, 1.0);

    // Only the rect should produce an instance; layer commands are no-ops.
    assert_eq!(writer.len(), 1);
}

#[test]
fn text_with_layer_bg_hint_routes_subpixel_with_bg() {
    // Create a subpixel atlas entry to verify bg routing.
    let mut map = HashMap::new();
    let key = RasterKey {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        size_q6: TEST_SIZE_Q6,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
        subpx_x: 0,
        font_realm: FontRealm::Ui,
    };
    map.insert(
        key,
        AtlasEntry {
            kind: AtlasKind::Subpixel,
            ..text_entry(42)
        },
    );
    let atlas = KeyTestAtlas(map);

    let mut mono = InstanceWriter::new();
    let mut subpx = InstanceWriter::new();
    let mut color_w = InstanceWriter::new();

    let st = shaped_text(vec![ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        x_advance: 7.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }]);

    // Use layer stack so push_text captures the bg.
    let mut dl = DrawList::new();
    dl.push_layer(Color::rgba(0.2, 0.2, 0.2, 1.0));
    dl.push_text(Point::new(0.0, 0.0), st, Color::WHITE);
    dl.pop_layer();

    let mut ui = InstanceWriter::new();
    let mut ctx = TextContext {
        atlas: &atlas,
        mono_writer: &mut mono,
        subpixel_writer: &mut subpx,
        color_writer: &mut color_w,
        size_q6: TEST_SIZE_Q6,
        hinted: true,
    };
    convert_draw_list(&dl, &mut ui, Some(&mut ctx), 1.0, 1.0);

    // Subpixel glyph should route to subpixel writer with bg_hint.
    assert!(mono.is_empty());
    assert_eq!(subpx.len(), 1, "should route to subpixel writer");
    assert!(color_w.is_empty());
}

#[test]
fn text_without_layer_has_no_bg_hint() {
    // Verify text drawn without a layer has bg_hint=None.
    let mut dl = DrawList::new();
    let st = shaped_text(vec![ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        x_advance: 7.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }]);
    dl.push_text(Point::new(0.0, 0.0), st, Color::WHITE);

    match &dl.commands()[0] {
        oriterm_ui::draw::DrawCommand::Text { bg_hint, .. } => {
            assert!(
                bg_hint.is_none(),
                "text outside layer should have no bg_hint"
            );
        }
        _ => panic!("expected Text command"),
    }
}
