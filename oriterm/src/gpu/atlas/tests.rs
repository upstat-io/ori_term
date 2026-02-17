//! Tests for the glyph atlas.

use crate::font::collection::RasterizedGlyph;
use crate::font::{GlyphFormat, RasterKey};
use crate::gpu::state::GpuState;

use super::{try_pack_in_page, GlyphAtlas, Shelf, GLYPH_PADDING, PAGE_SIZE};

// ── Helpers ──

fn test_glyph(width: u32, height: u32) -> RasterizedGlyph {
    RasterizedGlyph {
        width,
        height,
        bearing_x: 0,
        bearing_y: height as i32,
        advance: width as f32,
        format: GlyphFormat::Alpha,
        bitmap: vec![0xFF; (width * height) as usize],
    }
}

fn test_key(glyph_id: u16) -> RasterKey {
    RasterKey {
        glyph_id,
        face_idx: 0,
        size_q6: 896, // ~14px
    }
}

// ── Packing logic (no GPU) ──

#[test]
fn pack_first_glyph_on_empty_page() {
    let mut shelves = vec![];

    let result = try_pack_in_page(&mut shelves, 10, 12, 1024);

    assert_eq!(result, Some((0, 0)));
    assert_eq!(shelves.len(), 1);
    assert_eq!(shelves[0].y, 0);
    assert_eq!(shelves[0].height, 12);
    assert_eq!(shelves[0].x_cursor, 10);
}

#[test]
fn pack_second_glyph_same_shelf() {
    let mut shelves = vec![Shelf {
        y: 0,
        height: 12,
        x_cursor: 10,
    }];

    let result = try_pack_in_page(&mut shelves, 8, 12, 1024);

    assert_eq!(result, Some((10, 0)));
    assert_eq!(shelves.len(), 1);
    assert_eq!(shelves[0].x_cursor, 18);
}

#[test]
fn pack_tall_glyph_creates_new_shelf() {
    let mut shelves = vec![Shelf {
        y: 0,
        height: 12,
        x_cursor: 10,
    }];

    let result = try_pack_in_page(&mut shelves, 8, 20, 1024);

    assert_eq!(result, Some((0, 12)));
    assert_eq!(shelves.len(), 2);
    assert_eq!(shelves[1].y, 12);
    assert_eq!(shelves[1].height, 20);
}

#[test]
fn pack_best_fit_selects_smallest_sufficient_shelf() {
    let mut shelves = vec![
        Shelf {
            y: 0,
            height: 20,
            x_cursor: 100,
        },
        Shelf {
            y: 20,
            height: 12,
            x_cursor: 100,
        },
        Shelf {
            y: 32,
            height: 15,
            x_cursor: 100,
        },
    ];

    // Glyph needs height 11 — shelf 1 (height 12) is best fit.
    let result = try_pack_in_page(&mut shelves, 10, 11, 1024);

    assert_eq!(result, Some((100, 20)));
    assert_eq!(shelves[1].x_cursor, 110);
}

#[test]
fn pack_returns_none_when_page_full() {
    let mut shelves = vec![Shelf {
        y: 0,
        height: 1024,
        x_cursor: 1024,
    }];

    let result = try_pack_in_page(&mut shelves, 10, 10, 1024);

    assert!(result.is_none());
}

#[test]
fn pack_returns_none_when_no_vertical_room() {
    let mut shelves = vec![Shelf {
        y: 0,
        height: 1000,
        x_cursor: 1024, // Full horizontally.
    }];

    // Only 24 pixels of vertical room remain, glyph needs 30.
    let result = try_pack_in_page(&mut shelves, 10, 30, 1024);

    assert!(result.is_none());
}

#[test]
fn pack_fits_in_remaining_vertical_space() {
    let mut shelves = vec![Shelf {
        y: 0,
        height: 1000,
        x_cursor: 1024,
    }];

    // 24 pixels remain, glyph needs 20 — fits.
    let result = try_pack_in_page(&mut shelves, 10, 20, 1024);

    assert_eq!(result, Some((0, 1000)));
    assert_eq!(shelves.len(), 2);
}

#[test]
fn pack_overflows_horizontally_to_new_shelf() {
    let mut shelves = vec![Shelf {
        y: 0,
        height: 12,
        x_cursor: 1020,
    }];

    // Only 4 pixels of horizontal room — glyph needs 5.
    let result = try_pack_in_page(&mut shelves, 5, 12, 1024);

    assert_eq!(result, Some((0, 12)));
    assert_eq!(shelves.len(), 2);
}

#[test]
fn pack_exact_fit_at_horizontal_boundary() {
    let mut shelves = vec![Shelf {
        y: 0,
        height: 12,
        x_cursor: 1014,
    }];

    // Exactly 10 pixels remain — glyph needs 10.
    let result = try_pack_in_page(&mut shelves, 10, 12, 1024);

    assert_eq!(result, Some((1014, 0)));
    assert_eq!(shelves[0].x_cursor, 1024);
}

// ── GPU integration tests ──

#[test]
fn atlas_creation_succeeds() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let atlas = GlyphAtlas::new(&gpu.device);

    assert_eq!(atlas.page_count(), 1);
    assert!(atlas.is_empty());
    assert_eq!(atlas.len(), 0);
}

#[test]
fn insert_and_lookup_round_trip() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let key = test_key(65); // 'A'
    let glyph = test_glyph(8, 14);

    let entry = atlas.insert(key, &glyph, &gpu.device, &gpu.queue);
    assert!(entry.is_some());

    let looked_up = atlas.lookup(key);
    assert!(looked_up.is_some());

    let e = entry.unwrap();
    let l = looked_up.unwrap();
    assert_eq!(e.page, l.page);
    assert_eq!(e.width, 8);
    assert_eq!(e.height, 14);
    assert_eq!(e.bearing_y, 14);
}

#[test]
fn insert_zero_size_returns_none() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let key = test_key(32); // space
    let glyph = test_glyph(0, 0);

    assert!(atlas.insert(key, &glyph, &gpu.device, &gpu.queue).is_none());
    assert!(atlas.lookup(key).is_none());
}

#[test]
fn insert_duplicate_returns_cached() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let key = test_key(65);
    let glyph = test_glyph(8, 14);

    let first = atlas.insert(key, &glyph, &gpu.device, &gpu.queue).unwrap();
    let second = atlas.insert(key, &glyph, &gpu.device, &gpu.queue).unwrap();

    // Same UV coordinates — same cached entry.
    assert_eq!(first.uv_x, second.uv_x);
    assert_eq!(first.uv_y, second.uv_y);
    assert_eq!(atlas.len(), 1);
}

#[test]
fn uv_coordinates_are_normalized() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let key = test_key(65);
    let glyph = test_glyph(8, 14);

    let entry = atlas.insert(key, &glyph, &gpu.device, &gpu.queue).unwrap();

    assert!(entry.uv_x >= 0.0 && entry.uv_x <= 1.0);
    assert!(entry.uv_y >= 0.0 && entry.uv_y <= 1.0);
    assert!(entry.uv_w > 0.0 && entry.uv_w <= 1.0);
    assert!(entry.uv_h > 0.0 && entry.uv_h <= 1.0);

    let expected_w = 8.0 / PAGE_SIZE as f32;
    let expected_h = 14.0 / PAGE_SIZE as f32;
    assert!((entry.uv_w - expected_w).abs() < f32::EPSILON);
    assert!((entry.uv_h - expected_h).abs() < f32::EPSILON);
}

#[test]
fn clear_resets_atlas_state() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let key = test_key(65);
    let glyph = test_glyph(8, 14);

    atlas.insert(key, &glyph, &gpu.device, &gpu.queue);
    assert_eq!(atlas.len(), 1);

    atlas.clear();

    assert!(atlas.is_empty());
    assert!(atlas.lookup(key).is_none());
    assert_eq!(atlas.page_count(), 1);
}

#[test]
fn insert_many_glyphs_fits_on_one_page() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);

    // Insert 95 ASCII glyphs (0x20–0x7E), each 8×14.
    // Padded: 9×15. Per shelf: floor(1024/9) = 113. One shelf suffices.
    for glyph_id in 0x20u16..=0x7Eu16 {
        let key = test_key(glyph_id);
        let glyph = test_glyph(8, 14);
        atlas.insert(key, &glyph, &gpu.device, &gpu.queue);
    }

    assert_eq!(atlas.len(), 95);
    assert_eq!(atlas.page_count(), 1);
}

#[test]
fn insert_triggers_new_page_allocation() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);

    // Fill page 0 with 100×100 glyphs. Padded: 101×101.
    // Per shelf: floor(1024/101) = 10. Shelves: floor(1024/101) = 10.
    // Total per page: 10 × 10 = 100.
    for i in 0..100u16 {
        let key = test_key(i);
        let glyph = test_glyph(100, 100);
        atlas.insert(key, &glyph, &gpu.device, &gpu.queue);
    }
    assert_eq!(atlas.page_count(), 1);

    // The 101st glyph should trigger page 2.
    let key = test_key(100);
    let glyph = test_glyph(100, 100);
    let entry = atlas.insert(key, &glyph, &gpu.device, &gpu.queue).unwrap();

    assert_eq!(atlas.page_count(), 2);
    assert_eq!(entry.page, 1);
}

#[test]
fn insert_oversized_glyph_returns_none() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let key = test_key(1);
    // Width exceeds max (PAGE_SIZE - GLYPH_PADDING = 1023).
    let glyph = test_glyph(PAGE_SIZE, 1);

    assert!(atlas.insert(key, &glyph, &gpu.device, &gpu.queue).is_none());
}

#[test]
fn primary_view_returns_page_zero() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let atlas = GlyphAtlas::new(&gpu.device);

    let _view = atlas.primary_view();
    assert!(atlas.page_view(0).is_some());
    assert!(atlas.page_view(1).is_none());
}

#[test]
fn clear_after_multi_page_resets_to_one_page() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);

    // Trigger multi-page allocation (same as insert_triggers_new_page).
    for i in 0..=100u16 {
        let key = test_key(i);
        let glyph = test_glyph(100, 100);
        atlas.insert(key, &glyph, &gpu.device, &gpu.queue);
    }
    assert_eq!(atlas.page_count(), 2);

    atlas.clear();

    assert!(atlas.is_empty());
    assert_eq!(atlas.page_count(), 1);
}

#[test]
fn glyphs_do_not_overlap() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let ps = PAGE_SIZE as f32;

    // Insert several glyphs of different sizes.
    let sizes = [(8, 14), (16, 20), (12, 12), (10, 18), (24, 30)];
    let mut entries = Vec::new();

    for (i, &(w, h)) in sizes.iter().enumerate() {
        let key = test_key(i as u16);
        let glyph = test_glyph(w, h);
        if let Some(entry) = atlas.insert(key, &glyph, &gpu.device, &gpu.queue) {
            entries.push(entry);
        }
    }

    // Verify no two entries overlap (on the same page).
    for (i, a) in entries.iter().enumerate() {
        for b in entries.iter().skip(i + 1) {
            if a.page != b.page {
                continue;
            }
            let a_x = a.uv_x * ps;
            let a_y = a.uv_y * ps;
            let b_x = b.uv_x * ps;
            let b_y = b.uv_y * ps;

            let no_overlap = a_x + a.width as f32 + GLYPH_PADDING as f32 <= b_x
                || b_x + b.width as f32 + GLYPH_PADDING as f32 <= a_x
                || a_y + a.height as f32 + GLYPH_PADDING as f32 <= b_y
                || b_y + b.height as f32 + GLYPH_PADDING as f32 <= a_y;

            assert!(
                no_overlap,
                "glyphs {i} and {} overlap: ({a_x},{a_y} {}×{}) vs ({b_x},{b_y} {}×{})",
                i + 1,
                a.width,
                a.height,
                b.width,
                b.height,
            );
        }
    }
}
