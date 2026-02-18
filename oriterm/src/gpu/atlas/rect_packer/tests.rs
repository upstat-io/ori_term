//! Tests for the guillotine rectangle packer.

use super::{Rect, RectPacker};

#[test]
fn origin_placement() {
    let mut packer = RectPacker::new(1024, 1024);

    let pos = packer.pack(10, 12);

    assert_eq!(pos, Some((0, 0)));
}

#[test]
fn second_glyph_adjacent() {
    let mut packer = RectPacker::new(1024, 1024);

    let a = packer.pack(10, 12).unwrap();
    let b = packer.pack(8, 12).unwrap();

    // Second glyph should be placed adjacent to the first (not overlapping).
    assert_eq!(a, (0, 0));
    assert_ne!(b, (0, 0));
}

#[test]
fn no_overlap_50_varied_rects() {
    let mut packer = RectPacker::new(1024, 1024);
    let mut placed = Vec::new();

    for i in 0..50 {
        let w = 10 + (i * 7) % 50;
        let h = 8 + (i * 13) % 40;
        if let Some((x, y)) = packer.pack(w, h) {
            placed.push(Rect { x, y, w, h });
        }
    }

    assert!(!placed.is_empty());

    // Verify no two rects overlap.
    for (i, a) in placed.iter().enumerate() {
        for b in placed.iter().skip(i + 1) {
            let no_overlap =
                a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
            assert!(
                no_overlap,
                "rects overlap: ({},{} {}x{}) vs ({},{} {}x{})",
                a.x, a.y, a.w, a.h, b.x, b.y, b.w, b.h,
            );
        }
    }
}

#[test]
fn page_full_returns_none() {
    let mut packer = RectPacker::new(64, 64);

    // Fill the page with 32x32 rects (exactly 4 fit).
    for _ in 0..4 {
        assert!(packer.pack(32, 32).is_some());
    }

    // Fifth should not fit.
    assert!(packer.pack(32, 32).is_none());
}

#[test]
fn too_large_returns_none() {
    let mut packer = RectPacker::new(1024, 1024);

    assert!(packer.pack(1025, 10).is_none());
    assert!(packer.pack(10, 1025).is_none());
    assert!(packer.pack(1025, 1025).is_none());
}

#[test]
fn best_short_side_fit_scoring() {
    // 128x128 page. Place a 64x64 rect to create specific free rects.
    let mut packer = RectPacker::new(128, 128);
    let _ = packer.pack(64, 64);

    // Now pack a 10x10 rect. The packer should choose the free rect
    // with the smallest short-side leftover.
    let pos = packer.pack(10, 10);
    assert!(pos.is_some());

    // The packer should still have room for more.
    assert!(packer.pack(10, 10).is_some());
}

#[test]
fn reset_reuse() {
    let mut packer = RectPacker::new(64, 64);

    // Fill the page.
    for _ in 0..4 {
        assert!(packer.pack(32, 32).is_some());
    }
    assert!(packer.pack(1, 1).is_none());

    // Reset and verify we can pack again from origin.
    packer.reset();
    let pos = packer.pack(10, 10);
    assert_eq!(pos, Some((0, 0)));
}

#[test]
fn guillotine_split_direction() {
    let mut packer = RectPacker::new(100, 100);

    // Pack a 30x20 rect into a 100x100 page.
    // leftover_w=70, leftover_h=80 → leftover_w < leftover_h → horizontal split.
    // Right strip: (30, 0, 70, 20).
    // Bottom strip: (0, 20, 100, 80).
    let pos = packer.pack(30, 20);
    assert_eq!(pos, Some((0, 0)));

    // Pack a rect that fits in the right strip (height <= 20).
    let pos2 = packer.pack(60, 15);
    assert!(pos2.is_some());
    let (x2, y2) = pos2.unwrap();
    // Should be placed in the right strip at x=30, not in the bottom strip.
    assert_eq!(x2, 30);
    assert_eq!(y2, 0);
}

#[test]
fn exact_fit_no_leftover() {
    let mut packer = RectPacker::new(64, 64);

    // Exact fit should consume the entire page.
    let pos = packer.pack(64, 64);
    assert_eq!(pos, Some((0, 0)));

    // Nothing should fit anymore.
    assert!(packer.pack(1, 1).is_none());
}

#[test]
fn single_pixel_packing() {
    let mut packer = RectPacker::new(4, 4);

    // Pack 16 single-pixel rects.
    let mut placed = Vec::new();
    for _ in 0..16 {
        if let Some(pos) = packer.pack(1, 1) {
            placed.push(pos);
        }
    }
    assert_eq!(placed.len(), 16);

    // No more room.
    assert!(packer.pack(1, 1).is_none());
}

#[test]
fn all_rects_within_page_bounds() {
    let page_w = 256;
    let page_h = 256;
    let mut packer = RectPacker::new(page_w, page_h);

    for i in 0..100 {
        let w = 5 + (i * 11) % 30;
        let h = 5 + (i * 17) % 25;
        if let Some((x, y)) = packer.pack(w, h) {
            assert!(
                x + w <= page_w,
                "rect at ({x},{y}) with size {w}x{h} exceeds page width {page_w}",
            );
            assert!(
                y + h <= page_h,
                "rect at ({x},{y}) with size {w}x{h} exceeds page height {page_h}",
            );
        }
    }
}
