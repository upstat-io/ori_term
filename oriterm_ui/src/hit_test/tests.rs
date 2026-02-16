use crate::geometry::{Point, Rect, Size};

use super::{HitTestResult, ResizeDirection, hit_test};

/// Standard test window: 800x600, 5px border, 46px caption.
fn standard_window() -> (Size, f32, f32) {
    (Size::new(800.0, 600.0), 5.0, 46.0)
}

#[test]
fn client_area_in_grid() {
    let (size, border, caption) = standard_window();
    let point = Point::new(400.0, 300.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::Client
    );
}

#[test]
fn caption_area_in_tab_bar() {
    let (size, border, caption) = standard_window();
    // Point in the caption area, past the border width.
    let point = Point::new(400.0, 20.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::Caption
    );
}

#[test]
fn resize_top_edge() {
    let (size, border, caption) = standard_window();
    let point = Point::new(400.0, 2.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::ResizeBorder(ResizeDirection::Top)
    );
}

#[test]
fn resize_bottom_edge() {
    let (size, border, caption) = standard_window();
    let point = Point::new(400.0, 598.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::ResizeBorder(ResizeDirection::Bottom)
    );
}

#[test]
fn resize_left_edge() {
    let (size, border, caption) = standard_window();
    let point = Point::new(2.0, 300.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::ResizeBorder(ResizeDirection::Left)
    );
}

#[test]
fn resize_right_edge() {
    let (size, border, caption) = standard_window();
    let point = Point::new(798.0, 300.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::ResizeBorder(ResizeDirection::Right)
    );
}

#[test]
fn resize_top_left_corner() {
    let (size, border, caption) = standard_window();
    let point = Point::new(2.0, 2.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::ResizeBorder(ResizeDirection::TopLeft)
    );
}

#[test]
fn resize_top_right_corner() {
    let (size, border, caption) = standard_window();
    let point = Point::new(798.0, 2.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::ResizeBorder(ResizeDirection::TopRight)
    );
}

#[test]
fn resize_bottom_left_corner() {
    let (size, border, caption) = standard_window();
    let point = Point::new(2.0, 598.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::ResizeBorder(ResizeDirection::BottomLeft)
    );
}

#[test]
fn resize_bottom_right_corner() {
    let (size, border, caption) = standard_window();
    let point = Point::new(798.0, 598.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::ResizeBorder(ResizeDirection::BottomRight)
    );
}

#[test]
fn corner_priority_over_edge() {
    let (size, border, caption) = standard_window();
    // Point at (0, 0) — both on left edge and top edge.
    // Corner should win.
    let point = Point::new(0.0, 0.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::ResizeBorder(ResizeDirection::TopLeft)
    );
}

#[test]
fn maximized_suppresses_resize_borders() {
    let (size, border, caption) = standard_window();
    // Top edge when not maximized would be resize.
    let point = Point::new(400.0, 2.0);
    // When maximized, resize borders are suppressed -> caption.
    assert_eq!(
        hit_test(point, size, border, caption, &[], true),
        HitTestResult::Caption
    );
}

#[test]
fn maximized_no_resize_on_edges() {
    let (size, border, caption) = standard_window();
    // Bottom edge.
    let point = Point::new(400.0, 598.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], true),
        HitTestResult::Client
    );
}

#[test]
fn interactive_rect_in_caption_returns_client() {
    let (size, border, caption) = standard_window();
    // A close button rect in the caption area.
    let button = Rect::new(750.0, 5.0, 40.0, 36.0);
    let point = Point::new(770.0, 20.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[button], false),
        HitTestResult::Client
    );
}

#[test]
fn interactive_rect_outside_point_is_caption() {
    let (size, border, caption) = standard_window();
    // A close button rect that does NOT contain the point.
    let button = Rect::new(750.0, 5.0, 40.0, 36.0);
    let point = Point::new(100.0, 20.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[button], false),
        HitTestResult::Caption
    );
}

#[test]
fn point_on_border_width_boundary() {
    let (size, border, caption) = standard_window();
    // Exactly at x = border_width (5.0) — should NOT be on the left edge.
    let point = Point::new(5.0, 300.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::Client
    );
}

#[test]
fn point_just_inside_border() {
    let (size, border, caption) = standard_window();
    // x = 4.9 — just inside the left border.
    let point = Point::new(4.9, 300.0);
    assert_eq!(
        hit_test(point, size, border, caption, &[], false),
        HitTestResult::ResizeBorder(ResizeDirection::Left)
    );
}

#[test]
fn multiple_interactive_rects() {
    let (size, border, caption) = standard_window();
    let buttons = vec![
        Rect::new(700.0, 5.0, 30.0, 36.0),
        Rect::new(735.0, 5.0, 30.0, 36.0),
        Rect::new(770.0, 5.0, 25.0, 36.0),
    ];
    // Click on second button.
    let point = Point::new(750.0, 20.0);
    assert_eq!(
        hit_test(point, size, border, caption, &buttons, false),
        HitTestResult::Client
    );
}
