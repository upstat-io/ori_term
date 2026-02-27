//! Tests for `PaneRenderCache`.

use oriterm_core::Rgb;
use oriterm_mux::id::PaneId;
use oriterm_mux::layout::{PaneLayout, Rect};

use super::PaneRenderCache;
use crate::gpu::frame_input::ViewportSize;
use crate::gpu::instance_writer::ScreenRect;
use crate::gpu::prepared_frame::PreparedFrame;

fn make_layout(pane_id: PaneId, x: f32, y: f32, w: f32, h: f32) -> PaneLayout {
    PaneLayout {
        pane_id,
        pixel_rect: Rect {
            x,
            y,
            width: w,
            height: h,
        },
        cols: (w / 8.0) as u16,
        rows: (h / 16.0) as u16,
        is_focused: true,
        is_floating: false,
    }
}

/// Push a single background rect so we can detect whether prepare_fn was called.
fn push_marker(frame: &mut PreparedFrame, x: f32) {
    frame.backgrounds.push_rect(
        ScreenRect {
            x,
            y: 0.0,
            w: 8.0,
            h: 16.0,
        },
        Rgb { r: 255, g: 0, b: 0 },
        1.0,
    );
}

#[test]
fn clean_pane_returns_cached_frame() {
    let mut cache = PaneRenderCache::new();
    let id = PaneId::from_raw(1);
    let layout = make_layout(id, 0.0, 0.0, 640.0, 480.0);

    // First call: dirty=true → prepare_fn is called.
    let mut called = false;
    let frame = cache.get_or_prepare(id, &layout, true, |f| {
        called = true;
        push_marker(f, 42.0);
    });
    assert!(called, "prepare_fn should be called on first access");
    assert_eq!(frame.backgrounds.len(), 1);

    // Second call: dirty=false, same layout → cached, prepare_fn NOT called.
    let mut called = false;
    let frame = cache.get_or_prepare(id, &layout, false, |_f| {
        called = true;
    });
    assert!(!called, "prepare_fn should NOT be called for clean pane");
    assert_eq!(frame.backgrounds.len(), 1, "cached frame preserved");
}

#[test]
fn dirty_pane_calls_prepare_fn() {
    let mut cache = PaneRenderCache::new();
    let id = PaneId::from_raw(1);
    let layout = make_layout(id, 0.0, 0.0, 640.0, 480.0);

    // Seed cache.
    cache.get_or_prepare(id, &layout, true, |f| push_marker(f, 1.0));

    // Dirty=true → re-prepare.
    let mut called = false;
    let frame = cache.get_or_prepare(id, &layout, true, |f| {
        called = true;
        push_marker(f, 2.0);
        push_marker(f, 3.0);
    });
    assert!(called, "prepare_fn should be called for dirty pane");
    assert_eq!(frame.backgrounds.len(), 2, "old instances replaced");
}

#[test]
fn layout_change_triggers_reprepare() {
    let mut cache = PaneRenderCache::new();
    let id = PaneId::from_raw(1);
    let layout_a = make_layout(id, 0.0, 0.0, 640.0, 480.0);
    let layout_b = make_layout(id, 0.0, 0.0, 800.0, 600.0);

    // Seed cache with layout_a.
    cache.get_or_prepare(id, &layout_a, true, |f| push_marker(f, 1.0));

    // Clean but layout changed → re-prepare.
    let mut called = false;
    let frame = cache.get_or_prepare(id, &layout_b, false, |f| {
        called = true;
        push_marker(f, 2.0);
    });
    assert!(called, "layout change should trigger re-prepare");
    assert_eq!(frame.backgrounds.len(), 1);
}

#[test]
fn invalidate_all_forces_reprepare() {
    let mut cache = PaneRenderCache::new();
    let id1 = PaneId::from_raw(1);
    let id2 = PaneId::from_raw(2);
    let layout1 = make_layout(id1, 0.0, 0.0, 640.0, 480.0);
    let layout2 = make_layout(id2, 640.0, 0.0, 640.0, 480.0);

    // Seed cache for both panes.
    cache.get_or_prepare(id1, &layout1, true, |f| push_marker(f, 1.0));
    cache.get_or_prepare(id2, &layout2, true, |f| push_marker(f, 2.0));

    cache.invalidate_all();

    // Both panes should re-prepare despite dirty=false, same layout.
    let mut called1 = false;
    cache.get_or_prepare(id1, &layout1, false, |f| {
        called1 = true;
        push_marker(f, 10.0);
    });
    let mut called2 = false;
    cache.get_or_prepare(id2, &layout2, false, |f| {
        called2 = true;
        push_marker(f, 20.0);
    });
    assert!(called1, "pane 1 should re-prepare after invalidate_all");
    assert!(called2, "pane 2 should re-prepare after invalidate_all");
}

#[test]
fn remove_frees_entry() {
    let mut cache = PaneRenderCache::new();
    let id = PaneId::from_raw(1);
    let layout = make_layout(id, 0.0, 0.0, 640.0, 480.0);

    cache.get_or_prepare(id, &layout, true, |f| push_marker(f, 1.0));
    cache.remove(id);

    // Next access should call prepare_fn (entry gone).
    let mut called = false;
    cache.get_or_prepare(id, &layout, false, |f| {
        called = true;
        push_marker(f, 2.0);
    });
    assert!(called, "removed pane should re-prepare");
}

#[test]
fn extend_from_merges_cached_frames() {
    let mut cache = PaneRenderCache::new();
    let id1 = PaneId::from_raw(1);
    let id2 = PaneId::from_raw(2);
    let layout1 = make_layout(id1, 0.0, 0.0, 320.0, 240.0);
    let layout2 = make_layout(id2, 320.0, 0.0, 320.0, 240.0);

    cache.get_or_prepare(id1, &layout1, true, |f| {
        push_marker(f, 0.0);
        push_marker(f, 8.0);
    });
    cache.get_or_prepare(id2, &layout2, true, |f| {
        push_marker(f, 320.0);
    });

    // Merge both cached frames into a main frame.
    let viewport = ViewportSize::new(640, 240);
    let mut main = PreparedFrame::new(viewport, Rgb { r: 0, g: 0, b: 0 }, 1.0);

    let f1 = cache.get_or_prepare(id1, &layout1, false, |_| {});
    main.extend_from(f1);
    let f2 = cache.get_or_prepare(id2, &layout2, false, |_| {});
    main.extend_from(f2);

    assert_eq!(main.backgrounds.len(), 3, "2 from pane1 + 1 from pane2");
}

#[test]
fn position_change_same_size_triggers_reprepare() {
    let mut cache = PaneRenderCache::new();
    let id = PaneId::from_raw(1);
    let layout_a = make_layout(id, 0.0, 0.0, 640.0, 480.0);
    // Same dimensions but different position (pane shifted right after sibling closed).
    let layout_b = make_layout(id, 320.0, 0.0, 640.0, 480.0);

    cache.get_or_prepare(id, &layout_a, true, |f| push_marker(f, 1.0));

    let mut called = false;
    cache.get_or_prepare(id, &layout_b, false, |f| {
        called = true;
        push_marker(f, 2.0);
    });
    assert!(called, "position change should trigger re-prepare");
}

#[test]
fn selective_dirty_only_reprepares_dirty_pane() {
    let mut cache = PaneRenderCache::new();
    let id1 = PaneId::from_raw(1);
    let id2 = PaneId::from_raw(2);
    let layout1 = make_layout(id1, 0.0, 0.0, 640.0, 480.0);
    let layout2 = make_layout(id2, 640.0, 0.0, 640.0, 480.0);

    // Seed both.
    cache.get_or_prepare(id1, &layout1, true, |f| push_marker(f, 1.0));
    cache.get_or_prepare(id2, &layout2, true, |f| push_marker(f, 2.0));

    // Only pane 1 is dirty.
    let mut called1 = false;
    let frame1 = cache.get_or_prepare(id1, &layout1, true, |f| {
        called1 = true;
        push_marker(f, 10.0);
        push_marker(f, 11.0);
    });
    assert!(called1, "dirty pane 1 should re-prepare");
    assert_eq!(frame1.backgrounds.len(), 2);

    // Pane 2 is clean — should NOT re-prepare.
    let mut called2 = false;
    let frame2 = cache.get_or_prepare(id2, &layout2, false, |_f| {
        called2 = true;
    });
    assert!(!called2, "clean pane 2 should use cache");
    assert_eq!(frame2.backgrounds.len(), 1, "pane 2 cached frame untouched");
}
