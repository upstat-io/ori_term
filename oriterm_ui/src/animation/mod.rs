//! Animation system — smooth interpolation for widget state transitions.
//!
//! Provides [`Lerp`] (linear interpolation trait), [`Easing`] (timing curves),
//! [`Animation`] (raw `f32` interpolation), and [`AnimatedValue`] (widget-embeddable
//! wrapper that manages animation lifecycle).

use std::fmt;
use std::time::{Duration, Instant};

/// Linear interpolation between two values.
///
/// Implementations must satisfy: `lerp(a, b, 0.0) == a` and `lerp(a, b, 1.0) == b`.
pub trait Lerp: Copy {
    /// Interpolates between `a` and `b` at parameter `t` (0.0..1.0).
    fn lerp(a: Self, b: Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        a + (b - a) * t
    }
}

/// Easing curve applied to animation progress.
///
/// Maps a linear time fraction `t` in `[0.0, 1.0]` to an eased output
/// value, also in `[0.0, 1.0]` (though overshoot is possible with some
/// cubic bezier configurations).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    /// No easing — constant velocity.
    Linear,
    /// Starts slow, accelerates. Cubic: `t^3`.
    EaseIn,
    /// Starts fast, decelerates. Cubic: `1 - (1-t)^3`.
    EaseOut,
    /// Slow start and end, fast middle. Piecewise cubic.
    EaseInOut,
    /// Custom cubic bezier with control points `(x1, y1, x2, y2)`.
    ///
    /// The curve starts at `(0, 0)` and ends at `(1, 1)`. The two control
    /// points define the shape. CSS `ease` is `CubicBezier(0.25, 0.1, 0.25, 1.0)`.
    CubicBezier(f32, f32, f32, f32),
}

impl Easing {
    /// Applies the easing function to a linear time fraction.
    ///
    /// Input `t` is clamped to `[0.0, 1.0]`. Output is the eased value.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t * t,
            Self::EaseOut => {
                let inv = 1.0 - t;
                1.0 - inv * inv * inv
            }
            Self::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let inv = -2.0 * t + 2.0;
                    1.0 - inv * inv * inv / 2.0
                }
            }
            Self::CubicBezier(x1, y1, x2, y2) => cubic_bezier_y(x1, y1, x2, y2, t),
        }
    }
}

/// Solves a cubic bezier curve for the y-value at a given x (time).
///
/// Uses Newton's method (up to 8 iterations) with bisection fallback
/// to find the bezier parameter `s` such that `bezier_x(s) == t`,
/// then evaluates `bezier_y(s)`.
fn cubic_bezier_y(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }

    // Guard: non-finite control points degrade to linear.
    if !x1.is_finite() || !y1.is_finite() || !x2.is_finite() || !y2.is_finite() {
        return t;
    }

    // Bezier x(s) = 3*(1-s)^2*s*x1 + 3*(1-s)*s^2*x2 + s^3
    let bezier_x = |s: f32| -> f32 {
        let inv = 1.0 - s;
        3.0 * inv * inv * s * x1 + 3.0 * inv * s * s * x2 + s * s * s
    };

    // Derivative dx/ds.
    let bezier_dx = |s: f32| -> f32 {
        let inv = 1.0 - s;
        3.0 * inv * inv * x1 + 6.0 * inv * s * (x2 - x1) + 3.0 * s * s * (1.0 - x2)
    };

    // Bezier y(s) = 3*(1-s)^2*s*y1 + 3*(1-s)*s^2*y2 + s^3
    let bezier_y = |s: f32| -> f32 {
        let inv = 1.0 - s;
        3.0 * inv * inv * s * y1 + 3.0 * inv * s * s * y2 + s * s * s
    };

    // Newton's method to solve bezier_x(s) = t.
    let mut s = t; // Initial guess.
    for _ in 0..8 {
        let x = bezier_x(s) - t;
        if x.abs() < 1e-6 {
            return bezier_y(s);
        }
        let dx = bezier_dx(s);
        if dx.abs() < 1e-6 {
            break;
        }
        s -= x / dx;
        s = s.clamp(0.0, 1.0);
    }

    // Bisection fallback if Newton didn't converge.
    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    s = t;
    for _ in 0..20 {
        let x = bezier_x(s);
        if (x - t).abs() < 1e-6 {
            return bezier_y(s);
        }
        if x < t {
            lo = s;
        } else {
            hi = s;
        }
        s = f32::midpoint(lo, hi);
    }

    bezier_y(s)
}

/// A raw `f32` animation from one value to another over a duration.
///
/// Stateless — does not track whether it is "active". The caller provides
/// the current [`Instant`] to query progress.
#[derive(Debug, Clone, Copy)]
pub struct Animation {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
    easing: Easing,
}

impl Animation {
    /// Creates a new animation.
    pub fn new(from: f32, to: f32, duration: Duration, easing: Easing, start: Instant) -> Self {
        Self {
            from,
            to,
            start,
            duration,
            easing,
        }
    }

    /// Returns the eased interpolated value at the given instant.
    ///
    /// Before `start`, returns `from`. After `start + duration`, returns `to`.
    /// Zero-duration animations return `to` immediately.
    pub fn progress(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return self.to;
        }
        if now <= self.start {
            return self.from;
        }
        let elapsed = now.duration_since(self.start);
        if elapsed >= self.duration {
            return self.to;
        }
        let t = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        let eased = self.easing.apply(t);
        f32::lerp(self.from, self.to, eased)
    }

    /// Returns `true` if the animation has finished.
    pub fn is_finished(&self, now: Instant) -> bool {
        now.duration_since(self.start) >= self.duration
    }
}

/// An in-flight animation for [`AnimatedValue`].
#[derive(Debug, Clone, Copy)]
struct ActiveAnimation<T: Lerp> {
    from: T,
    to: T,
    start: Instant,
}

/// A value that transitions smoothly between states.
///
/// Embeddable in widget structs. Stores the resting value plus an optional
/// in-flight animation. Query with [`get`](Self::get) using the current
/// frame timestamp.
pub struct AnimatedValue<T: Lerp> {
    /// The target (resting) value.
    value: T,
    /// In-flight animation, if any.
    animation: Option<ActiveAnimation<T>>,
    /// Animation duration.
    duration: Duration,
    /// Easing curve.
    easing: Easing,
}

impl<T: Lerp> AnimatedValue<T> {
    /// Creates an animated value with no active animation.
    pub fn new(value: T, duration: Duration, easing: Easing) -> Self {
        Self {
            value,
            animation: None,
            duration,
            easing,
        }
    }

    /// Returns the current interpolated value at the given instant.
    ///
    /// If no animation is active, returns the resting value immediately.
    pub fn get(&self, now: Instant) -> T {
        let Some(anim) = &self.animation else {
            return self.value;
        };
        let elapsed = now.duration_since(anim.start);
        if elapsed >= self.duration {
            return self.value;
        }
        let t = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        let eased = self.easing.apply(t);
        T::lerp(anim.from, anim.to, eased)
    }

    /// Returns `true` if an animation is currently in flight.
    pub fn is_animating(&self, now: Instant) -> bool {
        self.animation
            .as_ref()
            .is_some_and(|anim| now.duration_since(anim.start) < self.duration)
    }

    /// Starts an animation from the current interpolated value to `new_value`.
    ///
    /// If an animation is already running, it restarts from the current
    /// interpolated position (smooth interruption).
    pub fn set(&mut self, new_value: T, now: Instant) {
        let current = self.get(now);
        self.value = new_value;
        self.animation = Some(ActiveAnimation {
            from: current,
            to: new_value,
            start: now,
        });
    }

    /// Sets the value immediately, cancelling any in-flight animation.
    pub fn set_immediate(&mut self, value: T) {
        self.value = value;
        self.animation = None;
    }

    /// Returns the final resting value (the last `set` or `set_immediate` value).
    pub fn target(&self) -> T {
        self.value
    }
}

impl<T: Lerp + fmt::Debug> fmt::Debug for AnimatedValue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnimatedValue")
            .field("value", &self.value)
            .field("has_animation", &self.animation.is_some())
            .field("duration", &self.duration)
            .field("easing", &self.easing)
            .finish()
    }
}

impl<T: Lerp + Clone> Clone for AnimatedValue<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value,
            animation: self.animation,
            duration: self.duration,
            easing: self.easing,
        }
    }
}

#[cfg(test)]
mod tests;
