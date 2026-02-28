//! Drives property transitions on compositor layers.
//!
//! The [`LayerAnimator`] interpolates layer properties (opacity, transform,
//! bounds) over time, applying eased values to the [`LayerTree`] each frame
//! via [`tick`](LayerAnimator::tick). Lives in `oriterm_ui` with no GPU
//! dependency.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::time::{Duration, Instant};

use crate::animation::group::{AnimationGroup, TransitionTarget};
use crate::animation::{AnimatableProperty, AnimationDelegate, Easing, Lerp};
use crate::geometry::Rect;

use super::Transform2D;
use super::layer::LayerId;
use super::layer_tree::LayerTree;

/// How to handle a new animation when one is already running.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PreemptionStrategy {
    /// Cancel the running animation and start from its current value.
    #[default]
    ReplaceCurrent,
    /// Queue the new animation after the current one finishes.
    Enqueue,
}

/// Bundles timing and context for starting an animation.
pub struct AnimationParams<'a> {
    /// How long the animation should take.
    pub duration: Duration,
    /// Easing curve to apply.
    pub easing: Easing,
    /// Layer tree used to read current property values.
    pub tree: &'a LayerTree,
    /// Current frame timestamp.
    pub now: Instant,
}

/// An in-flight property transition.
#[derive(Debug, Clone, Copy)]
struct PropertyTransition {
    kind: TransitionKind,
    start: Instant,
    duration: Duration,
    easing: Easing,
}

/// What property is being animated and between which values.
#[derive(Debug, Clone, Copy)]
enum TransitionKind {
    Opacity { from: f32, to: f32 },
    Transform { from: Transform2D, to: Transform2D },
    Bounds { from: Rect, to: Rect },
}

impl PropertyTransition {
    /// Returns the linear progress (0.0..1.0) at the given instant.
    fn progress(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return 1.0;
        }
        let elapsed = now.saturating_duration_since(self.start);
        if elapsed >= self.duration {
            return 1.0;
        }
        elapsed.as_secs_f32() / self.duration.as_secs_f32()
    }

    /// Returns `true` if the animation has finished.
    fn is_finished(&self, now: Instant) -> bool {
        self.progress(now) >= 1.0
    }
}

/// A queued animation waiting for the current one to finish.
#[derive(Debug, Clone, Copy)]
struct QueuedTransition {
    layer_id: LayerId,
    kind: TransitionKind,
    duration: Duration,
    easing: Easing,
}

/// Drives property transitions on compositor layers.
///
/// Each frame, call [`tick`](Self::tick) to advance all running
/// animations, apply interpolated values to the layer tree, and
/// fire delegate callbacks for completed/canceled transitions.
pub struct LayerAnimator {
    /// Active transitions keyed by (layer, property).
    transitions: HashMap<(LayerId, AnimatableProperty), PropertyTransition>,
    /// Queued transitions waiting for current ones to finish.
    queue: Vec<QueuedTransition>,
    /// Lifecycle callback receiver.
    delegate: Option<Box<dyn AnimationDelegate>>,
    /// How to handle preemption.
    preemption: PreemptionStrategy,
    /// Reusable buffer for transition keys during tick.
    scratch_keys: Vec<(LayerId, AnimatableProperty)>,
    /// Reusable buffer for ended transitions during tick.
    scratch_ended: Vec<(LayerId, AnimatableProperty)>,
}

impl LayerAnimator {
    // --- Constructors ---

    /// Creates a new animator with default preemption strategy.
    pub fn new() -> Self {
        Self {
            transitions: HashMap::new(),
            queue: Vec::new(),
            delegate: None,
            preemption: PreemptionStrategy::default(),
            scratch_keys: Vec::new(),
            scratch_ended: Vec::new(),
        }
    }

    /// Sets the preemption strategy.
    #[must_use]
    pub fn with_preemption(mut self, strategy: PreemptionStrategy) -> Self {
        self.preemption = strategy;
        self
    }

    /// Sets the animation delegate for lifecycle callbacks.
    #[must_use]
    pub fn with_delegate(mut self, delegate: Box<dyn AnimationDelegate>) -> Self {
        self.delegate = Some(delegate);
        self
    }

    // --- Animation starters ---

    /// Starts an opacity animation on the given layer.
    pub fn animate_opacity(&mut self, id: LayerId, target: f32, params: &AnimationParams<'_>) {
        let from = self.current_opacity(id, params.tree, params.now);
        let kind = TransitionKind::Opacity { from, to: target };
        self.start_transition(id, AnimatableProperty::Opacity, kind, params);
    }

    /// Starts a transform animation on the given layer.
    pub fn animate_transform(
        &mut self,
        id: LayerId,
        target: Transform2D,
        params: &AnimationParams<'_>,
    ) {
        let from = self.current_transform(id, params.tree, params.now);
        let kind = TransitionKind::Transform { from, to: target };
        self.start_transition(id, AnimatableProperty::Transform, kind, params);
    }

    /// Starts a bounds animation on the given layer.
    pub fn animate_bounds(&mut self, id: LayerId, target: Rect, params: &AnimationParams<'_>) {
        let from = self.current_bounds(id, params.tree, params.now);
        let kind = TransitionKind::Bounds { from, to: target };
        self.start_transition(id, AnimatableProperty::Bounds, kind, params);
    }

    // --- Group application ---

    /// Starts all property animations in an [`AnimationGroup`] simultaneously.
    ///
    /// Each animation uses the group's default duration/easing unless the
    /// individual [`PropertyAnimation`] provides overrides. If a property
    /// specifies an explicit `from` value, that is used; otherwise the
    /// current value is read from the layer tree.
    pub fn apply_group(&mut self, group: &AnimationGroup, tree: &LayerTree, now: Instant) {
        for anim in &group.animations {
            let duration = anim.duration.unwrap_or(group.duration);
            let easing = anim.easing.unwrap_or(group.easing);
            let params = AnimationParams {
                duration,
                easing,
                tree,
                now,
            };
            match anim.target {
                TransitionTarget::Opacity(to) => {
                    let from = match anim.from {
                        Some(TransitionTarget::Opacity(v)) => v,
                        _ => self.current_opacity(group.layer_id, tree, now),
                    };
                    let kind = TransitionKind::Opacity { from, to };
                    self.start_transition(
                        group.layer_id,
                        AnimatableProperty::Opacity,
                        kind,
                        &params,
                    );
                }
                TransitionTarget::Transform(to) => {
                    let from = match anim.from {
                        Some(TransitionTarget::Transform(v)) => v,
                        _ => self.current_transform(group.layer_id, tree, now),
                    };
                    let kind = TransitionKind::Transform { from, to };
                    self.start_transition(
                        group.layer_id,
                        AnimatableProperty::Transform,
                        kind,
                        &params,
                    );
                }
                TransitionTarget::Bounds(to) => {
                    let from = match anim.from {
                        Some(TransitionTarget::Bounds(v)) => v,
                        _ => self.current_bounds(group.layer_id, tree, now),
                    };
                    let kind = TransitionKind::Bounds { from, to };
                    self.start_transition(
                        group.layer_id,
                        AnimatableProperty::Bounds,
                        kind,
                        &params,
                    );
                }
            }
        }
    }

    // --- Tick ---

    /// Advances all animations and applies current values to the tree.
    ///
    /// Returns `true` if any animations are still running (caller
    /// should request another frame).
    pub fn tick(&mut self, tree: &mut LayerTree, now: Instant) -> bool {
        self.scratch_keys.clear();
        self.scratch_keys.extend(self.transitions.keys().copied());
        self.scratch_ended.clear();

        for key in &self.scratch_keys {
            let (layer_id, _) = *key;
            let transition = self.transitions[key];
            let t = transition.easing.apply(transition.progress(now));

            match transition.kind {
                TransitionKind::Opacity { from, to } => {
                    tree.set_opacity(layer_id, f32::lerp(from, to, t));
                }
                TransitionKind::Transform { from, to } => {
                    tree.set_transform(layer_id, Transform2D::lerp(from, to, t));
                }
                TransitionKind::Bounds { from, to } => {
                    tree.set_bounds(layer_id, Rect::lerp(from, to, t));
                }
            }

            if transition.is_finished(now) {
                self.scratch_ended.push(*key);
            }
        }

        // Remove finished and fire callbacks.
        for i in 0..self.scratch_ended.len() {
            let key = self.scratch_ended[i];
            self.transitions.remove(&key);
            if let Some(delegate) = &mut self.delegate {
                delegate.animation_ended(key.0, key.1);
            }
        }

        // Promote queued transitions into freed slots.
        self.promote_queued(now);

        !self.transitions.is_empty() || !self.queue.is_empty()
    }

    // --- Queries ---

    /// Returns `true` if a specific property on a layer is animating.
    pub fn is_animating(&self, id: LayerId, property: AnimatableProperty) -> bool {
        self.transitions.contains_key(&(id, property))
    }

    /// Returns `true` if any animation is running.
    pub fn is_any_animating(&self) -> bool {
        !self.transitions.is_empty() || !self.queue.is_empty()
    }

    /// Returns the target opacity for an in-flight animation.
    pub fn target_opacity(&self, id: LayerId) -> Option<f32> {
        self.transitions
            .get(&(id, AnimatableProperty::Opacity))
            .and_then(|t| match t.kind {
                TransitionKind::Opacity { to, .. } => Some(to),
                _ => None,
            })
    }

    /// Returns the target transform for an in-flight animation.
    pub fn target_transform(&self, id: LayerId) -> Option<Transform2D> {
        self.transitions
            .get(&(id, AnimatableProperty::Transform))
            .and_then(|t| match t.kind {
                TransitionKind::Transform { to, .. } => Some(to),
                _ => None,
            })
    }

    // --- Cancellation ---

    /// Cancels a specific property animation, keeping the current value.
    pub fn cancel(&mut self, id: LayerId, property: AnimatableProperty) {
        if self.transitions.remove(&(id, property)).is_some() {
            if let Some(delegate) = &mut self.delegate {
                delegate.animation_canceled(id, property);
            }
        }
    }

    /// Cancels all animations on a layer.
    pub fn cancel_all(&mut self, id: LayerId) {
        let props = [
            AnimatableProperty::Opacity,
            AnimatableProperty::Transform,
            AnimatableProperty::Bounds,
        ];
        for prop in props {
            self.cancel(id, prop);
        }
        self.queue.retain(|q| q.layer_id != id);
    }

    // --- Private helpers ---

    /// Starts or preempts a transition.
    fn start_transition(
        &mut self,
        id: LayerId,
        prop: AnimatableProperty,
        kind: TransitionKind,
        params: &AnimationParams<'_>,
    ) {
        let key = (id, prop);
        let transition = PropertyTransition {
            kind,
            start: params.now,
            duration: params.duration,
            easing: params.easing,
        };

        match self.preemption {
            PreemptionStrategy::ReplaceCurrent => {
                if self.transitions.insert(key, transition).is_some() {
                    if let Some(delegate) = &mut self.delegate {
                        delegate.animation_canceled(id, prop);
                    }
                }
            }
            PreemptionStrategy::Enqueue => {
                if let Entry::Vacant(e) = self.transitions.entry(key) {
                    e.insert(transition);
                } else {
                    self.queue.push(QueuedTransition {
                        layer_id: id,
                        kind,
                        duration: params.duration,
                        easing: params.easing,
                    });
                }
            }
        }
    }

    /// Promotes queued transitions into active slots that have freed up.
    fn promote_queued(&mut self, now: Instant) {
        let queue = std::mem::take(&mut self.queue);
        for queued in queue {
            let prop = match queued.kind {
                TransitionKind::Opacity { .. } => AnimatableProperty::Opacity,
                TransitionKind::Transform { .. } => AnimatableProperty::Transform,
                TransitionKind::Bounds { .. } => AnimatableProperty::Bounds,
            };
            let key = (queued.layer_id, prop);
            if let Entry::Vacant(e) = self.transitions.entry(key) {
                e.insert(PropertyTransition {
                    kind: queued.kind,
                    start: now,
                    duration: queued.duration,
                    easing: queued.easing,
                });
            } else {
                // Still occupied — re-queue.
                self.queue.push(queued);
            }
        }
    }

    /// Gets the current opacity (interpolated if animating).
    fn current_opacity(&self, id: LayerId, tree: &LayerTree, now: Instant) -> f32 {
        if let Some(transition) = self.transitions.get(&(id, AnimatableProperty::Opacity)) {
            let t = transition.easing.apply(transition.progress(now));
            if let TransitionKind::Opacity { from, to } = transition.kind {
                return f32::lerp(from, to, t);
            }
        }
        tree.get(id).map_or(1.0, |l| l.properties().opacity)
    }

    /// Gets the current transform (interpolated if animating).
    fn current_transform(&self, id: LayerId, tree: &LayerTree, now: Instant) -> Transform2D {
        if let Some(transition) = self.transitions.get(&(id, AnimatableProperty::Transform)) {
            let t = transition.easing.apply(transition.progress(now));
            if let TransitionKind::Transform { from, to } = transition.kind {
                return Transform2D::lerp(from, to, t);
            }
        }
        tree.get(id)
            .map_or_else(Transform2D::identity, |l| l.properties().transform)
    }

    /// Gets the current bounds (interpolated if animating).
    fn current_bounds(&self, id: LayerId, tree: &LayerTree, now: Instant) -> Rect {
        if let Some(transition) = self.transitions.get(&(id, AnimatableProperty::Bounds)) {
            let t = transition.easing.apply(transition.progress(now));
            if let TransitionKind::Bounds { from, to } = transition.kind {
                return Rect::lerp(from, to, t);
            }
        }
        tree.get(id)
            .map_or_else(Rect::default, |l| l.properties().bounds)
    }
}

impl Default for LayerAnimator {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for LayerAnimator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayerAnimator")
            .field("active", &self.transitions.len())
            .field("queued", &self.queue.len())
            .field("preemption", &self.preemption)
            .finish_non_exhaustive()
    }
}
