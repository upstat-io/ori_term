//! Tests for the glyph atlas.

use crate::font::collection::RasterizedGlyph;
use crate::font::{FaceIdx, GlyphFormat, RasterKey};
use crate::gpu::state::GpuState;

use super::{GlyphAtlas, GLYPH_PADDING, PAGE_SIZE};

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
        face_idx: FaceIdx::REGULAR,
        size_q6: 896, // ~14px
    }
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

    let entry = atlas.insert(key, &glyph, &gpu.queue);
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
fn insert_zero_size_returns_none_and_caches_as_empty() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let key = test_key(32); // space
    let glyph = test_glyph(0, 0);

    assert!(!atlas.is_known_empty(key));
    assert!(atlas.insert(key, &glyph, &gpu.queue).is_none());
    assert!(atlas.lookup(key).is_none());
    assert!(atlas.is_known_empty(key));
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

    let first = atlas.insert(key, &glyph, &gpu.queue).unwrap();
    let second = atlas.insert(key, &glyph, &gpu.queue).unwrap();

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

    let entry = atlas.insert(key, &glyph, &gpu.queue).unwrap();

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

    atlas.insert(key, &glyph, &gpu.queue);
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

    // Insert 95 ASCII glyphs (0x20–0x7E), each 8×14. Guillotine packer
    // on a 2048×2048 page handles these easily.
    for glyph_id in 0x20u16..=0x7Eu16 {
        let key = test_key(glyph_id);
        let glyph = test_glyph(8, 14);
        atlas.insert(key, &glyph, &gpu.queue);
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

    // Fill page 0 with 200×200 glyphs. Padded: 201×201.
    // Per row: floor(2048/201) = 10. Columns: floor(2048/201) = 10.
    // Total per page: 10 × 10 = 100.
    for i in 0..100u16 {
        let key = test_key(i);
        let glyph = test_glyph(200, 200);
        atlas.insert(key, &glyph, &gpu.queue);
    }
    assert_eq!(atlas.page_count(), 1);

    // The 101st glyph should trigger page 2.
    let key = test_key(100);
    let glyph = test_glyph(200, 200);
    let entry = atlas.insert(key, &glyph, &gpu.queue).unwrap();

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
    // Width exceeds max (PAGE_SIZE - GLYPH_PADDING = 2047).
    let glyph = test_glyph(PAGE_SIZE, 1);

    assert!(atlas.insert(key, &glyph, &gpu.queue).is_none());
}

#[test]
fn view_returns_d2_array_view() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let atlas = GlyphAtlas::new(&gpu.device);

    // Should not panic.
    let _view = atlas.view();
}

#[test]
fn clear_after_multi_page_resets_to_one_page() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);

    // Trigger multi-page allocation.
    for i in 0..=100u16 {
        let key = test_key(i);
        let glyph = test_glyph(200, 200);
        atlas.insert(key, &glyph, &gpu.queue);
    }
    assert!(atlas.page_count() >= 2);

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
        if let Some(entry) = atlas.insert(key, &glyph, &gpu.queue) {
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

#[test]
fn reinsert_after_clear_packs_from_origin() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);

    // Fill with some glyphs at various positions.
    for i in 0..10u16 {
        let key = test_key(i);
        let glyph = test_glyph(20, 20);
        atlas.insert(key, &glyph, &gpu.queue);
    }
    assert_eq!(atlas.len(), 10);

    atlas.clear();

    // Re-insert should pack from origin.
    let key = test_key(100);
    let glyph = test_glyph(8, 14);
    let entry = atlas
        .insert(key, &glyph, &gpu.queue)
        .unwrap();

    assert_eq!(entry.page, 0);
    assert!((entry.uv_x).abs() < f32::EPSILON);
    assert!((entry.uv_y).abs() < f32::EPSILON);
}

#[test]
fn insert_at_max_dimension_succeeds() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let max_dim = PAGE_SIZE - GLYPH_PADDING;

    let key = test_key(1);
    let glyph = test_glyph(max_dim, max_dim);

    // A glyph exactly at the max dimension should succeed.
    assert!(atlas.insert(key, &glyph, &gpu.queue).is_some());
}

#[test]
fn insert_one_over_max_dimension_fails() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let over = PAGE_SIZE - GLYPH_PADDING + 1;

    // Width one pixel over the max should fail.
    let key_w = test_key(1);
    let glyph_w = test_glyph(over, 1);
    assert!(atlas
        .insert(key_w, &glyph_w, &gpu.queue)
        .is_none());

    // Height one pixel over the max should also fail.
    let key_h = test_key(2);
    let glyph_h = test_glyph(1, over);
    assert!(atlas
        .insert(key_h, &glyph_h, &gpu.queue)
        .is_none());
}

#[test]
fn insert_zero_width_nonzero_height_returns_none() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let key = test_key(1);
    let glyph = test_glyph(0, 14);

    assert!(atlas.insert(key, &glyph, &gpu.queue).is_none());
    assert!(atlas.lookup(key).is_none());
    assert!(atlas.is_known_empty(key));
}

#[test]
fn insert_nonzero_width_zero_height_returns_none() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let key = test_key(1);
    let glyph = test_glyph(8, 0);

    assert!(atlas.insert(key, &glyph, &gpu.queue).is_none());
    assert!(atlas.lookup(key).is_none());
    assert!(atlas.is_known_empty(key));
}

#[test]
fn is_known_empty_false_for_unseen_key() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let atlas = GlyphAtlas::new(&gpu.device);

    assert!(!atlas.is_known_empty(test_key(99)));
}

#[test]
fn is_known_empty_false_for_normal_glyph() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let key = test_key(65);
    let glyph = test_glyph(8, 14);

    atlas.insert(key, &glyph, &gpu.queue);

    assert!(!atlas.is_known_empty(key));
}

#[test]
fn clear_resets_empty_keys() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let key = test_key(32);
    let glyph = test_glyph(0, 0);

    atlas.insert(key, &glyph, &gpu.queue);
    assert!(atlas.is_known_empty(key));

    atlas.clear();

    assert!(!atlas.is_known_empty(key));
}

#[test]
fn repeated_insert_of_empty_glyph_is_idempotent() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let key = test_key(32);
    let glyph = test_glyph(0, 0);

    // Insert the same zero-size glyph multiple times.
    for _ in 0..5 {
        assert!(atlas.insert(key, &glyph, &gpu.queue).is_none());
    }

    assert!(atlas.is_known_empty(key));
    // Zero-size glyphs don't count as cached entries.
    assert_eq!(atlas.len(), 0);
}

// ── LRU and frame counter tests ──

#[test]
fn begin_frame_increments_counter() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);

    assert_eq!(atlas.frame_counter(), 0);
    atlas.begin_frame();
    assert_eq!(atlas.frame_counter(), 1);
    atlas.begin_frame();
    assert_eq!(atlas.frame_counter(), 2);
}

#[test]
fn lru_eviction_evicts_oldest_page() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);

    // Fill all 4 pages with large glyphs (one page-filling glyph each),
    // advancing the frame counter between pages.
    let max_dim = PAGE_SIZE - GLYPH_PADDING;
    for i in 0..4u16 {
        atlas.begin_frame();
        let key = test_key(i);
        let glyph = test_glyph(max_dim, max_dim);
        atlas.insert(key, &glyph, &gpu.queue);
    }
    assert_eq!(atlas.page_count(), 4);

    // All 4 pages are full. Page 0 was used at frame 1 (oldest).
    // Inserting another glyph should evict page 0.
    atlas.begin_frame();
    let key = test_key(10);
    let glyph = test_glyph(8, 14);
    let entry = atlas.insert(key, &glyph, &gpu.queue).unwrap();

    // The new glyph should be on page 0 (the evicted page).
    assert_eq!(entry.page, 0);

    // Page 0's original glyph should be gone.
    assert!(atlas.lookup(test_key(0)).is_none());

    // Other pages' glyphs should still exist.
    assert!(atlas.lookup(test_key(1)).is_some());
    assert!(atlas.lookup(test_key(2)).is_some());
    assert!(atlas.lookup(test_key(3)).is_some());
}

#[test]
fn lru_eviction_preserves_newer_pages() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);
    let max_dim = PAGE_SIZE - GLYPH_PADDING;

    // Fill 4 pages.
    for i in 0..4u16 {
        atlas.begin_frame();
        let key = test_key(i);
        let glyph = test_glyph(max_dim, max_dim);
        atlas.insert(key, &glyph, &gpu.queue);
    }

    // Touch page 0 by looking up its glyph, making it the most recently used.
    atlas.begin_frame();
    let page = atlas.lookup(test_key(0)).map(|e| e.page);
    assert!(page.is_some());
    atlas.touch_page(page.unwrap());

    // Now page 1 is the oldest (frame 2). Inserting should evict page 1.
    atlas.begin_frame();
    let key = test_key(10);
    let glyph = test_glyph(8, 14);
    let entry = atlas.insert(key, &glyph, &gpu.queue).unwrap();

    assert_eq!(entry.page, 1);
    assert!(atlas.lookup(test_key(1)).is_none()); // evicted
    assert!(atlas.lookup(test_key(0)).is_some()); // preserved (touched)
    assert!(atlas.lookup(test_key(2)).is_some()); // preserved
    assert!(atlas.lookup(test_key(3)).is_some()); // preserved
}

#[test]
fn q6_keying_distinct_sizes() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let mut atlas = GlyphAtlas::new(&gpu.device);

    // Same glyph_id but different size_q6 → different cache entries.
    let key_14 = RasterKey {
        glyph_id: 65,
        face_idx: FaceIdx::REGULAR,
        size_q6: 896, // ~14px
    };
    let key_16 = RasterKey {
        glyph_id: 65,
        face_idx: FaceIdx::REGULAR,
        size_q6: 1024, // ~16px
    };

    let glyph_14 = test_glyph(8, 14);
    let glyph_16 = test_glyph(9, 16);

    let e14 = atlas
        .insert(key_14, &glyph_14, &gpu.queue)
        .unwrap();
    let e16 = atlas
        .insert(key_16, &glyph_16, &gpu.queue)
        .unwrap();

    assert_eq!(atlas.len(), 2);
    assert_ne!(e14.uv_x, e16.uv_x);
    assert_eq!(e14.width, 8);
    assert_eq!(e16.width, 9);
}
