use super::{Insets, Logical};

// Concrete type aliases pin U = Logical so inference works in expressions
// like `Rect::new(...)` and `Point::default()` without type annotations.
type Point = super::Point<Logical>;
type Size = super::Size<Logical>;
type Rect = super::Rect<Logical>;

// Point
// Ported from Chromium ui/gfx/geometry/point_f_unittest.cc

#[test]
fn point_default_is_origin() {
    let p = Point::default();
    assert_eq!(p.x, 0.0);
    assert_eq!(p.y, 0.0);
}

#[test]
fn point_is_origin_checks() {
    // From PointFTest.IsOrigin — verify non-zero in either component.
    assert_ne!(Point::new(0.1, 0.0), Point::default());
    assert_ne!(Point::new(0.0, 0.1), Point::default());
    assert_ne!(Point::new(0.1, 2.0), Point::default());
    assert_ne!(Point::new(-0.1, 0.0), Point::default());
    assert_ne!(Point::new(0.0, -0.1), Point::default());
    assert_eq!(Point::new(0.0, 0.0), Point::default());
}

#[test]
fn point_offset() {
    let p = Point::new(10.0, 20.0).offset(5.0, -3.0);
    assert_eq!(p, Point::new(15.0, 17.0));
}

#[test]
fn point_offset_from_chrome() {
    // From PointTest.Offset — int test ported to float.
    let p = Point::new(3.0, 4.0).offset(5.0, -8.0);
    assert_eq!(p, Point::new(8.0, -4.0));
}

#[test]
fn point_scale() {
    let p = Point::new(10.0, 20.0).scale(2.0, 0.5);
    assert_eq!(p, Point::new(20.0, 10.0));
}

#[test]
fn point_scale_from_chrome() {
    // From PointFTest.Scale — chained scales.
    let p = Point::new(1.0, -1.0).scale(2.0, 2.0);
    assert_eq!(p, Point::new(2.0, -2.0));

    // Scale origin remains origin.
    let zero = Point::default().scale(2.0, 2.0).scale(3.0, 1.5);
    assert_eq!(zero, Point::default());

    // Chain two scales.
    let p2 = Point::new(1.0, -1.0).scale(2.0, 2.0).scale(3.0, 1.5);
    assert_eq!(p2, Point::new(6.0, -3.0));
}

#[test]
fn point_scale_negative() {
    // Negative scale factors flip sign.
    let p = Point::new(5.0, -3.0).scale(-1.0, -1.0);
    assert_eq!(p, Point::new(-5.0, 3.0));
}

#[test]
fn point_distance_to() {
    let a = Point::new(0.0, 0.0);
    let b = Point::new(3.0, 4.0);
    assert!((a.distance_to(b) - 5.0).abs() < f32::EPSILON);
}

#[test]
fn point_distance_to_self_is_zero() {
    let p = Point::new(7.0, 11.0);
    assert_eq!(p.distance_to(p), 0.0);
}

#[test]
fn point_distance_to_pythagorean_triples() {
    // From Vector2dFTest.Length — Pythagorean triples.
    let origin = Point::default();
    assert!((origin.distance_to(Point::new(3.0, 4.0)) - 5.0).abs() < 1e-6);
    assert!((origin.distance_to(Point::new(5.0, 12.0)) - 13.0).abs() < 1e-6);
}

#[test]
fn point_distance_symmetric() {
    // Distance is symmetric: d(a,b) == d(b,a).
    let a = Point::new(-20.0, 8.0);
    let b = Point::new(1.0, 5.0);
    assert_eq!(a.distance_to(b), b.distance_to(a));
}

#[test]
fn point_add() {
    let a = Point::new(1.0, 2.0);
    let b = Point::new(3.0, 4.0);
    assert_eq!(a + b, Point::new(4.0, 6.0));
}

#[test]
fn point_sub() {
    let a = Point::new(5.0, 7.0);
    let b = Point::new(3.0, 2.0);
    assert_eq!(a - b, Point::new(2.0, 5.0));
}

#[test]
fn point_sub_from_chrome() {
    // From PointTest.OffsetFromPoint.
    let a = Point::new(-20.0, 8.0);
    let b = Point::new(1.0, 5.0);
    let diff = a - b;
    assert_eq!(diff, Point::new(-21.0, 3.0));
}

#[test]
fn point_add_sub_roundtrip() {
    // From PointTest.VectorArithmetic — roundtrip.
    let a = Point::new(1.0, 5.0);
    let v = Point::new(3.0, -3.0);
    assert_eq!(a - v + v, a);
    assert_eq!(a + v - v, a);
}

#[test]
fn point_add_identity() {
    // Adding zero point is identity.
    let a = Point::new(1.0, 5.0);
    assert_eq!(a + Point::default(), a);
}

#[test]
fn point_chained_add_sub() {
    // From PointTest.VectorArithmetic — chain.
    let p = Point::new(1.0, 5.0);
    let v1 = Point::new(3.0, -3.0);
    let v2 = Point::new(-8.0, 1.0);
    assert_eq!(p + v1 - v2, Point::new(12.0, 1.0));
    assert_eq!(p - v1 + v2, Point::new(-10.0, 9.0));
}

#[test]
fn point_negative_coordinates() {
    let p = Point::new(-5.0, -10.0);
    assert_eq!(p.offset(5.0, 10.0), Point::default());
}

// Size
// Ported from Chromium ui/gfx/geometry/size_f_unittest.cc

#[test]
fn size_default_is_empty() {
    let s = Size::default();
    assert_eq!(s.width(), 0.0);
    assert_eq!(s.height(), 0.0);
    assert!(s.is_empty());
}

#[test]
fn size_normal_values() {
    let s = Size::new(100.0, 50.0);
    assert_eq!(s.width(), 100.0);
    assert_eq!(s.height(), 50.0);
    assert!(!s.is_empty());
}

#[test]
fn size_epsilon_clamping_near_zero_becomes_zero() {
    // A value smaller than 8 * f32::EPSILON should be clamped to 0.
    let tiny = f32::EPSILON;
    let s = Size::new(tiny, tiny);
    assert_eq!(s.width(), 0.0);
    assert_eq!(s.height(), 0.0);
    assert!(s.is_empty());
}

#[test]
fn size_epsilon_clamping_preserves_normal_values() {
    let s = Size::new(1.0, 0.001);
    assert_eq!(s.width(), 1.0);
    assert_eq!(s.height(), 0.001);
}

#[test]
fn size_zero_width_is_empty() {
    let s = Size::new(0.0, 100.0);
    assert!(s.is_empty());
}

#[test]
fn size_zero_height_is_empty() {
    let s = Size::new(100.0, 0.0);
    assert!(s.is_empty());
}

#[test]
fn size_is_empty_with_trivial_width() {
    // From SizeFTest.IsEmpty — width below kTrivial threshold.
    let trivial = 8.0 * f32::EPSILON;
    let s = Size::new(trivial / 2.0, 1.0);
    assert!(s.is_empty());
}

#[test]
fn size_is_empty_with_trivial_height() {
    // From SizeFTest.IsEmpty — height below kTrivial threshold.
    let trivial = 8.0 * f32::EPSILON;
    let s = Size::new(0.01, trivial / 2.0);
    assert!(s.is_empty());
}

#[test]
fn size_is_not_empty_with_small_values() {
    // From SizeFTest.IsEmpty — both dimensions above threshold.
    let s = Size::new(0.01, 0.01);
    assert!(!s.is_empty());
}

#[test]
fn size_clamps_to_zero() {
    // From SizeFTest.ClampsToZero — trivial width clamped to zero.
    let trivial = 8.0 * f32::EPSILON;
    let s = Size::new(trivial / 2.0, 1.0);
    assert_eq!(s.width(), 0.0);

    // Trivial height clamped to zero.
    let s2 = Size::new(0.01, trivial / 2.0);
    assert_eq!(s2.height(), 0.0);
}

#[test]
fn size_scale_to_trivial_clamps() {
    // From SizeFTest.ClampsToZero — scaling a near-trivial value below threshold.
    let trivial = 8.0 * f32::EPSILON;
    let nearly_trivial = trivial * 1.5;
    let s = Size::new(nearly_trivial, nearly_trivial).scale(0.5, 0.5);
    assert_eq!(s.width(), 0.0);
    assert_eq!(s.height(), 0.0);
    assert!(s.is_empty());
}

#[test]
fn size_consistent_clamping_constructor_vs_setter() {
    // From SizeFTest.ConsistentClamping — constructor and setter behave identically.
    let trivial = 8.0 * f32::EPSILON;
    let from_ctor = Size::new(trivial, 0.0);
    let mut from_setter = Size::default();
    from_setter.set_width(trivial);
    from_setter.set_height(0.0);
    assert_eq!(from_ctor.width(), from_setter.width());
    assert_eq!(from_ctor.height(), from_setter.height());
}

#[test]
fn size_area() {
    let s = Size::new(10.0, 20.0);
    assert_eq!(s.area(), 200.0);
}

#[test]
fn size_area_empty() {
    assert_eq!(Size::default().area(), 0.0);
    assert_eq!(Size::new(10.0, 0.0).area(), 0.0);
    assert_eq!(Size::new(0.0, 10.0).area(), 0.0);
}

#[test]
fn size_scale() {
    let s = Size::new(10.0, 20.0).scale(2.0, 0.5);
    assert_eq!(s.width(), 20.0);
    assert_eq!(s.height(), 10.0);
}

#[test]
fn size_scale_uniform() {
    let s = Size::new(4.5, 1.2).scale(3.3, 3.3);
    assert!((s.width() - 14.85).abs() < 1e-5);
    assert!((s.height() - 3.96).abs() < 1e-5);
}

#[test]
fn size_set_width_height_clamp() {
    let mut s = Size::new(10.0, 10.0);
    s.set_width(f32::EPSILON);
    s.set_height(f32::EPSILON);
    assert_eq!(s.width(), 0.0);
    assert_eq!(s.height(), 0.0);
}

#[test]
fn size_set_width_height_normal() {
    let mut s = Size::default();
    s.set_width(100.5);
    s.set_height(200.25);
    assert_eq!(s.width(), 100.5);
    assert_eq!(s.height(), 200.25);
    assert!(!s.is_empty());
}

// Rect
// Ported from Chromium ui/gfx/geometry/rect_unittest.cc and rect_f_unittest.cc

#[test]
fn rect_from_origin_size() {
    let r = Rect::from_origin_size(Point::new(10.0, 20.0), Size::new(30.0, 40.0));
    assert_eq!(r.x(), 10.0);
    assert_eq!(r.y(), 20.0);
    assert_eq!(r.width(), 30.0);
    assert_eq!(r.height(), 40.0);
    assert_eq!(r.right(), 40.0);
    assert_eq!(r.bottom(), 60.0);
}

#[test]
fn rect_from_ltrb() {
    let r = Rect::from_ltrb(10.0, 20.0, 40.0, 60.0);
    assert_eq!(r.x(), 10.0);
    assert_eq!(r.y(), 20.0);
    assert_eq!(r.width(), 30.0);
    assert_eq!(r.height(), 40.0);
    assert_eq!(r.right(), 40.0);
    assert_eq!(r.bottom(), 60.0);
}

#[test]
fn rect_from_ltrb_equivalence() {
    // from_ltrb(l, t, r, b) == new(l, t, r-l, b-t).
    let a = Rect::from_ltrb(10.0, 20.0, 110.0, 120.0);
    let b = Rect::new(10.0, 20.0, 100.0, 100.0);
    assert_eq!(a, b);
}

#[test]
fn rect_from_ltrb_zero_area() {
    // Same left/right or top/bottom produces an empty rect.
    assert!(Rect::from_ltrb(5.0, 5.0, 5.0, 10.0).is_empty());
    assert!(Rect::from_ltrb(5.0, 5.0, 10.0, 5.0).is_empty());
}

#[test]
fn rect_new_equivalence() {
    let a = Rect::new(1.0, 2.0, 3.0, 4.0);
    let b = Rect::from_origin_size(Point::new(1.0, 2.0), Size::new(3.0, 4.0));
    assert_eq!(a, b);
}

#[test]
fn rect_center() {
    let r = Rect::new(0.0, 0.0, 100.0, 200.0);
    assert_eq!(r.center(), Point::new(50.0, 100.0));
}

#[test]
fn rect_center_from_chrome() {
    // From RectTest.CenterPoint — various cases.
    assert_eq!(
        Rect::new(0.0, 0.0, 20.0, 20.0).center(),
        Point::new(10.0, 10.0)
    );
    assert_eq!(
        Rect::new(10.0, 10.0, 20.0, 20.0).center(),
        Point::new(20.0, 20.0)
    );

    // From RectFTest.CenterPoint — odd dimensions yield exact midpoint.
    assert_eq!(
        Rect::new(10.0, 10.0, 21.0, 21.0).center(),
        Point::new(20.5, 20.5)
    );

    // Zero-width rect: center.x is at origin.x.
    assert_eq!(
        Rect::new(10.0, 10.0, 0.0, 20.0).center(),
        Point::new(10.0, 20.0)
    );
}

#[test]
fn rect_default_is_empty() {
    let r = Rect::default();
    assert!(r.is_empty());
}

#[test]
fn rect_is_empty_from_chrome() {
    // From RectTest.IsEmpty.
    assert!(Rect::new(0.0, 0.0, 0.0, 0.0).is_empty());
    assert!(Rect::new(0.0, 0.0, 10.0, 0.0).is_empty());
    assert!(Rect::new(0.0, 0.0, 0.0, 10.0).is_empty());
    assert!(!Rect::new(0.0, 0.0, 10.0, 10.0).is_empty());
}

#[test]
fn rect_contains_from_chrome() {
    // From RectTest.Contains — comprehensive containment.
    let r = Rect::new(0.0, 0.0, 10.0, 10.0);
    assert!(r.contains(Point::new(0.0, 0.0))); // Top-left corner.
    assert!(r.contains(Point::new(5.0, 5.0))); // Interior.
    assert!(r.contains(Point::new(9.0, 9.0))); // Near bottom-right (inside).
    assert!(!r.contains(Point::new(5.0, 10.0))); // Bottom edge excluded.
    assert!(!r.contains(Point::new(10.0, 5.0))); // Right edge excluded.
    assert!(!r.contains(Point::new(-1.0, -1.0))); // Outside.
    assert!(!r.contains(Point::new(50.0, 50.0))); // Far outside.
}

#[test]
fn rect_contains_float_precision() {
    // From RectFTest.ContainsPointF — near boundary.
    let r = Rect::new(10.0, 20.0, 30.0, 40.0);
    assert!(r.contains(Point::new(10.0, 20.0))); // Top-left inclusive.
    assert!(r.contains(Point::new(39.9999, 59.9999))); // Near bottom-right.
    assert!(!r.contains(Point::new(40.0, 60.0))); // Bottom-right exclusive.
}

#[test]
fn rect_contains_inside() {
    let r = Rect::new(10.0, 10.0, 100.0, 100.0);
    assert!(r.contains(Point::new(50.0, 50.0)));
}

#[test]
fn rect_contains_outside() {
    let r = Rect::new(10.0, 10.0, 100.0, 100.0);
    assert!(!r.contains(Point::new(0.0, 0.0)));
    assert!(!r.contains(Point::new(200.0, 200.0)));
}

#[test]
fn rect_contains_half_open_left_top_inclusive() {
    let r = Rect::new(10.0, 10.0, 100.0, 100.0);
    assert!(r.contains(Point::new(10.0, 10.0)));
}

#[test]
fn rect_contains_half_open_right_bottom_exclusive() {
    let r = Rect::new(10.0, 10.0, 100.0, 100.0);
    assert!(!r.contains(Point::new(110.0, 50.0)));
    assert!(!r.contains(Point::new(50.0, 110.0)));
    assert!(!r.contains(Point::new(110.0, 110.0)));
}

#[test]
fn rect_contains_just_inside_right_bottom() {
    let r = Rect::new(10.0, 10.0, 100.0, 100.0);
    assert!(r.contains(Point::new(109.9, 109.9)));
}

#[test]
fn rect_intersects_from_chrome() {
    // From RectTest.Intersects — comprehensive cases.
    // Empty rects never intersect.
    assert!(!Rect::new(0.0, 0.0, 0.0, 0.0).intersects(Rect::new(0.0, 0.0, 0.0, 0.0)));

    // Identical rects intersect.
    assert!(Rect::new(0.0, 0.0, 10.0, 10.0).intersects(Rect::new(0.0, 0.0, 10.0, 10.0)));

    // Touching corner — no intersection (half-open).
    assert!(!Rect::new(0.0, 0.0, 10.0, 10.0).intersects(Rect::new(10.0, 10.0, 10.0, 10.0)));
    assert!(!Rect::new(10.0, 10.0, 10.0, 10.0).intersects(Rect::new(0.0, 0.0, 10.0, 10.0)));

    // Overlapping.
    assert!(Rect::new(10.0, 10.0, 10.0, 10.0).intersects(Rect::new(5.0, 5.0, 10.0, 10.0)));
    assert!(Rect::new(10.0, 10.0, 10.0, 10.0).intersects(Rect::new(15.0, 15.0, 10.0, 10.0)));

    // Edge-adjacent (right edge at x=20, next starts at x=20) — no intersection.
    assert!(!Rect::new(10.0, 10.0, 10.0, 10.0).intersects(Rect::new(20.0, 15.0, 10.0, 10.0)));

    // Separated.
    assert!(!Rect::new(10.0, 10.0, 10.0, 10.0).intersects(Rect::new(21.0, 15.0, 10.0, 10.0)));
}

#[test]
fn rect_intersects_overlapping() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(50.0, 50.0, 100.0, 100.0);
    assert!(a.intersects(b));
    assert!(b.intersects(a));
}

#[test]
fn rect_intersects_adjacent_no_overlap() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(100.0, 0.0, 100.0, 100.0);
    assert!(!a.intersects(b));
}

#[test]
fn rect_intersects_contained() {
    let outer = Rect::new(0.0, 0.0, 100.0, 100.0);
    let inner = Rect::new(25.0, 25.0, 50.0, 50.0);
    assert!(outer.intersects(inner));
    assert!(inner.intersects(outer));
}

#[test]
fn rect_intersects_disjoint() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    let b = Rect::new(50.0, 50.0, 10.0, 10.0);
    assert!(!a.intersects(b));
}

#[test]
fn rect_intersects_empty_never() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let empty = Rect::default();
    assert!(!a.intersects(empty));
    assert!(!empty.intersects(a));
}

#[test]
fn rect_intersection_from_chrome() {
    // From RectTest.Intersect.
    // Empty with empty.
    assert_eq!(
        Rect::default().intersection(Rect::default()),
        Rect::default()
    );
    // Identical.
    assert_eq!(
        Rect::new(0.0, 0.0, 4.0, 4.0).intersection(Rect::new(0.0, 0.0, 4.0, 4.0)),
        Rect::new(0.0, 0.0, 4.0, 4.0)
    );
    // No overlap (touching corner).
    assert!(
        Rect::new(0.0, 0.0, 4.0, 4.0)
            .intersection(Rect::new(4.0, 4.0, 4.0, 4.0))
            .is_empty()
    );
    // Partial overlap.
    assert_eq!(
        Rect::new(0.0, 0.0, 4.0, 4.0).intersection(Rect::new(2.0, 2.0, 4.0, 4.0)),
        Rect::new(2.0, 2.0, 2.0, 2.0)
    );
    // T-junction.
    assert_eq!(
        Rect::new(0.0, 0.0, 4.0, 4.0).intersection(Rect::new(3.0, 1.0, 4.0, 2.0)),
        Rect::new(3.0, 1.0, 1.0, 2.0)
    );
    // Gap between rects.
    assert!(
        Rect::new(3.0, 0.0, 2.0, 2.0)
            .intersection(Rect::new(0.0, 0.0, 2.0, 2.0))
            .is_empty()
    );
}

#[test]
fn rect_intersection_overlapping() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(50.0, 50.0, 100.0, 100.0);
    let i = a.intersection(b);
    assert_eq!(i, Rect::new(50.0, 50.0, 50.0, 50.0));
}

#[test]
fn rect_intersection_disjoint_is_empty() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    let b = Rect::new(50.0, 50.0, 10.0, 10.0);
    assert!(a.intersection(b).is_empty());
}

#[test]
fn rect_union_from_chrome() {
    // From RectTest.Union — comprehensive cases.
    // Empty union.
    assert!(Rect::default().union(Rect::default()).is_empty());

    // Identical.
    assert_eq!(
        Rect::new(1.0, 2.0, 3.0, 4.0).union(Rect::new(1.0, 2.0, 3.0, 4.0)),
        Rect::new(1.0, 2.0, 3.0, 4.0)
    );

    // Adjacent rects.
    assert_eq!(
        Rect::new(0.0, 0.0, 3.0, 4.0).union(Rect::new(3.0, 4.0, 5.0, 6.0)),
        Rect::new(0.0, 0.0, 8.0, 10.0)
    );

    // Vertical stack.
    assert_eq!(
        Rect::new(0.0, 1.0, 3.0, 4.0).union(Rect::new(0.0, 5.0, 3.0, 4.0)),
        Rect::new(0.0, 1.0, 3.0, 8.0)
    );

    // Diagonal.
    assert_eq!(
        Rect::new(0.0, 1.0, 3.0, 4.0).union(Rect::new(4.0, 5.0, 6.0, 7.0)),
        Rect::new(0.0, 1.0, 10.0, 11.0)
    );

    // Empty rect ignored.
    assert_eq!(
        Rect::new(8.0, 9.0, 0.0, 2.0).union(Rect::new(2.0, 3.0, 4.0, 5.0)),
        Rect::new(2.0, 3.0, 4.0, 5.0)
    );
}

#[test]
fn rect_union_two_rects() {
    let a = Rect::new(10.0, 10.0, 10.0, 10.0);
    let b = Rect::new(30.0, 30.0, 10.0, 10.0);
    let u = a.union(b);
    assert_eq!(u, Rect::new(10.0, 10.0, 30.0, 30.0));
}

#[test]
fn rect_union_one_empty() {
    let a = Rect::new(10.0, 10.0, 50.0, 50.0);
    let empty = Rect::default();
    assert_eq!(a.union(empty), a);
    assert_eq!(empty.union(a), a);
}

#[test]
fn rect_union_both_empty() {
    let a = Rect::default();
    let b = Rect::default();
    assert!(a.union(b).is_empty());
}

#[test]
fn rect_inset_from_chrome() {
    // From RectFTest.Inset — uniform inset.
    let r = Rect::new(10.0, 20.0, 30.0, 40.0).inset(Insets::all(1.5));
    assert_eq!(r, Rect::new(11.5, 21.5, 27.0, 37.0));
}

#[test]
fn rect_inset_vh_from_chrome() {
    // From RectFTest.Inset — vertical/horizontal inset.
    let r = Rect::new(10.0, 20.0, 30.0, 40.0).inset(Insets::vh(2.0, 1.0));
    assert_eq!(r, Rect::new(11.0, 22.0, 28.0, 36.0));
}

#[test]
fn rect_inset_per_edge_from_chrome() {
    // From RectFTest.Inset — per-edge inset.
    let r = Rect::new(10.0, 20.0, 30.0, 40.0).inset(Insets::tlbr(2.25, 1.5, 4.0, 3.75));
    assert_eq!(r, Rect::new(11.5, 22.25, 24.75, 33.75));
}

#[test]
fn rect_inset_positive_shrinks() {
    let r = Rect::new(10.0, 10.0, 100.0, 100.0);
    let insets = Insets::all(5.0);
    let shrunk = r.inset(insets);
    assert_eq!(shrunk, Rect::new(15.0, 15.0, 90.0, 90.0));
}

#[test]
fn rect_inset_negative_expands() {
    let r = Rect::new(10.0, 10.0, 100.0, 100.0);
    let insets = Insets::all(-5.0);
    let expanded = r.inset(insets);
    assert_eq!(expanded, Rect::new(5.0, 5.0, 110.0, 110.0));
}

#[test]
fn rect_offset() {
    let r = Rect::new(10.0, 20.0, 30.0, 40.0);
    let moved = r.offset(5.0, -10.0);
    assert_eq!(moved, Rect::new(15.0, 10.0, 30.0, 40.0));
}

#[test]
fn rect_offset_from_chrome() {
    // From RectFTest.Offset.
    let r = Rect::new(1.0, 2.0, 3.0, 4.0).offset(1.0, -1.0);
    assert_eq!(r, Rect::new(2.0, 1.0, 3.0, 4.0));

    let r2 = Rect::new(1.0, 2.0, 3.0, 4.0).offset(2.0, -2.0);
    assert_eq!(r2, Rect::new(3.0, 0.0, 3.0, 4.0));
}

#[test]
fn rect_offset_preserves_size() {
    // Offset never changes size.
    let r = Rect::new(10.0, 20.0, 30.0, 40.0);
    let moved = r.offset(100.0, -200.0);
    assert_eq!(moved.width(), r.width());
    assert_eq!(moved.height(), r.height());
}

#[test]
fn rect_equality() {
    // From RectTest.Equals.
    assert_eq!(Rect::default(), Rect::default());
    assert_eq!(Rect::new(1.0, 2.0, 3.0, 4.0), Rect::new(1.0, 2.0, 3.0, 4.0));
    assert_ne!(Rect::new(0.0, 0.0, 0.0, 0.0), Rect::new(0.0, 0.0, 0.0, 1.0));
    assert_ne!(Rect::new(0.0, 0.0, 0.0, 0.0), Rect::new(0.0, 0.0, 1.0, 0.0));
    assert_ne!(Rect::new(0.0, 0.0, 0.0, 0.0), Rect::new(0.0, 1.0, 0.0, 0.0));
    assert_ne!(Rect::new(0.0, 0.0, 0.0, 0.0), Rect::new(1.0, 0.0, 0.0, 0.0));
}

#[test]
fn rect_intersects_self() {
    // From RectTest.Intersects — a rect always intersects itself if non-empty.
    let r = Rect::new(5.0, 5.0, 10.0, 10.0);
    assert!(r.intersects(r));
}

#[test]
fn rect_intersection_symmetric() {
    // Intersection is symmetric: a ∩ b == b ∩ a.
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(50.0, 50.0, 100.0, 100.0);
    assert_eq!(a.intersection(b), b.intersection(a));
}

#[test]
fn rect_union_symmetric() {
    // Union is symmetric: a ∪ b == b ∪ a.
    let a = Rect::new(0.0, 0.0, 3.0, 4.0);
    let b = Rect::new(3.0, 4.0, 5.0, 6.0);
    assert_eq!(a.union(b), b.union(a));
}

// Insets
// Ported from Chromium ui/gfx/geometry/insets_f_unittest.cc

#[test]
fn insets_default() {
    // From InsetsFTest.Default.
    let i = Insets::default();
    assert_eq!(i.top, 0.0);
    assert_eq!(i.left, 0.0);
    assert_eq!(i.bottom, 0.0);
    assert_eq!(i.right, 0.0);
    assert_eq!(i.width(), 0.0);
    assert_eq!(i.height(), 0.0);
}

#[test]
fn insets_all() {
    let i = Insets::all(10.0);
    assert_eq!(i.top, 10.0);
    assert_eq!(i.right, 10.0);
    assert_eq!(i.bottom, 10.0);
    assert_eq!(i.left, 10.0);
}

#[test]
fn insets_vh() {
    // From InsetsFTest.VH.
    let i = Insets::vh(1.0, 2.0);
    assert_eq!(i.top, 1.0);
    assert_eq!(i.left, 2.0);
    assert_eq!(i.bottom, 1.0);
    assert_eq!(i.right, 2.0);
}

#[test]
fn insets_tlbr() {
    // From InsetsFTest.TLBR.
    let i = Insets::tlbr(1.0, 2.0, 3.0, 4.0);
    assert_eq!(i.top, 1.0);
    assert_eq!(i.left, 2.0);
    assert_eq!(i.bottom, 3.0);
    assert_eq!(i.right, 4.0);
}

#[test]
fn insets_width_height() {
    let i = Insets::tlbr(10.0, 20.0, 30.0, 40.0);
    assert_eq!(i.width(), 60.0); // left + right = 20 + 40
    assert_eq!(i.height(), 40.0); // top + bottom = 10 + 30
}

#[test]
fn insets_width_height_from_chrome() {
    // From InsetsFTest.WidthHeightAndIsEmpty — targeted checks.
    let lr = Insets::tlbr(0.0, 3.0, 0.0, 4.0);
    assert_eq!(lr.width(), 7.0);
    assert_eq!(lr.height(), 0.0);

    let tb = Insets::tlbr(1.0, 0.0, 2.0, 0.0);
    assert_eq!(tb.width(), 0.0);
    assert_eq!(tb.height(), 3.0);
}

#[test]
fn insets_add() {
    let a = Insets::all(5.0);
    let b = Insets::all(3.0);
    let sum = a + b;
    assert_eq!(sum, Insets::all(8.0));
}

#[test]
fn insets_add_per_edge_from_chrome() {
    // From InsetsFTest.Operators — per-edge addition.
    let a = Insets::tlbr(1.0, 2.0, 3.0, 4.0);
    let b = Insets::tlbr(5.0, 6.0, 7.0, 8.0);
    let sum = a + b;
    assert_eq!(sum, Insets::tlbr(6.0, 8.0, 10.0, 12.0));
}

#[test]
fn insets_sub() {
    let a = Insets::all(5.0);
    let b = Insets::all(3.0);
    let diff = a - b;
    assert_eq!(diff, Insets::all(2.0));
}

#[test]
fn insets_sub_per_edge_from_chrome() {
    // From InsetsFTest.Operators — per-edge subtraction.
    let a = Insets::tlbr(5.0, 6.0, 7.0, 8.0);
    let b = Insets::tlbr(1.0, 2.0, 3.0, 4.0);
    let diff = a - b;
    assert_eq!(diff, Insets::tlbr(4.0, 4.0, 4.0, 4.0));
}

#[test]
fn insets_neg() {
    let i = Insets::all(5.0);
    let neg = -i;
    assert_eq!(neg, Insets::all(-5.0));
}

#[test]
fn insets_neg_per_edge() {
    let i = Insets::tlbr(1.0, 2.0, 3.0, 4.0);
    let neg = -i;
    assert_eq!(neg, Insets::tlbr(-1.0, -2.0, -3.0, -4.0));
}

#[test]
fn insets_neg_double_is_identity() {
    // Negating twice is identity.
    let i = Insets::tlbr(1.5, 2.5, 3.5, 4.5);
    assert_eq!(-(-i), i);
}

#[test]
fn insets_equality_from_chrome() {
    // From InsetsFTest.Equality.
    assert_eq!(
        Insets::tlbr(1.0, 2.0, 3.0, 4.0),
        Insets::tlbr(1.0, 2.0, 3.0, 4.0)
    );
    assert_ne!(
        Insets::tlbr(1.0, 2.0, 3.0, 4.0),
        Insets::tlbr(0.0, 2.0, 3.0, 4.0)
    );
    assert_ne!(
        Insets::tlbr(1.0, 2.0, 3.0, 4.0),
        Insets::tlbr(1.0, 0.0, 3.0, 4.0)
    );
    assert_ne!(
        Insets::tlbr(1.0, 2.0, 3.0, 4.0),
        Insets::tlbr(1.0, 2.0, 0.0, 4.0)
    );
    assert_ne!(
        Insets::tlbr(1.0, 2.0, 3.0, 4.0),
        Insets::tlbr(1.0, 2.0, 3.0, 0.0)
    );
}

#[test]
fn insets_add_sub_roundtrip() {
    // a + b - b == a.
    let a = Insets::tlbr(1.5, 2.5, 3.5, 4.5);
    let b = Insets::tlbr(10.0, 20.0, 30.0, 40.0);
    assert_eq!(a + b - b, a);
}
