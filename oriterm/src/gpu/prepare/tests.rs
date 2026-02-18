//! Unit tests for the prepare phase.

use std::collections::HashMap;

use oriterm_core::{CellFlags, Column, CursorShape, Rgb};

use super::{
    AtlasLookup, ShapedFrame, prepare_frame, prepare_frame_into, prepare_frame_shaped,
    prepare_frame_shaped_into,
};
use crate::font::shaper::ShapedGlyph;
use crate::font::{FaceIdx, GlyphStyle, RasterKey, SyntheticFlags};
use crate::gpu::atlas::AtlasEntry;
use crate::gpu::frame_input::{FrameInput, ViewportSize};
use crate::gpu::instance_writer::INSTANCE_SIZE;
use crate::gpu::prepared_frame::PreparedFrame;

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
        is_color: false,
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

/// Convert Rgb to the [f32; 4] that push_rect writes to bg_color.
fn rgb_f32(c: Rgb) -> [f32; 4] {
    [
        f32::from(c.r) / 255.0,
        f32::from(c.g) / 255.0,
        f32::from(c.b) / 255.0,
        1.0,
    ]
}

// ── Instance buffer correctness ──

#[test]
fn single_char_produces_one_bg_and_one_fg() {
    let input = FrameInput::test_grid(1, 1, "A");
    let atlas = atlas_with(&['A']);

    let frame = prepare_frame(&input, &atlas);

    // 1 bg for the cell, 1 fg for the glyph, 1 cursor (block at 0,0).
    assert_counts(&frame, 1, 1, 1);
}

#[test]
fn single_char_bg_position_and_size() {
    let input = FrameInput::test_grid(2, 2, "A");
    let atlas = atlas_with(&['A']);

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    assert_eq!(fg.fg_color, rgb_f32(fg_rgb));
}

#[test]
fn single_char_bg_color_matches_cell() {
    let input = FrameInput::test_grid(1, 1, "A");
    let atlas = atlas_with(&['A']);
    let bg_rgb = input.content.cells[0].bg;

    let frame = prepare_frame(&input, &atlas);

    let bg = nth_instance(frame.backgrounds.as_bytes(), 0);
    assert_eq!(bg.bg_color, rgb_f32(bg_rgb));
}

// ── Empty cells ──

#[test]
fn empty_cell_produces_bg_only() {
    let input = FrameInput::test_grid(1, 1, " ");
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas);

    assert_eq!(frame.backgrounds.len(), 1);
    assert_eq!(frame.glyphs.len(), 0);
}

#[test]
fn all_spaces_grid_no_fg_instances() {
    let input = FrameInput::test_grid(10, 5, "");
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas);

    assert_eq!(frame.backgrounds.len(), 50);
    assert_eq!(frame.glyphs.len(), 0);
}

#[test]
fn all_chars_grid_equal_bg_and_fg() {
    let text: String = std::iter::repeat_n('A', 10).collect();
    let input = FrameInput::test_grid(10, 1, &text);
    let atlas = atlas_with(&['A']);

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

    // Only 1 bg (the wide char covers both columns), not 2.
    assert_eq!(frame.backgrounds.len(), 1);
}

// ── Cell positions are pixel-perfect ──

#[test]
fn cell_positions_are_pixel_perfect() {
    let input = FrameInput::test_grid(3, 3, "ABCDEFGHI");
    let atlas = atlas_with(&['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I']);

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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

    let frame1 = prepare_frame(&input, &atlas);
    let frame2 = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

    assert_eq!(frame.cursors.len(), 4);
}

#[test]
fn hollow_block_edges() {
    let mut input = FrameInput::test_grid(10, 5, "");
    input.content.cursor.shape = CursorShape::HollowBlock;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

    assert_eq!(frame.cursors.len(), 0);
}

#[test]
fn cursor_invisible_zero_instances() {
    let mut input = FrameInput::test_grid(10, 5, "");
    input.content.cursor.visible = false;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas);

    assert_eq!(frame.cursors.len(), 0);
}

#[test]
fn cursor_at_position() {
    let mut input = FrameInput::test_grid(10, 10, "");
    input.content.cursor.column = Column(5);
    input.content.cursor.line = 3;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas);

    let c = nth_instance(frame.cursors.as_bytes(), 0);
    assert_eq!(c.pos, (40.0, 48.0)); // 5*8=40, 3*16=48
}

#[test]
fn cursor_color_from_palette() {
    let input = FrameInput::test_grid(10, 5, "");
    let atlas = empty_atlas();
    let cursor_color = input.palette.cursor_color;

    let frame = prepare_frame(&input, &atlas);

    let c = nth_instance(frame.cursors.as_bytes(), 0);
    // Cursor color is in bg_color (rendered via bg_pipeline as solid-fill rect).
    assert_eq!(c.bg_color, rgb_f32(cursor_color));
}

// ── Missing atlas entries ──

#[test]
fn missing_glyph_skips_fg_instance() {
    let input = FrameInput::test_grid(1, 1, "Z");
    let atlas = empty_atlas(); // No entry for 'Z'.

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

    assert_eq!(frame.glyphs.len(), 1);
}

#[test]
fn bold_italic_cell_uses_bold_italic_style() {
    let mut input = FrameInput::test_grid(1, 1, "X");
    input.content.cells[0].flags = CellFlags::BOLD | CellFlags::ITALIC;

    let mut map = HashMap::new();
    map.insert(('X', GlyphStyle::BoldItalic), test_entry('X'));
    let atlas = TestAtlas(map);

    let frame = prepare_frame(&input, &atlas);

    assert_eq!(frame.glyphs.len(), 1);
}

// ── Instance count for larger grids ──

#[test]
fn ten_by_five_all_spaces() {
    let input = FrameInput::test_grid(10, 5, "");
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas);

    assert_counts(&frame, 50, 0, 1); // 1 cursor (block, visible)
}

#[test]
fn clear_color_matches_palette_background() {
    let input = FrameInput::test_grid(10, 5, "");
    let atlas = empty_atlas();
    let bg = input.palette.background;

    let frame = prepare_frame(&input, &atlas);

    let expected = [
        f64::from(bg.r) / 255.0,
        f64::from(bg.g) / 255.0,
        f64::from(bg.b) / 255.0,
        1.0,
    ];
    assert_eq!(frame.clear_color, expected);
}

#[test]
fn clear_color_respects_palette_opacity() {
    let mut input = FrameInput::test_grid(10, 5, "");
    input.palette.opacity = 0.5;
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas);

    let bg = input.palette.background;
    let expected = [
        f64::from(bg.r) / 255.0 * 0.5,
        f64::from(bg.g) / 255.0 * 0.5,
        f64::from(bg.b) / 255.0 * 0.5,
        0.5,
    ];
    assert_eq!(frame.clear_color, expected);
}

// ── prepare_frame_into ──

#[test]
fn prepare_into_matches_prepare() {
    let input = FrameInput::test_grid(10, 5, "Hello World!");
    let atlas = atlas_with(&['H', 'e', 'l', 'o', 'W', 'r', 'd', '!']);

    let fresh = prepare_frame(&input, &atlas);

    let mut reused = PreparedFrame::new(ViewportSize::new(1, 1), Rgb { r: 0, g: 0, b: 0 }, 1.0);
    prepare_frame_into(&input, &atlas, &mut reused);

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
    let mut frame = prepare_frame(&input, &atlas);
    let first_bg_count = frame.backgrounds.len();
    let first_fg_count = frame.glyphs.len();

    // Second prepare with smaller input reuses (clear + refill).
    let small = FrameInput::test_grid(2, 1, "A");
    prepare_frame_into(&small, &atlas, &mut frame);

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

    let mut frame = prepare_frame(&input1, &atlas);
    let first_bg = frame.backgrounds.len();
    let first_fg = frame.glyphs.len();

    // Second frame with different content.
    let input2 = FrameInput::test_grid(2, 1, "B");
    prepare_frame_into(&input2, &atlas, &mut frame);

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

    let mut frame = prepare_frame(&input1, &atlas);
    let first_clear = frame.clear_color;

    // Change palette background.
    let mut input2 = FrameInput::test_grid(2, 1, "");
    input2.palette.background = Rgb { r: 255, g: 0, b: 0 };
    prepare_frame_into(&input2, &atlas, &mut frame);

    assert_ne!(frame.clear_color, first_clear);
    assert_eq!(frame.clear_color, [1.0, 0.0, 0.0, 1.0]);
}

// ── Full-size grid instance counts (80×24) ──

#[test]
fn full_grid_all_spaces_1920_bg_zero_fg() {
    let input = FrameInput::test_grid(80, 24, "");
    let atlas = empty_atlas();

    let frame = prepare_frame(&input, &atlas);

    assert_eq!(frame.backgrounds.len(), 80 * 24);
    assert_eq!(frame.glyphs.len(), 0);
}

#[test]
fn full_grid_all_chars_1920_bg_and_fg() {
    let text: String = std::iter::repeat_n('A', 80 * 24).collect();
    let input = FrameInput::test_grid(80, 24, &text);
    let atlas = atlas_with(&['A']);

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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

    let frame = prepare_frame(&input, &atlas);

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
        is_color: false,
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
        };
        map.insert(key, test_entry_for_glyph(gid));
    }
    KeyTestAtlas(map)
}

/// Build a ShapedFrame for a 1-row grid from a slice of ShapedGlyphs.
fn shaped_one_row(cols: usize, glyphs: &[ShapedGlyph], size_q6: u32) -> ShapedFrame {
    let mut sf = ShapedFrame::new(cols, size_q6);
    let mut col_map = Vec::new();
    crate::font::shaper::build_col_glyph_map(glyphs, cols, &mut col_map);
    sf.push_row(glyphs, &col_map);
    sf
}

#[test]
fn shaped_single_glyph_one_bg_one_fg() {
    let size_q6 = 768; // 12px * 64
    let input = FrameInput::test_grid(3, 1, "A  ");
    let atlas = key_atlas_with(&[42], size_q6);

    let glyphs = vec![ShapedGlyph {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        synthetic: SyntheticFlags::NONE,
        col_start: 0,
        col_span: 1,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let shaped = shaped_one_row(3, &glyphs, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped);

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
        face_idx: FaceIdx::REGULAR,
        synthetic: SyntheticFlags::NONE,
        col_start: 0,
        col_span: 2, // ligature spans 2 columns
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let shaped = shaped_one_row(3, &glyphs, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped);

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
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
            col_start: 0,
            col_span: 1,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 51,
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
            col_start: 0, // same col — combining mark
            col_span: 1,
            x_offset: 2.0,
            y_offset: 3.0,
        },
    ];
    let shaped = shaped_one_row(2, &glyphs, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped);

    // 2 bg (per-cell), 2 fg (base + combining mark), 1 cursor.
    assert_counts(&frame, 2, 2, 1);
}

#[test]
fn shaped_offset_applied_to_glyph_position() {
    let size_q6 = 768;
    let input = FrameInput::test_grid(1, 1, "X");
    let atlas = key_atlas_with(&[60], size_q6);

    let glyphs = vec![ShapedGlyph {
        glyph_id: 60,
        face_idx: FaceIdx::REGULAR,
        synthetic: SyntheticFlags::NONE,
        col_start: 0,
        col_span: 1,
        x_offset: 1.5,
        y_offset: 2.0,
    }];
    let shaped = shaped_one_row(1, &glyphs, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped);

    assert_eq!(frame.glyphs.len(), 1);
    let fg = nth_instance(frame.glyphs.as_bytes(), 0);
    let entry = test_entry_for_glyph(60);

    // glyph_x = 0.0 + bearing_x(1) + x_offset(1.5) = 2.5
    let expected_x = 0.0 + entry.bearing_x as f32 + 1.5;
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
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
            col_start: 0,
            col_span: 2,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 101,
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
            col_start: 2,
            col_span: 1,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 102,
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
            col_start: 3,
            col_span: 1,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];
    let shaped = shaped_one_row(4, &glyphs, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped);

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
        face_idx: FaceIdx::REGULAR,
        synthetic: SyntheticFlags::NONE,
        col_start: 0,
        col_span: 1,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let shaped = shaped_one_row(1, &glyphs, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped);

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
    let mut col_map = Vec::new();
    crate::font::shaper::build_col_glyph_map(&empty_glyphs, 3, &mut col_map);

    let mut sf = shaped;
    sf.push_row(&empty_glyphs, &col_map);
    let frame = prepare_frame_shaped(&input, &atlas, &sf);

    assert_counts(&frame, 3, 0, 1);
}

// ── Color glyph routing (Section 6.10) ──

#[test]
fn color_glyph_routes_to_color_glyphs_buffer() {
    // A shaped glyph with is_color=true should go to frame.color_glyphs,
    // not frame.glyphs.
    let size_q6 = 768;
    let input = FrameInput::test_grid(1, 1, "E"); // emoji placeholder

    let mut map = HashMap::new();
    let key = RasterKey {
        glyph_id: 200,
        face_idx: FaceIdx::REGULAR,
        size_q6,
        synthetic: SyntheticFlags::NONE,
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
            is_color: true, // Color emoji!
        },
    );
    let atlas = KeyTestAtlas(map);

    let glyphs = vec![ShapedGlyph {
        glyph_id: 200,
        face_idx: FaceIdx::REGULAR,
        synthetic: SyntheticFlags::NONE,
        col_start: 0,
        col_span: 1,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let shaped = shaped_one_row(1, &glyphs, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped);

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
        },
        AtlasEntry {
            is_color: true,
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
        },
        test_entry_for_glyph(11),
    );
    let atlas = KeyTestAtlas(map);

    let glyphs = vec![
        ShapedGlyph {
            glyph_id: 10,
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
            col_start: 0,
            col_span: 1,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 200,
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
            col_start: 1,
            col_span: 1,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 11,
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
            col_start: 2,
            col_span: 1,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];
    let shaped = shaped_one_row(3, &glyphs, size_q6);
    let frame = prepare_frame_shaped(&input, &atlas, &shaped);

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
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
            col_start: 0,
            col_span: 2,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 101,
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
            col_start: 2,
            col_span: 1,
            x_offset: 0.0,
            y_offset: 0.0,
        },
        ShapedGlyph {
            glyph_id: 102,
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
            col_start: 3,
            col_span: 1,
            x_offset: 0.0,
            y_offset: 0.0,
        },
    ];
    let shaped = shaped_one_row(4, &glyphs, size_q6);

    let fresh = prepare_frame_shaped(&input, &atlas, &shaped);

    let mut reused = PreparedFrame::new(ViewportSize::new(1, 1), Rgb { r: 0, g: 0, b: 0 }, 1.0);
    prepare_frame_shaped_into(&input, &atlas, &shaped, &mut reused);

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
        .map(|i| ShapedGlyph {
            glyph_id: 42,
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
            col_start: i % 10,
            col_span: 1,
            x_offset: 0.0,
            y_offset: 0.0,
        })
        .collect();
    let atlas = key_atlas_with(&[42], size_q6);

    // Build shaped frame with all 5 rows.
    let mut sf = ShapedFrame::new(10, size_q6);
    for row_start in (0..50).step_by(10) {
        let row_glyphs = &glyphs[row_start..row_start + 10];
        let mut col_map = Vec::new();
        crate::font::shaper::build_col_glyph_map(row_glyphs, 10, &mut col_map);
        sf.push_row(row_glyphs, &col_map);
    }

    // First prepare.
    let mut frame = prepare_frame_shaped(&input, &atlas, &sf);
    let first_bg = frame.backgrounds.len();
    let first_fg = frame.glyphs.len();

    // Second prepare with smaller input reuses allocations.
    let small = FrameInput::test_grid(2, 1, "A ");
    let small_glyphs = vec![ShapedGlyph {
        glyph_id: 42,
        face_idx: FaceIdx::REGULAR,
        synthetic: SyntheticFlags::NONE,
        col_start: 0,
        col_span: 1,
        x_offset: 0.0,
        y_offset: 0.0,
    }];
    let small_shaped = shaped_one_row(2, &small_glyphs, size_q6);
    prepare_frame_shaped_into(&small, &atlas, &small_shaped, &mut frame);

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
    let frame = prepare_frame(&input, &atlas);

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
    let frame = prepare_frame(&input, &atlas);

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
    let frame = prepare_frame(&input, &atlas);

    let ul = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(ul.bg_color, rgb_f32(sgr58));
}

#[test]
fn double_underline_two_extra_bgs() {
    let input = frame_with_flags(CellFlags::DOUBLE_UNDERLINE);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas);

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
    let frame = prepare_frame(&input, &atlas);

    // cell_width=8 → 8 per-pixel rects.
    assert_eq!(decoration_bg_count(&frame), 8);
}

#[test]
fn dotted_underline_alternating() {
    let input = frame_with_flags(CellFlags::DOTTED_UNDERLINE);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas);

    // cell_width=8, step_by(2) → 4 dots (at 0, 2, 4, 6).
    assert_eq!(decoration_bg_count(&frame), 4);
}

#[test]
fn dashed_underline_pattern() {
    let input = frame_with_flags(CellFlags::DASHED_UNDERLINE);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas);

    // cell_width=8, pattern 3-on-2-off: dx 0,1,2 on, 3,4 off, 5,6,7 on → 6.
    assert_eq!(decoration_bg_count(&frame), 6);
}

#[test]
fn strikethrough_at_center() {
    let input = frame_with_flags(CellFlags::STRIKETHROUGH);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas);

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
    let frame = prepare_frame(&input, &atlas);

    let st = nth_instance(frame.backgrounds.as_bytes(), 1);
    assert_eq!(st.bg_color, rgb_f32(fg));
}

#[test]
fn underline_and_strikethrough_coexist() {
    let input = frame_with_flags(CellFlags::UNDERLINE | CellFlags::STRIKETHROUGH);
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas);

    // 1 base bg + 1 underline + 1 strikethrough = 2 decoration rects.
    assert_eq!(decoration_bg_count(&frame), 2);
}

#[test]
fn no_flags_no_decorations() {
    let input = frame_with_flags(CellFlags::empty());
    let atlas = atlas_with(&['A']);
    let frame = prepare_frame(&input, &atlas);

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
    let frame = prepare_frame(&input, &atlas);

    // Find the underline rect (second bg instance for the wide char cell).
    let ul = nth_instance(frame.backgrounds.as_bytes(), 1);
    // Wide char bg_w = 2 * cell_width = 16.0, underline should match.
    assert_eq!(ul.size.0, 16.0);
    assert_eq!(ul.size.1, 1.0);
}
