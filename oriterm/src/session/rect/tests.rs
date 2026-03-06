//! Tests for the `Rect` primitive.

use super::Rect;

#[test]
fn contains_point_interior() {
    let r = Rect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 50.0,
    };
    assert!(r.contains_point(50.0, 40.0));
}

#[test]
fn contains_point_on_left_top_edge() {
    let r = Rect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 50.0,
    };
    // Left/top edges are inclusive.
    assert!(r.contains_point(10.0, 20.0));
}

#[test]
fn contains_point_on_right_bottom_edge_exclusive() {
    let r = Rect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 50.0,
    };
    // Right/bottom edges are exclusive (half-open).
    assert!(!r.contains_point(110.0, 70.0));
    assert!(!r.contains_point(110.0, 40.0));
    assert!(!r.contains_point(50.0, 70.0));
}

#[test]
fn contains_point_exterior() {
    let r = Rect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 50.0,
    };
    assert!(!r.contains_point(5.0, 40.0));
    assert!(!r.contains_point(50.0, 15.0));
    assert!(!r.contains_point(200.0, 40.0));
    assert!(!r.contains_point(50.0, 200.0));
}

#[test]
fn center() {
    let r = Rect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 50.0,
    };
    assert_eq!(r.center(), (60.0, 45.0));
}
