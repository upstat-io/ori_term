//! Unit tests for the prepare phase.

use std::collections::HashMap;

use oriterm_core::{CellFlags, Column, CursorShape, Rgb, Selection, Side, StableRowIndex};

use super::{
    AtlasLookup, ShapedFrame, prepare_frame, prepare_frame_into, prepare_frame_shaped,
    prepare_frame_shaped_into,
};
use crate::font::{FaceIdx, FontRealm, GlyphStyle, RasterKey, SyntheticFlags};
use crate::gpu::atlas::{AtlasEntry, AtlasKind};
use crate::gpu::frame_input::{FrameInput, FrameSelection, ViewportSize};
use crate::gpu::instance_writer::INSTANCE_SIZE;
use crate::gpu::prepared_frame::PreparedFrame;
use crate::gpu::srgb_to_linear;
use oriterm_ui::text::ShapedGlyph;

// ── Test atlas ──

/// Test atlas backed by a `HashMap`.
struct TestAtlas(HashMap<(char, GlyphStyle), AtlasEntry>);

impl AtlasLookup for TestAtlas {
    fn lookup(&self, ch: char, style: GlyphStyle) -> Option<&AtlasEntry> {
        self.0.get(&(ch, style))
    }

    fn lookup_key(&self, _key: RasterKey) -> Option<&AtlasEntry> {
        None
    }
}

/// Create a deterministic atlas entry for a character.
///
/// UV coordinates are derived from the char code for predictable assertions.
fn test_entry(ch: char) -> AtlasEntry {
    let code = ch as u32;
    AtlasEntry {
        page: 0,
        uv_x: (code % 16) as f32 / 16.0,
        uv_y: (code / 16) as f32 / 16.0,
        uv_w: 7.0 / 1024.0,
        uv_h: 14.0 / 1024.0,
        width: 7,
        height: 14,
        bearing_x: 1,
        bearing_y: 12,
        kind: AtlasKind::Mono,
    }
}

/// Build a test atlas with entries for the given characters (Regular style).
fn atlas_with(chars: &[char]) -> TestAtlas {
    let mut map = HashMap::new();
    for &c in chars {
        map.insert((c, GlyphStyle::Regular), test_entry(c));
    }
    TestAtlas(map)
}

/// Empty atlas that returns `None` for every lookup.
fn empty_atlas() -> TestAtlas {
    TestAtlas(HashMap::new())
}

// ── Decoded instance for assertions ──

/// Parsed 80-byte instance record for test assertions.
#[derive(Debug)]
struct DecodedInstance {
    pos: (f32, f32),
    size: (f32, f32),
    uv: [f32; 4],
    fg_color: [f32; 4],
    bg_color: [f32; 4],
    kind: u32,
}

fn read_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn decode_instance(bytes: &[u8]) -> DecodedInstance {
    assert_eq!(bytes.len(), INSTANCE_SIZE);
    DecodedInstance {
        pos: (read_f32(bytes, 0), read_f32(bytes, 4)),
        size: (read_f32(bytes, 8), read_f32(bytes, 12)),
        uv: [
            read_f32(bytes, 16),
            read_f32(bytes, 20),
            read_f32(bytes, 24),
            read_f32(bytes, 28),
        ],
        fg_color: [
            read_f32(bytes, 32),
            read_f32(bytes, 36),
            read_f32(bytes, 40),
            read_f32(bytes, 44),
        ],
        bg_color: [
            read_f32(bytes, 48),
            read_f32(bytes, 52),
            read_f32(bytes, 56),
            read_f32(bytes, 60),
        ],
        kind: read_u32(bytes, 64),
    }
}

/// Decode the nth instance from a writer's byte buffer.
fn nth_instance(bytes: &[u8], n: usize) -> DecodedInstance {
    let start = n * INSTANCE_SIZE;
    decode_instance(&bytes[start..start + INSTANCE_SIZE])
}

/// Assert instance counts across all three buffers.
fn assert_counts(frame: &PreparedFrame, bg: usize, fg: usize, cursor: usize) {
    assert_eq!(
        frame.backgrounds.len(),
        bg,
        "expected {bg} bg instances, got {}",
        frame.backgrounds.len(),
    );
    assert_eq!(
        frame.glyphs.len(),
        fg,
        "expected {fg} fg instances, got {}",
        frame.glyphs.len(),
    );
    assert_eq!(
        frame.cursors.len(),
        cursor,
        "expected {cursor} cursor instances, got {}",
        frame.cursors.len(),
    );
}

/// Convert Rgb to the linear-light `[f32; 4]` that push_rect writes to bg_color.
fn rgb_f32(c: Rgb) -> [f32; 4] {
    [
        srgb_to_linear(c.r),
        srgb_to_linear(c.g),
        srgb_to_linear(c.b),
        1.0,
    ]
}

// ── Instance buffer correctness ──

#[test]
fn single_char_produces_one_bg_and_one_fg() {
    let input = FrameInput::test_grid(1, 1, "A");
    let atlas = atlas_with(&['A']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // 1 bg for the cell, 1 fg for the glyph, 1 cursor (block at 0,0).
    assert_counts(&frame, 1, 1, 1);
}

#[test]
fn single_char_bg_position_and_size() {
    let input = FrameInput::test_grid(2, 2, "A");
    let atlas = atlas_with(&['A']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let bg = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg.pos, (0.0, 0.0));
    assert_eq!(bg.size, (8.0, 16.0));
    assert_eq!(bg.kind, 0); // InstanceKind::Rect
}

#[test]
fn single_char_fg_position_with_bearing() {
    let input = FrameInput::test_grid(2, 2, "A");
    let atlas = atlas_with(&['A']);
    let entry = test_entry('A');

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    // glyph_x = 0.0 + bearing_x(1) = 1.0
    // glyph_y = 0.0 + baseline(12.0) - bearing_y(12) = 0.0
    assert_eq!(fg.pos, (1.0, 0.0));
    assert_eq!(fg.size, (entry.width as f32, entry.height as f32));
    assert_eq!(fg.uv, [entry.uv_x, entry.uv_y, entry.uv_w, entry.uv_h]);
    assert_eq!(fg.kind, 1); // InstanceKind::Glyph
}

#[test]
fn single_char_fg_color_matches_cell() {
    let input = FrameInput::test_grid(1, 1, "A");
    let atlas = atlas_with(&['A']);
    let fg_rgb = input.content.cells[0].fg;

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    assert_eq!(fg.fg_color, rgb_f32(fg_rgb));
}

#[test]
fn single_char_bg_color_matches_cell() {
    let input = FrameInput::test_grid(1, 1, "A");
    let atlas = atlas_with(&['A']);
    let bg_rgb = input.content.cells[0].bg;

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let bg = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg.bg_color, rgb_f32(bg_rgb));
}

// ── Empty cells ──

#[test]
fn empty_cell_produces_bg_only() {
    let input = FrameInput::test_grid(1, 1, " ");
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.backgrounds.len(), 1);
    assert_eq!(frame.glyphs.len(), 0);
}

#[test]
fn all_spaces_grid_no_fg_instances() {
    let input = FrameInput::test_grid(10, 5, "");
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.backgrounds.len(), 50);
    assert_eq!(frame.glyphs.len(), 0);
}

#[test]
fn all_chars_grid_equal_bg_and_fg() {
    let text: String = std::iter::repeat_n('A', 10).collect();
    let input = FrameInput::test_grid(10, 1, &text);
    let atlas = atlas_with(&['A']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.backgrounds.len(), 10);
    assert_eq!(frame.glyphs.len(), 10);
}

// ── Wide characters ──

#[test]
fn wide_char_produces_double_width_bg() {
    let mut input = FrameInput::test_grid(4, 1, "");
    // Manually set up a wide char at column 0.
    input.content.cells[0].ch = '\u{4E16}'; // 世
    input.content.cells[0].flags = CellFlags::WIDE_CHAR;
    input.content.cells[1].ch = ' ';
    input.content.cells[1].flags = CellFlags::WIDE_CHAR_SPACER;

    let atlas = atlas_with(&['\u{4E16}']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // 1 bg for wide char (double width) + 2 bg for remaining cells = 3 bg.
    // 1 fg for the wide char glyph.
    assert_eq!(frame.backgrounds.len(), 3);
    assert_eq!(frame.glyphs.len(), 1);

    let bg = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg.size, (16.0, 16.0)); // 2 * cell_width
}

#[test]
fn wide_char_spacer_skipped() {
    let mut input = FrameInput::test_grid(2, 1, "");
    input.content.cells[0].ch = '\u{4E16}';
    input.content.cells[0].flags = CellFlags::WIDE_CHAR;
    input.content.cells[1].ch = ' ';
    input.content.cells[1].flags = CellFlags::WIDE_CHAR_SPACER;

    let atlas = atlas_with(&['\u{4E16}']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Only 1 bg (the wide char covers both columns), not 2.
    assert_eq!(frame.backgrounds.len(), 1);
}

// ── Cell positions are pixel-perfect ──

#[test]
fn cell_positions_are_pixel_perfect() {
    let input = FrameInput::test_grid(3, 3, "ABCDEFGHI");
    let atlas = atlas_with(&['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Cell (0,0) → (0, 0), (1,0) → (8, 0), (2,0) → (16, 0)
    // Cell (0,1) → (0, 16), (1,1) → (8, 16), etc.
    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg0.pos, (0.0, 0.0));

    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(bg1.pos, (8.0, 0.0));

    let bg2 = nth_instance(frame.backgrounds.as_bytes(), 2);
    assert_eq!(bg2.pos, (16.0, 0.0));

    let bg3 = nth_instance(frame.backgrounds.as_bytes(), 3);
    assert_eq!(bg3.pos, (0.0, 16.0));

    let bg4 = nth_instance(frame.backgrounds.as_bytes(), 4);
    assert_eq!(bg4.pos, (8.0, 16.0));
}

#[test]
fn glyph_bearing_offsets_applied() {
    let input = FrameInput::test_grid(2, 2, "A");
    let atlas = atlas_with(&['A']);
    let entry = test_entry('A');

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    let expected_x = 0.0 + entry.bearing_x as f32;
    let expected_y = 0.0 + 12.0 - entry.bearing_y as f32; // baseline=12
    assert_eq!(fg.pos, (expected_x, expected_y));
}

// ── Color resolution (passthrough from extract phase) ──

#[test]
fn default_colors_in_instances() {
    let input = FrameInput::test_grid(1, 1, "A");
    let atlas = atlas_with(&['A']);
    let cell = &input.content.cells[0];

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let bg = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg.bg_color, rgb_f32(cell.bg));

    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    assert_eq!(fg.fg_color, rgb_f32(cell.fg));
}

#[test]
fn inverse_colors_passed_through() {
    // Extract phase already swaps fg/bg for INVERSE cells. Prepare just
    // copies them through. Verify the passthrough works.
    let mut input = FrameInput::test_grid(1, 1, "X");
    let original_fg = input.content.cells[0].fg;
    let original_bg = input.content.cells[0].bg;
    // Simulate what extract would have done: swap fg/bg.
    input.content.cells[0].fg = original_bg;
    input.content.cells[0].bg = original_fg;

    let atlas = atlas_with(&['X']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let bg = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg.bg_color, rgb_f32(original_fg));

    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    assert_eq!(fg.fg_color, rgb_f32(original_bg));
}

// ── Determinism ──

#[test]
fn same_input_produces_identical_output() {
    let input = FrameInput::test_grid(10, 5, "Hello World! Testing determinism.");
    let atlas = atlas_with(&[
        'H', 'e', 'l', 'o', 'W', 'r', 'd', '!', 'T', 's', 't', 'i', 'n', 'g', 'm', '.',
    ]);

    let frame1 = prepare_frame(&input, &atlas, (0.0, 0.0));
    let frame2 = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame1.backgrounds.as_bytes(), frame2.backgrounds.as_bytes());
    assert_eq!(frame1.glyphs.as_bytes(), frame2.glyphs.as_bytes());
    assert_eq!(frame1.cursors.as_bytes(), frame2.cursors.as_bytes());
    assert_eq!(frame1.clear_color, frame2.clear_color);
}

// ── Cursor shapes ──

#[test]
fn block_cursor_one_instance() {
    let input = FrameInput::test_grid(10, 5, "");
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Default cursor is Block at (0,0), visible.
    assert_eq!(frame.cursors.len(), 1);

    let c = nth_instance(frame.cursors.as_bytes(), 0);
    assert_eq!(c.pos, (0.0, 0.0));
    assert_eq!(c.size, (8.0, 16.0));
    assert_eq!(c.kind, 2); // InstanceKind::Cursor
}

#[test]
fn bar_cursor_one_instance_2px_wide() {
    let mut input = FrameInput::test_grid(10, 5, "");
    input.content.cursor.shape = CursorShape::Bar;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.cursors.len(), 1);

    let c = nth_instance(frame.cursors.as_bytes(), 0);
    assert_eq!(c.pos, (0.0, 0.0));
    assert_eq!(c.size, (2.0, 16.0));
}

#[test]
fn underline_cursor_one_instance_2px_tall() {
    let mut input = FrameInput::test_grid(10, 5, "");
    input.content.cursor.shape = CursorShape::Underline;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.cursors.len(), 1);

    let c = nth_instance(frame.cursors.as_bytes(), 0);
    assert_eq!(c.pos, (0.0, 14.0)); // y + ch - 2.0 = 0 + 16 - 2 = 14
    assert_eq!(c.size, (8.0, 2.0));
}

#[test]
fn hollow_block_cursor_four_instances() {
    let mut input = FrameInput::test_grid(10, 5, "");
    input.content.cursor.shape = CursorShape::HollowBlock;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.cursors.len(), 4);
}

#[test]
fn hollow_block_edges() {
    let mut input = FrameInput::test_grid(10, 5, "");
    input.content.cursor.shape = CursorShape::HollowBlock;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let top = nth_instance(frame.cursors.as_bytes(), 0);
    assert_eq!(top.pos, (0.0, 0.0));
    assert_eq!(top.size, (8.0, 2.0));

    let bottom = nth_instance(frame.cursors.as_bytes(), 1);
    assert_eq!(bottom.pos, (0.0, 14.0));
    assert_eq!(bottom.size, (8.0, 2.0));

    let left = nth_instance(frame.cursors.as_bytes(), 2);
    assert_eq!(left.pos, (0.0, 0.0));
    assert_eq!(left.size, (2.0, 16.0));

    let right = nth_instance(frame.cursors.as_bytes(), 3);
    assert_eq!(right.pos, (6.0, 0.0)); // cw - 2.0 = 8 - 2 = 6
    assert_eq!(right.size, (2.0, 16.0));
}

#[test]
fn hidden_cursor_zero_instances() {
    let mut input = FrameInput::test_grid(10, 5, "");
    input.content.cursor.shape = CursorShape::Hidden;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.cursors.len(), 0);
}

#[test]
fn cursor_invisible_zero_instances() {
    let mut input = FrameInput::test_grid(10, 5, "");
    input.content.cursor.visible = false;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.cursors.len(), 0);
}

#[test]
fn cursor_at_position() {
    let mut input = FrameInput::test_grid(10, 10, "");
    input.content.cursor.column = Column(5);
    input.content.cursor.line = 3;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let c = nth_instance(frame.cursors.as_bytes(), 0);
    assert_eq!(c.pos, (40.0, 48.0)); // 5*8=40, 3*16=48
}

#[test]
fn cursor_color_from_palette() {
    let input = FrameInput::test_grid(10, 5, "");
    let atlas = empty_atlas();
    let cursor_color = input.palette.cursor_color;

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let c = nth_instance(frame.cursors.as_bytes(), 0);
    // Cursor color is in bg_color (rendered via bg_pipeline as solid-fill rect).
    assert_eq!(c.bg_color, rgb_f32(cursor_color));
}

// ── Missing atlas entries ──

#[test]
fn missing_glyph_skips_fg_instance() {
    let input = FrameInput::test_grid(1, 1, "Z");
    let atlas = empty_atlas(); // No entry for 'Z'.

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.backgrounds.len(), 1);
    assert_eq!(frame.glyphs.len(), 0);
}

// ── Glyph style from flags ──

#[test]
fn bold_cell_uses_bold_style() {
    let mut input = FrameInput::test_grid(1, 1, "B");
    input.content.cells[0].flags = CellFlags::BOLD;

    let mut map = HashMap::new();
    map.insert((('B'), GlyphStyle::Bold), test_entry('B'));
    let atlas = TestAtlas(map);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Should find the Bold entry and produce a glyph.
    assert_eq!(frame.glyphs.len(), 1);
}

#[test]
fn italic_cell_uses_italic_style() {
    let mut input = FrameInput::test_grid(1, 1, "I");
    input.content.cells[0].flags = CellFlags::ITALIC;

    let mut map = HashMap::new();
    map.insert(('I', GlyphStyle::Italic), test_entry('I'));
    let atlas = TestAtlas(map);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.glyphs.len(), 1);
}

#[test]
fn bold_italic_cell_uses_bold_italic_style() {
    let mut input = FrameInput::test_grid(1, 1, "X");
    input.content.cells[0].flags = CellFlags::BOLD | CellFlags::ITALIC;

    let mut map = HashMap::new();
    map.insert(('X', GlyphStyle::BoldItalic), test_entry('X'));
    let atlas = TestAtlas(map);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.glyphs.len(), 1);
}

// ── Instance count for larger grids ──

#[test]
fn ten_by_five_all_spaces() {
    let input = FrameInput::test_grid(10, 5, "");
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_counts(&frame, 50, 0, 1); // 1 cursor (block, visible)
}

#[test]
fn clear_color_matches_palette_background() {
    let input = FrameInput::test_grid(10, 5, "");
    let atlas = empty_atlas();
    let bg = input.palette.background;

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let expected = [
        f64::from(srgb_to_linear(bg.r)),
        f64::from(srgb_to_linear(bg.g)),
        f64::from(srgb_to_linear(bg.b)),
        1.0,
    ];
    assert_eq!(frame.clear_color, expected);
}

#[test]
fn clear_color_respects_palette_opacity() {
    let mut input = FrameInput::test_grid(10, 5, "");
    input.palette.opacity = 0.5;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let bg = input.palette.background;
    let expected = [
        f64::from(srgb_to_linear(bg.r)) * 0.5,
        f64::from(srgb_to_linear(bg.g)) * 0.5,
        f64::from(srgb_to_linear(bg.b)) * 0.5,
        0.5,
    ];
    assert_eq!(frame.clear_color, expected);
}

// ── prepare_frame_into ──

#[test]
fn prepare_into_matches_prepare() {
    let input = FrameInput::test_grid(10, 5, "Hello World!");
    let atlas = atlas_with(&['H', 'e', 'l', 'o', 'W', 'r', 'd', '!']);

    let fresh = prepare_frame(&input, &atlas, (0.0, 0.0));

    let mut reused = PreparedFrame::new(ViewportSize::new(1, 1), Rgb { r: 0, g: 0, b: 0 }, 1.0);
    prepare_frame_into(&input, &atlas, &mut reused, (0.0, 0.0));

    assert_eq!(fresh.backgrounds.as_bytes(), reused.backgrounds.as_bytes());
    assert_eq!(fresh.glyphs.as_bytes(), reused.glyphs.as_bytes());
    assert_eq!(fresh.cursors.as_bytes(), reused.cursors.as_bytes());
    assert_eq!(fresh.clear_color, reused.clear_color);
}

#[test]
fn prepare_into_reuses_allocation() {
    let large_text: String = std::iter::repeat_n('A', 50).collect();
    let input = FrameInput::test_grid(10, 5, &large_text);
    let atlas = atlas_with(&['A']);

    // First prepare allocates large buffers.
    let mut frame = prepare_frame(&input, &atlas, (0.0, 0.0));
    let first_bg_count = frame.backgrounds.len();
    let first_fg_count = frame.glyphs.len();

    // Second prepare with smaller input reuses (clear + refill).
    let small = FrameInput::test_grid(2, 1, "A");
    prepare_frame_into(&small, &atlas, &mut frame, (0.0, 0.0));

    // Counts reflect new input, not old.
    assert_eq!(frame.backgrounds.len(), 2);
    assert_eq!(frame.glyphs.len(), 1);
    assert!(first_bg_count > frame.backgrounds.len());
    assert!(first_fg_count > frame.glyphs.len());
}

#[test]
fn prepare_into_clears_previous_content() {
    let input1 = FrameInput::test_grid(10, 5, "AAAAAAAAAA");
    let atlas = atlas_with(&['A', 'B']);

    let mut frame = prepare_frame(&input1, &atlas, (0.0, 0.0));
    let first_bg = frame.backgrounds.len();
    let first_fg = frame.glyphs.len();

    // Second frame with different content.
    let input2 = FrameInput::test_grid(2, 1, "B");
    prepare_frame_into(&input2, &atlas, &mut frame, (0.0, 0.0));

    // Counts should reflect the new input, not accumulate.
    assert_eq!(frame.backgrounds.len(), 2); // 2 cells
    assert_eq!(frame.glyphs.len(), 1); // 1 glyph ('B')
    assert_ne!(frame.backgrounds.len(), first_bg + 2);
    assert_ne!(frame.glyphs.len(), first_fg + 1);
}

#[test]
fn prepare_into_updates_clear_color() {
    let input1 = FrameInput::test_grid(2, 1, "");
    let atlas = empty_atlas();

    let mut frame = prepare_frame(&input1, &atlas, (0.0, 0.0));
    let first_clear = frame.clear_color;

    // Change palette background.
    let mut input2 = FrameInput::test_grid(2, 1, "");
    input2.palette.background = Rgb { r: 255, g: 0, b: 0 };
    prepare_frame_into(&input2, &atlas, &mut frame, (0.0, 0.0));

    assert_ne!(frame.clear_color, first_clear);
    assert_eq!(frame.clear_color, [1.0, 0.0, 0.0, 1.0]);
}

// ── Full-size grid instance counts (80×24) ──

#[test]
fn full_grid_all_spaces_1920_bg_zero_fg() {
    let input = FrameInput::test_grid(80, 24, "");
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.backgrounds.len(), 80 * 24);
    assert_eq!(frame.glyphs.len(), 0);
}

#[test]
fn full_grid_all_chars_1920_bg_and_fg() {
    let text: String = std::iter::repeat_n('A', 80 * 24).collect();
    let input = FrameInput::test_grid(80, 24, &text);
    let atlas = atlas_with(&['A']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.backgrounds.len(), 80 * 24);
    assert_eq!(frame.glyphs.len(), 80 * 24);
}

// ── Color resolution: bold, 256-color, truecolor ──

#[test]
fn bold_color_variant_in_instance_bytes() {
    // Bold cells: the extract phase resolves the bold color. The prepare phase
    // passes it through. Verify the bold flag affects glyph style selection and
    // that the fg_color in the instance matches what was set on the cell.
    let bright_red = Rgb {
        r: 255,
        g: 100,
        b: 100,
    };
    let mut input = FrameInput::test_grid(1, 1, "B");
    input.content.cells[0].flags = CellFlags::BOLD;
    input.content.cells[0].fg = bright_red;

    let mut map = HashMap::new();
    map.insert(('B', GlyphStyle::Bold), test_entry('B'));
    let atlas = TestAtlas(map);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.glyphs.len(), 1);
    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    assert_eq!(fg.fg_color, rgb_f32(bright_red));
}

#[test]
fn ansi_256_color_in_instance_bytes() {
    let color_208 = Rgb {
        r: 255,
        g: 135,
        b: 0,
    };
    let mut input = FrameInput::test_grid(1, 1, "X");
    input.content.cells[0].fg = color_208;

    let atlas = atlas_with(&['X']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    assert_eq!(fg.fg_color, rgb_f32(color_208));
}

#[test]
fn truecolor_in_instance_bytes() {
    let tc = Rgb {
        r: 100,
        g: 200,
        b: 50,
    };
    let mut input = FrameInput::test_grid(1, 1, "T");
    input.content.cells[0].fg = tc;
    input.content.cells[0].bg = Rgb {
        r: 30,
        g: 30,
        b: 30,
    };

    let atlas = atlas_with(&['T']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    assert_eq!(fg.fg_color, rgb_f32(tc));

    let bg = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(
        bg.bg_color,
        rgb_f32(Rgb {
            r: 30,
            g: 30,
            b: 30,
        }),
    );
}

// ── Viewport bounds ──

#[test]
fn no_instances_outside_grid_bounds() {
    // 3×2 grid at 8×16 cell size = 24×32 viewport.
    let input = FrameInput::test_grid(3, 2, "ABCDEF");
    let atlas = atlas_with(&['A', 'B', 'C', 'D', 'E', 'F']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let vp_w = 3.0 * 8.0; // 24.0
    let vp_h = 2.0 * 16.0; // 32.0

    // Verify all bg instances are within viewport.
    for i in 0..frame.backgrounds.len() {
        let inst = nth_instance(frame.backgrounds.as_bytes(), i);
        assert!(
            inst.pos.0 >= 0.0 && inst.pos.0 + inst.size.0 <= vp_w,
            "bg instance {i} x out of bounds: pos={}, size={}",
            inst.pos.0,
            inst.size.0,
        );
        assert!(
            inst.pos.1 >= 0.0 && inst.pos.1 + inst.size.1 <= vp_h,
            "bg instance {i} y out of bounds: pos={}, size={}",
            inst.pos.1,
            inst.size.1,
        );
    }
}

// ── Shaped rendering tests ──

/// Test atlas that looks up glyphs by [`RasterKey`] (shaped path).
struct KeyTestAtlas(HashMap<RasterKey, AtlasEntry>);

impl AtlasLookup for KeyTestAtlas {
    fn lookup(&self, _ch: char, _style: GlyphStyle) -> Option<&AtlasEntry> {
        None
    }

    fn lookup_key(&self, key: RasterKey) -> Option<&AtlasEntry> {
        self.0.get(&key)
    }
}

/// Create a deterministic atlas entry for a glyph ID.
fn test_entry_for_glyph(glyph_id: u16) -> AtlasEntry {
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

/// Build a `KeyTestAtlas` with entries for the given glyph IDs.
fn key_atlas_with(glyph_ids: &[u16], size_q6: u32) -> KeyTestAtlas {
    let mut map = HashMap::new();
    for &gid in glyph_ids {
        let key = RasterKey {
            glyph_id: gid,
            face_idx: FaceIdx::REGULAR,
            size_q6,
            synthetic: SyntheticFlags::NONE,
            hinted: true,
            subpx_x: 0,
            font_realm: FontRealm::Terminal,
        };
        map.insert(key, test_entry_for_glyph(gid));
    }
    KeyTestAtlas(map)
}

/// Build a ShapedFrame for a 1-row grid from a slice of ShapedGlyphs.
fn shaped_one_row(
    cols: usize,
    glyphs: &[ShapedGlyph],
    col_starts: &[usize],
    size_q6: u32,
) -> ShapedFrame {
    let mut sf = ShapedFrame::new(cols, size_q6);
    let mut col_map = Vec::new();
    crate::font::build_col_glyph_map(col_starts, cols, &mut col_map);
    sf.push_row(glyphs, col_starts, &col_map);
    sf
}

#[test]
fn shaped_single_glyph_one_bg_one_fg() {
    let size_q6 = 768; // 12px * 64
    let input = FrameInput::test_grid(3, 1, "A  ");
    let atlas = key_atlas_with(&[42], size_q6);

    let glyphs = vec![ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        synthetic: 0,
        x_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let col_starts = vec![0];
    let shaped = shaped_one_row(3, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    // 3 bg instances (one per cell), 1 fg instance (shaped glyph at col 0), 1 cursor.
    assert_counts(&frame, 3, 1, 1);
}

#[test]
fn shaped_ligature_one_fg_two_bg() {
    // Simulate a ligature spanning cols 0-1 (e.g. "fi" → single glyph).
    let size_q6 = 768;
    let mut input = FrameInput::test_grid(3, 1, "fi ");
    // Mark col 0 as the ligature origin, col 1 as regular (the shaper
    // handles the merge — bg instances come from the cell data).
    input.content.cells[0].ch = 'f';
    input.content.cells[1].ch = 'i';

    let atlas = key_atlas_with(&[100], size_q6);
    let glyphs = vec![ShapedGlyph {
        glyph_id: 100,
        face_index: 0,
        synthetic: 0,
        x_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let col_starts = vec![0]; // ligature starts at col 0, spans 2 columns via col_map
    let shaped = shaped_one_row(3, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    // 3 bg (per-cell), 1 fg (single ligature glyph at col 0), 1 cursor.
    assert_counts(&frame, 3, 1, 1);

    // The fg glyph should be at col 0 position.
    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    let entry = test_entry_for_glyph(100);
    assert_eq!(fg.pos.0, 0.0 + entry.bearing_x as f32);
}

#[test]
fn shaped_combining_marks_two_fg_instances() {
    // Base glyph at col 0 + combining mark at col 0 → 2 fg instances.
    let size_q6 = 768;
    let input = FrameInput::test_grid(2, 1, "a ");
    let atlas = key_atlas_with(&[50, 51], size_q6);

    let glyphs = vec![
        ShapedGlyph {
            glyph_id: 50,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 51,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 2.0,
            y_offset: 3.0,
        },
    ];
    let col_starts = vec![0, 0]; // both at col 0 — combining mark
    let shaped = shaped_one_row(2, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    // 2 bg (per-cell), 2 fg (base + combining mark), 1 cursor.
    assert_counts(&frame, 2, 2, 1);
}

#[test]
fn shaped_offset_applied_to_glyph_position() {
    use crate::font::{subpx_bin, subpx_offset};

    let size_q6 = 768;
    let input = FrameInput::test_grid(1, 1, "X");

    // x_offset 1.5 → fract 0.5 → subpx phase 2.
    let subpx = subpx_bin(1.5);
    let mut map = HashMap::new();
    map.insert(
        RasterKey {
            glyph_id: 60,
            face_idx: FaceIdx::REGULAR,
            size_q6,
            synthetic: SyntheticFlags::NONE,
            hinted: true,
            subpx_x: subpx,
            font_realm: FontRealm::Terminal,
        },
        test_entry_for_glyph(60),
    );
    let atlas = KeyTestAtlas(map);

    let glyphs = vec![ShapedGlyph {
        glyph_id: 60,
        face_index: 0,
        synthetic: 0,
        x_advance: 0.0,
        x_offset: 1.5,
        y_offset: 2.0,
    }];
    let col_starts = vec![0];
    let shaped = shaped_one_row(1, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    assert_eq!(frame.glyphs.len(), 1);
    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    let entry = test_entry_for_glyph(60);

    // glyph_x = 0.0 + bearing_x(1) + x_offset(1.5) - absorbed(0.5) = 2.0
    let absorbed = subpx_offset(subpx);
    let expected_x = 0.0 + entry.bearing_x as f32 + 1.5 - absorbed;
    // glyph_y = 0.0 + baseline(12.0) - bearing_y(12) - y_offset(2.0) = -2.0
    let expected_y = 0.0 + 12.0 - entry.bearing_y as f32 - 2.0;
    assert_eq!(fg.pos, (expected_x, expected_y));
}

#[test]
fn shaped_backgrounds_independent_of_glyphs() {
    // Backgrounds should be per-cell regardless of shaped glyph layout.
    let size_q6 = 768;
    let input = FrameInput::test_grid(4, 1, "ABCD");
    // Ligature spans cols 0-1, normal glyphs at 2 and 3.
    let atlas = key_atlas_with(&[100, 101, 102], size_q6);
    let glyphs = vec![
        ShapedGlyph {
            glyph_id: 100,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 101,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 102,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];
    let col_starts = vec![0, 2, 3];
    let shaped = shaped_one_row(4, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    // 4 bg instances (one per cell), 3 fg instances (ligature + 2 normal).
    assert_counts(&frame, 4, 3, 1);

    // Each bg is cell_width × cell_height at the correct position.
    for i in 0..4 {
        let bg = nth_instance(frame.backgrounds.as_bytes(), i);
        assert_eq!(bg.size, (8.0, 16.0), "bg {i} should be cell-sized");
        assert_eq!(bg.pos.0, i as f32 * 8.0, "bg {i} x position");
    }
}

#[test]
fn shaped_missing_glyph_in_atlas_skips_fg() {
    // Shaped glyph exists but atlas doesn't have it → no fg instance.
    let size_q6 = 768;
    let input = FrameInput::test_grid(1, 1, "X");
    let atlas = KeyTestAtlas(HashMap::new()); // empty atlas

    let glyphs = vec![ShapedGlyph {
        glyph_id: 99,
        face_index: 0,
        synthetic: 0,
        x_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let col_starts = vec![0];
    let shaped = shaped_one_row(1, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    // 1 bg, 0 fg (atlas miss), 1 cursor.
    assert_counts(&frame, 1, 0, 1);
}

#[test]
fn shaped_empty_glyphs_produces_bg_only() {
    // All cells are spaces → no shaped glyphs → bg only.
    let size_q6 = 768;
    let input = FrameInput::test_grid(3, 1, "   ");
    let atlas = KeyTestAtlas(HashMap::new());

    let shaped = ShapedFrame::new(3, size_q6);
    // Push an empty row (no glyphs).
    let empty_glyphs: Vec<ShapedGlyph> = Vec::new();
    let empty_col_starts: Vec<usize> = Vec::new();
    let mut col_map = Vec::new();
    crate::font::build_col_glyph_map(&empty_col_starts, 3, &mut col_map);

    let mut sf = shaped;
    sf.push_row(&empty_glyphs, &empty_col_starts, &col_map);
    let frame = prepare_frame_shaped(&input, &atlas, &sf, (0.0, 0.0));

    assert_counts(&frame, 3, 0, 1);
}

// ── Color glyph routing (Section 6.10) ──

#[test]
fn color_glyph_routes_to_color_glyphs_buffer() {
    // A shaped glyph with AtlasKind::Color should go to frame.color_glyphs,
    // not frame.glyphs.
    let size_q6 = 768;
    let input = FrameInput::test_grid(1, 1, "E"); // emoji placeholder

    let mut map = HashMap::new();
    let key = RasterKey {
        glyph_id: 200,
        face_idx: FaceIdx::REGULAR,
        size_q6,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
        subpx_x: 0,
        font_realm: FontRealm::Terminal,
    };
    map.insert(
        key,
        AtlasEntry {
            page: 0,
            uv_x: 0.1,
            uv_y: 0.2,
            uv_w: 0.05,
            uv_h: 0.05,
            width: 14,
            height: 14,
            bearing_x: 0,
            bearing_y: 12,
            kind: AtlasKind::Color, // Color emoji!
        },
    );
    let atlas = KeyTestAtlas(map);

    let glyphs = vec![ShapedGlyph {
        glyph_id: 200,
        face_index: 0,
        synthetic: 0,
        x_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let col_starts = vec![0];
    let shaped = shaped_one_row(1, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    // Monochrome glyphs should be empty — the color glyph went to color_glyphs.
    assert_eq!(
        frame.glyphs.len(),
        0,
        "color glyph should NOT be in monochrome buffer"
    );
    assert_eq!(
        frame.color_glyphs.len(),
        1,
        "color glyph should be in color buffer"
    );
}

#[test]
fn mixed_color_and_mono_glyphs_route_correctly() {
    // Mix of monochrome and color glyphs in the same row.
    let size_q6 = 768;
    let input = FrameInput::test_grid(3, 1, "AEB");

    let mut map = HashMap::new();
    // Mono glyph 'A' at col 0.
    map.insert(
        RasterKey {
            glyph_id: 10,
            face_idx: FaceIdx::REGULAR,
            size_q6,
            synthetic: SyntheticFlags::NONE,
            hinted: true,
            subpx_x: 0,
            font_realm: FontRealm::Terminal,
        },
        test_entry_for_glyph(10),
    );
    // Color emoji 'E' at col 1.
    map.insert(
        RasterKey {
            glyph_id: 200,
            face_idx: FaceIdx::REGULAR,
            size_q6,
            synthetic: SyntheticFlags::NONE,
            hinted: true,
            subpx_x: 0,
            font_realm: FontRealm::Terminal,
        },
        AtlasEntry {
            kind: AtlasKind::Color,
            ..test_entry_for_glyph(200)
        },
    );
    // Mono glyph 'B' at col 2.
    map.insert(
        RasterKey {
            glyph_id: 11,
            face_idx: FaceIdx::REGULAR,
            size_q6,
            synthetic: SyntheticFlags::NONE,
            hinted: true,
            subpx_x: 0,
            font_realm: FontRealm::Terminal,
        },
        test_entry_for_glyph(11),
    );
    let atlas = KeyTestAtlas(map);

    let glyphs = vec![
        ShapedGlyph {
            glyph_id: 10,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 200,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 11,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];
    let col_starts = vec![0, 1, 2];
    let shaped = shaped_one_row(3, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    assert_eq!(frame.glyphs.len(), 2, "2 mono glyphs in monochrome buffer");
    assert_eq!(frame.color_glyphs.len(), 1, "1 color glyph in color buffer");
    assert_eq!(frame.backgrounds.len(), 3, "3 backgrounds (one per cell)");
}

// ── prepare_frame_shaped_into ──

#[test]
fn shaped_into_matches_shaped() {
    let size_q6 = 768;
    let input = FrameInput::test_grid(4, 1, "ABCD");
    let atlas = key_atlas_with(&[100, 101, 102], size_q6);
    let glyphs = vec![
        ShapedGlyph {
            glyph_id: 100,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 101,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 102,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];
    let col_starts = vec![0, 2, 3];
    let shaped = shaped_one_row(4, &glyphs, &col_starts, size_q6);

    let fresh = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    let mut reused = PreparedFrame::new(ViewportSize::new(1, 1), Rgb { r: 0, g: 0, b: 0 }, 1.0);
    prepare_frame_shaped_into(&input, &atlas, &shaped, &mut reused, (0.0, 0.0), true);

    assert_eq!(fresh.backgrounds.as_bytes(), reused.backgrounds.as_bytes());
    assert_eq!(fresh.glyphs.as_bytes(), reused.glyphs.as_bytes());
    assert_eq!(fresh.cursors.as_bytes(), reused.cursors.as_bytes());
    assert_eq!(fresh.clear_color, reused.clear_color);
    assert_eq!(fresh.viewport, reused.viewport);
}

#[test]
fn shaped_into_reuses_allocation() {
    let size_q6 = 768;
    let large_text: String = std::iter::repeat_n('A', 50).collect();
    let input = FrameInput::test_grid(10, 5, &large_text);

    // Build shaped data for 50 glyphs.
    let glyphs: Vec<ShapedGlyph> = (0..50)
        .map(|_| ShapedGlyph {
            glyph_id: 42,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        })
        .collect();
    let col_starts: Vec<usize> = (0..50).map(|i| i % 10).collect();
    let atlas = key_atlas_with(&[42], size_q6);

    // Build shaped frame with all 5 rows.
    let mut sf = ShapedFrame::new(10, size_q6);
    for row_start in (0..50).step_by(10) {
        let row_glyphs = &glyphs[row_start..row_start + 10];
        let row_col_starts = &col_starts[row_start..row_start + 10];
        let mut col_map = Vec::new();
        crate::font::build_col_glyph_map(row_col_starts, 10, &mut col_map);
        sf.push_row(row_glyphs, row_col_starts, &col_map);
    }

    // First prepare.
    let mut frame = prepare_frame_shaped(&input, &atlas, &sf, (0.0, 0.0));
    let first_bg = frame.backgrounds.len();
    let first_fg = frame.glyphs.len();

    // Second prepare with smaller input reuses allocations.
    let small = FrameInput::test_grid(2, 1, "A ");
    let small_glyphs = vec![ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        synthetic: 0,
        x_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let small_col_starts = vec![0];
    let small_shaped = shaped_one_row(2, &small_glyphs, &small_col_starts, size_q6);
    prepare_frame_shaped_into(&small, &atlas, &small_shaped, &mut frame, (0.0, 0.0), true);

    assert_eq!(frame.backgrounds.len(), 2);
    assert!(first_bg > frame.backgrounds.len());
    assert!(first_fg > frame.glyphs.len());
}

// ── Text decoration tests (Section 6.12) ──

/// Build a 1×1 test grid with the given flags on cell 0.
fn frame_with_flags(flags: CellFlags) -> FrameInput {
    let mut input = FrameInput::test_grid(1, 1, "A");
    input.content.cells[0].flags = flags;
    input
}

/// Build a 1×1 test grid with flags and an explicit underline color.
fn frame_with_underline_color(flags: CellFlags, color: Rgb) -> FrameInput {
    let mut input = FrameInput::test_grid(1, 1, "A");
    input.content.cells[0].flags = flags;
    input.content.cells[0].underline_color = Some(color);
    input
}

/// Count background instances beyond the 1 base background rect per cell.
///
/// In a 1×1 grid, the first bg instance is always the cell background.
/// Any additional instances come from decorations.
fn decoration_bg_count(frame: &PreparedFrame) -> usize {
    frame.backgrounds.len() - 1
}

#[test]
fn single_underline_one_extra_bg() {
    let input = frame_with_flags(CellFlags::UNDERLINE);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // 1 base bg + 1 underline rect.
    assert_eq!(decoration_bg_count(&frame), 1);

    // Underline Y = y + cell_height - 2.0 = 0 + 16 - 2 = 14.
    let ul = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(ul.pos.1, 14.0);
    assert_eq!(ul.size, (8.0, 1.0));
}

#[test]
fn single_underline_uses_fg_color() {
    let input = frame_with_flags(CellFlags::UNDERLINE);
    let fg = input.content.cells[0].fg;
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let ul = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(ul.bg_color, rgb_f32(fg));
}

#[test]
fn single_underline_uses_sgr58_color() {
    let sgr58 = Rgb {
        r: 255,
        g: 0,
        b: 128,
    };
    let input = frame_with_underline_color(CellFlags::UNDERLINE, sgr58);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let ul = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(ul.bg_color, rgb_f32(sgr58));
}

#[test]
fn double_underline_two_extra_bgs() {
    let input = frame_with_flags(CellFlags::DOUBLE_UNDERLINE);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // 1 base bg + 2 underline rects.
    assert_eq!(decoration_bg_count(&frame), 2);

    // First line at underline_y = 14, second at underline_y - 2 = 12.
    let ul1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(ul1.pos.1, 14.0);
    assert_eq!(ul1.size, (8.0, 1.0));

    let ul2 = nth_instance(frame.backgrounds.as_bytes(), 2);
    assert_eq!(ul2.pos.1, 12.0);
    assert_eq!(ul2.size, (8.0, 1.0));
}

#[test]
fn curly_underline_per_pixel_rects() {
    let input = frame_with_flags(CellFlags::CURLY_UNDERLINE);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // cell_width=8 → 8 per-pixel rects.
    assert_eq!(decoration_bg_count(&frame), 8);
}

#[test]
fn dotted_underline_alternating() {
    let input = frame_with_flags(CellFlags::DOTTED_UNDERLINE);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // cell_width=8, step_by(2) → 4 dots (at 0, 2, 4, 6).
    assert_eq!(decoration_bg_count(&frame), 4);
}

#[test]
fn dashed_underline_pattern() {
    let input = frame_with_flags(CellFlags::DASHED_UNDERLINE);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // cell_width=8, pattern 3-on-2-off: dx 0,1,2 on, 3,4 off, 5,6,7 on → 6.
    assert_eq!(decoration_bg_count(&frame), 6);
}

#[test]
fn strikethrough_at_center() {
    let input = frame_with_flags(CellFlags::STRIKETHROUGH);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // 1 base bg + 1 strikethrough rect.
    assert_eq!(decoration_bg_count(&frame), 1);

    // Strikethrough Y = y + cell_height / 2.0 = 0 + 8.0.
    let st = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(st.pos.1, 8.0);
    assert_eq!(st.size, (8.0, 1.0));
}

#[test]
fn strikethrough_uses_fg_color() {
    let input = frame_with_flags(CellFlags::STRIKETHROUGH);
    let fg = input.content.cells[0].fg;
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let st = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(st.bg_color, rgb_f32(fg));
}

#[test]
fn underline_and_strikethrough_coexist() {
    let input = frame_with_flags(CellFlags::UNDERLINE | CellFlags::STRIKETHROUGH);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // 1 base bg + 1 underline + 1 strikethrough = 2 decoration rects.
    assert_eq!(decoration_bg_count(&frame), 2);
}

#[test]
fn no_flags_no_decorations() {
    let input = frame_with_flags(CellFlags::empty());
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // 1 base bg only, no decorations.
    assert_eq!(decoration_bg_count(&frame), 0);
}

#[test]
fn wide_char_underline_spans_double_width() {
    let mut input = FrameInput::test_grid(4, 1, "");
    // Wide char at col 0.
    input.content.cells[0].ch = '\u{4E16}';
    input.content.cells[0].flags = CellFlags::WIDE_CHAR | CellFlags::UNDERLINE;
    input.content.cells[1].ch = ' ';
    input.content.cells[1].flags = CellFlags::WIDE_CHAR_SPACER;

    let atlas = atlas_with(&['\u{4E16}']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Find the underline rect (second bg instance for the wide char cell).
    let ul = nth_instance(frame.backgrounds.as_bytes(), 1);
    // Wide char bg_w = 2 * cell_width = 16.0, underline should match.
    assert_eq!(ul.size.0, 16.0);
    assert_eq!(ul.size.1, 1.0);
}

// ── Subpixel glyph routing (Section 6.16) ──

#[test]
fn subpixel_glyph_routes_to_subpixel_buffer() {
    // A shaped glyph with AtlasKind::Subpixel should go to frame.subpixel_glyphs,
    // not frame.glyphs (mono) or frame.color_glyphs.
    let size_q6 = 768;
    let input = FrameInput::test_grid(1, 1, "A");

    let mut map = HashMap::new();
    let key = RasterKey {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        size_q6,
        synthetic: SyntheticFlags::NONE,
        hinted: true,
        subpx_x: 0,
        font_realm: FontRealm::Terminal,
    };
    map.insert(
        key,
        AtlasEntry {
            kind: AtlasKind::Subpixel,
            ..test_entry_for_glyph(42)
        },
    );
    let atlas = KeyTestAtlas(map);

    let glyphs = vec![ShapedGlyph {
        glyph_id: 42,
        face_index: 0,
        synthetic: 0,
        x_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let col_starts = vec![0];
    let shaped = shaped_one_row(1, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    assert_eq!(
        frame.glyphs.len(),
        0,
        "subpixel glyph should NOT be in monochrome buffer",
    );
    assert_eq!(
        frame.subpixel_glyphs.len(),
        1,
        "subpixel glyph should be in subpixel buffer",
    );
    assert_eq!(
        frame.color_glyphs.len(),
        0,
        "subpixel glyph should NOT be in color buffer",
    );
}

#[test]
fn mixed_mono_subpixel_color_route_to_separate_buffers() {
    // Three glyphs, one per atlas kind, all route to their correct buffers.
    let size_q6 = 768;
    let input = FrameInput::test_grid(3, 1, "ABC");

    let mut map = HashMap::new();
    // Mono glyph.
    map.insert(
        RasterKey {
            glyph_id: 10,
            face_idx: FaceIdx::REGULAR,
            size_q6,
            synthetic: SyntheticFlags::NONE,
            hinted: true,
            subpx_x: 0,
            font_realm: FontRealm::Terminal,
        },
        test_entry_for_glyph(10), // default: AtlasKind::Mono
    );
    // Subpixel glyph.
    map.insert(
        RasterKey {
            glyph_id: 20,
            face_idx: FaceIdx::REGULAR,
            size_q6,
            synthetic: SyntheticFlags::NONE,
            hinted: true,
            subpx_x: 0,
            font_realm: FontRealm::Terminal,
        },
        AtlasEntry {
            kind: AtlasKind::Subpixel,
            ..test_entry_for_glyph(20)
        },
    );
    // Color glyph.
    map.insert(
        RasterKey {
            glyph_id: 30,
            face_idx: FaceIdx::REGULAR,
            size_q6,
            synthetic: SyntheticFlags::NONE,
            hinted: true,
            subpx_x: 0,
            font_realm: FontRealm::Terminal,
        },
        AtlasEntry {
            kind: AtlasKind::Color,
            ..test_entry_for_glyph(30)
        },
    );
    let atlas = KeyTestAtlas(map);

    let glyphs = vec![
        ShapedGlyph {
            glyph_id: 10,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 20,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 30,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];
    let col_starts = vec![0, 1, 2];
    let shaped = shaped_one_row(3, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    assert_eq!(frame.glyphs.len(), 1, "1 mono glyph");
    assert_eq!(frame.subpixel_glyphs.len(), 1, "1 subpixel glyph");
    assert_eq!(frame.color_glyphs.len(), 1, "1 color glyph");
}

// ── Async resize guard tests ──

#[test]
fn shaped_frame_smaller_than_viewport_skips_excess_cells() {
    // Shaped frame has 2 cols, but viewport grid has 4 cols.
    // Cells beyond shaped.cols() should produce bg but no fg panic.
    let size_q6 = 768;
    let input = FrameInput::test_grid(4, 1, "ABCD");

    // Atlas has entries for glyph IDs used in the shaped frame.
    let atlas = key_atlas_with(&[10, 11], size_q6);

    // Shaped frame only covers 2 columns (not 4).
    let glyphs = vec![
        ShapedGlyph {
            glyph_id: 10,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 11,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];
    let col_starts = vec![0, 1];
    let shaped = shaped_one_row(2, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    // All 4 cells produce backgrounds.
    assert_eq!(frame.backgrounds.len(), 4);
    // Only 2 shaped glyphs (cols 2-3 skipped by the resize guard).
    assert_eq!(frame.glyphs.len(), 2);
}

#[test]
fn shaped_frame_fewer_rows_than_viewport_skips_excess_rows() {
    // Viewport has 3 rows, shaped frame has 1 row.
    let size_q6 = 768;
    let input = FrameInput::test_grid(2, 3, "AB    ");

    let atlas = key_atlas_with(&[10], size_q6);

    // Only 1 row in the shaped frame.
    let glyphs = vec![ShapedGlyph {
        glyph_id: 10,
        face_index: 0,
        synthetic: 0,
        x_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let col_starts = vec![0];
    let shaped = shaped_one_row(2, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    // 6 backgrounds (2 cols * 3 rows).
    assert_eq!(frame.backgrounds.len(), 6);
    // Only 1 glyph (from row 0); rows 1-2 skipped by guard.
    assert_eq!(frame.glyphs.len(), 1);
}

#[test]
fn shaped_frame_larger_than_viewport_no_panic() {
    // Shaped frame has more data than the viewport — should not panic,
    // only viewport cells get iterated.
    let size_q6 = 768;
    let input = FrameInput::test_grid(2, 1, "AB");

    // Atlas has both glyph IDs.
    let atlas = key_atlas_with(&[10, 11, 12, 13], size_q6);

    // Shaped frame has 4 columns (more than viewport's 2).
    let glyphs = vec![
        ShapedGlyph {
            glyph_id: 10,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 11,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 12,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 13,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];
    let col_starts = vec![0, 1, 2, 3];
    let shaped = shaped_one_row(4, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    // Only 2 backgrounds (viewport is 2×1).
    assert_eq!(frame.backgrounds.len(), 2);
    // Only 2 glyphs (viewport cols 0 and 1).
    assert_eq!(frame.glyphs.len(), 2);
}

// ── Origin offset tests (Section 07.11) ──

#[test]
fn origin_offset_shifts_bg_positions() {
    let input = FrameInput::test_grid(2, 1, "AB");
    let atlas = atlas_with(&['A', 'B']);

    let frame = prepare_frame(&input, &atlas, (10.0, 20.0));

    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg0.pos, (10.0, 20.0));

    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(bg1.pos, (18.0, 20.0)); // 10.0 + 1*8.0
}

#[test]
fn origin_offset_shifts_glyph_positions() {
    let input = FrameInput::test_grid(1, 1, "A");
    let atlas = atlas_with(&['A']);
    let entry = test_entry('A');

    let frame = prepare_frame(&input, &atlas, (5.0, 15.0));

    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    // glyph_x = 5.0 + 0*8 + bearing_x(1) = 6.0
    // glyph_y = 15.0 + 0*16 + baseline(12.0) - bearing_y(12) = 15.0
    assert_eq!(fg.pos, (5.0 + entry.bearing_x as f32, 15.0));
}

#[test]
fn origin_offset_shifts_cursor_position() {
    let mut input = FrameInput::test_grid(10, 5, "");
    input.content.cursor.column = Column(2);
    input.content.cursor.line = 3;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (30.0, 50.0));

    let c = nth_instance(frame.cursors.as_bytes(), 0);
    // x = 30.0 + 2*8 = 46.0, y = 50.0 + 3*16 = 98.0
    assert_eq!(c.pos, (46.0, 98.0));
}

#[test]
fn zero_origin_matches_no_origin() {
    let input = FrameInput::test_grid(3, 2, "ABCDEF");
    let atlas = atlas_with(&['A', 'B', 'C', 'D', 'E', 'F']);

    // Default origin is (0.0, 0.0).
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg0.pos, (0.0, 0.0));

    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(bg1.pos, (8.0, 0.0));
}

#[test]
fn origin_offset_shaped_shifts_all_instances() {
    let size_q6 = 768;
    let input = FrameInput::test_grid(2, 1, "AB");

    let atlas = key_atlas_with(&[10, 11], size_q6);
    let glyphs = vec![
        ShapedGlyph {
            glyph_id: 10,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 11,
            face_index: 0,
            synthetic: 0,
            x_advance: 0.0,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];
    let col_starts = vec![0, 1];
    let shaped = shaped_one_row(2, &glyphs, &col_starts, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (100.0, 200.0));

    // Backgrounds shifted by origin.
    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg0.pos, (100.0, 200.0));

    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(bg1.pos, (108.0, 200.0)); // 100 + 1*8

    // Cursor shifted by origin.
    let c = nth_instance(frame.cursors.as_bytes(), 0);
    assert_eq!(c.pos, (100.0, 200.0));
}

// ── Selection rendering ──

/// Helper: create a FrameSelection covering columns `start_col..=end_col` on
/// viewport line `line`. Uses `stable_row_base = 0` so stable row == viewport line.
fn selection_range(line: usize, start_col: usize, end_col: usize) -> FrameSelection {
    let anchor = oriterm_core::SelectionPoint {
        row: StableRowIndex(line as u64),
        col: start_col,
        side: Side::Left,
    };
    let end = oriterm_core::SelectionPoint {
        row: StableRowIndex(line as u64),
        col: end_col,
        side: Side::Right,
    };
    let sel = Selection::new_char(anchor.row, anchor.col, Side::Left);
    // Build a selection spanning the range by constructing bounds directly.
    let mut sel = sel;
    sel.end = end;
    FrameSelection::new(&sel, 0)
}

#[test]
fn selection_inverts_bg_color() {
    let mut input = FrameInput::test_grid(3, 1, "ABC");
    let atlas = atlas_with(&['A', 'B', 'C']);

    // Select column 1 ("B").
    input.selection = Some(selection_range(0, 1, 1));

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // 3 bg instances: col 0 (normal), col 1 (selected), col 2 (normal).
    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    let bg2 = nth_instance(frame.backgrounds.as_bytes(), 2);

    let normal_bg = rgb_f32(Rgb { r: 0, g: 0, b: 0 });
    let selected_bg = rgb_f32(Rgb {
        r: 211,
        g: 215,
        b: 207,
    });

    assert_eq!(bg0.bg_color, normal_bg, "col 0 should be normal bg");
    assert_eq!(bg1.bg_color, selected_bg, "col 1 should have inverted bg");
    assert_eq!(bg2.bg_color, normal_bg, "col 2 should be normal bg");
}

#[test]
fn selection_inverts_fg_color() {
    let mut input = FrameInput::test_grid(2, 1, "AB");
    let atlas = atlas_with(&['A', 'B']);

    // Hide cursor so block cursor exclusion doesn't interfere.
    input.content.cursor.visible = false;

    // Select column 0 ("A").
    input.selection = Some(selection_range(0, 0, 0));

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Glyph "A" (col 0) should have inverted fg (black instead of light gray).
    let fg0 = nth_instance(frame.glyphs.as_bytes(), 0);
    let selected_fg = rgb_f32(Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(
        fg0.fg_color, selected_fg,
        "selected glyph should have inverted fg"
    );

    // Glyph "B" (col 1) should have normal fg.
    let fg1 = nth_instance(frame.glyphs.as_bytes(), 1);
    let normal_fg = rgb_f32(Rgb {
        r: 211,
        g: 215,
        b: 207,
    });
    assert_eq!(
        fg1.fg_color, normal_fg,
        "unselected glyph should have normal fg"
    );
}

#[test]
fn selection_no_effect_when_none() {
    let input = FrameInput::test_grid(2, 1, "AB");
    let atlas = atlas_with(&['A', 'B']);

    // No selection.
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    let normal_bg = rgb_f32(Rgb { r: 0, g: 0, b: 0 });

    assert_eq!(bg0.bg_color, normal_bg);
    assert_eq!(bg1.bg_color, normal_bg);
}

#[test]
fn selection_wide_char_highlights_both_cells() {
    use oriterm_core::RenderableCell;

    // Build a grid with a wide char at col 0: 'Ａ' (fullwidth A, 2 cells wide).
    let fg = Rgb {
        r: 211,
        g: 215,
        b: 207,
    };
    let bg = Rgb { r: 0, g: 0, b: 0 };

    let cells = vec![
        RenderableCell {
            line: 0,
            column: Column(0),
            ch: 'Ａ',
            fg,
            bg,
            flags: CellFlags::WIDE_CHAR,
            underline_color: None,
            has_hyperlink: false,
            zerowidth: Vec::new(),
        },
        RenderableCell {
            line: 0,
            column: Column(1),
            ch: ' ',
            fg,
            bg,
            flags: CellFlags::WIDE_CHAR_SPACER,
            underline_color: None,
            has_hyperlink: false,
            zerowidth: Vec::new(),
        },
        RenderableCell {
            line: 0,
            column: Column(2),
            ch: 'B',
            fg,
            bg,
            flags: CellFlags::empty(),
            underline_color: None,
            has_hyperlink: false,
            zerowidth: Vec::new(),
        },
    ];

    let mut input = FrameInput::test_grid(3, 1, "");
    input.content.cells = cells;
    input.content.cursor.visible = false;

    // Select just col 0 (the wide char base cell).
    input.selection = Some(selection_range(0, 0, 0));

    let atlas = atlas_with(&['Ａ', 'B']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Wide char spacers are skipped, so we get 2 bg instances:
    // bg[0] = wide char (2 cells wide, selected), bg[1] = 'B' (normal).
    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);

    let selected_bg = rgb_f32(Rgb {
        r: 211,
        g: 215,
        b: 207,
    });
    let normal_bg = rgb_f32(Rgb { r: 0, g: 0, b: 0 });

    assert_eq!(bg0.bg_color, selected_bg, "wide char should be selected");
    assert_eq!(bg0.size, (16.0, 16.0), "wide char bg should span 2 cells");
    assert_eq!(bg1.bg_color, normal_bg, "'B' should be normal");
}

#[test]
fn selection_block_mode_rectangular() {
    use oriterm_core::SelectionPoint;

    // 4x2 grid: "ABCD" / "EFGH". Block select cols 1..2, rows 0..1.
    let mut input = FrameInput::test_grid(4, 2, "ABCDEFGH");
    let atlas = atlas_with(&['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H']);

    let anchor = SelectionPoint {
        row: StableRowIndex(0),
        col: 1,
        side: Side::Left,
    };
    let pivot = SelectionPoint {
        row: StableRowIndex(0),
        col: 1,
        side: Side::Left,
    };
    let mut sel = Selection::new_word(anchor, pivot);
    sel.mode = oriterm_core::SelectionMode::Block;
    sel.end = SelectionPoint {
        row: StableRowIndex(1),
        col: 2,
        side: Side::Right,
    };
    input.selection = Some(FrameSelection::new(&sel, 0));

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let selected_bg = rgb_f32(Rgb {
        r: 211,
        g: 215,
        b: 207,
    });
    let normal_bg = rgb_f32(Rgb { r: 0, g: 0, b: 0 });

    // Row 0: A(normal) B(selected) C(selected) D(normal).
    let a = nth_instance(frame.backgrounds.as_bytes(), 0);
    let b = nth_instance(frame.backgrounds.as_bytes(), 1);
    let c = nth_instance(frame.backgrounds.as_bytes(), 2);
    let d = nth_instance(frame.backgrounds.as_bytes(), 3);

    assert_eq!(a.bg_color, normal_bg, "A should be normal");
    assert_eq!(b.bg_color, selected_bg, "B should be selected");
    assert_eq!(c.bg_color, selected_bg, "C should be selected");
    assert_eq!(d.bg_color, normal_bg, "D should be normal");

    // Row 1: E(normal) F(selected) G(selected) H(normal).
    let e = nth_instance(frame.backgrounds.as_bytes(), 4);
    let f = nth_instance(frame.backgrounds.as_bytes(), 5);
    let g = nth_instance(frame.backgrounds.as_bytes(), 6);
    let h = nth_instance(frame.backgrounds.as_bytes(), 7);

    assert_eq!(e.bg_color, normal_bg, "E should be normal");
    assert_eq!(f.bg_color, selected_bg, "F should be selected");
    assert_eq!(g.bg_color, selected_bg, "G should be selected");
    assert_eq!(h.bg_color, normal_bg, "H should be normal");
}

#[test]
fn selection_wide_char_spacer_only_highlights_both() {
    use oriterm_core::RenderableCell;

    // Wide char at col 0, spacer at col 1, narrow 'B' at col 2.
    // Selection covers only col 1 (the spacer). The wide char should
    // still be highlighted because you can't render half a wide char.
    let fg = Rgb {
        r: 211,
        g: 215,
        b: 207,
    };
    let bg = Rgb { r: 0, g: 0, b: 0 };

    let cells = vec![
        RenderableCell {
            line: 0,
            column: Column(0),
            ch: 'Ａ',
            fg,
            bg,
            flags: CellFlags::WIDE_CHAR,
            underline_color: None,
            has_hyperlink: false,
            zerowidth: Vec::new(),
        },
        RenderableCell {
            line: 0,
            column: Column(1),
            ch: ' ',
            fg,
            bg,
            flags: CellFlags::WIDE_CHAR_SPACER,
            underline_color: None,
            has_hyperlink: false,
            zerowidth: Vec::new(),
        },
        RenderableCell {
            line: 0,
            column: Column(2),
            ch: 'B',
            fg,
            bg,
            flags: CellFlags::empty(),
            underline_color: None,
            has_hyperlink: false,
            zerowidth: Vec::new(),
        },
    ];

    let mut input = FrameInput::test_grid(3, 1, "");
    input.content.cells = cells;
    input.content.cursor.visible = false;

    // Select only col 1 (the spacer column).
    input.selection = Some(selection_range(0, 1, 1));

    let atlas = atlas_with(&['Ａ', 'B']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let selected_bg = rgb_f32(Rgb {
        r: 211,
        g: 215,
        b: 207,
    });
    let normal_bg = rgb_f32(Rgb { r: 0, g: 0, b: 0 });

    // bg[0] = wide char (should be selected because spacer col is in range).
    // bg[1] = 'B' (normal).
    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);

    assert_eq!(
        bg0.bg_color, selected_bg,
        "wide char should be selected via spacer"
    );
    assert_eq!(bg1.bg_color, normal_bg, "'B' should be normal");
}

#[test]
fn selection_across_wrapped_lines_no_gap() {
    // Two rows, selection spans from row 0 col 2 to row 1 col 1.
    // All cells from col 2 on row 0 and cols 0..1 on row 1 should be selected.
    let mut input = FrameInput::test_grid(4, 2, "ABCDEFGH");
    let atlas = atlas_with(&['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H']);

    let anchor = oriterm_core::SelectionPoint {
        row: StableRowIndex(0),
        col: 2,
        side: Side::Left,
    };
    let sel = Selection::new_char(anchor.row, anchor.col, Side::Left);
    let mut sel = sel;
    sel.end = oriterm_core::SelectionPoint {
        row: StableRowIndex(1),
        col: 1,
        side: Side::Right,
    };
    input.selection = Some(FrameSelection::new(&sel, 0));

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let selected_bg = rgb_f32(Rgb {
        r: 211,
        g: 215,
        b: 207,
    });
    let normal_bg = rgb_f32(Rgb { r: 0, g: 0, b: 0 });

    // Row 0: A(norm) B(norm) C(sel) D(sel).
    let a = nth_instance(frame.backgrounds.as_bytes(), 0);
    let b = nth_instance(frame.backgrounds.as_bytes(), 1);
    let c = nth_instance(frame.backgrounds.as_bytes(), 2);
    let d = nth_instance(frame.backgrounds.as_bytes(), 3);
    assert_eq!(a.bg_color, normal_bg, "A should be normal");
    assert_eq!(b.bg_color, normal_bg, "B should be normal");
    assert_eq!(c.bg_color, selected_bg, "C should be selected");
    assert_eq!(d.bg_color, selected_bg, "D should be selected");

    // Row 1: E(sel) F(sel) G(norm) H(norm).
    let e = nth_instance(frame.backgrounds.as_bytes(), 4);
    let f = nth_instance(frame.backgrounds.as_bytes(), 5);
    let g = nth_instance(frame.backgrounds.as_bytes(), 6);
    let h = nth_instance(frame.backgrounds.as_bytes(), 7);
    assert_eq!(
        e.bg_color, selected_bg,
        "E should be selected (wrap continues)"
    );
    assert_eq!(f.bg_color, selected_bg, "F should be selected");
    assert_eq!(g.bg_color, normal_bg, "G should be normal");
    assert_eq!(h.bg_color, normal_bg, "H should be normal");
}

#[test]
fn selection_block_cursor_skips_inversion() {
    use oriterm_core::RenderableCell;

    // 3x1 grid: "ABC". Select all three columns. Visible block cursor at col 1.
    // Col 1 should NOT be inverted (cursor overlay dominates).
    let fg = Rgb {
        r: 211,
        g: 215,
        b: 207,
    };
    let bg = Rgb { r: 0, g: 0, b: 0 };

    let cells = vec![
        RenderableCell {
            line: 0,
            column: Column(0),
            ch: 'A',
            fg,
            bg,
            flags: CellFlags::empty(),
            underline_color: None,
            has_hyperlink: false,
            zerowidth: Vec::new(),
        },
        RenderableCell {
            line: 0,
            column: Column(1),
            ch: 'B',
            fg,
            bg,
            flags: CellFlags::empty(),
            underline_color: None,
            has_hyperlink: false,
            zerowidth: Vec::new(),
        },
        RenderableCell {
            line: 0,
            column: Column(2),
            ch: 'C',
            fg,
            bg,
            flags: CellFlags::empty(),
            underline_color: None,
            has_hyperlink: false,
            zerowidth: Vec::new(),
        },
    ];

    let mut input = FrameInput::test_grid(3, 1, "");
    input.content.cells = cells;
    // Visible block cursor at col 1.
    input.content.cursor.visible = true;
    input.content.cursor.shape = CursorShape::Block;
    input.content.cursor.line = 0;
    input.content.cursor.column = Column(1);

    input.selection = Some(selection_range(0, 0, 2));

    let atlas = atlas_with(&['A', 'B', 'C']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let selected_bg = rgb_f32(fg);
    let normal_bg = rgb_f32(bg);

    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    let bg2 = nth_instance(frame.backgrounds.as_bytes(), 2);

    assert_eq!(bg0.bg_color, selected_bg, "A should be selected");
    assert_eq!(
        bg1.bg_color, normal_bg,
        "B at cursor should NOT be inverted"
    );
    assert_eq!(bg2.bg_color, selected_bg, "C should be selected");
}

#[test]
fn selection_inverse_cell_uses_palette_defaults() {
    use oriterm_core::RenderableCell;

    // A cell with INVERSE flag already has fg/bg swapped by the renderable layer.
    // Selection on this cell should use palette defaults, not double-swap.
    let fg = Rgb {
        r: 211,
        g: 215,
        b: 207,
    };
    let bg = Rgb { r: 0, g: 0, b: 0 };

    // INVERSE cell: renderable layer already swapped fg↔bg.
    let cells = vec![RenderableCell {
        line: 0,
        column: Column(0),
        ch: 'A',
        fg: bg, // Swapped by renderable layer.
        bg: fg, // Swapped by renderable layer.
        flags: CellFlags::INVERSE,
        underline_color: None,
        has_hyperlink: false,
        zerowidth: Vec::new(),
    }];

    let mut input = FrameInput::test_grid(1, 1, "");
    input.content.cells = cells;
    input.content.cursor.visible = false;
    input.selection = Some(selection_range(0, 0, 0));

    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // INVERSE + selected: should use palette defaults (bg=foreground, fg=background).
    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    let fg0 = nth_instance(frame.glyphs.as_bytes(), 0);

    let palette_fg = rgb_f32(fg);
    let palette_bg = rgb_f32(bg);

    assert_eq!(
        bg0.bg_color, palette_fg,
        "INVERSE selected bg should be palette foreground"
    );
    assert_eq!(
        fg0.fg_color, palette_bg,
        "INVERSE selected fg should be palette background"
    );
}

#[test]
fn selection_fg_eq_bg_falls_back_to_palette() {
    use oriterm_core::RenderableCell;

    // A cell where fg == bg (e.g., both red). Naive inversion would keep them
    // equal, making text invisible. Should fall back to palette defaults.
    let red = Rgb {
        r: 200,
        g: 50,
        b: 50,
    };

    let cells = vec![RenderableCell {
        line: 0,
        column: Column(0),
        ch: 'X',
        fg: red,
        bg: red,
        flags: CellFlags::empty(),
        underline_color: None,
        has_hyperlink: false,
        zerowidth: Vec::new(),
    }];

    let mut input = FrameInput::test_grid(1, 1, "");
    input.content.cells = cells;
    input.content.cursor.visible = false;
    input.selection = Some(selection_range(0, 0, 0));

    let atlas = atlas_with(&['X']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // fg==bg swap still produces fg==bg. Should fall back to palette defaults.
    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    let fg0 = nth_instance(frame.glyphs.as_bytes(), 0);

    let palette_fg = rgb_f32(Rgb {
        r: 211,
        g: 215,
        b: 207,
    });
    let palette_bg = rgb_f32(Rgb { r: 0, g: 0, b: 0 });

    assert_eq!(
        bg0.bg_color, palette_fg,
        "fg==bg selected should fall back to palette fg as bg"
    );
    assert_eq!(
        fg0.fg_color, palette_bg,
        "fg==bg selected should fall back to palette bg as fg"
    );
}

#[test]
fn selection_hidden_cell_stays_invisible() {
    use oriterm_core::RenderableCell;

    // A HIDDEN (SGR 8) cell where fg == bg intentionally hides text.
    // Selection should NOT reveal it — the fg==bg fallback should be skipped.
    let bg = Rgb { r: 0, g: 0, b: 0 };

    let cells = vec![RenderableCell {
        line: 0,
        column: Column(0),
        ch: 'S',
        fg: bg, // Hidden: fg set to bg.
        bg,
        flags: CellFlags::HIDDEN,
        underline_color: None,
        has_hyperlink: false,
        zerowidth: Vec::new(),
    }];

    let mut input = FrameInput::test_grid(1, 1, "");
    input.content.cells = cells;
    input.content.cursor.visible = false;
    input.selection = Some(selection_range(0, 0, 0));

    let atlas = atlas_with(&['S']);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // HIDDEN + selected: swap produces fg==bg, but HIDDEN guard skips fallback.
    // Result: sel_fg = cell.bg = black, sel_bg = cell.fg = black → both black.
    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    let fg0 = nth_instance(frame.glyphs.as_bytes(), 0);

    assert_eq!(
        bg0.bg_color, fg0.fg_color,
        "HIDDEN cell should remain invisible when selected"
    );
}

#[test]
fn selection_preserves_instance_counts() {
    // Selection is implemented as color inversion on existing instances, not
    // as a separate overlay layer.  Instance counts must be identical
    // regardless of whether a selection is active.
    let text: String = std::iter::repeat_n('A', 10).collect();
    let atlas = atlas_with(&['A']);

    // Baseline: no selection.
    let input_no_sel = FrameInput::test_grid(10, 3, &text);
    let frame_no_sel = prepare_frame(&input_no_sel, &atlas, (0.0, 0.0));

    // With selection covering a partial range on row 0.
    let mut input_sel = FrameInput::test_grid(10, 3, &text);
    input_sel.selection = Some(selection_range(0, 2, 7));
    let frame_sel = prepare_frame(&input_sel, &atlas, (0.0, 0.0));

    assert_eq!(
        frame_no_sel.backgrounds.len(),
        frame_sel.backgrounds.len(),
        "selection should not change bg instance count"
    );
    assert_eq!(
        frame_no_sel.glyphs.len(),
        frame_sel.glyphs.len(),
        "selection should not change fg instance count"
    );
    assert_eq!(
        frame_no_sel.cursors.len(),
        frame_sel.cursors.len(),
        "selection should not change cursor instance count"
    );

    // Verify selected cells have inverted colors while unselected cells are unchanged.
    let normal_bg = rgb_f32(Rgb { r: 0, g: 0, b: 0 });
    let selected_bg = rgb_f32(Rgb {
        r: 211,
        g: 215,
        b: 207,
    });

    let bg_col1 = nth_instance(frame_sel.backgrounds.as_bytes(), 1);
    assert_eq!(bg_col1.bg_color, normal_bg, "col 1 should be normal bg");

    let bg_col3 = nth_instance(frame_sel.backgrounds.as_bytes(), 3);
    assert_eq!(
        bg_col3.bg_color, selected_bg,
        "col 3 (in selection) should have inverted bg"
    );
}

#[test]
fn selection_underline_cursor_does_not_skip_inversion() {
    // Non-block cursors (underline, beam) should NOT prevent selection inversion.
    let mut input = FrameInput::test_grid(2, 1, "AB");
    let atlas = atlas_with(&['A', 'B']);

    // Visible underline cursor at col 0.
    input.content.cursor.visible = true;
    input.content.cursor.shape = CursorShape::Underline;
    input.content.cursor.line = 0;
    input.content.cursor.column = Column(0);

    input.selection = Some(selection_range(0, 0, 0));

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let selected_bg = rgb_f32(Rgb {
        r: 211,
        g: 215,
        b: 207,
    });

    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(
        bg0.bg_color, selected_bg,
        "underline cursor should not block selection inversion"
    );
}

// ── Hyperlink underline tests ──

/// Build a 1×1 hyperlink cell (no explicit underline flags).
fn frame_with_hyperlink() -> FrameInput {
    let mut input = FrameInput::test_grid(1, 1, "A");
    input.content.cells[0].has_hyperlink = true;
    input.content.cursor.visible = false;
    input
}

#[test]
fn hyperlink_not_hovered_emits_dotted_underline() {
    let input = frame_with_hyperlink();
    let atlas = empty_atlas();
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Dotted underline fallback: cell_width=8, step_by(2) → 4 dots.
    assert_eq!(
        decoration_bg_count(&frame),
        4,
        "hyperlink (not hovered) should emit dotted underline rects",
    );
}

#[test]
fn hyperlink_hovered_emits_solid_underline() {
    let mut input = frame_with_hyperlink();
    input.hovered_cell = Some((0, 0));
    let atlas = empty_atlas();
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Solid underline: 1 rect.
    assert_eq!(
        decoration_bg_count(&frame),
        1,
        "hyperlink (hovered) should emit single solid underline rect",
    );

    // Verify geometry matches a single underline.
    let ul = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(ul.size, (8.0, 1.0));
}

#[test]
fn hyperlink_hovered_uses_fg_color() {
    let mut input = frame_with_hyperlink();
    input.hovered_cell = Some((0, 0));
    let fg = input.content.cells[0].fg;
    let atlas = empty_atlas();
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let ul = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(ul.bg_color, rgb_f32(fg));
}

#[test]
fn hyperlink_with_explicit_underline_uses_explicit_style() {
    // When a cell has both a hyperlink and an explicit SGR underline,
    // the explicit underline takes priority — no dotted link decoration.
    let mut input = FrameInput::test_grid(1, 1, "A");
    input.content.cells[0].has_hyperlink = true;
    input.content.cells[0].flags = CellFlags::UNDERLINE;
    input.content.cursor.visible = false;

    let atlas = empty_atlas();
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Only the explicit single underline (1 rect), not the dotted link decoration.
    assert_eq!(
        decoration_bg_count(&frame),
        1,
        "explicit underline should override hyperlink decoration",
    );
}

#[test]
fn non_hyperlink_cell_no_extra_decorations() {
    // Verify that a plain cell without hyperlink or underline flags produces
    // no decoration instances — baseline sanity check.
    let input = FrameInput::test_grid(1, 1, "A");
    let atlas = empty_atlas();
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(decoration_bg_count(&frame), 0);
}

// ── Viewport / coordinate system alignment ──
//
// The shader maps pixel positions to NDC: ndc = pos / screen_size * 2 - 1.
// screen_size comes from FrameInput.viewport.  Cell positions come from
// origin + col * cell_width.  For cells to fill the viewport correctly,
// viewport and cell positions must be in the same coordinate system.

#[test]
fn cells_fill_viewport_when_viewport_matches_cell_units() {
    // 10 cols × 2 rows, cell = 8×16. Default viewport = 80×32 = 10*8 × 2*16.
    let input = FrameInput::test_grid(10, 2, "ABCDEFGHIJKLMNOPQRST");
    let atlas = atlas_with(&[
        'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R',
        'S', 'T',
    ]);
    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Last cell in row 0 at col 9: x = 9*8 = 72, right edge = 72+8 = 80.
    let last_bg = nth_instance(frame.backgrounds.as_bytes(), 9);
    let right_edge = last_bg.pos.0 + last_bg.size.0;
    assert_eq!(
        right_edge, frame.viewport.width as f32,
        "cells should fill viewport width"
    );

    // NDC fraction for right edge: 80/80 = 1.0.
    let ndc_frac = right_edge / frame.viewport.width as f32;
    assert!(
        (ndc_frac - 1.0).abs() < 0.001,
        "right edge NDC should be 1.0, got {ndc_frac}",
    );
}

#[test]
fn oversized_viewport_causes_cells_to_underfill() {
    // Demonstrate the bug: physical viewport > logical cell grid.
    // At 1.25x DPI, physical viewport is 100×40 but cells are 10*8 × 2*16 = 80×32.
    let mut input = FrameInput::test_grid(10, 2, "ABCDEFGHIJKLMNOPQRST");
    let atlas = atlas_with(&[
        'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R',
        'S', 'T',
    ]);

    // Override viewport to physical (larger than cell grid).
    input.viewport = ViewportSize::new(100, 40);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Cell positions are unchanged: right edge still at 80.
    let last_bg = nth_instance(frame.backgrounds.as_bytes(), 9);
    let right_edge = last_bg.pos.0 + last_bg.size.0;
    assert_eq!(
        right_edge, 80.0,
        "cell positions use cell_size, not viewport"
    );

    // NDC fraction: 80/100 = 0.8 — cells only fill 80% of the screen!
    let ndc_frac = right_edge / frame.viewport.width as f32;
    assert!(
        (ndc_frac - 0.8).abs() < 0.001,
        "oversized viewport: cells fill {ndc_frac}, not 1.0",
    );
}

#[test]
fn chrome_origin_aligns_when_viewport_is_logical() {
    // Simulates chrome (caption_height = 46 logical, unified tab bar) with
    // grid below. Viewport must be logical so that chrome and grid NDC match.
    let caption_height = 46.0_f32;
    let scale = 1.25_f32;

    // Logical viewport: 1016×640.
    let logical_h = 640_u32;

    // Chrome bar bottom in NDC (logical coords): 46 / 640 = 0.071875.
    let chrome_bottom_ndc = caption_height / logical_h as f32;

    // Grid origin = caption_height in logical coords.
    let grid_top_ndc = caption_height / logical_h as f32;

    // They match: chrome bottom == grid top.
    assert!(
        (chrome_bottom_ndc - grid_top_ndc).abs() < 0.001,
        "logical viewport: chrome={chrome_bottom_ndc}, grid={grid_top_ndc}",
    );

    // Now demonstrate the mismatch with physical viewport.
    let physical_h = (logical_h as f32 * scale).round() as u32; // 800

    // Chrome draws at physical pixels: 46 * 1.25 = 57.5.
    let chrome_bottom_physical_ndc = (caption_height * scale) / physical_h as f32;
    // Grid origin in logical: 46 / 800 = 0.0575.
    let grid_top_physical_ndc = caption_height / physical_h as f32;

    // Mismatch: chrome (0.071875) > grid (0.0575) — grid starts ABOVE chrome!
    assert!(
        chrome_bottom_physical_ndc > grid_top_physical_ndc,
        "physical viewport mismatch: chrome={chrome_bottom_physical_ndc}, grid={grid_top_physical_ndc}",
    );
}

#[test]
fn origin_with_logical_viewport_fills_grid_area() {
    // After chrome: grid starts at y=caption_height (unified tab bar),
    // viewport is logical. Cells should fill from caption to bottom.
    let caption_height = 46.0_f32;
    let cell_h = 16.0_f32;
    let logical_h = 640_u32;
    let grid_h = logical_h as f32 - caption_height; // 594
    let rows = (grid_h / cell_h).floor() as usize; // 37

    let mut input = FrameInput::test_grid(10, rows, "");
    input.viewport = ViewportSize::new(80, logical_h);

    let atlas = empty_atlas();
    let frame = prepare_frame(&input, &atlas, (0.0, caption_height));

    // First row starts at origin y.
    let first_bg = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(
        first_bg.pos.1, caption_height,
        "first row at caption height"
    );

    // Last row: y = 46 + 36*16 = 46 + 576 = 622.
    // Bottom edge: 622 + 16 = 638.
    let last_row_idx = (rows - 1) * 10; // First cell of last row
    let last_bg = nth_instance(frame.backgrounds.as_bytes(), last_row_idx);
    let bottom_edge = last_bg.pos.1 + last_bg.size.1;

    // Bottom edge (638) < viewport (640): grid doesn't quite reach bottom
    // (because 594/16 = 37.125, we only get 37 rows). This is normal —
    // there's a small gap at the bottom. But it's close.
    assert!(bottom_edge <= logical_h as f32, "grid fits within viewport");
    assert!(
        bottom_edge > logical_h as f32 - cell_h,
        "grid fills most of viewport: bottom={bottom_edge}, viewport={logical_h}",
    );
}

// ── Ligature + selection interaction (Section 6.5) ──

#[test]
fn shaped_ligature_selection_col1_does_not_duplicate_glyph() {
    // A 2-column ligature (glyph 100 at cols 0-1) with selection covering
    // only col 1. The glyph must be emitted exactly once at col 0.
    // Selection highlighting applies per-cell to backgrounds independently.
    let size_q6 = 768;
    let mut input = FrameInput::test_grid(3, 1, "fi ");
    input.content.cells[0].ch = 'f';
    input.content.cells[1].ch = 'i';
    input.content.cursor.visible = false;

    let atlas = key_atlas_with(&[100], size_q6);
    let glyphs = vec![ShapedGlyph {
        glyph_id: 100,
        face_index: 0,
        synthetic: 0,
        x_advance: 0.0,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let col_starts = vec![0]; // ligature starts at col 0
    let shaped = shaped_one_row(3, &glyphs, &col_starts, size_q6);

    // Select only col 1 (the continuation column of the ligature).
    input.selection = Some(selection_range(0, 1, 1));

    let frame = prepare_frame_shaped(&input, &atlas, &shaped, (0.0, 0.0));

    // 3 bg instances (one per cell), still only 1 fg instance (ligature glyph).
    assert_counts(&frame, 3, 1, 0);

    // Col 0 (unselected) should have normal bg.
    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    let normal_bg = rgb_f32(Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(bg0.bg_color, normal_bg, "col 0 should have normal bg");

    // Col 1 (selected continuation) should have inverted bg.
    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    let selected_bg = rgb_f32(Rgb {
        r: 211,
        g: 215,
        b: 207,
    });
    assert_eq!(
        bg1.bg_color, selected_bg,
        "col 1 (ligature continuation) should have selected bg"
    );

    // Col 2 (space, unselected) should have normal bg.
    let bg2 = nth_instance(frame.backgrounds.as_bytes(), 2);
    assert_eq!(bg2.bg_color, normal_bg, "col 2 should have normal bg");
}

// ── fg_dim dimming ──

#[test]
fn fg_dim_default_alpha_is_one() {
    let input = FrameInput::test_grid(1, 1, "A");
    let atlas = atlas_with(&['A']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    // fg_color[3] is the alpha component — default fg_dim=1.0.
    assert_eq!(
        fg.fg_color[3], 1.0,
        "default fg_dim should produce alpha 1.0"
    );
}

#[test]
fn fg_dim_reduces_glyph_alpha() {
    let mut input = FrameInput::test_grid(1, 1, "A");
    input.fg_dim = 0.7;
    input.content.cursor.visible = false;
    let atlas = atlas_with(&['A']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    assert!(
        (fg.fg_color[3] - 0.7).abs() < 0.001,
        "fg_dim=0.7 should produce alpha ~0.7, got {}",
        fg.fg_color[3],
    );
}

// ── Multi-pane instance accumulation ──

#[test]
fn fill_frame_shaped_accumulates_without_clearing() {
    use super::fill_frame_shaped;

    let input_a = FrameInput::test_grid(2, 1, "AB");
    let input_b = FrameInput::test_grid(2, 1, "CD");
    let atlas = empty_atlas();

    // Shape empty frames (no glyph hits, but backgrounds still accumulate).
    let shaped_a = ShapedFrame::new(2, 0);
    let shaped_b = ShapedFrame::new(2, 0);

    let mut frame = PreparedFrame::new(ViewportSize::new(32, 16), Rgb { r: 0, g: 0, b: 0 }, 1.0);

    // First fill: pane A at origin (0,0).
    fill_frame_shaped(&input_a, &atlas, &shaped_a, &mut frame, (0.0, 0.0), true);
    let count_after_a = frame.backgrounds.len();

    // Second fill: pane B at origin (16,0) — appends, does NOT clear.
    fill_frame_shaped(&input_b, &atlas, &shaped_b, &mut frame, (16.0, 0.0), false);
    let count_after_b = frame.backgrounds.len();

    assert_eq!(count_after_a, 2, "pane A should produce 2 bg instances");
    assert_eq!(
        count_after_b, 4,
        "pane B should append 2 more, total 4 bg instances"
    );
}

#[test]
fn two_panes_at_correct_offsets() {
    use super::fill_frame_shaped;

    let input_a = FrameInput::test_grid(1, 1, "A");
    let input_b = FrameInput::test_grid(1, 1, "B");
    let atlas = empty_atlas();
    let shaped = ShapedFrame::new(1, 0);

    let mut frame = PreparedFrame::new(ViewportSize::new(16, 16), Rgb { r: 0, g: 0, b: 0 }, 1.0);

    // Pane A at (0, 0).
    fill_frame_shaped(&input_a, &atlas, &shaped, &mut frame, (0.0, 0.0), true);
    // Pane B at (400, 0).
    fill_frame_shaped(&input_b, &atlas, &shaped, &mut frame, (400.0, 0.0), false);

    // Pane A background at x=0.
    let bg_a = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg_a.pos.0, 0.0, "pane A bg should be at x=0");

    // Pane B background at x=400.
    let bg_b = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(bg_b.pos.0, 400.0, "pane B bg should be at x=400");
}

#[test]
fn cursor_only_in_focused_pane() {
    use super::fill_frame_shaped;

    let input_focused = FrameInput::test_grid(1, 1, "A");
    let mut input_unfocused = FrameInput::test_grid(1, 1, "B");
    input_unfocused.content.cursor.visible = true;

    let atlas = empty_atlas();
    let shaped = ShapedFrame::new(1, 0);

    let mut frame = PreparedFrame::new(ViewportSize::new(16, 16), Rgb { r: 0, g: 0, b: 0 }, 1.0);

    // Focused pane: cursor_blink_visible = true.
    fill_frame_shaped(
        &input_focused,
        &atlas,
        &shaped,
        &mut frame,
        (0.0, 0.0),
        true,
    );
    let cursor_after_focused = frame.cursors.len();

    // Unfocused pane: cursor_blink_visible = false.
    fill_frame_shaped(
        &input_unfocused,
        &atlas,
        &shaped,
        &mut frame,
        (100.0, 0.0),
        false,
    );
    let cursor_after_unfocused = frame.cursors.len();

    assert_eq!(
        cursor_after_focused, 1,
        "focused pane should emit 1 cursor instance"
    );
    assert_eq!(
        cursor_after_unfocused, 1,
        "unfocused pane should not add more cursor instances"
    );
}

// ── Search match highlighting ──

/// Helper: build a `FrameSearch` with a single match at the given viewport
/// position (`line`, `start_col..=end_col`) with `focused` as the match index.
fn search_with_match(
    line: usize,
    start_col: usize,
    end_col: usize,
    focused: usize,
) -> crate::gpu::frame_input::FrameSearch {
    use oriterm_core::SearchMatch;

    let m = SearchMatch {
        start_row: StableRowIndex(line as u64),
        start_col,
        end_row: StableRowIndex(line as u64),
        end_col,
    };
    crate::gpu::frame_input::FrameSearch::for_test(vec![m], focused, 0)
}

#[test]
fn search_match_highlights_bg() {
    // A non-focused search match should use SEARCH_MATCH_BG for the bg
    // and keep the original fg.
    let match_bg = Rgb {
        r: 100,
        g: 100,
        b: 30,
    };

    let mut input = FrameInput::test_grid(3, 1, "ABC");
    // Match on col 1 only, focused index out of range → no focused match.
    input.search = Some(search_with_match(0, 1, 1, 99));
    input.content.cursor.visible = false;
    let atlas = atlas_with(&['A', 'B', 'C']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Col 0: normal bg.
    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg0.bg_color, rgb_f32(input.palette.background));

    // Col 1: search match bg.
    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(
        bg1.bg_color,
        rgb_f32(match_bg),
        "match bg should be yellow-tinted"
    );

    // Col 2: normal bg.
    let bg2 = nth_instance(frame.backgrounds.as_bytes(), 2);
    assert_eq!(bg2.bg_color, rgb_f32(input.palette.background));
}

#[test]
fn search_match_preserves_fg() {
    // Non-focused match keeps the cell's original fg color.
    let mut input = FrameInput::test_grid(1, 1, "A");
    input.search = Some(search_with_match(0, 0, 0, 99));
    input.content.cursor.visible = false;
    let fg = input.content.cells[0].fg;
    let atlas = atlas_with(&['A']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let glyph = nth_instance(frame.glyphs.as_bytes(), 0);
    assert_eq!(
        glyph.fg_color,
        rgb_f32(fg),
        "non-focused match keeps original fg"
    );
}

#[test]
fn search_focused_match_overrides_fg_and_bg() {
    // The focused match uses SEARCH_FOCUSED_FG and SEARCH_FOCUSED_BG.
    let focused_fg = Rgb { r: 0, g: 0, b: 0 };
    let focused_bg = Rgb {
        r: 200,
        g: 170,
        b: 40,
    };

    let mut input = FrameInput::test_grid(1, 1, "A");
    input.search = Some(search_with_match(0, 0, 0, 0)); // focused index = 0
    input.content.cursor.visible = false;
    let atlas = atlas_with(&['A']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let bg = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg.bg_color, rgb_f32(focused_bg), "focused match bg");

    let glyph = nth_instance(frame.glyphs.as_bytes(), 0);
    assert_eq!(
        glyph.fg_color,
        rgb_f32(focused_fg),
        "focused match fg should be dark"
    );
}

#[test]
fn search_match_skips_block_cursor_cell() {
    // The cell under a visible block cursor should NOT get search
    // highlighting — the cursor overlay handles its own visual.
    let mut input = FrameInput::test_grid(3, 1, "ABC");
    input.search = Some(search_with_match(0, 0, 2, 99));
    // Block cursor at col 0.
    input.content.cursor.column = Column(0);
    input.content.cursor.line = 0;
    input.content.cursor.shape = CursorShape::Block;
    input.content.cursor.visible = true;
    let atlas = atlas_with(&['A', 'B', 'C']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let match_bg = Rgb {
        r: 100,
        g: 100,
        b: 30,
    };

    // Col 0 (under block cursor): normal bg, NOT match bg.
    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_ne!(
        bg0.bg_color,
        rgb_f32(match_bg),
        "block cursor cell should skip search highlighting"
    );

    // Col 1 (not under cursor): match bg.
    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(
        bg1.bg_color,
        rgb_f32(match_bg),
        "non-cursor cell should be highlighted"
    );
}

#[test]
fn search_no_match_uses_default_colors() {
    // When search is active but no cells match, colors are unchanged.
    let mut input = FrameInput::test_grid(2, 1, "AB");
    // Match on row 5 (not in our 1-row grid).
    input.search = Some(search_with_match(5, 0, 0, 0));
    input.content.cursor.visible = false;
    let atlas = atlas_with(&['A', 'B']);
    let bg_color = input.palette.background;

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg0.bg_color, rgb_f32(bg_color));
    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(bg1.bg_color, rgb_f32(bg_color));
}

// ── URL hover underline ──

#[test]
fn url_hover_produces_cursor_layer_underline() {
    // Hovering a URL should produce cursor-layer underline rects.
    let mut input = FrameInput::test_grid(10, 1, "");
    // URL spans cols 2..5 on line 0.
    input.hovered_url_segments = vec![(0, 2, 5)];
    input.content.cursor.visible = false;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // 1 cursor instance for the URL underline (no terminal cursor).
    assert_eq!(frame.cursors.len(), 1, "should have 1 URL underline rect");

    let ul = nth_instance(frame.cursors.as_bytes(), 0);
    // x = 2 * 8.0 = 16.0
    assert_eq!(ul.pos.0, 16.0);
    // w = (5 - 2 + 1) * 8.0 = 32.0
    assert_eq!(ul.size.0, 32.0);
    // h = stroke_size = 1.0
    assert_eq!(ul.size.1, 1.0);
}

#[test]
fn url_hover_multiple_segments() {
    // A URL wrapping across lines produces multiple segments.
    let mut input = FrameInput::test_grid(10, 3, "");
    input.hovered_url_segments = vec![(0, 5, 9), (1, 0, 9), (2, 0, 3)];
    input.content.cursor.visible = false;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.cursors.len(), 3, "3 URL underline segments");
}

#[test]
fn url_hover_empty_segments_no_extra_instances() {
    // No hovered URL → no extra cursor instances.
    let mut input = FrameInput::test_grid(10, 1, "");
    input.hovered_url_segments = Vec::new();
    input.content.cursor.visible = false;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.cursors.len(), 0, "no URL hover → no cursor instances");
}

#[test]
fn url_hover_with_origin_offset() {
    // URL underline positions should respect the origin offset.
    let mut input = FrameInput::test_grid(10, 1, "");
    input.hovered_url_segments = vec![(0, 0, 2)];
    input.content.cursor.visible = false;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (50.0, 100.0));

    let ul = nth_instance(frame.cursors.as_bytes(), 0);
    // x = 50.0 + 0 * 8.0 = 50.0
    assert_eq!(ul.pos.0, 50.0);
    // y includes origin offset + underline position.
    assert!(ul.pos.1 > 100.0, "y should be offset from origin");
}

// ── Mark cursor override ──

#[test]
fn mark_cursor_overrides_terminal_cursor() {
    // When mark_cursor is set, it should override the terminal cursor position
    // and shape (HollowBlock).
    let mut input = FrameInput::test_grid(10, 5, "");
    // Terminal cursor at (0, 0) as Block.
    input.content.cursor.column = Column(0);
    input.content.cursor.line = 0;
    input.content.cursor.shape = CursorShape::Block;
    input.content.cursor.visible = true;
    // Mark cursor at (3, 5) as HollowBlock.
    input.mark_cursor = Some(crate::gpu::frame_input::MarkCursorOverride {
        line: 3,
        column: Column(5),
        shape: CursorShape::HollowBlock,
    });
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // HollowBlock = 4 cursor instances (top, bottom, left, right).
    assert_eq!(frame.cursors.len(), 4);

    // All 4 edges should be around col 5, row 3.
    let top = nth_instance(frame.cursors.as_bytes(), 0);
    assert_eq!(top.pos, (40.0, 48.0)); // col 5 * 8 = 40, row 3 * 16 = 48
}

#[test]
fn mark_cursor_none_uses_terminal_cursor() {
    // When mark_cursor is None, the terminal cursor is used.
    let mut input = FrameInput::test_grid(10, 5, "");
    input.content.cursor.column = Column(7);
    input.content.cursor.line = 2;
    input.content.cursor.shape = CursorShape::Block;
    input.content.cursor.visible = true;
    input.mark_cursor = None;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.cursors.len(), 1);

    let c = nth_instance(frame.cursors.as_bytes(), 0);
    assert_eq!(c.pos, (56.0, 32.0)); // col 7 * 8 = 56, row 2 * 16 = 32
}

#[test]
fn mark_cursor_is_always_visible() {
    // Mark cursor overrides visibility — it's always rendered even if the
    // terminal cursor is hidden.
    let mut input = FrameInput::test_grid(10, 5, "");
    input.content.cursor.visible = false; // terminal cursor hidden
    input.mark_cursor = Some(crate::gpu::frame_input::MarkCursorOverride {
        line: 1,
        column: Column(3),
        shape: CursorShape::HollowBlock,
    });
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // HollowBlock = 4 cursor instances.
    assert_eq!(
        frame.cursors.len(),
        4,
        "mark cursor should render even when terminal cursor is hidden"
    );
}

// ── Explicit selection colors ──

#[test]
fn selection_explicit_colors_override_inversion() {
    // When palette.selection_fg and palette.selection_bg are set,
    // selected cells use those colors instead of fg/bg inversion.
    let sel_fg = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let sel_bg = Rgb {
        r: 58,
        g: 61,
        b: 92,
    };

    let mut input = FrameInput::test_grid(3, 1, "ABC");
    input.palette.selection_fg = Some(sel_fg);
    input.palette.selection_bg = Some(sel_bg);
    input.selection = Some(selection_range(0, 1, 1));
    input.content.cursor.visible = false;
    let atlas = atlas_with(&['A', 'B', 'C']);

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Col 1 (selected): should use explicit selection colors.
    let bg1 = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(bg1.bg_color, rgb_f32(sel_bg), "explicit selection bg");

    let fg1 = nth_instance(frame.glyphs.as_bytes(), 1);
    assert_eq!(fg1.fg_color, rgb_f32(sel_fg), "explicit selection fg");

    // Col 0 (not selected): normal colors.
    let bg0 = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg0.bg_color, rgb_f32(input.palette.background));
}

// ── Empty cells still produce bg instances ──

#[test]
fn null_char_cell_produces_bg_only() {
    // A cell with '\0' should produce a BG instance but no FG instance,
    // same as a space cell.
    let mut input = FrameInput::test_grid(2, 1, "");
    input.content.cells[0].ch = '\0';
    input.content.cells[1].ch = '\0';
    input.content.cursor.visible = false;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(
        frame.backgrounds.len(),
        2,
        "2 bg instances for 2 null cells"
    );
    assert_eq!(frame.glyphs.len(), 0, "no fg instances for null cells");
}

#[test]
fn cells_with_custom_bg_produce_bg_instances() {
    // Cells that are spaces but have non-default background should still
    // produce BG instances with the correct color.
    let mut input = FrameInput::test_grid(3, 1, "");
    let custom_bg = Rgb {
        r: 100,
        g: 50,
        b: 200,
    };
    for cell in &mut input.content.cells {
        cell.bg = custom_bg;
    }
    input.content.cursor.visible = false;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.backgrounds.len(), 3);
    for i in 0..3 {
        let bg = nth_instance(frame.backgrounds.as_bytes(), i);
        assert_eq!(
            bg.bg_color,
            rgb_f32(custom_bg),
            "cell {i} should have custom bg color",
        );
    }
}

// ── Zero-size viewport ──

#[test]
fn zero_cols_zero_rows_produces_empty_frame() {
    let mut input = FrameInput::test_grid(0, 0, "");
    input.content.cursor.visible = false;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.backgrounds.len(), 0);
    assert_eq!(frame.glyphs.len(), 0);
    assert_eq!(frame.cursors.len(), 0);
}

#[test]
fn zero_cols_nonzero_rows_produces_empty_frame() {
    let mut input = FrameInput::test_grid(0, 5, "");
    input.content.cursor.visible = false;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.backgrounds.len(), 0);
    assert_eq!(frame.glyphs.len(), 0);
}

#[test]
fn nonzero_cols_zero_rows_produces_empty_frame() {
    let mut input = FrameInput::test_grid(80, 0, "");
    input.content.cursor.visible = false;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.backgrounds.len(), 0);
    assert_eq!(frame.glyphs.len(), 0);
}

// ── Prompt marker tests ──

#[test]
fn prompt_markers_emit_cursor_rects() {
    let mut input = FrameInput::test_grid(4, 3, "");
    input.content.cursor.visible = false;
    input.prompt_marker_rows = vec![0, 2];
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    // Two prompt marker bars should appear in the cursor layer.
    assert_eq!(frame.cursors.len(), 2, "expected 2 prompt marker rects");
}

#[test]
fn prompt_markers_empty_emits_no_rects() {
    let mut input = FrameInput::test_grid(4, 3, "");
    input.content.cursor.visible = false;
    input.prompt_marker_rows = Vec::new();
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(
        frame.cursors.len(),
        0,
        "no prompt markers = no cursor rects"
    );
}

#[test]
fn prompt_markers_with_origin_offset() {
    let mut input = FrameInput::test_grid(4, 3, "");
    input.content.cursor.visible = false;
    input.prompt_marker_rows = vec![1];
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (10.0, 20.0));

    // One marker rect at row 1 with origin offset applied.
    assert_eq!(frame.cursors.len(), 1, "expected 1 prompt marker rect");
}

// ── Image z-index splitting ──

fn placement(z: i32, x: f32, y: f32) -> oriterm_core::RenderablePlacement {
    oriterm_core::RenderablePlacement {
        image_id: oriterm_core::image::ImageId::from_raw(1),
        viewport_x: x,
        viewport_y: y,
        display_width: 32.0,
        display_height: 32.0,
        source_x: 0.0,
        source_y: 0.0,
        source_w: 1.0,
        source_h: 1.0,
        z_index: z,
        opacity: 1.0,
    }
}

#[test]
fn image_z_negative_goes_to_below_list() {
    let mut input = FrameInput::test_grid(4, 2, "");
    input.content.cursor.visible = false;
    input.content.images = vec![placement(-1, 0.0, 0.0)];
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.image_quads_below.len(), 1);
    assert_eq!(frame.image_quads_above.len(), 0);
    assert_eq!(frame.image_quads_below[0].x, 0.0);
}

#[test]
fn image_z_zero_goes_to_above_list() {
    let mut input = FrameInput::test_grid(4, 2, "");
    input.content.cursor.visible = false;
    input.content.images = vec![placement(0, 10.0, 20.0)];
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.image_quads_below.len(), 0);
    assert_eq!(frame.image_quads_above.len(), 1);
    assert_eq!(frame.image_quads_above[0].x, 10.0);
}

#[test]
fn image_z_positive_goes_to_above_list() {
    let mut input = FrameInput::test_grid(4, 2, "");
    input.content.cursor.visible = false;
    input.content.images = vec![placement(5, 0.0, 0.0)];
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.image_quads_below.len(), 0);
    assert_eq!(frame.image_quads_above.len(), 1);
}

#[test]
fn mixed_z_images_split_correctly() {
    let mut input = FrameInput::test_grid(4, 2, "");
    input.content.cursor.visible = false;
    input.content.images = vec![
        placement(-2, 0.0, 0.0),
        placement(1, 10.0, 0.0),
        placement(-1, 20.0, 0.0),
        placement(0, 30.0, 0.0),
    ];
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    assert_eq!(frame.image_quads_below.len(), 2, "z<0 images");
    assert_eq!(frame.image_quads_above.len(), 2, "z>=0 images");
}

#[test]
fn image_origin_offset_applied() {
    let mut input = FrameInput::test_grid(4, 2, "");
    input.content.cursor.visible = false;
    input.content.images = vec![placement(-1, 5.0, 10.0)];
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (100.0, 200.0));

    let q = &frame.image_quads_below[0];
    assert_eq!(q.x, 105.0, "origin x added to viewport_x");
    assert_eq!(q.y, 210.0, "origin y added to viewport_y");
}

#[test]
fn image_uv_and_opacity_propagated() {
    let mut input = FrameInput::test_grid(4, 2, "");
    input.content.cursor.visible = false;
    let mut img = placement(0, 0.0, 0.0);
    img.source_x = 0.25;
    img.source_y = 0.5;
    img.source_w = 0.5;
    img.source_h = 0.25;
    img.opacity = 0.8;
    input.content.images = vec![img];
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas, (0.0, 0.0));

    let q = &frame.image_quads_above[0];
    assert_eq!(q.uv_x, 0.25);
    assert_eq!(q.uv_y, 0.5);
    assert_eq!(q.uv_w, 0.5);
    assert_eq!(q.uv_h, 0.25);
    assert_eq!(q.opacity, 0.8);
}
