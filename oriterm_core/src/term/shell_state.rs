//! Shell integration state accessors and navigation.
//!
//! Extracted from `term/mod.rs` to keep the main file under the 500-line
//! limit. These methods manage prompt state (OSC 133), CWD (OSC 7),
//! title resolution, notifications, and prompt-based navigation.

use super::{Notification, PromptMarker, PromptState, Term, cwd_short_path};
use crate::event::EventListener;

impl<T: EventListener> Term<T> {
    // -- Prompt state --

    /// Current shell integration prompt state (OSC 133).
    pub fn prompt_state(&self) -> PromptState {
        self.prompt_state
    }

    /// Set the prompt state (for raw interceptor).
    pub fn set_prompt_state(&mut self, state: PromptState) {
        self.prompt_state = state;
    }

    /// Whether OSC 133;A was received and the prompt row hasn't been marked yet.
    pub fn prompt_mark_pending(&self) -> bool {
        self.prompt_mark_pending
    }

    /// Set/clear the prompt-mark-pending flag.
    pub fn set_prompt_mark_pending(&mut self, pending: bool) {
        self.prompt_mark_pending = pending;
    }

    /// Record the current cursor row as a prompt line (OSC 133;A).
    ///
    /// Called after both VTE parsers finish processing a chunk, when
    /// `prompt_mark_pending` is `true`. Uses the cursor row from the
    /// high-level processor (which is at the correct position).
    pub fn mark_prompt_row(&mut self) {
        if !self.prompt_mark_pending {
            return;
        }
        self.prompt_mark_pending = false;
        let abs_row = self.grid.scrollback().len() + self.grid.cursor().line();
        // Avoid duplicate entries (e.g. shell redrawing prompt on resize).
        if self
            .prompt_markers
            .last()
            .is_some_and(|m| m.prompt == abs_row)
        {
            return;
        }
        self.prompt_markers.push(PromptMarker {
            prompt: abs_row,
            command: None,
            output: None,
        });
    }

    /// Record the current cursor row as a command start (OSC 133;B).
    ///
    /// Fills `command_start` on the most recent prompt marker.
    pub fn mark_command_start_row(&mut self) {
        if !self.command_start_mark_pending {
            return;
        }
        self.command_start_mark_pending = false;
        let abs_row = self.grid.scrollback().len() + self.grid.cursor().line();
        if let Some(marker) = self.prompt_markers.last_mut() {
            marker.command = Some(abs_row);
        }
    }

    /// Record the current cursor row as output start (OSC 133;C).
    ///
    /// Fills `output` on the most recent prompt marker.
    pub fn mark_output_start_row(&mut self) {
        if !self.output_start_mark_pending {
            return;
        }
        self.output_start_mark_pending = false;
        let abs_row = self.grid.scrollback().len() + self.grid.cursor().line();
        if let Some(marker) = self.prompt_markers.last_mut() {
            marker.output = Some(abs_row);
        }
    }

    /// Whether OSC 133;B was received and hasn't been marked yet.
    pub fn command_start_mark_pending(&self) -> bool {
        self.command_start_mark_pending
    }

    /// Set/clear the command-start-mark-pending flag.
    pub fn set_command_start_mark_pending(&mut self, pending: bool) {
        self.command_start_mark_pending = pending;
    }

    /// Whether OSC 133;C was received and hasn't been marked yet.
    pub fn output_start_mark_pending(&self) -> bool {
        self.output_start_mark_pending
    }

    /// Set/clear the output-start-mark-pending flag.
    pub fn set_output_start_mark_pending(&mut self, pending: bool) {
        self.output_start_mark_pending = pending;
    }

    /// All prompt lifecycle markers.
    pub fn prompt_markers(&self) -> &[PromptMarker] {
        &self.prompt_markers
    }

    /// Prune prompt markers evicted from scrollback.
    ///
    /// When scrollback lines are evicted (the buffer is full and new lines
    /// push old ones out), markers with `prompt_start` below the eviction
    /// threshold are removed. Remaining row indices are shifted down.
    pub fn prune_prompt_markers(&mut self, evicted: usize) {
        if evicted == 0 {
            return;
        }
        self.prompt_markers.retain_mut(|marker| {
            if marker.prompt < evicted {
                false
            } else {
                marker.prompt -= evicted;
                if let Some(ref mut cr) = marker.command {
                    *cr = cr.saturating_sub(evicted);
                }
                if let Some(ref mut or) = marker.output {
                    *or = or.saturating_sub(evicted);
                }
                true
            }
        });
    }

    /// Find the output range for the prompt nearest to `near_row`.
    ///
    /// Returns `(output_start_row, end_row)` where `end_row` is one before
    /// the next prompt's `prompt_start`, or the current cursor row if this
    /// is the last marker.
    pub fn command_output_range(&self, near_row: usize) -> Option<(usize, usize)> {
        let (idx, marker) = self.find_nearest_marker(near_row)?;
        let output_row = marker.output?;
        let end = if idx + 1 < self.prompt_markers.len() {
            self.prompt_markers[idx + 1].prompt.saturating_sub(1)
        } else {
            // Last marker: end at the current cursor row.
            self.grid.scrollback().len() + self.grid.cursor().line()
        };
        if end < output_row {
            return None;
        }
        Some((output_row, end))
    }

    /// Find the command input range for the prompt nearest to `near_row`.
    ///
    /// Returns `(command_start_row, end_row)` where `end_row` is one before
    /// `output_start`, or one before the next prompt if no output marker.
    pub fn command_input_range(&self, near_row: usize) -> Option<(usize, usize)> {
        let (idx, marker) = self.find_nearest_marker(near_row)?;
        let cmd_row = marker.command?;
        let end = if let Some(or) = marker.output {
            or.saturating_sub(1)
        } else if idx + 1 < self.prompt_markers.len() {
            self.prompt_markers[idx + 1].prompt.saturating_sub(1)
        } else {
            self.grid.scrollback().len() + self.grid.cursor().line()
        };
        if end < cmd_row {
            return None;
        }
        Some((cmd_row, end))
    }

    // -- Command timing --

    /// Record command execution start (when OSC 133;C is received).
    pub fn set_command_start(&mut self, start: std::time::Instant) {
        self.command_start = Some(start);
    }

    /// Compute and store command duration (when OSC 133;D is received).
    ///
    /// Returns the duration if a matching start time existed.
    pub fn finish_command(&mut self) -> Option<std::time::Duration> {
        let start = self.command_start.take()?;
        let duration = start.elapsed();
        self.last_command_duration = Some(duration);
        Some(duration)
    }

    /// Duration of the last completed command.
    pub fn last_command_duration(&self) -> Option<std::time::Duration> {
        self.last_command_duration
    }

    // -- Notifications --

    /// Drain pending desktop notifications (OSC 9/99/777).
    pub fn drain_notifications(&mut self) -> Vec<Notification> {
        std::mem::take(&mut self.pending_notifications)
    }

    /// Push a notification from the raw interceptor.
    pub fn push_notification(&mut self, notification: Notification) {
        self.pending_notifications.push(notification);
    }

    // -- Title state --

    /// Whether the current title was explicitly set via OSC 0/2.
    pub fn has_explicit_title(&self) -> bool {
        self.has_explicit_title
    }

    /// Set the explicit title flag.
    pub fn set_has_explicit_title(&mut self, explicit: bool) {
        self.has_explicit_title = explicit;
    }

    /// Whether the title needs refreshing (CWD or explicit title changed).
    pub fn is_title_dirty(&self) -> bool {
        self.title_dirty
    }

    /// Clear the title dirty flag after the UI has refreshed.
    pub fn clear_title_dirty(&mut self) {
        self.title_dirty = false;
    }

    /// Mark the title as needing a refresh.
    pub fn mark_title_dirty(&mut self) {
        self.title_dirty = true;
    }

    /// Set the current working directory (for raw interceptor).
    pub fn set_cwd(&mut self, cwd: Option<String>) {
        self.cwd = cwd;
    }

    /// Resolved display title with 3-source priority:
    /// 1. Explicit title from OSC 0/2.
    /// 2. Last component of CWD path.
    /// 3. Fallback to raw title (may be empty).
    pub fn effective_title(&self) -> &str {
        if self.has_explicit_title {
            return &self.title;
        }
        if let Some(ref cwd) = self.cwd {
            return cwd_short_path(cwd);
        }
        &self.title
    }

    // -- Prompt navigation --

    /// Scroll to the nearest prompt row above the current viewport position.
    ///
    /// Returns `true` if the viewport was scrolled, `false` if there are no
    /// prompts above (no-op).
    pub fn scroll_to_previous_prompt(&mut self) -> bool {
        if self.prompt_markers.is_empty() {
            return false;
        }
        // Current viewport top in absolute row coordinates.
        let sb_len = self.grid.scrollback().len();
        let viewport_top = sb_len.saturating_sub(self.grid.display_offset());
        // Find the last prompt row strictly above viewport top.
        let target = self
            .prompt_markers
            .iter()
            .rev()
            .find(|m| m.prompt < viewport_top);
        if let Some(marker) = target {
            let row = marker.prompt;
            self.scroll_to_absolute_row(row);
            true
        } else {
            false
        }
    }

    /// Scroll to the nearest prompt row below the current viewport position.
    ///
    /// Returns `true` if the viewport was scrolled, `false` if there are no
    /// prompts below (no-op).
    pub fn scroll_to_next_prompt(&mut self) -> bool {
        if self.prompt_markers.is_empty() {
            return false;
        }
        let sb_len = self.grid.scrollback().len();
        // Current viewport bottom in absolute row coordinates.
        let viewport_bottom = sb_len.saturating_sub(self.grid.display_offset()) + self.grid.lines();
        let target = self
            .prompt_markers
            .iter()
            .find(|m| m.prompt >= viewport_bottom);
        if let Some(marker) = target {
            let row = marker.prompt;
            self.scroll_to_absolute_row(row);
            true
        } else {
            false
        }
    }

    /// Scroll the viewport to center the given absolute row.
    fn scroll_to_absolute_row(&mut self, abs_row: usize) {
        let sb_len = self.grid.scrollback().len();
        let half = self.grid.lines() / 2;
        // Compute display_offset that places abs_row near the center.
        // viewport_top = sb_len - display_offset
        // We want: viewport_top = abs_row - half (so abs_row is centered)
        let viewport_top = abs_row.saturating_sub(half);
        let offset = sb_len.saturating_sub(viewport_top);
        let clamped = offset.min(sb_len);
        if clamped != self.grid.display_offset() {
            // Use isize delta to go through scroll_display for dirty marking.
            let delta = clamped as isize - self.grid.display_offset() as isize;
            self.grid.scroll_display(delta);
        }
    }

    /// Find the prompt marker whose zone contains `near_row`.
    ///
    /// Returns the index and a reference to the last marker whose
    /// `prompt_start <= near_row`.
    fn find_nearest_marker(&self, near_row: usize) -> Option<(usize, &PromptMarker)> {
        self.prompt_markers
            .iter()
            .enumerate()
            .rev()
            .find(|(_, m)| m.prompt <= near_row)
    }
}
