//! Cursor blink state machine.
//!
//! Tracks the on/off phase of a blinking cursor at a configurable interval.
//! The application owns this state — the terminal only declares whether
//! blinking is enabled (`TermMode::CURSOR_BLINKING`); the actual
//! visibility toggle is driven here.

use std::time::{Duration, Instant};

/// Default xterm cursor blink interval (530ms on, 530ms off).
#[cfg(test)]
const DEFAULT_BLINK_INTERVAL: Duration = Duration::from_millis(530);

/// Cursor blink state.
///
/// Toggles between visible and hidden every [`interval`](Self::interval).
/// Reset on keypress to keep the cursor visible while the user types.
pub(crate) struct CursorBlink {
    /// Whether the cursor is in the "visible" phase.
    visible: bool,
    /// When the current phase started.
    phase_start: Instant,
    /// Duration of each blink phase (on/off).
    interval: Duration,
}

impl CursorBlink {
    /// Create a new blink state with the given interval, starting visible.
    pub(crate) fn new(interval: Duration) -> Self {
        Self {
            visible: true,
            phase_start: Instant::now(),
            interval,
        }
    }

    /// Whether the cursor is currently in the visible phase.
    pub(crate) fn is_visible(&self) -> bool {
        self.visible
    }

    /// Update the blink interval (e.g. on config reload).
    pub(crate) fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    /// Reset blink to the visible phase.
    ///
    /// Called on keypress so the cursor stays visible while the user types.
    pub(crate) fn reset(&mut self) {
        self.visible = true;
        self.phase_start = Instant::now();
    }

    /// Check elapsed time and toggle phase if the interval has passed.
    ///
    /// Returns `true` if visibility changed (caller should mark dirty).
    pub(crate) fn update(&mut self) -> bool {
        if self.phase_start.elapsed() >= self.interval {
            self.visible = !self.visible;
            self.phase_start = Instant::now();
            true
        } else {
            false
        }
    }

    /// The instant at which the next phase toggle should occur.
    ///
    /// Used with `ControlFlow::WaitUntil` to schedule the event loop
    /// wakeup without busy-waiting.
    pub(crate) fn next_toggle(&self) -> Instant {
        self.phase_start + self.interval
    }
}

#[cfg(test)]
mod tests;
