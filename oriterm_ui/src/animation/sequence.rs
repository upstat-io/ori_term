//! Sequential animation chains for compositor layers.
//!
//! An [`AnimationSequence`] chains [`AnimationStep`]s end-to-end: animate
//! a property, pause, fire a callback, then animate again. Use case:
//! toast notification — slide in → hold → slide out → remove layer.

use std::time::{Duration, Instant};

use crate::geometry::LayerId;

use super::group::AnimationGroup;

/// A single step in an animation sequence.
pub enum AnimationStep {
    /// Run an animation group (parallel property animations).
    Animate(AnimationGroup),
    /// Pause for a fixed duration before the next step.
    Delay(Duration),
    /// Fire a callback, then immediately proceed to the next step.
    Callback(Option<Box<dyn FnOnce(LayerId)>>),
}

/// Tracks which step is active and when it started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceState {
    /// Not yet started.
    Pending,
    /// Running step at the given index.
    Running(usize),
    /// All steps finished.
    Finished,
}

/// Chains animation steps end-to-end on a single layer.
///
/// Call [`tick`](Self::tick) each frame. When a step finishes, the
/// sequence automatically advances to the next step, firing callbacks
/// and starting animations as needed.
pub struct AnimationSequence {
    /// Layer this sequence operates on.
    layer_id: LayerId,
    /// Ordered list of steps.
    steps: Vec<AnimationStep>,
    /// Current execution state.
    state: SequenceState,
    /// When the current step started.
    step_start: Option<Instant>,
}

impl AnimationSequence {
    /// Creates a new sequence for the given layer.
    pub fn new(layer_id: LayerId, steps: Vec<AnimationStep>) -> Self {
        Self {
            layer_id,
            steps,
            state: SequenceState::Pending,
            step_start: None,
        }
    }

    /// Returns the layer this sequence operates on.
    pub fn layer_id(&self) -> LayerId {
        self.layer_id
    }

    /// Returns the current execution state.
    pub fn state(&self) -> SequenceState {
        self.state
    }

    /// Returns `true` if the sequence has finished all steps.
    pub fn is_finished(&self) -> bool {
        self.state == SequenceState::Finished
    }

    /// Returns the current step, if running.
    pub fn current_step(&self) -> Option<&AnimationStep> {
        match self.state {
            SequenceState::Running(idx) => self.steps.get(idx),
            _ => None,
        }
    }

    /// Returns the default duration for the current step.
    ///
    /// For `Animate` steps, returns the group's default duration.
    /// For `Delay` steps, returns the delay duration.
    /// For `Callback` steps, returns zero (instant).
    pub fn current_step_duration(&self) -> Option<Duration> {
        self.current_step().map(|step| match step {
            AnimationStep::Animate(group) => group.duration,
            AnimationStep::Delay(d) => *d,
            AnimationStep::Callback(_) => Duration::ZERO,
        })
    }

    /// Starts the sequence at the first step.
    ///
    /// Returns the animation group to apply if the first step is `Animate`.
    pub fn start(&mut self, now: Instant) -> Option<&AnimationGroup> {
        if self.steps.is_empty() {
            self.state = SequenceState::Finished;
            return None;
        }
        self.state = SequenceState::Running(0);
        self.step_start = Some(now);
        self.fire_callbacks_and_get_animate(0)
    }

    /// Advances the sequence, returning the next animation group if a
    /// new `Animate` step just started.
    ///
    /// Call this each frame after ticking the animator. Pass `true` for
    /// `step_finished` when the current step's animations have completed
    /// (or the delay has elapsed).
    pub fn advance(&mut self, now: Instant, step_finished: bool) -> Option<&AnimationGroup> {
        let SequenceState::Running(idx) = self.state else {
            return None;
        };

        if !step_finished {
            // Check if this is a Delay step that has elapsed.
            if let Some(AnimationStep::Delay(d)) = self.steps.get(idx) {
                if let Some(start) = self.step_start {
                    if now.saturating_duration_since(start) >= *d {
                        return self.advance_to(idx + 1, now);
                    }
                }
            }
            return None;
        }

        self.advance_to(idx + 1, now)
    }

    // Private helpers

    /// Moves to the given step index, firing callbacks along the way.
    fn advance_to(&mut self, next_idx: usize, now: Instant) -> Option<&AnimationGroup> {
        if next_idx >= self.steps.len() {
            self.state = SequenceState::Finished;
            self.step_start = None;
            return None;
        }
        self.state = SequenceState::Running(next_idx);
        self.step_start = Some(now);
        self.fire_callbacks_and_get_animate(next_idx)
    }

    /// Starting at `idx`, fires any Callback steps and returns the first
    /// Animate group found (or `None` if only Callbacks/end remain).
    fn fire_callbacks_and_get_animate(&mut self, mut idx: usize) -> Option<&AnimationGroup> {
        while idx < self.steps.len() {
            match &self.steps[idx] {
                AnimationStep::Callback(_) => {
                    // Take the callback out to fire it.
                    let step = std::mem::replace(
                        &mut self.steps[idx],
                        AnimationStep::Delay(Duration::ZERO),
                    );
                    if let AnimationStep::Callback(Some(cb)) = step {
                        cb(self.layer_id);
                    }
                    idx += 1;
                    self.state = SequenceState::Running(idx);
                }
                AnimationStep::Animate(_) => {
                    self.state = SequenceState::Running(idx);
                    // Return a reference to the group.
                    if let AnimationStep::Animate(group) = &self.steps[idx] {
                        return Some(group);
                    }
                    unreachable!();
                }
                AnimationStep::Delay(_) => {
                    self.state = SequenceState::Running(idx);
                    return None;
                }
            }
        }
        self.state = SequenceState::Finished;
        self.step_start = None;
        None
    }
}

impl std::fmt::Debug for AnimationSequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnimationSequence")
            .field("layer_id", &self.layer_id)
            .field("steps", &self.steps.len())
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for AnimationStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Animate(group) => f
                .debug_tuple("Animate")
                .field(&group.animations.len())
                .finish(),
            Self::Delay(d) => f.debug_tuple("Delay").field(d).finish(),
            Self::Callback(_) => f.debug_tuple("Callback").finish(),
        }
    }
}
