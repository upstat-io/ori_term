//! Periodic performance statistics logging.
//!
//! Counts renders, mux wakeups, and cursor-move events per interval, then
//! logs a summary line. Helps diagnose contention, rendering bottlenecks,
//! and unnecessary wakeups without runtime overhead beyond an atomic
//! increment per event.

use std::time::{Duration, Instant};

/// Interval between performance log lines.
const LOG_INTERVAL: Duration = Duration::from_secs(5);

/// Per-interval performance counters.
pub(super) struct PerfStats {
    /// Start of the current measurement window.
    window_start: Instant,
    /// Number of `handle_redraw` calls this window.
    renders: u32,
    /// Number of `MuxWakeup` / `pump_mux_events` calls this window.
    wakeups: u32,
    /// Number of `CursorMoved` events this window.
    cursor_moves: u32,
    /// Number of `about_to_wait` calls this window.
    ticks: u32,
}

impl PerfStats {
    pub(super) fn new() -> Self {
        Self {
            window_start: Instant::now(),
            renders: 0,
            wakeups: 0,
            cursor_moves: 0,
            ticks: 0,
        }
    }

    /// Record a render frame.
    pub(super) fn record_render(&mut self) {
        self.renders += 1;
    }

    /// Record a mux wakeup (PTY reader thread notification).
    pub(super) fn record_wakeup(&mut self) {
        self.wakeups += 1;
    }

    /// Record a cursor-move event.
    pub(super) fn record_cursor_move(&mut self) {
        self.cursor_moves += 1;
    }

    /// Record an `about_to_wait` tick.
    pub(super) fn record_tick(&mut self) {
        self.ticks += 1;
    }

    /// Flush counters and log if the interval has elapsed.
    ///
    /// Returns `true` if a log line was emitted.
    pub(super) fn maybe_log(&mut self) -> bool {
        let elapsed = self.window_start.elapsed();
        if elapsed < LOG_INTERVAL {
            return false;
        }

        let secs = elapsed.as_secs_f64();
        log::debug!(
            "perf: {:.0} renders/s, {:.0} wakeups/s, {:.0} cursor/s, {:.0} ticks/s",
            f64::from(self.renders) / secs,
            f64::from(self.wakeups) / secs,
            f64::from(self.cursor_moves) / secs,
            f64::from(self.ticks) / secs,
        );

        self.renders = 0;
        self.wakeups = 0;
        self.cursor_moves = 0;
        self.ticks = 0;
        self.window_start = Instant::now();
        true
    }
}
