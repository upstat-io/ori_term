//! Unit tests for built-in geometric glyph rasterization.

use super::*;
use crate::font::{FaceIdx, GlyphFormat};

// ── is_builtin range tests ──

#[test]
fn is_builtin_box_drawing_range() {
    for c in '\u{2500}'..='\u{257F}' {
        assert!(is_builtin(c), "U+{:04X} should be builtin", c as u32);
    }
}

#[test]
fn is_builtin_block_elements_range() {
    for c in '\u{2580}'..='\u{259F}' {
        assert!(is_builtin(c), "U+{:04X} should be builtin", c as u32);
    }
}

#[test]
fn is_builtin_braille_range() {
    for c in '\u{2800}'..='\u{28FF}' {
        assert!(is_builtin(c), "U+{:04X} should be builtin", c as u32);
    }
}

#[test]
fn is_builtin_powerline_handled_codepoints() {
    let handled = [
        '\u{E0B0}', '\u{E0B1}', '\u{E0B2}', '\u{E0B3}', '\u{E0B4}', '\u{E0B6}',
    ];
    for &c in &handled {
        assert!(is_builtin(c), "U+{:04X} should be builtin", c as u32);
    }
}

#[test]
fn is_builtin_excludes_powerline_icons() {
    // Icon codepoints (branch, lock) should fall through to font rendering.
    for c in '\u{E0A0}'..='\u{E0A3}' {
        assert!(!is_builtin(c), "U+{:04X} should NOT be builtin (icon)", c as u32);
    }
    // Unhandled extras beyond E0B6.
    assert!(!is_builtin('\u{E0B5}'), "U+E0B5 should NOT be builtin");
    assert!(!is_builtin('\u{E0B7}'), "U+E0B7 should NOT be builtin");
    assert!(!is_builtin('\u{E0D4}'), "U+E0D4 should NOT be builtin");
}

#[test]
fn is_builtin_excludes_normal_chars() {
    assert!(!is_builtin('A'));
    assert!(!is_builtin(' '));
    assert!(!is_builtin('0'));
    assert!(!is_builtin('\u{4E00}')); // CJK
    assert!(!is_builtin('\u{1F600}')); // Emoji
}

#[test]
fn is_builtin_excludes_gap_between_ranges() {
    // Gap between block elements and braille.
    assert!(!is_builtin('\u{25A0}'));
    assert!(!is_builtin('\u{27FF}'));
    // Gap between braille and powerline.
    assert!(!is_builtin('\u{2900}'));
    assert!(!is_builtin('\u{E0AF}'));
}

// ── raster_key tests ──

#[test]
fn raster_key_uses_builtin_face() {
    let key = raster_key('─', 1024);
    assert_eq!(key.face_idx, FaceIdx::BUILTIN);
    assert_eq!(key.glyph_id, '─' as u16);
    assert_eq!(key.size_q6, 1024);
}

#[test]
fn raster_key_different_chars_different_keys() {
    let k1 = raster_key('\u{2500}', 1024);
    let k2 = raster_key('\u{2502}', 1024);
    assert_ne!(k1.glyph_id, k2.glyph_id);
}

// ── rasterize tests ──

#[test]
fn rasterize_returns_none_for_ascii() {
    assert!(rasterize('A', 8, 16).is_none());
    assert!(rasterize(' ', 8, 16).is_none());
}

#[test]
fn rasterize_returns_none_for_zero_dimensions() {
    assert!(rasterize('█', 0, 16).is_none());
    assert!(rasterize('█', 8, 0).is_none());
}

// ── Block element tests ──

#[test]
fn rasterize_full_block() {
    let glyph = rasterize('\u{2588}', 8, 16).expect("full block should rasterize");
    assert_eq!(glyph.width, 8);
    assert_eq!(glyph.height, 16);
    assert_eq!(glyph.format, GlyphFormat::Alpha);
    assert_eq!(glyph.bearing_x, 0);
    assert_eq!(glyph.bearing_y, 0);
    // Every pixel should be fully opaque.
    assert!(glyph.bitmap.iter().all(|&b| b == 255));
}

#[test]
fn rasterize_lower_half() {
    let glyph = rasterize('\u{2584}', 8, 16).expect("lower half should rasterize");
    // Lower half: top 8 rows empty, bottom 8 rows filled.
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(glyph.bitmap[y * 8 + x], 0, "pixel ({x},{y}) should be empty");
        }
    }
    for y in 8..16 {
        for x in 0..8 {
            assert_eq!(
                glyph.bitmap[y * 8 + x],
                255,
                "pixel ({x},{y}) should be filled"
            );
        }
    }
}

#[test]
fn rasterize_upper_half() {
    let glyph = rasterize('\u{2580}', 8, 16).expect("upper half should rasterize");
    // Upper half: top 8 rows filled, bottom 8 rows empty.
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(
                glyph.bitmap[y * 8 + x],
                255,
                "pixel ({x},{y}) should be filled"
            );
        }
    }
    for y in 8..16 {
        for x in 0..8 {
            assert_eq!(glyph.bitmap[y * 8 + x], 0, "pixel ({x},{y}) should be empty");
        }
    }
}

#[test]
fn rasterize_shade_25_percent() {
    let glyph = rasterize('\u{2591}', 8, 16).expect("25% shade should rasterize");
    // 25% shade: all pixels should be ~64 alpha.
    assert!(glyph.bitmap.iter().all(|&b| b == 64));
}

#[test]
fn rasterize_shade_50_percent() {
    let glyph = rasterize('\u{2592}', 8, 16).expect("50% shade should rasterize");
    assert!(glyph.bitmap.iter().all(|&b| b == 128));
}

#[test]
fn rasterize_shade_75_percent() {
    let glyph = rasterize('\u{2593}', 8, 16).expect("75% shade should rasterize");
    assert!(glyph.bitmap.iter().all(|&b| b == 191));
}

#[test]
fn rasterize_right_half() {
    let glyph = rasterize('\u{2590}', 8, 16).expect("right half should rasterize");
    // Right half: left 4 cols empty, right 4 cols filled.
    for y in 0..16 {
        for x in 0..4 {
            assert_eq!(glyph.bitmap[y * 8 + x], 0, "pixel ({x},{y}) should be empty");
        }
        for x in 4..8 {
            assert_eq!(
                glyph.bitmap[y * 8 + x],
                255,
                "pixel ({x},{y}) should be filled"
            );
        }
    }
}

// ── Box drawing tests ──

#[test]
fn rasterize_horizontal_line() {
    let glyph = rasterize('\u{2500}', 8, 16).expect("horizontal line should rasterize");
    assert_eq!(glyph.width, 8);
    assert_eq!(glyph.height, 16);
    // Center row should have filled pixels.
    let cy = 8; // floor(16 / 2)
    let row_start = cy * 8;
    let center_row = &glyph.bitmap[row_start..row_start + 8];
    assert!(
        center_row.iter().all(|&b| b == 255),
        "center row should be fully filled"
    );
    // Top and bottom rows should be empty.
    assert!(
        glyph.bitmap[0..8].iter().all(|&b| b == 0),
        "top row should be empty"
    );
    assert!(
        glyph.bitmap[15 * 8..16 * 8].iter().all(|&b| b == 0),
        "bottom row should be empty"
    );
}

#[test]
fn rasterize_vertical_line() {
    let glyph = rasterize('\u{2502}', 8, 16).expect("vertical line should rasterize");
    // Center column should have filled pixels in every row.
    let cx = 4; // floor(8 / 2)
    for y in 0..16 {
        assert_eq!(
            glyph.bitmap[y * 8 + cx],
            255,
            "pixel ({cx},{y}) should be filled"
        );
    }
    // Leftmost column should be empty.
    for y in 0..16 {
        assert_eq!(
            glyph.bitmap[y * 8],
            0,
            "pixel (0,{y}) should be empty"
        );
    }
}

#[test]
fn rasterize_cross() {
    let glyph = rasterize('\u{253C}', 8, 16).expect("cross should rasterize");
    // Center pixel should be filled.
    let cx = 4;
    let cy = 8;
    assert_eq!(glyph.bitmap[cy * 8 + cx], 255, "center should be filled");
    // Center row should be fully filled (horizontal line).
    let row = &glyph.bitmap[cy * 8..(cy + 1) * 8];
    assert!(row.iter().all(|&b| b == 255), "center row should be filled");
    // Center column should be filled in every row.
    for y in 0..16 {
        assert_eq!(glyph.bitmap[y * 8 + cx], 255, "center col at row {y}");
    }
}

#[test]
fn rasterize_double_horizontal() {
    let glyph = rasterize('\u{2550}', 8, 16).expect("double horizontal should rasterize");
    // Should have two horizontal lines (non-zero pixels above and below center).
    let cy = 8;
    // Center row itself may or may not be filled depending on gap.
    // At least check that there are filled rows above and below center.
    let has_above = (0..cy).any(|y| glyph.bitmap[y * 8 + 4] == 255);
    let has_below = (cy..16).any(|y| glyph.bitmap[y * 8 + 4] == 255);
    assert!(has_above, "double horizontal should have line above center");
    assert!(has_below, "double horizontal should have line below center");
}

#[test]
fn rasterize_rounded_corner() {
    // U+256D ╭ — should produce non-zero output (right + down segments).
    let glyph = rasterize('\u{256D}', 8, 16).expect("rounded corner should rasterize");
    let has_content = glyph.bitmap.iter().any(|&b| b > 0);
    assert!(has_content, "rounded corner should produce visible pixels");
}

#[test]
fn rasterize_diagonal() {
    // U+2571 ╱ — anti-aliased diagonal.
    let glyph = rasterize('\u{2571}', 8, 16).expect("diagonal should rasterize");
    let has_content = glyph.bitmap.iter().any(|&b| b > 0);
    assert!(has_content, "diagonal should produce visible pixels");
    // Check anti-aliasing: should have pixels with intermediate alpha.
    let has_aa = glyph.bitmap.iter().any(|&b| b > 0 && b < 255);
    assert!(has_aa, "diagonal should have anti-aliased pixels");
}

// ── Braille tests ──

#[test]
fn rasterize_braille_empty() {
    // U+2800 — empty braille, no dots.
    let glyph = rasterize('\u{2800}', 8, 16).expect("empty braille should rasterize");
    assert!(
        glyph.bitmap.iter().all(|&b| b == 0),
        "empty braille should have no dots"
    );
}

#[test]
fn rasterize_braille_all_eight_dots() {
    // U+28FF ⣿ — all 8 dots.
    let glyph = rasterize('\u{28FF}', 8, 16).expect("all-dots braille should rasterize");
    let filled_count = glyph.bitmap.iter().filter(|&&b| b > 0).count();
    // 8 dots, each at least 2×2 pixels = minimum 32 filled pixels.
    assert!(
        filled_count >= 32,
        "all-dots braille should have at least 32 filled pixels, got {filled_count}"
    );
}

#[test]
fn rasterize_braille_six_dots() {
    // U+283F ⠿ — lower 6 dots (bits 0–5).
    let glyph = rasterize('\u{283F}', 8, 16).expect("six-dot braille should rasterize");
    let filled_count = glyph.bitmap.iter().filter(|&&b| b > 0).count();
    // 6 dots, each at least 2×2 = minimum 24.
    assert!(
        filled_count >= 24,
        "six-dot braille should have at least 24 filled pixels, got {filled_count}"
    );
}

#[test]
fn rasterize_braille_single_dot() {
    // U+2801 — single dot at position (0,0) = bit 0.
    let glyph = rasterize('\u{2801}', 8, 16).expect("single-dot braille should rasterize");
    let filled_count = glyph.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(
        filled_count >= 4,
        "single-dot braille should have at least 4 filled pixels, got {filled_count}"
    );
}

// ── Powerline tests ──

#[test]
fn rasterize_powerline_right_triangle() {
    // U+E0B0  — solid right-pointing triangle.
    let glyph = rasterize('\u{E0B0}', 8, 16).expect("powerline right triangle should rasterize");
    assert_eq!(glyph.width, 8);
    assert_eq!(glyph.height, 16);
    // Middle row should have the most filled pixels (widest part).
    let mid_row = 8;
    let mid_filled: usize = glyph.bitmap[mid_row * 8..(mid_row + 1) * 8]
        .iter()
        .filter(|&&b| b > 0)
        .count();
    // Top row should have fewer filled pixels.
    let top_filled: usize = glyph.bitmap[0..8].iter().filter(|&&b| b > 0).count();
    assert!(
        mid_filled > top_filled,
        "middle row ({mid_filled}) should be wider than top ({top_filled})"
    );
}

#[test]
fn rasterize_powerline_left_triangle() {
    // U+E0B2 — solid left-pointing triangle.
    let glyph = rasterize('\u{E0B2}', 8, 16).expect("powerline left triangle should rasterize");
    // Middle row should have the most filled pixels.
    let mid_row = 8;
    let mid_filled: usize = glyph.bitmap[mid_row * 8..(mid_row + 1) * 8]
        .iter()
        .filter(|&&b| b > 0)
        .count();
    assert!(
        mid_filled > 4,
        "middle row should be mostly filled, got {mid_filled}/8"
    );
}

#[test]
fn rasterize_powerline_unrecognized_falls_through() {
    // U+E0A0 is in the powerline range but not a handled glyph.
    assert!(
        rasterize('\u{E0A0}', 8, 16).is_none(),
        "unrecognized powerline char should return None"
    );
}

#[test]
fn rasterize_powerline_thin_triangle() {
    // U+E0B1 — right-pointing outline.
    let glyph = rasterize('\u{E0B1}', 8, 16).expect("thin triangle should rasterize");
    let filled_count = glyph.bitmap.iter().filter(|&&b| b > 0).count();
    let total = 8 * 16;
    // Outline should fill much less than solid.
    assert!(
        filled_count < total / 2,
        "outline should fill less than half the cell"
    );
}

// ── Canvas tests ──

#[test]
fn canvas_dimensions() {
    let canvas = Canvas::new(10, 20);
    assert_eq!(canvas.width(), 10);
    assert_eq!(canvas.height(), 20);
}

#[test]
fn canvas_fill_rect_clips() {
    let mut canvas = Canvas::new(8, 8);
    // This should not panic even with out-of-bounds coordinates.
    canvas.fill_rect(-2.0, -2.0, 12.0, 12.0, 255);
    assert!(canvas.data.iter().all(|&b| b == 255));
}

#[test]
fn canvas_blend_pixel_saturates() {
    let mut canvas = Canvas::new(4, 4);
    canvas.blend_pixel(1, 1, 200);
    canvas.blend_pixel(1, 1, 200);
    // Should saturate at 255, not overflow.
    assert_eq!(canvas.data[1 * 4 + 1], 255);
}

#[test]
fn canvas_blend_pixel_out_of_bounds() {
    let mut canvas = Canvas::new(4, 4);
    // Should not panic.
    canvas.blend_pixel(-1, 0, 255);
    canvas.blend_pixel(0, -1, 255);
    canvas.blend_pixel(4, 0, 255);
    canvas.blend_pixel(0, 4, 255);
}

#[test]
fn canvas_fill_line_produces_antialiased_output() {
    let mut canvas = Canvas::new(16, 32);
    canvas.fill_line(0.0, 0.0, 16.0, 32.0, 2.0);
    let has_full = canvas.data.iter().any(|&b| b == 255);
    let has_partial = canvas.data.iter().any(|&b| b > 0 && b < 255);
    assert!(has_full, "anti-aliased line should have fully opaque pixels");
    assert!(
        has_partial,
        "anti-aliased line should have partially transparent pixels"
    );
}

#[test]
fn canvas_into_rasterized_glyph_format() {
    let canvas = Canvas::new(8, 16);
    let glyph = canvas.into_rasterized_glyph();
    assert_eq!(glyph.width, 8);
    assert_eq!(glyph.height, 16);
    assert_eq!(glyph.bearing_x, 0);
    assert_eq!(glyph.bearing_y, 0);
    assert_eq!(glyph.format, GlyphFormat::Alpha);
    assert_eq!(glyph.bitmap.len(), 8 * 16);
}

// ── Decoration rasterization tests ──

use crate::font::CellMetrics;
use super::decorations;

/// Standard test metrics: 8x16 cell, 1px stroke.
fn test_metrics() -> CellMetrics {
    CellMetrics::new(8.0, 16.0, 12.0, 2.0, 1.0, 4.0)
}

#[test]
fn decoration_key_uses_builtin_face() {
    let key = decorations::decoration_key(decorations::CURLY_GLYPH_ID, 1024);
    assert_eq!(key.face_idx, FaceIdx::BUILTIN);
    assert_eq!(key.glyph_id, decorations::CURLY_GLYPH_ID);
}

#[test]
fn decoration_keys_are_distinct() {
    let k1 = decorations::decoration_key(decorations::CURLY_GLYPH_ID, 1024);
    let k2 = decorations::decoration_key(decorations::DOTTED_GLYPH_ID, 1024);
    let k3 = decorations::decoration_key(decorations::DASHED_GLYPH_ID, 1024);
    assert_ne!(k1.glyph_id, k2.glyph_id);
    assert_ne!(k2.glyph_id, k3.glyph_id);
}

#[test]
fn rasterize_curly_produces_bitmap() {
    let m = test_metrics();
    let glyph = decorations::rasterize_curly(&m).expect("curly should rasterize");
    assert_eq!(glyph.width, 8);
    assert!(glyph.height > 0);
    assert_eq!(glyph.format, GlyphFormat::Alpha);
    assert_eq!(glyph.bitmap.len(), (glyph.width * glyph.height) as usize);
    // Should have some filled pixels.
    assert!(glyph.bitmap.iter().any(|&b| b > 0), "curly should have visible pixels");
}

#[test]
fn rasterize_curly_has_wave_pattern() {
    let m = test_metrics();
    let glyph = decorations::rasterize_curly(&m).expect("curly should rasterize");
    // The sine wave should create filled pixels in multiple rows (not just one).
    let filled_rows: Vec<u32> = (0..glyph.height)
        .filter(|&y| {
            let start = (y * glyph.width) as usize;
            let end = start + glyph.width as usize;
            glyph.bitmap[start..end].iter().any(|&b| b > 0)
        })
        .collect();
    assert!(
        filled_rows.len() >= 3,
        "curly wave should span at least 3 rows, got {}",
        filled_rows.len()
    );
}

#[test]
fn rasterize_dotted_produces_bitmap() {
    let m = test_metrics();
    let glyph = decorations::rasterize_dotted(&m).expect("dotted should rasterize");
    assert_eq!(glyph.width, 8);
    assert!(glyph.height > 0);
    // Should have alternating pattern: ~4 filled columns, ~4 empty.
    let filled_cols: usize = (0..glyph.width as usize)
        .filter(|&x| glyph.bitmap[x] > 0)
        .count();
    assert_eq!(filled_cols, 4, "dotted: 4 of 8 columns should be filled (step_by 2)");
}

#[test]
fn rasterize_dashed_produces_bitmap() {
    let m = test_metrics();
    let glyph = decorations::rasterize_dashed(&m).expect("dashed should rasterize");
    assert_eq!(glyph.width, 8);
    assert!(glyph.height > 0);
    // Pattern: 3 on, 2 off, 3 on → 6 filled of 8.
    let filled_cols: usize = (0..glyph.width as usize)
        .filter(|&x| glyph.bitmap[x] > 0)
        .count();
    assert_eq!(filled_cols, 6, "dashed: 6 of 8 columns should be filled (3-on-2-off)");
}

#[test]
fn rasterize_zero_width_returns_none() {
    let m = CellMetrics::new(0.1, 16.0, 12.0, 2.0, 1.0, 4.0);
    // 0.1 rounds to 0 → should return None.
    assert!(decorations::rasterize_curly(&m).is_none());
    assert!(decorations::rasterize_dotted(&m).is_none());
    assert!(decorations::rasterize_dashed(&m).is_none());
}

#[test]
fn rasterize_thick_stroke_produces_taller_curly() {
    let thin = CellMetrics::new(8.0, 16.0, 12.0, 2.0, 1.0, 4.0);
    let thick = CellMetrics::new(8.0, 16.0, 12.0, 2.0, 3.0, 4.0);
    let g_thin = decorations::rasterize_curly(&thin).unwrap();
    let g_thick = decorations::rasterize_curly(&thick).unwrap();
    assert!(
        g_thick.height > g_thin.height,
        "thicker stroke should produce taller curly bitmap"
    );
}
