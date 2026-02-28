//! Unit tests for the compositor module.

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::time::{Duration, Instant};

use crate::animation::{AnimatableProperty, Easing, Lerp};
use crate::color::Color;
use crate::geometry::{Point, Rect};

use crate::animation::AnimationBuilder;
use crate::animation::group::{AnimationGroup, PropertyAnimation, TransitionTarget};

use super::{
    AnimationParams, Layer, LayerAnimator, LayerId, LayerProperties, LayerTree, LayerType,
    PreemptionStrategy, Transform2D,
};

// --- Helpers ---

/// Asserts two points are approximately equal.
fn assert_point_near(actual: Point, expected: Point, eps: f32) {
    assert!(
        (actual.x - expected.x).abs() < eps && (actual.y - expected.y).abs() < eps,
        "expected Point({}, {}), got Point({}, {})",
        expected.x,
        expected.y,
        actual.x,
        actual.y,
    );
}

/// Asserts two transforms are approximately equal (per-element).
fn assert_transform_near(actual: Transform2D, expected: Transform2D, eps: f32) {
    let a = actual.to_mat3x2();
    let e = expected.to_mat3x2();
    for i in 0..6 {
        assert!(
            (a[i] - e[i]).abs() < eps,
            "element [{i}]: expected {}, got {}",
            e[i],
            a[i],
        );
    }
}

// --- 43.1 Transform2D ---

// Identity

#[test]
fn identity_roundtrip() {
    let id = Transform2D::identity();
    let p = Point::new(42.0, -17.5);
    let result = id.apply(p);
    assert_eq!(result, p);
}

#[test]
fn identity_matrix_values() {
    let id = Transform2D::identity();
    assert_eq!(id.to_mat3x2(), [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
}

#[test]
fn identity_column_major_3x3() {
    let id = Transform2D::identity();
    assert_eq!(
        id.to_column_major_3x3(),
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    );
}

#[test]
fn translate_column_major_3x3() {
    let t = Transform2D::translate(50.0, 30.0);
    assert_eq!(
        t.to_column_major_3x3(),
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [50.0, 30.0, 1.0]]
    );
}

#[test]
fn scale_column_major_3x3() {
    let t = Transform2D::scale(2.0, 3.0);
    assert_eq!(
        t.to_column_major_3x3(),
        [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 1.0]]
    );
}

#[test]
fn default_is_identity() {
    assert_eq!(Transform2D::default(), Transform2D::identity());
}

// Translate

#[test]
fn translate_point() {
    let t = Transform2D::translate(10.0, -5.0);
    let result = t.apply(Point::new(3.0, 7.0));
    assert_eq!(result, Point::new(13.0, 2.0));
}

#[test]
fn translate_origin() {
    let t = Transform2D::translate(100.0, 200.0);
    let result = t.apply(Point::new(0.0, 0.0));
    assert_eq!(result, Point::new(100.0, 200.0));
}

// Scale

#[test]
fn scale_point() {
    let t = Transform2D::scale(2.0, 3.0);
    let result = t.apply(Point::new(4.0, 5.0));
    assert_eq!(result, Point::new(8.0, 15.0));
}

#[test]
fn scale_uniform() {
    let t = Transform2D::scale(0.5, 0.5);
    let result = t.apply(Point::new(10.0, 20.0));
    assert_eq!(result, Point::new(5.0, 10.0));
}

#[test]
fn scale_negative_mirrors() {
    let t = Transform2D::scale(-1.0, 1.0);
    let result = t.apply(Point::new(5.0, 3.0));
    assert_eq!(result, Point::new(-5.0, 3.0));
}

// Rotate

#[test]
fn rotate_90_degrees() {
    let t = Transform2D::rotate(FRAC_PI_2);
    let result = t.apply(Point::new(1.0, 0.0));
    assert_point_near(result, Point::new(0.0, 1.0), 1e-6);
}

#[test]
fn rotate_180_degrees() {
    let t = Transform2D::rotate(PI);
    let result = t.apply(Point::new(1.0, 0.0));
    assert_point_near(result, Point::new(-1.0, 0.0), 1e-6);
}

#[test]
fn rotate_360_degrees() {
    let t = Transform2D::rotate(TAU);
    let result = t.apply(Point::new(1.0, 0.0));
    // Full rotation returns to original (within float precision).
    assert_point_near(result, Point::new(1.0, 0.0), 1e-5);
}

#[test]
fn rotate_negative_90() {
    let t = Transform2D::rotate(-FRAC_PI_2);
    let result = t.apply(Point::new(0.0, 1.0));
    assert_point_near(result, Point::new(1.0, 0.0), 1e-6);
}

// Concat

#[test]
fn concat_translate_then_scale() {
    // concat: self * other → apply other first, then self.
    // Scale first, then translate.
    let t = Transform2D::translate(10.0, 0.0).concat(&Transform2D::scale(2.0, 2.0));
    let result = t.apply(Point::new(1.0, 0.0));
    // Scale (1,0) → (2,0), then translate → (12,0).
    assert_eq!(result, Point::new(12.0, 0.0));
}

#[test]
fn concat_scale_then_translate() {
    // Scale applied after translate: scale(translate(point)).
    let t = Transform2D::scale(2.0, 2.0).concat(&Transform2D::translate(10.0, 0.0));
    let result = t.apply(Point::new(1.0, 0.0));
    // Translate (1,0) → (11,0), then scale → (22,0).
    assert_eq!(result, Point::new(22.0, 0.0));
}

#[test]
fn concat_associativity() {
    let a = Transform2D::translate(1.0, 2.0);
    let b = Transform2D::scale(3.0, 4.0);
    let c = Transform2D::rotate(0.5);

    // (a * b) * c == a * (b * c)
    let ab_c = a.concat(&b).concat(&c);
    let a_bc = a.concat(&b.concat(&c));
    assert_transform_near(ab_c, a_bc, 1e-6);
}

#[test]
fn concat_with_identity() {
    let t = Transform2D::translate(5.0, 10.0);
    let id = Transform2D::identity();

    // t * identity == t
    assert_eq!(t.concat(&id), t);
    // identity * t == t
    assert_eq!(id.concat(&t), t);
}

// Pre-translate / pre-scale

#[test]
fn pre_translate_equivalent_to_concat() {
    let base = Transform2D::scale(2.0, 3.0);
    let via_pre = base.pre_translate(10.0, 20.0);
    let via_concat = base.concat(&Transform2D::translate(10.0, 20.0));
    assert_transform_near(via_pre, via_concat, 1e-6);
}

#[test]
fn pre_scale_equivalent_to_concat() {
    let base = Transform2D::translate(10.0, 20.0);
    let via_pre = base.pre_scale(2.0, 3.0);
    let via_concat = base.concat(&Transform2D::scale(2.0, 3.0));
    assert_transform_near(via_pre, via_concat, 1e-6);
}

// Apply rect

#[test]
fn apply_rect_identity() {
    let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    let result = Transform2D::identity().apply_rect(r);
    assert_eq!(result, r);
}

#[test]
fn apply_rect_translate() {
    let r = Rect::new(0.0, 0.0, 10.0, 10.0);
    let result = Transform2D::translate(5.0, 15.0).apply_rect(r);
    assert_eq!(result, Rect::new(5.0, 15.0, 10.0, 10.0));
}

#[test]
fn apply_rect_scale() {
    let r = Rect::new(10.0, 10.0, 20.0, 30.0);
    let result = Transform2D::scale(2.0, 0.5).apply_rect(r);
    assert_eq!(result, Rect::new(20.0, 5.0, 40.0, 15.0));
}

#[test]
fn apply_rect_rotation_expands_bounds() {
    // A 45-degree rotation of a unit square should produce a larger AABB.
    let r = Rect::new(0.0, 0.0, 1.0, 1.0);
    let result = Transform2D::rotate(PI / 4.0).apply_rect(r);
    // Rotated square's AABB is wider and taller than the original.
    assert!(result.width() > 1.0);
    assert!(result.height() > 1.0);
}

// Inverse

#[test]
fn inverse_identity() {
    let inv = Transform2D::identity()
        .inverse()
        .expect("identity is invertible");
    assert_eq!(inv, Transform2D::identity());
}

#[test]
fn inverse_translate_roundtrip() {
    let t = Transform2D::translate(5.0, -10.0);
    let inv = t.inverse().expect("translation is invertible");
    let p = Point::new(42.0, 17.0);
    let roundtrip = inv.apply(t.apply(p));
    assert_point_near(roundtrip, p, 1e-5);
}

#[test]
fn inverse_scale_roundtrip() {
    let t = Transform2D::scale(2.0, 0.5);
    let inv = t.inverse().expect("non-zero scale is invertible");
    let p = Point::new(7.0, 13.0);
    let roundtrip = inv.apply(t.apply(p));
    assert_point_near(roundtrip, p, 1e-5);
}

#[test]
fn inverse_rotation_roundtrip() {
    let t = Transform2D::rotate(1.23);
    let inv = t.inverse().expect("rotation is invertible");
    let p = Point::new(3.0, 4.0);
    let roundtrip = inv.apply(t.apply(p));
    assert_point_near(roundtrip, p, 1e-4);
}

#[test]
fn inverse_complex_roundtrip() {
    let t = Transform2D::translate(10.0, 20.0)
        .concat(&Transform2D::scale(3.0, 2.0))
        .concat(&Transform2D::rotate(0.7));
    let inv = t.inverse().expect("composed transform is invertible");
    let p = Point::new(-5.0, 8.0);
    let roundtrip = inv.apply(t.apply(p));
    assert_point_near(roundtrip, p, 1e-3);
}

// Degenerate (no inverse)

#[test]
fn degenerate_zero_scale_no_inverse() {
    let t = Transform2D::scale(0.0, 1.0);
    assert!(t.inverse().is_none());
}

#[test]
fn degenerate_both_zero_no_inverse() {
    let t = Transform2D::scale(0.0, 0.0);
    assert!(t.inverse().is_none());
}

// is_identity

#[test]
fn is_identity_true() {
    assert!(Transform2D::identity().is_identity());
    assert!(Transform2D::default().is_identity());
}

#[test]
fn is_identity_false_for_translate() {
    assert!(!Transform2D::translate(1.0, 0.0).is_identity());
}

#[test]
fn is_identity_false_for_scale() {
    assert!(!Transform2D::scale(2.0, 1.0).is_identity());
}

#[test]
fn is_identity_false_for_rotate() {
    assert!(!Transform2D::rotate(0.1).is_identity());
}

// Lerp

#[test]
fn lerp_at_zero_returns_start() {
    let a = Transform2D::translate(0.0, 0.0);
    let b = Transform2D::translate(10.0, 20.0);
    let result = Transform2D::lerp(a, b, 0.0);
    assert_eq!(result, a);
}

#[test]
fn lerp_at_one_returns_end() {
    let a = Transform2D::translate(0.0, 0.0);
    let b = Transform2D::translate(10.0, 20.0);
    let result = Transform2D::lerp(a, b, 1.0);
    assert_eq!(result, b);
}

#[test]
fn lerp_at_midpoint() {
    let a = Transform2D::identity();
    let b = Transform2D::translate(10.0, 20.0);
    let result = Transform2D::lerp(a, b, 0.5);
    let expected = Transform2D::translate(5.0, 10.0);
    assert_transform_near(result, expected, 1e-6);
}

#[test]
fn lerp_scale_interpolation() {
    let a = Transform2D::scale(1.0, 1.0);
    let b = Transform2D::scale(3.0, 5.0);
    let result = Transform2D::lerp(a, b, 0.5);
    let expected = Transform2D::scale(2.0, 3.0);
    assert_transform_near(result, expected, 1e-6);
}

#[test]
fn lerp_between_identity_and_translate() {
    let a = Transform2D::identity();
    let b = Transform2D::translate(100.0, 200.0);
    let result = Transform2D::lerp(a, b, 0.25);

    // At t=0.25: tx = 25, ty = 50. Other elements stay at identity.
    let p = result.apply(Point::new(0.0, 0.0));
    assert_point_near(p, Point::new(25.0, 50.0), 1e-5);
}

// Debug

#[test]
fn debug_format_includes_components() {
    let t = Transform2D::translate(1.0, 2.0);
    let debug = format!("{t:?}");
    assert!(debug.contains("Transform2D"));
    assert!(debug.contains("tx"));
    assert!(debug.contains("ty"));
}

// Concat with rotation

#[test]
fn rotate_then_translate() {
    // Rotate 90° then translate right. Point (1,0):
    // Rotate: (0, 1). Translate right by 5: (5, 1).
    let t = Transform2D::translate(5.0, 0.0).concat(&Transform2D::rotate(FRAC_PI_2));
    let result = t.apply(Point::new(1.0, 0.0));
    assert_point_near(result, Point::new(5.0, 1.0), 1e-5);
}

// Edge cases

#[test]
fn apply_zero_point() {
    let t = Transform2D::translate(3.0, 7.0).concat(&Transform2D::scale(2.0, 2.0));
    let result = t.apply(Point::new(0.0, 0.0));
    // Scale (0,0) → (0,0), translate → (3,7).
    assert_eq!(result, Point::new(3.0, 7.0));
}

#[test]
fn concat_two_translates() {
    let a = Transform2D::translate(3.0, 4.0);
    let b = Transform2D::translate(7.0, 6.0);
    let result = a.concat(&b);
    // translate(3,4) * translate(7,6) = translate(10,10).
    assert_eq!(result, Transform2D::translate(10.0, 10.0));
}

#[test]
fn concat_two_scales() {
    let a = Transform2D::scale(2.0, 3.0);
    let b = Transform2D::scale(4.0, 5.0);
    let result = a.concat(&b);
    // scale(2,3) * scale(4,5) = scale(8,15).
    assert_eq!(result, Transform2D::scale(8.0, 15.0));
}

// --- 43.2 Layer Primitives ---

// LayerId

#[test]
fn layer_id_equality() {
    let id_a = LayerId::new(1);
    let id_b = LayerId::new(1);
    let id_c = LayerId::new(2);
    assert_eq!(id_a, id_b);
    assert_ne!(id_a, id_c);
}

#[test]
fn layer_id_hash_consistency() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(LayerId::new(1));
    set.insert(LayerId::new(2));
    set.insert(LayerId::new(1)); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn layer_id_debug_format() {
    let id = LayerId::new(42);
    assert_eq!(format!("{id:?}"), "LayerId(42)");
}

#[test]
fn layer_id_display_format() {
    let id = LayerId::new(7);
    assert_eq!(format!("{id}"), "7");
}

// LayerProperties

#[test]
fn layer_properties_default_is_identity() {
    let props = LayerProperties::default();
    assert_eq!(props.opacity, 1.0);
    assert!(props.transform.is_identity());
    assert!(props.visible);
    assert!(!props.clip_children);
    assert!(props.bounds.is_empty()); // default Rect is empty
}

// Layer — needs_texture

#[test]
fn needs_texture_false_for_defaults() {
    let layer = Layer::new(
        LayerId::new(1),
        LayerType::Textured,
        LayerProperties::default(),
    );
    assert!(!layer.needs_texture());
}

#[test]
fn needs_texture_true_when_opacity_below_one() {
    let mut props = LayerProperties::default();
    props.opacity = 0.5;
    let layer = Layer::new(LayerId::new(1), LayerType::Textured, props);
    assert!(layer.needs_texture());
}

#[test]
fn needs_texture_true_when_transform_non_identity() {
    let mut props = LayerProperties::default();
    props.transform = Transform2D::translate(10.0, 0.0);
    let layer = Layer::new(LayerId::new(1), LayerType::Textured, props);
    assert!(layer.needs_texture());
}

// Layer — dirty flags

#[test]
fn new_layer_starts_dirty() {
    let layer = Layer::new(
        LayerId::new(1),
        LayerType::Group,
        LayerProperties::default(),
    );
    assert!(layer.needs_paint());
    assert!(layer.needs_composite());
}

#[test]
fn clear_dirty_flags_resets_both() {
    let mut layer = Layer::new(
        LayerId::new(1),
        LayerType::Group,
        LayerProperties::default(),
    );
    layer.clear_dirty_flags();
    assert!(!layer.needs_paint());
    assert!(!layer.needs_composite());
}

#[test]
fn set_opacity_marks_needs_composite() {
    let mut layer = Layer::new(
        LayerId::new(1),
        LayerType::Textured,
        LayerProperties::default(),
    );
    layer.clear_dirty_flags();
    layer.set_opacity(0.5);
    assert!(layer.needs_composite());
    assert!(!layer.needs_paint());
}

#[test]
fn set_transform_marks_needs_composite() {
    let mut layer = Layer::new(
        LayerId::new(1),
        LayerType::Textured,
        LayerProperties::default(),
    );
    layer.clear_dirty_flags();
    layer.set_transform(Transform2D::scale(2.0, 2.0));
    assert!(layer.needs_composite());
}

#[test]
fn set_bounds_marks_needs_composite() {
    let mut layer = Layer::new(
        LayerId::new(1),
        LayerType::Textured,
        LayerProperties::default(),
    );
    layer.clear_dirty_flags();
    layer.set_bounds(Rect::new(0.0, 0.0, 100.0, 50.0));
    assert!(layer.needs_composite());
}

#[test]
fn schedule_paint_marks_needs_paint() {
    let mut layer = Layer::new(
        LayerId::new(1),
        LayerType::Textured,
        LayerProperties::default(),
    );
    layer.clear_dirty_flags();
    layer.schedule_paint();
    assert!(layer.needs_paint());
    assert!(!layer.needs_composite());
}

// Layer — accessors

#[test]
fn layer_accessors() {
    let props = LayerProperties {
        bounds: Rect::new(10.0, 20.0, 100.0, 50.0),
        opacity: 0.8,
        ..LayerProperties::default()
    };
    let layer = Layer::new(LayerId::new(5), LayerType::SolidColor(Color::BLACK), props);
    assert_eq!(layer.id(), LayerId::new(5));
    assert_eq!(layer.kind(), LayerType::SolidColor(Color::BLACK));
    assert_eq!(layer.properties().opacity, 0.8);
    assert!(layer.parent().is_none());
    assert!(layer.children().is_empty());
}

// --- 43.3 Layer Tree ---

fn make_tree() -> LayerTree {
    LayerTree::new(Rect::new(0.0, 0.0, 1920.0, 1080.0))
}

// Construction

#[test]
fn tree_new_has_root() {
    let tree = make_tree();
    assert!(tree.get(tree.root()).is_some());
    assert_eq!(tree.len(), 1);
}

#[test]
fn tree_root_is_group() {
    let tree = make_tree();
    let root = tree.get(tree.root()).unwrap();
    assert_eq!(root.kind(), LayerType::Group);
}

// Add

#[test]
fn add_single_layer() {
    let mut tree = make_tree();
    let root = tree.root();
    let child = tree.add(root, LayerType::Textured, LayerProperties::default());

    assert_eq!(tree.len(), 2);
    let child_layer = tree.get(child).unwrap();
    assert_eq!(child_layer.parent(), Some(root));

    let root_layer = tree.get(root).unwrap();
    assert_eq!(root_layer.children(), &[child]);
}

#[test]
fn add_nested_layers() {
    let mut tree = make_tree();
    let root = tree.root();
    let parent = tree.add(root, LayerType::Group, LayerProperties::default());
    let child_a = tree.add(parent, LayerType::Textured, LayerProperties::default());
    let child_b = tree.add(parent, LayerType::Textured, LayerProperties::default());

    let parent_layer = tree.get(parent).unwrap();
    assert_eq!(parent_layer.children(), &[child_a, child_b]);
    assert_eq!(tree.get(child_a).unwrap().parent(), Some(parent));
    assert_eq!(tree.get(child_b).unwrap().parent(), Some(parent));
    assert_eq!(tree.len(), 4);
}

// Remove with reparenting

#[test]
fn remove_reparents_children() {
    let mut tree = make_tree();
    let root = tree.root();
    let mid = tree.add(root, LayerType::Group, LayerProperties::default());
    let leaf = tree.add(mid, LayerType::Textured, LayerProperties::default());

    tree.remove(mid);

    // `leaf` should now be a child of root.
    assert!(tree.get(mid).is_none());
    assert_eq!(tree.get(leaf).unwrap().parent(), Some(root));
    assert!(tree.get(root).unwrap().children().contains(&leaf));
}

#[test]
fn remove_root_fails() {
    let mut tree = make_tree();
    assert!(!tree.remove(tree.root()));
    assert_eq!(tree.len(), 1);
}

#[test]
fn remove_nonexistent_returns_false() {
    let mut tree = make_tree();
    assert!(!tree.remove(LayerId::new(999)));
}

// Remove subtree

#[test]
fn remove_subtree_cleans_all_descendants() {
    let mut tree = make_tree();
    let root = tree.root();
    let parent = tree.add(root, LayerType::Group, LayerProperties::default());
    let child = tree.add(parent, LayerType::Textured, LayerProperties::default());
    let grandchild = tree.add(child, LayerType::Textured, LayerProperties::default());

    tree.remove_subtree(parent);

    assert!(tree.get(parent).is_none());
    assert!(tree.get(child).is_none());
    assert!(tree.get(grandchild).is_none());
    assert!(tree.get(root).unwrap().children().is_empty());
    assert_eq!(tree.len(), 1); // only root
}

// Z-order

#[test]
fn stack_above_reorders() {
    let mut tree = make_tree();
    let root = tree.root();
    let back = tree.add(root, LayerType::Textured, LayerProperties::default());
    let front = tree.add(root, LayerType::Textured, LayerProperties::default());

    // Initially: [back, front]. Move back above front.
    tree.stack_above(back, front);
    let children = tree.get(root).unwrap().children();
    assert_eq!(children, &[front, back]);
}

#[test]
fn stack_below_reorders() {
    let mut tree = make_tree();
    let root = tree.root();
    let back = tree.add(root, LayerType::Textured, LayerProperties::default());
    let front = tree.add(root, LayerType::Textured, LayerProperties::default());

    // Initially: [back, front]. Move front below back.
    tree.stack_below(front, back);
    let children = tree.get(root).unwrap().children();
    assert_eq!(children, &[front, back]);
}

// Reparent

#[test]
fn reparent_moves_layer() {
    let mut tree = make_tree();
    let root = tree.root();
    let group_a = tree.add(root, LayerType::Group, LayerProperties::default());
    let group_b = tree.add(root, LayerType::Group, LayerProperties::default());
    let layer = tree.add(group_a, LayerType::Textured, LayerProperties::default());

    tree.reparent(layer, group_b);

    assert!(tree.get(group_a).unwrap().children().is_empty());
    assert_eq!(tree.get(group_b).unwrap().children(), &[layer]);
    assert_eq!(tree.get(layer).unwrap().parent(), Some(group_b));
}

// Paint order traversal

#[test]
fn iter_back_to_front_paint_order() {
    let mut tree = make_tree();
    let root = tree.root();
    let back = tree.add(root, LayerType::Textured, LayerProperties::default());
    let front = tree.add(root, LayerType::Textured, LayerProperties::default());

    let order = tree.iter_back_to_front();
    // Back-to-front: back first, then front, then root (post-order).
    assert_eq!(order, vec![back, front, root]);
}

#[test]
fn iter_back_to_front_nested() {
    let mut tree = make_tree();
    let root = tree.root();
    let group = tree.add(root, LayerType::Group, LayerProperties::default());
    let child = tree.add(group, LayerType::Textured, LayerProperties::default());
    let sibling = tree.add(root, LayerType::Textured, LayerProperties::default());

    let order = tree.iter_back_to_front();
    // group's child first, then group, then sibling, then root.
    assert_eq!(order, vec![child, group, sibling, root]);
}

// Accumulated properties

#[test]
fn accumulated_opacity_multiplies_chain() {
    let mut tree = make_tree();
    let root = tree.root();
    tree.set_opacity(root, 0.5);
    let child = tree.add(
        root,
        LayerType::Group,
        LayerProperties {
            opacity: 0.5,
            ..LayerProperties::default()
        },
    );
    let grandchild = tree.add(
        child,
        LayerType::Textured,
        LayerProperties {
            opacity: 0.8,
            ..LayerProperties::default()
        },
    );

    let acc = tree.accumulated_opacity(grandchild);
    // 0.5 * 0.5 * 0.8 = 0.2
    assert!((acc - 0.2).abs() < 1e-6, "expected 0.2, got {acc}");
}

#[test]
fn accumulated_transform_concatenates_chain() {
    let mut tree = make_tree();
    let root = tree.root();
    tree.set_transform(root, Transform2D::translate(10.0, 0.0));
    let child = tree.add(
        root,
        LayerType::Group,
        LayerProperties {
            transform: Transform2D::scale(2.0, 2.0),
            ..LayerProperties::default()
        },
    );

    let acc = tree.accumulated_transform(child);
    // Root translates, child scales. Combined: translate(10,0) * scale(2,2).
    // Apply to (1, 0): scale → (2, 0), translate → (12, 0).
    let p = acc.apply(Point::new(1.0, 0.0));
    assert_point_near(p, Point::new(12.0, 0.0), 1e-5);
}

// Dirty tracking

#[test]
fn dirty_tracking_paint_and_composite() {
    let mut tree = make_tree();
    let root = tree.root();
    let layer = tree.add(root, LayerType::Textured, LayerProperties::default());

    // New layers start dirty.
    assert!(!tree.layers_needing_paint().is_empty());
    assert!(!tree.layers_needing_composite().is_empty());

    tree.clear_dirty_flags();

    assert!(tree.layers_needing_paint().is_empty());
    assert!(tree.layers_needing_composite().is_empty());

    // Mutate a property.
    tree.set_opacity(layer, 0.5);
    assert!(tree.layers_needing_composite().contains(&layer));
    // Paint not affected by property change.
    assert!(!tree.layers_needing_paint().contains(&layer));

    tree.clear_dirty_flags();

    // Schedule paint.
    tree.schedule_paint(layer);
    assert!(tree.layers_needing_paint().contains(&layer));
}

#[test]
fn clear_dirty_flags_clears_all() {
    let mut tree = make_tree();
    let root = tree.root();
    let _a = tree.add(root, LayerType::Textured, LayerProperties::default());
    let _b = tree.add(root, LayerType::Textured, LayerProperties::default());

    tree.clear_dirty_flags();

    assert!(tree.layers_needing_paint().is_empty());
    assert!(tree.layers_needing_composite().is_empty());
}

// Property setters through tree

#[test]
fn tree_set_bounds_updates_layer() {
    let mut tree = make_tree();
    let root = tree.root();
    let layer = tree.add(root, LayerType::Textured, LayerProperties::default());
    tree.clear_dirty_flags();

    let bounds = Rect::new(10.0, 20.0, 200.0, 100.0);
    tree.set_bounds(layer, bounds);

    assert_eq!(tree.get(layer).unwrap().properties().bounds, bounds);
    assert!(tree.get(layer).unwrap().needs_composite());
}

#[test]
fn tree_set_visible_updates_layer() {
    let mut tree = make_tree();
    let root = tree.root();
    let layer = tree.add(root, LayerType::Textured, LayerProperties::default());
    tree.clear_dirty_flags();

    tree.set_visible(layer, false);

    assert!(!tree.get(layer).unwrap().properties().visible);
    assert!(tree.get(layer).unwrap().needs_composite());
}

// --- 43.7 Layer Animator ---

fn make_animator_tree() -> (LayerTree, LayerId) {
    let mut tree = make_tree();
    let root = tree.root();
    let layer = tree.add(root, LayerType::Textured, LayerProperties::default());
    tree.clear_dirty_flags();
    (tree, layer)
}

fn linear_params(duration_ms: u64, tree: &LayerTree, now: Instant) -> AnimationParams<'_> {
    AnimationParams {
        duration: Duration::from_millis(duration_ms),
        easing: Easing::Linear,
        tree,
        now,
    }
}

#[test]
fn opacity_animation_start_to_end() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    animator.animate_opacity(layer, 0.0, &linear_params(100, &tree, now));
    assert!(animator.is_animating(layer, AnimatableProperty::Opacity));
    assert!(animator.is_any_animating());

    // At start: opacity should still be 1.0 (from value).
    animator.tick(&mut tree, now);
    let opacity = tree.get(layer).unwrap().properties().opacity;
    assert!((opacity - 1.0).abs() < 0.01, "at start: {opacity}");

    // At midpoint.
    let mid = now + Duration::from_millis(50);
    animator.tick(&mut tree, mid);
    let opacity = tree.get(layer).unwrap().properties().opacity;
    assert!((opacity - 0.5).abs() < 0.05, "at midpoint: {opacity}");

    // At end.
    let end = now + Duration::from_millis(100);
    animator.tick(&mut tree, end);
    let opacity = tree.get(layer).unwrap().properties().opacity;
    assert!((opacity - 0.0).abs() < 0.01, "at end: {opacity}");

    // Animation removed.
    assert!(!animator.is_animating(layer, AnimatableProperty::Opacity));
}

#[test]
fn transform_animation_start_to_end() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let target = Transform2D::translate(100.0, 0.0);
    animator.animate_transform(layer, target, &linear_params(100, &tree, now));

    let end = now + Duration::from_millis(100);
    animator.tick(&mut tree, end);

    let transform = tree.get(layer).unwrap().properties().transform;
    assert_transform_near(transform, target, 1e-4);
    assert!(!animator.is_any_animating());
}

#[test]
fn bounds_animation_start_to_end() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let target = Rect::new(50.0, 50.0, 200.0, 100.0);
    animator.animate_bounds(layer, target, &linear_params(100, &tree, now));

    let end = now + Duration::from_millis(100);
    animator.tick(&mut tree, end);

    let bounds = tree.get(layer).unwrap().properties().bounds;
    assert_eq!(bounds, target);
}

#[test]
fn tick_advances_interpolation() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    animator.animate_opacity(layer, 0.0, &linear_params(200, &tree, now));

    // 25% through.
    let t1 = now + Duration::from_millis(50);
    animator.tick(&mut tree, t1);
    let opacity = tree.get(layer).unwrap().properties().opacity;
    assert!((opacity - 0.75).abs() < 0.05, "at 25%: {opacity}");

    // 75% through.
    let t2 = now + Duration::from_millis(150);
    animator.tick(&mut tree, t2);
    let opacity = tree.get(layer).unwrap().properties().opacity;
    assert!((opacity - 0.25).abs() < 0.05, "at 75%: {opacity}");
}

#[test]
fn animation_completes_and_is_removed() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    animator.animate_opacity(layer, 0.5, &linear_params(50, &tree, now));
    let still_running = animator.tick(&mut tree, now + Duration::from_millis(100));
    assert!(!still_running);
    assert!(!animator.is_any_animating());
}

#[test]
fn preemption_replaces_running() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    // Start animation to 0.0.
    animator.animate_opacity(layer, 0.0, &linear_params(100, &tree, now));

    // Tick to midpoint (opacity ≈ 0.5).
    let mid = now + Duration::from_millis(50);
    animator.tick(&mut tree, mid);

    // Preempt: new animation from current (≈0.5) to 1.0.
    animator.animate_opacity(layer, 1.0, &linear_params(100, &tree, mid));

    // At the new animation's midpoint, should be ≈0.75.
    let t = mid + Duration::from_millis(50);
    animator.tick(&mut tree, t);
    let opacity = tree.get(layer).unwrap().properties().opacity;
    assert!(
        (opacity - 0.75).abs() < 0.1,
        "preempted mid: expected ~0.75, got {opacity}"
    );
}

#[test]
fn cancel_keeps_current_value() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    animator.animate_opacity(layer, 0.0, &linear_params(100, &tree, now));
    let mid = now + Duration::from_millis(50);
    animator.tick(&mut tree, mid);

    let before_cancel = tree.get(layer).unwrap().properties().opacity;
    animator.cancel(layer, AnimatableProperty::Opacity);

    // Value stays at whatever it was.
    let after_cancel = tree.get(layer).unwrap().properties().opacity;
    assert_eq!(before_cancel, after_cancel);
    assert!(!animator.is_any_animating());
}

#[test]
fn is_any_animating_tracks_state() {
    let (tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    assert!(!animator.is_any_animating());

    animator.animate_opacity(layer, 0.5, &linear_params(100, &tree, now));
    assert!(animator.is_any_animating());

    animator.cancel_all(layer);
    assert!(!animator.is_any_animating());
}

#[test]
fn target_opacity_query() {
    let (tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    assert!(animator.target_opacity(layer).is_none());

    animator.animate_opacity(layer, 0.3, &linear_params(100, &tree, now));
    assert_eq!(animator.target_opacity(layer), Some(0.3));
}

#[test]
fn target_transform_query() {
    let (tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let target = Transform2D::translate(50.0, 25.0);
    animator.animate_transform(layer, target, &linear_params(100, &tree, now));
    assert_eq!(animator.target_transform(layer), Some(target));
}

// --- 43.9 Animation Groups ---

#[test]
fn apply_group_runs_all_transitions_in_parallel() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let group = AnimationGroup {
        layer_id: layer,
        animations: vec![
            PropertyAnimation {
                from: Some(TransitionTarget::Opacity(1.0)),
                target: TransitionTarget::Opacity(0.0),
                duration: None,
                easing: None,
            },
            PropertyAnimation {
                from: Some(TransitionTarget::Transform(Transform2D::identity())),
                target: TransitionTarget::Transform(Transform2D::translate(100.0, 0.0)),
                duration: None,
                easing: None,
            },
        ],
        duration: Duration::from_millis(100),
        easing: Easing::Linear,
    };

    animator.apply_group(&group, &tree, now);

    // Both properties should be animating.
    assert!(animator.is_animating(layer, AnimatableProperty::Opacity));
    assert!(animator.is_animating(layer, AnimatableProperty::Transform));

    // At midpoint, both should be halfway.
    let mid = now + Duration::from_millis(50);
    animator.tick(&mut tree, mid);

    let props = tree.get(layer).unwrap().properties();
    assert!(
        (props.opacity - 0.5).abs() < 0.1,
        "opacity at mid: expected ~0.5, got {}",
        props.opacity
    );
    let tx = props.transform.to_mat3x2();
    assert!(
        (tx[4] - 50.0).abs() < 5.0,
        "translate X at mid: expected ~50, got {}",
        tx[4]
    );
}

#[test]
fn apply_group_explicit_from_overrides_current() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    // Set initial opacity to 0.5.
    tree.set_opacity(layer, 0.5);

    // Group with explicit from=0.0 (overrides the tree value of 0.5).
    let group = AnimationGroup {
        layer_id: layer,
        animations: vec![PropertyAnimation {
            from: Some(TransitionTarget::Opacity(0.0)),
            target: TransitionTarget::Opacity(1.0),
            duration: None,
            easing: None,
        }],
        duration: Duration::from_millis(100),
        easing: Easing::Linear,
    };

    animator.apply_group(&group, &tree, now);

    // At start, opacity should lerp from explicit 0.0 (not tree's 0.5).
    animator.tick(&mut tree, now);
    let opacity = tree.get(layer).unwrap().properties().opacity;
    assert!(
        opacity < 0.1,
        "explicit from=0.0: at start, expected ~0.0, got {opacity}"
    );
}

#[test]
fn apply_group_none_from_reads_current() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    // Set initial opacity to 0.8.
    tree.set_opacity(layer, 0.8);

    // Group with from=None (reads current value from tree).
    let group = AnimationGroup {
        layer_id: layer,
        animations: vec![PropertyAnimation {
            from: None,
            target: TransitionTarget::Opacity(0.0),
            duration: None,
            easing: None,
        }],
        duration: Duration::from_millis(100),
        easing: Easing::Linear,
    };

    animator.apply_group(&group, &tree, now);

    // At midpoint, should be interpolating from 0.8 to 0.0.
    let mid = now + Duration::from_millis(50);
    animator.tick(&mut tree, mid);
    let opacity = tree.get(layer).unwrap().properties().opacity;
    assert!(
        (opacity - 0.4).abs() < 0.1,
        "from=None with tree 0.8: at mid, expected ~0.4, got {opacity}"
    );
}

#[test]
fn apply_group_per_property_duration_override() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let group = AnimationGroup {
        layer_id: layer,
        animations: vec![
            PropertyAnimation {
                from: Some(TransitionTarget::Opacity(1.0)),
                target: TransitionTarget::Opacity(0.0),
                duration: Some(Duration::from_millis(50)), // Fast.
                easing: None,
            },
            PropertyAnimation {
                from: Some(TransitionTarget::Transform(Transform2D::identity())),
                target: TransitionTarget::Transform(Transform2D::translate(100.0, 0.0)),
                duration: Some(Duration::from_millis(200)), // Slow.
                easing: None,
            },
        ],
        duration: Duration::from_millis(100), // Group default (ignored).
        easing: Easing::Linear,
    };

    animator.apply_group(&group, &tree, now);

    // At 50ms: opacity should be finished, transform at 25%.
    let t = now + Duration::from_millis(50);
    animator.tick(&mut tree, t);

    let props = tree.get(layer).unwrap().properties();
    assert!(
        props.opacity < 0.05,
        "fast opacity should be done at 50ms, got {}",
        props.opacity
    );
    let tx = props.transform.to_mat3x2();
    assert!(
        (tx[4] - 25.0).abs() < 5.0,
        "slow transform at 50ms: expected ~25, got {}",
        tx[4]
    );
}

#[test]
fn builder_group_integrates_with_animator() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let group = AnimationBuilder::new(layer)
        .duration(Duration::from_millis(100))
        .easing(Easing::Linear)
        .opacity(1.0, 0.0)
        .transform(Transform2D::identity(), Transform2D::scale(2.0, 2.0))
        .build();

    animator.apply_group(&group, &tree, now);
    assert!(animator.is_animating(layer, AnimatableProperty::Opacity));
    assert!(animator.is_animating(layer, AnimatableProperty::Transform));

    // Run to completion.
    let end = now + Duration::from_millis(100);
    let running = animator.tick(&mut tree, end);
    assert!(!running);

    let props = tree.get(layer).unwrap().properties();
    assert!(
        props.opacity < 0.05,
        "opacity should be ~0 at end, got {}",
        props.opacity
    );
}

// --- Degenerate Transform Applied to Points/Rects ---

#[test]
fn apply_zero_scale_transform_to_point() {
    let t = Transform2D::scale(0.0, 0.0);
    let result = t.apply(Point::new(42.0, 17.0));
    assert_eq!(result, Point::new(0.0, 0.0));
}

#[test]
fn apply_zero_scale_transform_to_rect() {
    let t = Transform2D::scale(0.0, 0.0);
    let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    let result = t.apply_rect(r);
    // All four corners map to origin, so the AABB has zero area.
    assert_eq!(result.width(), 0.0);
    assert_eq!(result.height(), 0.0);
}

#[test]
fn apply_near_zero_scale_produces_finite() {
    let t = Transform2D::scale(f32::MIN_POSITIVE, f32::MIN_POSITIVE);
    let result = t.apply(Point::new(1.0, 1.0));
    assert!(result.x.is_finite());
    assert!(result.y.is_finite());
}

// --- Transform apply_rect on empty/zero-size rect ---

#[test]
fn apply_rect_default_empty_rect() {
    let result = Transform2D::translate(100.0, 200.0).apply_rect(Rect::default());
    // Default rect is (0,0,0,0). Translate origin → (100,200). Width/height stay 0.
    assert_eq!(result, Rect::new(100.0, 200.0, 0.0, 0.0));
}

#[test]
fn apply_rect_zero_width_height() {
    let r = Rect::new(50.0, 50.0, 0.0, 0.0);
    let result = Transform2D::scale(2.0, 3.0).apply_rect(r);
    assert_eq!(result, Rect::new(100.0, 150.0, 0.0, 0.0));
}

// --- Transform concat self ---

#[test]
fn concat_translate_self() {
    let t = Transform2D::translate(5.0, 10.0);
    let result = t.concat(&t);
    assert_eq!(result, Transform2D::translate(10.0, 20.0));
}

#[test]
fn concat_scale_self() {
    let t = Transform2D::scale(2.0, 3.0);
    let result = t.concat(&t);
    assert_eq!(result, Transform2D::scale(4.0, 9.0));
}

#[test]
fn concat_rotate_self_90() {
    let t = Transform2D::rotate(FRAC_PI_2);
    let result = t.concat(&t);
    // 90° + 90° = 180°.
    assert_transform_near(result, Transform2D::rotate(PI), 1e-5);
}

// --- Enqueue Preemption Strategy ---

#[test]
fn enqueue_strategy_queues_second_animation() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new().with_preemption(PreemptionStrategy::Enqueue);
    let now = Instant::now();

    // Start first animation.
    animator.animate_opacity(layer, 0.5, &linear_params(100, &tree, now));
    assert!(animator.is_animating(layer, AnimatableProperty::Opacity));

    // Start second animation — should be enqueued, not replace.
    animator.animate_opacity(layer, 0.0, &linear_params(100, &tree, now));
    assert!(animator.is_any_animating());

    // Tick past first animation's end.
    let after_first = now + Duration::from_millis(100);
    animator.tick(&mut tree, after_first);
    let opacity = tree.get(layer).unwrap().properties().opacity;
    assert!(
        (opacity - 0.5).abs() < 0.05,
        "after first ends, opacity should be ~0.5, got {opacity}"
    );
    // Queued animation should now be active.
    assert!(animator.is_any_animating());

    // Tick past second animation's end.
    let after_second = after_first + Duration::from_millis(100);
    let running = animator.tick(&mut tree, after_second);
    assert!(!running);
    let opacity = tree.get(layer).unwrap().properties().opacity;
    assert!(
        opacity < 0.05,
        "after second ends, opacity should be ~0.0, got {opacity}"
    );
}

// --- LayerAnimator Delegate Callbacks ---

#[test]
fn delegate_animation_ended_fires() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::animation::AnimationDelegate;

    struct TestDelegate {
        ended: Arc<AtomicBool>,
    }

    impl AnimationDelegate for TestDelegate {
        fn animation_ended(&mut self, _: LayerId, _: AnimatableProperty) {
            self.ended.store(true, Ordering::Relaxed);
        }
        fn animation_canceled(&mut self, _: LayerId, _: AnimatableProperty) {}
    }

    let (mut tree, layer) = make_animator_tree();
    let ended = Arc::new(AtomicBool::new(false));
    let mut animator = LayerAnimator::new().with_delegate(Box::new(TestDelegate {
        ended: ended.clone(),
    }));
    let now = Instant::now();

    animator.animate_opacity(layer, 0.0, &linear_params(100, &tree, now));

    let end = now + Duration::from_millis(100);
    animator.tick(&mut tree, end);
    assert!(
        ended.load(Ordering::Relaxed),
        "animation_ended should fire when animation completes"
    );
}

#[test]
fn delegate_animation_canceled_fires_on_preemption() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::animation::AnimationDelegate;

    struct TestDelegate {
        canceled: Arc<AtomicBool>,
    }

    impl AnimationDelegate for TestDelegate {
        fn animation_ended(&mut self, _: LayerId, _: AnimatableProperty) {}
        fn animation_canceled(&mut self, _: LayerId, _: AnimatableProperty) {
            self.canceled.store(true, Ordering::Relaxed);
        }
    }

    let (mut tree, layer) = make_animator_tree();
    let canceled = Arc::new(AtomicBool::new(false));
    let mut animator = LayerAnimator::new().with_delegate(Box::new(TestDelegate {
        canceled: canceled.clone(),
    }));
    let now = Instant::now();

    animator.animate_opacity(layer, 0.0, &linear_params(100, &tree, now));

    // Preempt with a new animation (default ReplaceCurrent strategy).
    let mid = now + Duration::from_millis(50);
    animator.tick(&mut tree, mid);
    animator.animate_opacity(layer, 1.0, &linear_params(100, &tree, mid));

    assert!(
        canceled.load(Ordering::Relaxed),
        "animation_canceled should fire on preemption"
    );
}

// --- Nested Opacity Accumulation Precision ---

#[test]
fn accumulated_opacity_deep_chain_10_layers() {
    let mut tree = make_tree();
    let root = tree.root();

    // Build a chain of 10 layers, each at 0.9 opacity.
    tree.set_opacity(root, 0.9);
    let mut current = root;
    for _ in 0..9 {
        let child = tree.add(
            current,
            LayerType::Group,
            LayerProperties {
                opacity: 0.9,
                ..LayerProperties::default()
            },
        );
        current = child;
    }

    let acc = tree.accumulated_opacity(current);
    // 0.9^10 ≈ 0.3486784401.
    let expected = 0.9_f32.powi(10);
    assert!(
        (acc - expected).abs() < 1e-5,
        "expected 0.9^10 ≈ {expected}, got {acc}"
    );
    assert!(acc > 0.0, "deep opacity chain should not underflow to zero");
}

// --- Layer Reparent Edge Cases ---

#[test]
fn reparent_to_self_does_not_panic() {
    let mut tree = make_tree();
    let root = tree.root();
    let layer = tree.add(root, LayerType::Textured, LayerProperties::default());

    // Reparent to self should not corrupt the tree.
    tree.reparent(layer, layer);
    assert!(tree.get(layer).is_some());
    assert_eq!(tree.len(), 2);
}

#[test]
fn reparent_parent_under_child_does_not_panic() {
    let mut tree = make_tree();
    let root = tree.root();
    let parent = tree.add(root, LayerType::Group, LayerProperties::default());
    let child = tree.add(parent, LayerType::Textured, LayerProperties::default());

    // Reparent parent under its own child — could create a cycle.
    // Just verify no panic.
    tree.reparent(parent, child);
    assert!(tree.get(parent).is_some());
    assert!(tree.get(child).is_some());
}

// --- Stack Above/Below Edge Cases ---

#[test]
fn stack_above_same_layer_does_not_panic() {
    let mut tree = make_tree();
    let root = tree.root();
    let layer = tree.add(root, LayerType::Textured, LayerProperties::default());
    let _other = tree.add(root, LayerType::Textured, LayerProperties::default());

    let count_before = tree.get(root).unwrap().children().len();
    tree.stack_above(layer, layer);
    let count_after = tree.get(root).unwrap().children().len();
    assert_eq!(count_before, count_after);
}

#[test]
fn stack_above_different_parents_is_noop() {
    let mut tree = make_tree();
    let root = tree.root();
    let group_a = tree.add(root, LayerType::Group, LayerProperties::default());
    let group_b = tree.add(root, LayerType::Group, LayerProperties::default());
    let child_a = tree.add(group_a, LayerType::Textured, LayerProperties::default());
    let child_b = tree.add(group_b, LayerType::Textured, LayerProperties::default());

    // Cross-parent stacking should be a no-op.
    tree.stack_above(child_a, child_b);
    assert_eq!(tree.get(child_a).unwrap().parent(), Some(group_a));
}

#[test]
fn stack_below_nonexistent_sibling_is_noop() {
    let mut tree = make_tree();
    let root = tree.root();
    let layer = tree.add(root, LayerType::Textured, LayerProperties::default());

    tree.stack_below(layer, LayerId::new(999));
    assert!(tree.get(layer).is_some());
}

// --- LayerTree set_* on Nonexistent Layers ---

#[test]
fn set_opacity_nonexistent_is_noop() {
    let mut tree = make_tree();
    tree.set_opacity(LayerId::new(999), 0.5);
    // No panic; tree unchanged.
    assert_eq!(tree.len(), 1);
}

#[test]
fn set_transform_nonexistent_is_noop() {
    let mut tree = make_tree();
    tree.set_transform(LayerId::new(999), Transform2D::translate(10.0, 20.0));
    assert_eq!(tree.len(), 1);
}

#[test]
fn schedule_paint_nonexistent_is_noop() {
    let mut tree = make_tree();
    tree.schedule_paint(LayerId::new(999));
    assert_eq!(tree.len(), 1);
}

// --- Zero-Duration Animation ---

#[test]
fn zero_duration_animation_immediately_sets_value() {
    let (mut tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let params = AnimationParams {
        duration: Duration::ZERO,
        easing: Easing::Linear,
        tree: &tree,
        now,
    };

    animator.animate_opacity(layer, 0.0, &params);

    let running = animator.tick(&mut tree, now);
    assert!(!running);
    let opacity = tree.get(layer).unwrap().properties().opacity;
    assert!(
        opacity < 0.01,
        "zero-duration animation should immediately set target: got {opacity}"
    );
}

// --- Animation Group with Empty Animations ---

#[test]
fn apply_group_empty_animations_is_noop() {
    let (tree, layer) = make_animator_tree();
    let mut animator = LayerAnimator::new();
    let now = Instant::now();

    let group = AnimationGroup {
        layer_id: layer,
        animations: vec![],
        duration: Duration::from_millis(100),
        easing: Easing::Linear,
    };

    animator.apply_group(&group, &tree, now);
    assert!(!animator.is_any_animating());
}

// --- Large Tree Traversal ---

#[test]
fn large_flat_tree_traversal_visits_all() {
    let mut tree = make_tree();
    let root = tree.root();

    let mut layer_ids = Vec::with_capacity(50);
    for _ in 0..50 {
        let id = tree.add(root, LayerType::Textured, LayerProperties::default());
        layer_ids.push(id);
    }

    let order = tree.iter_back_to_front();
    // 50 children + root = 51.
    assert_eq!(order.len(), 51);

    for id in &layer_ids {
        assert!(order.contains(id), "layer {id:?} missing from traversal");
    }
    // Root is last (post-order).
    assert_eq!(*order.last().unwrap(), root);
}

#[test]
fn deep_chain_traversal_order() {
    let mut tree = make_tree();
    let root = tree.root();

    // Build a deep chain: root → g0 → g1 → ... → g9 → leaf.
    let mut current = root;
    for _ in 0..10 {
        current = tree.add(current, LayerType::Group, LayerProperties::default());
    }
    let leaf = tree.add(current, LayerType::Textured, LayerProperties::default());

    let order = tree.iter_back_to_front();
    // leaf + 10 groups + root = 12.
    assert_eq!(order.len(), 12);
    // Leaf (deepest) should be first in back-to-front order.
    assert_eq!(order[0], leaf);
    // Root should be last.
    assert_eq!(*order.last().unwrap(), root);
}
