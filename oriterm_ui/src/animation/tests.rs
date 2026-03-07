//! Unit tests for the animation system.

use std::time::{Duration, Instant};

use super::{AnimatedValue, Animation, Easing, Lerp};

// Lerp f32

#[test]
fn lerp_f32_at_zero() {
    assert_eq!(f32::lerp(10.0, 20.0, 0.0), 10.0);
}

#[test]
fn lerp_f32_at_one() {
    assert_eq!(f32::lerp(10.0, 20.0, 1.0), 20.0);
}

#[test]
fn lerp_f32_at_midpoint() {
    assert_eq!(f32::lerp(10.0, 20.0, 0.5), 15.0);
}

#[test]
fn lerp_f32_arbitrary_range() {
    let result = f32::lerp(-5.0, 15.0, 0.25);
    assert!((result - 0.0).abs() < 1e-6);
}

#[test]
fn lerp_f32_identity() {
    assert_eq!(f32::lerp(42.0, 42.0, 0.5), 42.0);
}

// Easing

#[test]
fn easing_linear_at_boundaries() {
    assert_eq!(Easing::Linear.apply(0.0), 0.0);
    assert_eq!(Easing::Linear.apply(1.0), 1.0);
}

#[test]
fn easing_linear_at_midpoint() {
    assert_eq!(Easing::Linear.apply(0.5), 0.5);
}

#[test]
fn easing_ease_in_at_boundaries() {
    assert_eq!(Easing::EaseIn.apply(0.0), 0.0);
    assert_eq!(Easing::EaseIn.apply(1.0), 1.0);
}

#[test]
fn easing_ease_in_slower_than_linear() {
    // EaseIn at midpoint should be less than 0.5 (slower start).
    let value = Easing::EaseIn.apply(0.5);
    assert!(value < 0.5, "EaseIn at 0.5 should be < 0.5, got {value}");
}

#[test]
fn easing_ease_out_at_boundaries() {
    assert_eq!(Easing::EaseOut.apply(0.0), 0.0);
    assert_eq!(Easing::EaseOut.apply(1.0), 1.0);
}

#[test]
fn easing_ease_out_faster_than_linear() {
    // EaseOut at midpoint should be greater than 0.5 (faster start).
    let value = Easing::EaseOut.apply(0.5);
    assert!(value > 0.5, "EaseOut at 0.5 should be > 0.5, got {value}");
}

#[test]
fn easing_ease_in_out_at_boundaries() {
    assert_eq!(Easing::EaseInOut.apply(0.0), 0.0);
    assert_eq!(Easing::EaseInOut.apply(1.0), 1.0);
}

#[test]
fn easing_ease_in_out_symmetric() {
    let at_quarter = Easing::EaseInOut.apply(0.25);
    let at_three_quarters = Easing::EaseInOut.apply(0.75);
    // Symmetric: f(0.25) + f(0.75) ≈ 1.0.
    assert!(
        (at_quarter + at_three_quarters - 1.0).abs() < 1e-6,
        "EaseInOut should be symmetric: {at_quarter} + {at_three_quarters} ≈ 1.0"
    );
}

#[test]
fn easing_ease_in_out_at_midpoint() {
    let value = Easing::EaseInOut.apply(0.5);
    assert!(
        (value - 0.5).abs() < 1e-6,
        "EaseInOut at 0.5 should be 0.5, got {value}"
    );
}

#[test]
fn easing_cubic_bezier_linear() {
    // CubicBezier(0,0,1,1) ≈ linear.
    let bezier = Easing::CubicBezier(0.0, 0.0, 1.0, 1.0);
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let value = bezier.apply(t);
        assert!(
            (value - t).abs() < 0.01,
            "CubicBezier(0,0,1,1) at {t} should ≈ {t}, got {value}"
        );
    }
}

#[test]
fn easing_cubic_bezier_css_ease_monotonic() {
    // CSS ease: (0.25, 0.1, 0.25, 1.0). Must be monotonically non-decreasing.
    let ease = Easing::CubicBezier(0.25, 0.1, 0.25, 1.0);
    let mut prev = 0.0;
    for i in 0..=100 {
        let t = i as f32 / 100.0;
        let value = ease.apply(t);
        assert!(
            value >= prev - 1e-4,
            "CSS ease should be monotonic: at t={t}, value={value} < prev={prev}"
        );
        prev = value;
    }
}

#[test]
fn easing_cubic_bezier_boundaries() {
    let bezier = Easing::CubicBezier(0.25, 0.1, 0.25, 1.0);
    assert_eq!(bezier.apply(0.0), 0.0);
    assert_eq!(bezier.apply(1.0), 1.0);
}

#[test]
fn easing_clamps_input() {
    // Values outside [0, 1] should be clamped.
    assert_eq!(Easing::Linear.apply(-0.5), 0.0);
    assert_eq!(Easing::Linear.apply(1.5), 1.0);
    assert_eq!(Easing::EaseIn.apply(-1.0), 0.0);
    assert_eq!(Easing::EaseOut.apply(2.0), 1.0);
}

// Animation

#[test]
fn animation_progress_at_start() {
    let now = Instant::now();
    let anim = Animation::new(0.0, 100.0, Duration::from_millis(200), Easing::Linear, now);
    assert_eq!(anim.progress(now), 0.0);
}

#[test]
fn animation_progress_at_end() {
    let now = Instant::now();
    let anim = Animation::new(0.0, 100.0, Duration::from_millis(200), Easing::Linear, now);
    let end = now + Duration::from_millis(200);
    assert_eq!(anim.progress(end), 100.0);
}

#[test]
fn animation_progress_at_midpoint() {
    let now = Instant::now();
    let anim = Animation::new(0.0, 100.0, Duration::from_millis(200), Easing::Linear, now);
    let mid = now + Duration::from_millis(100);
    let value = anim.progress(mid);
    assert!(
        (value - 50.0).abs() < 1.0,
        "expected ~50 at midpoint, got {value}"
    );
}

#[test]
fn animation_progress_past_end() {
    let now = Instant::now();
    let anim = Animation::new(0.0, 100.0, Duration::from_millis(200), Easing::Linear, now);
    let past = now + Duration::from_millis(500);
    assert_eq!(anim.progress(past), 100.0);
}

#[test]
fn animation_progress_before_start() {
    let start = Instant::now() + Duration::from_secs(10);
    let now = Instant::now();
    let anim = Animation::new(5.0, 95.0, Duration::from_millis(200), Easing::Linear, start);
    assert_eq!(anim.progress(now), 5.0);
}

#[test]
fn animation_is_finished_timing() {
    let now = Instant::now();
    let anim = Animation::new(0.0, 1.0, Duration::from_millis(100), Easing::Linear, now);

    assert!(!anim.is_finished(now));
    assert!(!anim.is_finished(now + Duration::from_millis(50)));
    assert!(anim.is_finished(now + Duration::from_millis(100)));
    assert!(anim.is_finished(now + Duration::from_millis(200)));
}

#[test]
fn animation_zero_duration() {
    let now = Instant::now();
    let anim = Animation::new(0.0, 100.0, Duration::ZERO, Easing::Linear, now);
    // Zero duration: immediately finished.
    assert!(anim.is_finished(now));
    assert_eq!(anim.progress(now), 100.0);
}

#[test]
fn animation_with_easing() {
    let now = Instant::now();
    let anim = Animation::new(0.0, 100.0, Duration::from_millis(200), Easing::EaseIn, now);
    let mid = now + Duration::from_millis(100);
    let value = anim.progress(mid);
    // EaseIn at 0.5 → 0.125 → value ≈ 12.5.
    assert!(value < 50.0, "EaseIn midpoint should be < 50, got {value}");
}

// AnimatedValue

#[test]
fn animated_value_get_without_animation() {
    let av: AnimatedValue<f32> =
        AnimatedValue::new(42.0, Duration::from_millis(100), Easing::Linear);
    let now = Instant::now();
    assert_eq!(av.get(now), 42.0);
}

#[test]
fn animated_value_not_animating_initially() {
    let av: AnimatedValue<f32> =
        AnimatedValue::new(0.0, Duration::from_millis(100), Easing::Linear);
    assert!(!av.is_animating(Instant::now()));
}

#[test]
fn animated_value_set_starts_animation() {
    let mut av: AnimatedValue<f32> =
        AnimatedValue::new(0.0, Duration::from_millis(100), Easing::Linear);
    let now = Instant::now();
    av.set(1.0, now);

    assert!(av.is_animating(now));
    assert_eq!(av.target(), 1.0);
    // At start, get() should return the starting value (0.0).
    assert_eq!(av.get(now), 0.0);
}

#[test]
fn animated_value_lifecycle() {
    let mut av: AnimatedValue<f32> =
        AnimatedValue::new(0.0, Duration::from_millis(100), Easing::Linear);
    let now = Instant::now();
    av.set(1.0, now);

    // Midpoint.
    let mid = now + Duration::from_millis(50);
    let mid_val = av.get(mid);
    assert!(
        (mid_val - 0.5).abs() < 0.05,
        "expected ~0.5 at midpoint, got {mid_val}"
    );
    assert!(av.is_animating(mid));

    // End.
    let end = now + Duration::from_millis(100);
    assert_eq!(av.get(end), 1.0);
    assert!(!av.is_animating(end));
}

#[test]
fn animated_value_set_immediate_bypasses() {
    let mut av: AnimatedValue<f32> =
        AnimatedValue::new(0.0, Duration::from_millis(100), Easing::Linear);
    let now = Instant::now();

    // Start an animation.
    av.set(1.0, now);
    assert!(av.is_animating(now));

    // Immediately override.
    av.set_immediate(0.5);
    assert!(!av.is_animating(now));
    assert_eq!(av.get(now), 0.5);
    assert_eq!(av.target(), 0.5);
}

#[test]
fn animated_value_interruption() {
    let mut av: AnimatedValue<f32> =
        AnimatedValue::new(0.0, Duration::from_millis(100), Easing::Linear);
    let now = Instant::now();
    av.set(1.0, now);

    // At midpoint, interrupt with a new target.
    let mid = now + Duration::from_millis(50);
    let mid_val = av.get(mid);
    av.set(0.0, mid);

    // The new animation starts from the interrupted position.
    assert_eq!(av.target(), 0.0);
    assert_eq!(av.get(mid), mid_val); // At the moment of interruption.

    // After the new animation completes.
    let end = mid + Duration::from_millis(100);
    assert_eq!(av.get(end), 0.0);
}

#[test]
fn animated_value_target_always_final() {
    let mut av: AnimatedValue<f32> =
        AnimatedValue::new(0.0, Duration::from_millis(100), Easing::Linear);
    assert_eq!(av.target(), 0.0);

    let now = Instant::now();
    av.set(1.0, now);
    assert_eq!(av.target(), 1.0);

    av.set(0.5, now);
    assert_eq!(av.target(), 0.5);

    av.set_immediate(0.75);
    assert_eq!(av.target(), 0.75);
}

#[test]
fn animated_value_debug_format() {
    let av: AnimatedValue<f32> =
        AnimatedValue::new(1.0, Duration::from_millis(100), Easing::Linear);
    let debug = format!("{av:?}");
    assert!(debug.contains("AnimatedValue"));
    assert!(debug.contains("value"));
}

#[test]
fn animated_value_clone() {
    let av: AnimatedValue<f32> =
        AnimatedValue::new(42.0, Duration::from_millis(100), Easing::EaseInOut);
    let cloned = av.clone();
    assert_eq!(cloned.target(), 42.0);
    assert_eq!(cloned.get(Instant::now()), 42.0);
}

// Cubic bezier robustness (Chromium cubic_bezier_unittest.cc patterns)

#[test]
fn cubic_bezier_nan_control_points_produce_finite_output() {
    // NaN control points must not propagate — output should remain finite.
    let bezier = Easing::CubicBezier(f32::NAN, 0.0, 1.0, 1.0);
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let val = bezier.apply(t);
        assert!(val.is_finite(), "NaN x1: at t={t}, got {val}");
    }

    let bezier2 = Easing::CubicBezier(0.25, f32::NAN, 0.25, 1.0);
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let val = bezier2.apply(t);
        assert!(val.is_finite(), "NaN y1: at t={t}, got {val}");
    }
}

#[test]
fn cubic_bezier_infinity_control_points_produce_finite_output() {
    // Infinity control points must produce finite output.
    let bezier = Easing::CubicBezier(f32::INFINITY, 0.0, 1.0, 1.0);
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let val = bezier.apply(t);
        assert!(val.is_finite(), "Inf x1: at t={t}, got {val}");
    }

    let bezier2 = Easing::CubicBezier(0.0, f32::NEG_INFINITY, 1.0, 1.0);
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let val = bezier2.apply(t);
        assert!(val.is_finite(), "NegInf y1: at t={t}, got {val}");
    }
}

#[test]
fn cubic_bezier_overshoot_y_outside_zero_one() {
    // CSS allows y control points outside [0,1], producing overshoot.
    // Curve with y values that overshoot: y1=1.5 means output can exceed 1.0.
    let bezier = Easing::CubicBezier(0.0, 1.5, 1.0, 1.0);
    let mut found_overshoot = false;
    for i in 0..=100 {
        let t = i as f32 / 100.0;
        let val = bezier.apply(t);
        if val > 1.0 + 1e-4 {
            found_overshoot = true;
        }
        assert!(val.is_finite(), "overshoot curve at t={t} should be finite");
    }
    assert!(
        found_overshoot,
        "curve with y1=1.5 should overshoot above 1.0 at some point"
    );
}

#[test]
fn cubic_bezier_undershoot_y_negative() {
    // y control points below 0 produce output below 0.
    let bezier = Easing::CubicBezier(1.0, -0.5, 0.0, 1.0);
    let mut found_undershoot = false;
    for i in 0..=100 {
        let t = i as f32 / 100.0;
        let val = bezier.apply(t);
        if val < -1e-4 {
            found_undershoot = true;
        }
        assert!(
            val.is_finite(),
            "undershoot curve at t={t} should be finite"
        );
    }
    assert!(
        found_undershoot,
        "curve with y1=-0.5 should undershoot below 0.0 at some point"
    );
}

#[test]
fn cubic_bezier_solver_round_trip_accuracy() {
    // For well-behaved curves, verify the solver is accurate by checking
    // that standard easing curves hit expected properties.
    // CSS ease: (0.25, 0.1, 0.25, 1.0) — starts slow, ends fast.
    let ease = Easing::CubicBezier(0.25, 0.1, 0.25, 1.0);

    // At t=0 and t=1, output must be exact.
    assert_eq!(ease.apply(0.0), 0.0);
    assert_eq!(ease.apply(1.0), 1.0);

    // At t=0.5, CSS ease is known to produce ~0.80 (faster in second half).
    let mid = ease.apply(0.5);
    assert!(
        mid > 0.6 && mid < 0.95,
        "CSS ease at t=0.5 should be ~0.80, got {mid}"
    );

    // All sample points must be in [0, 1] for this well-behaved curve.
    for i in 0..=100 {
        let t = i as f32 / 100.0;
        let val = ease.apply(t);
        assert!(
            val >= -1e-4 && val <= 1.0 + 1e-4,
            "CSS ease at t={t}: {val} outside [0, 1]"
        );
    }
}

#[test]
fn easing_ease_in_ease_out_complementary() {
    // Property: EaseIn(t) and EaseOut(t) are complementary curves.
    // EaseOut(t) == 1 - EaseIn(1 - t) for the cubic forms.
    for i in 0..=20 {
        let t = i as f32 / 20.0;
        let ease_in_val = Easing::EaseIn.apply(t);
        let ease_out_complement = 1.0 - Easing::EaseIn.apply(1.0 - t);
        let ease_out_val = Easing::EaseOut.apply(t);
        assert!(
            (ease_out_val - ease_out_complement).abs() < 1e-5,
            "at t={t}: EaseOut({t})={ease_out_val} != 1-EaseIn(1-{t})={ease_out_complement}"
        );
        // Also: EaseIn is always <= Linear, EaseOut is always >= Linear in [0,1].
        assert!(
            ease_in_val <= t + 1e-6,
            "EaseIn({t})={ease_in_val} should be <= {t}"
        );
        assert!(
            ease_out_val >= t - 1e-6,
            "EaseOut({t})={ease_out_val} should be >= {t}"
        );
    }
}

#[test]
fn cubic_bezier_zero_zero_zero_zero_is_linear() {
    // CubicBezier(0,0,0,0) — degenerate, but must not crash or produce NaN.
    let bezier = Easing::CubicBezier(0.0, 0.0, 0.0, 0.0);
    assert_eq!(bezier.apply(0.0), 0.0);
    assert_eq!(bezier.apply(1.0), 1.0);
    for i in 1..10 {
        let t = i as f32 / 10.0;
        let val = bezier.apply(t);
        assert!(
            val.is_finite(),
            "degenerate bezier at t={t} should be finite"
        );
    }
}

#[test]
fn cubic_bezier_one_one_one_one_is_linear() {
    // CubicBezier(1,1,1,1) should behave approximately linearly.
    let bezier = Easing::CubicBezier(1.0, 1.0, 1.0, 1.0);
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let val = bezier.apply(t);
        assert!(
            (val - t).abs() < 0.05,
            "CubicBezier(1,1,1,1) at t={t}: expected ~{t}, got {val}"
        );
    }
}

// AnimatedValue edge cases

#[test]
fn animated_value_rapid_set_same_frame() {
    // Multiple `set()` calls at the same instant — should not panic or produce
    // weird intermediate states.
    let mut av: AnimatedValue<f32> =
        AnimatedValue::new(0.0, Duration::from_millis(100), Easing::Linear);
    let now = Instant::now();

    av.set(1.0, now);
    av.set(0.5, now);
    av.set(0.8, now);

    // Target should be the last set value.
    assert_eq!(av.target(), 0.8);
    // At `now`, the animation just started from wherever the previous get() was.
    assert!(av.is_animating(now));
    let val = av.get(now);
    assert!(val.is_finite(), "rapid set should produce finite values");

    // After duration, should reach the final target.
    let end = now + Duration::from_millis(100);
    assert_eq!(av.get(end), 0.8);
}

#[test]
fn animated_value_set_to_same_value() {
    // Setting to the same value should still produce a valid animation
    // (from current to same target).
    let mut av: AnimatedValue<f32> =
        AnimatedValue::new(0.5, Duration::from_millis(100), Easing::Linear);
    let now = Instant::now();

    av.set(0.5, now);
    assert_eq!(av.target(), 0.5);
    // At start of animation, from == to, so get() returns 0.5 at all times.
    assert_eq!(av.get(now), 0.5);
    assert_eq!(av.get(now + Duration::from_millis(50)), 0.5);
}

#[test]
fn animation_reverse_range() {
    // Animation from high to low value.
    let now = Instant::now();
    let anim = Animation::new(100.0, 0.0, Duration::from_millis(200), Easing::Linear, now);

    assert_eq!(anim.progress(now), 100.0);
    let mid = now + Duration::from_millis(100);
    let val = anim.progress(mid);
    assert!(
        (val - 50.0).abs() < 1.0,
        "reverse animation midpoint: expected ~50, got {val}"
    );
    assert_eq!(anim.progress(now + Duration::from_millis(200)), 0.0);
}

#[test]
fn animation_negative_range() {
    // Animation across negative values.
    let now = Instant::now();
    let anim = Animation::new(-10.0, 10.0, Duration::from_millis(200), Easing::Linear, now);

    assert_eq!(anim.progress(now), -10.0);
    let mid = now + Duration::from_millis(100);
    let val = anim.progress(mid);
    assert!(
        val.abs() < 1.0,
        "negative range midpoint: expected ~0, got {val}"
    );
    assert_eq!(anim.progress(now + Duration::from_millis(200)), 10.0);
}

// AnimationGroup

#[test]
fn group_has_correct_defaults() {
    use super::group::{AnimationGroup, PropertyAnimation, TransitionTarget};
    use crate::geometry::LayerId;

    let id = LayerId::new(1);
    let group = AnimationGroup {
        layer_id: id,
        animations: vec![PropertyAnimation {
            from: Some(TransitionTarget::Opacity(0.0)),
            target: TransitionTarget::Opacity(1.0),
            duration: None,
            easing: None,
        }],
        duration: Duration::from_millis(200),
        easing: Easing::EaseOut,
    };

    assert_eq!(group.layer_id, id);
    assert_eq!(group.animations.len(), 1);
    assert_eq!(group.duration, Duration::from_millis(200));
}

#[test]
fn group_property_animation_per_property_overrides() {
    use super::group::{PropertyAnimation, TransitionTarget};

    let anim = PropertyAnimation {
        from: Some(TransitionTarget::Opacity(0.0)),
        target: TransitionTarget::Opacity(1.0),
        duration: Some(Duration::from_millis(500)),
        easing: Some(Easing::EaseIn),
    };

    assert_eq!(anim.duration.unwrap(), Duration::from_millis(500));
    assert_eq!(anim.easing.unwrap(), Easing::EaseIn);
}

// AnimationBuilder

#[test]
fn builder_produces_correct_group() {
    use super::builder::AnimationBuilder;
    use crate::geometry::Rect;
    use crate::geometry::{LayerId, Transform2D};

    let id = LayerId::new(1);
    let group = AnimationBuilder::new(id)
        .duration(Duration::from_millis(150))
        .easing(Easing::EaseOut)
        .opacity(0.0, 1.0)
        .transform(Transform2D::scale(0.95, 0.95), Transform2D::identity())
        .bounds(
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Rect::new(10.0, 10.0, 200.0, 100.0),
        )
        .build();

    assert_eq!(group.layer_id, id);
    assert_eq!(group.animations.len(), 3);
    assert_eq!(group.duration, Duration::from_millis(150));
    assert_eq!(group.easing, Easing::EaseOut);
}

#[test]
fn builder_default_duration_and_easing() {
    use super::builder::AnimationBuilder;
    use crate::geometry::LayerId;

    let group = AnimationBuilder::new(LayerId::new(1))
        .opacity(0.0, 1.0)
        .build();

    // Defaults: 200ms, EaseOut.
    assert_eq!(group.duration, Duration::from_millis(200));
    assert_eq!(group.easing, Easing::EaseOut);
}

#[test]
fn builder_build_sequence_with_on_end() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::builder::AnimationBuilder;
    use super::sequence::SequenceState;
    use crate::geometry::LayerId;

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();

    let id = LayerId::new(1);
    let mut seq = AnimationBuilder::new(id)
        .opacity(0.0, 1.0)
        .on_end(move |_| {
            called_clone.store(true, Ordering::Relaxed);
        })
        .build_sequence();

    assert_eq!(seq.layer_id(), id);
    assert_eq!(seq.state(), SequenceState::Pending);

    // Start: first step is Animate.
    let now = Instant::now();
    let group = seq.start(now);
    assert!(group.is_some());

    // Advance past the animation (pretend it finished).
    let group = seq.advance(now + Duration::from_secs(1), true);
    // Callback step fires automatically.
    assert!(group.is_none());
    assert!(called.load(Ordering::Relaxed));
    assert!(seq.is_finished());
}

// AnimationSequence

#[test]
fn sequence_empty_is_immediately_finished() {
    use super::sequence::{AnimationSequence, SequenceState};
    use crate::geometry::LayerId;

    let mut seq = AnimationSequence::new(LayerId::new(1), vec![]);
    let now = Instant::now();
    let result = seq.start(now);
    assert!(result.is_none());
    assert_eq!(seq.state(), SequenceState::Finished);
}

#[test]
fn sequence_delay_step_pauses() {
    use super::sequence::{AnimationSequence, AnimationStep, SequenceState};
    use crate::geometry::LayerId;

    let mut seq = AnimationSequence::new(
        LayerId::new(1),
        vec![AnimationStep::Delay(Duration::from_millis(100))],
    );

    let now = Instant::now();
    seq.start(now);
    assert_eq!(seq.state(), SequenceState::Running(0));

    // Before delay elapses — still on step 0.
    let result = seq.advance(now + Duration::from_millis(50), false);
    assert!(result.is_none());
    assert_eq!(seq.state(), SequenceState::Running(0));

    // After delay elapses — finishes.
    let result = seq.advance(now + Duration::from_millis(100), false);
    assert!(result.is_none());
    assert!(seq.is_finished());
}

#[test]
fn sequence_callback_fires_immediately() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::sequence::{AnimationSequence, AnimationStep};
    use crate::geometry::LayerId;

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();

    let mut seq = AnimationSequence::new(
        LayerId::new(1),
        vec![AnimationStep::Callback(Some(Box::new(move |_| {
            called_clone.store(true, Ordering::Relaxed);
        })))],
    );

    let now = Instant::now();
    seq.start(now);

    // Callback fires during start, then sequence finishes.
    assert!(called.load(Ordering::Relaxed));
    assert!(seq.is_finished());
}

#[test]
fn sequence_steps_execute_in_order() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::group::{AnimationGroup, PropertyAnimation, TransitionTarget};
    use super::sequence::{AnimationSequence, AnimationStep, SequenceState};
    use crate::geometry::LayerId;

    let counter = Arc::new(AtomicU32::new(0));
    let id = LayerId::new(1);

    let c1 = counter.clone();
    let c2 = counter.clone();

    let group = AnimationGroup {
        layer_id: id,
        animations: vec![PropertyAnimation {
            from: Some(TransitionTarget::Opacity(0.0)),
            target: TransitionTarget::Opacity(1.0),
            duration: None,
            easing: None,
        }],
        duration: Duration::from_millis(100),
        easing: Easing::Linear,
    };

    let mut seq = AnimationSequence::new(
        id,
        vec![
            AnimationStep::Callback(Some(Box::new(move |_| {
                c1.store(1, Ordering::Relaxed);
            }))),
            AnimationStep::Animate(group),
            AnimationStep::Callback(Some(Box::new(move |_| {
                c2.store(2, Ordering::Relaxed);
            }))),
        ],
    );

    let now = Instant::now();

    // Start: fires first callback, then returns animate group.
    let group_ref = seq.start(now);
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    assert!(group_ref.is_some());
    assert_eq!(seq.state(), SequenceState::Running(1));

    // Advance after animation finishes → fires second callback.
    let result = seq.advance(now + Duration::from_millis(200), true);
    assert!(result.is_none());
    assert_eq!(counter.load(Ordering::Relaxed), 2);
    assert!(seq.is_finished());
}

// Lerp geometry types

#[test]
fn lerp_point_at_boundaries() {
    use crate::geometry::Point;
    let a: Point = Point::new(0.0, 0.0);
    let b: Point = Point::new(10.0, 20.0);
    assert_eq!(Point::lerp(a, b, 0.0), a);
    assert_eq!(Point::lerp(a, b, 1.0), b);
}

#[test]
fn lerp_point_at_midpoint() {
    use crate::geometry::Point;
    let a: Point = Point::new(0.0, 0.0);
    let b: Point = Point::new(10.0, 20.0);
    let mid = Point::lerp(a, b, 0.5);
    assert_eq!(mid, Point::new(5.0, 10.0));
}

#[test]
fn lerp_size_at_boundaries() {
    use crate::geometry::Size;
    let a: Size = Size::new(100.0, 200.0);
    let b: Size = Size::new(300.0, 400.0);
    assert_eq!(Size::lerp(a, b, 0.0), a);
    assert_eq!(Size::lerp(a, b, 1.0), b);
}

#[test]
fn lerp_size_at_midpoint() {
    use crate::geometry::Size;
    let a: Size = Size::new(100.0, 200.0);
    let b: Size = Size::new(300.0, 400.0);
    let mid = Size::lerp(a, b, 0.5);
    assert_eq!(mid, Size::new(200.0, 300.0));
}

#[test]
fn lerp_rect_at_boundaries() {
    use crate::geometry::Rect;
    let a: Rect = Rect::new(0.0, 0.0, 100.0, 50.0);
    let b: Rect = Rect::new(10.0, 20.0, 200.0, 100.0);
    assert_eq!(Rect::lerp(a, b, 0.0), a);
    assert_eq!(Rect::lerp(a, b, 1.0), b);
}

#[test]
fn lerp_rect_at_midpoint() {
    use crate::geometry::Rect;
    let a: Rect = Rect::new(0.0, 0.0, 100.0, 50.0);
    let b: Rect = Rect::new(10.0, 20.0, 200.0, 100.0);
    let mid = Rect::lerp(a, b, 0.5);
    assert_eq!(mid, Rect::new(5.0, 10.0, 150.0, 75.0));
}

// AnimatedValue with EaseInOut easing

#[test]
fn animated_value_ease_in_out_symmetric() {
    let mut av: AnimatedValue<f32> =
        AnimatedValue::new(0.0, Duration::from_millis(200), Easing::EaseInOut);
    let now = Instant::now();
    av.set(1.0, now);

    // At 25%: should be < 0.5 (slow start).
    let at_quarter = av.get(now + Duration::from_millis(50));
    assert!(
        at_quarter < 0.5,
        "EaseInOut at 25% should be < 0.5, got {at_quarter}"
    );

    // At midpoint: should be ~0.5.
    let at_mid = av.get(now + Duration::from_millis(100));
    assert!(
        (at_mid - 0.5).abs() < 0.05,
        "EaseInOut at 50% should be ~0.5, got {at_mid}"
    );

    // Symmetry: at_quarter + at_three_quarters ≈ 1.0.
    let at_three_quarters = av.get(now + Duration::from_millis(150));
    assert!(
        (at_quarter + at_three_quarters - 1.0).abs() < 0.05,
        "EaseInOut should be symmetric: {at_quarter} + {at_three_quarters} ≈ 1.0"
    );
}
