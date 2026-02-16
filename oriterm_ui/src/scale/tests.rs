use crate::geometry::{Point, Rect, Size};

use super::ScaleFactor;

#[test]
fn default_is_1x() {
    let sf = ScaleFactor::default();
    assert_eq!(sf.factor(), 1.0);
}

#[test]
fn clamping_below_minimum() {
    let sf = ScaleFactor::new(0.1);
    assert_eq!(sf.factor(), 0.25);
}

#[test]
fn clamping_above_maximum() {
    let sf = ScaleFactor::new(16.0);
    assert_eq!(sf.factor(), 8.0);
}

#[test]
fn clamping_at_boundaries() {
    assert_eq!(ScaleFactor::new(0.25).factor(), 0.25);
    assert_eq!(ScaleFactor::new(8.0).factor(), 8.0);
}

#[test]
fn normal_values_pass_through() {
    assert_eq!(ScaleFactor::new(1.0).factor(), 1.0);
    assert_eq!(ScaleFactor::new(2.0).factor(), 2.0);
    assert_eq!(ScaleFactor::new(1.5).factor(), 1.5);
}

#[test]
fn scale_unscale_roundtrip() {
    let sf = ScaleFactor::new(2.0);
    let logical = 100.0;
    let physical = sf.scale(logical);
    let back = sf.unscale(physical);
    assert!((back - logical).abs() < f64::EPSILON);
}

#[test]
fn scale_unscale_fractional() {
    let sf = ScaleFactor::new(1.5);
    let logical = 80.0;
    let physical = sf.scale(logical);
    assert_eq!(physical, 120.0);
    let back = sf.unscale(physical);
    assert!((back - logical).abs() < 1e-10);
}

#[test]
fn scale_u32_rounding() {
    let sf = ScaleFactor::new(1.5);
    // 1.5 * 7 = 10.5 -> rounds to 11.
    assert_eq!(sf.scale_u32(7.0), 11);
    // 1.5 * 10 = 15.0 -> exactly 15.
    assert_eq!(sf.scale_u32(10.0), 15);
}

#[test]
fn scale_u32_rounds_half_up() {
    let sf = ScaleFactor::new(1.25);
    // 1.25 * 3 = 3.75 -> rounds to 4.
    assert_eq!(sf.scale_u32(3.0), 4);
}

#[test]
fn scale_point() {
    let sf = ScaleFactor::new(2.0);
    let p = Point::new(10.0, 20.0);
    let scaled = sf.scale_point(p);
    assert_eq!(scaled, Point::new(20.0, 40.0));
}

#[test]
fn scale_size() {
    let sf = ScaleFactor::new(2.0);
    let s = Size::new(100.0, 50.0);
    let scaled = sf.scale_size(s);
    assert_eq!(scaled.width(), 200.0);
    assert_eq!(scaled.height(), 100.0);
}

#[test]
fn scale_rect() {
    let sf = ScaleFactor::new(2.0);
    let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    let scaled = sf.scale_rect(r);
    assert_eq!(scaled.x(), 20.0);
    assert_eq!(scaled.y(), 40.0);
    assert_eq!(scaled.width(), 200.0);
    assert_eq!(scaled.height(), 100.0);
}

#[test]
fn scale_rect_origin_and_size_both_scaled() {
    let sf = ScaleFactor::new(1.5);
    let r = Rect::new(10.0, 10.0, 20.0, 20.0);
    let scaled = sf.scale_rect(r);
    assert_eq!(scaled.x(), 15.0);
    assert_eq!(scaled.y(), 15.0);
    assert_eq!(scaled.width(), 30.0);
    assert_eq!(scaled.height(), 30.0);
}
