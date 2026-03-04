//! Search bar overlay and keyboard dispatch.
//!
//! Manages the search lifecycle: open, close, query input, match
//! navigation, and viewport scrolling to the focused match.

use winit::event::ElementState;
use winit::keyboard::{Key, NamedKey};

use super::App;

impl App {
    /// Open the search bar for the active pane.
    pub(super) fn open_search(&mut self) {
        if let Some(pane_id) = self.active_pane_id() {
            if let Some(mux) = self.mux.as_mut() {
                mux.open_search(pane_id);
            }
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Close the search bar and clear search state.
    pub(super) fn close_search(&mut self) {
        if let Some(pane_id) = self.active_pane_id() {
            if let Some(mux) = self.mux.as_mut() {
                mux.close_search(pane_id);
            }
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Whether search mode is active.
    pub(super) fn is_search_active(&self) -> bool {
        let Some(pane_id) = self.active_pane_id() else {
            return false;
        };
        self.mux
            .as_ref()
            .is_some_and(|m| m.is_search_active(pane_id))
    }

    /// Dispatch a key event while search is active.
    ///
    /// Returns `true` if the event was consumed by search.
    pub(super) fn handle_search_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return true; // Consume releases to prevent leaking to PTY.
        }

        let Some(pane_id) = self.active_pane_id() else {
            return true;
        };

        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.close_search();
            }
            Key::Named(NamedKey::Enter) => {
                let shift = self.modifiers.shift_key();
                if let Some(mux) = self.mux.as_mut() {
                    if shift {
                        mux.search_prev_match(pane_id);
                    } else {
                        mux.search_next_match(pane_id);
                    }
                }
                self.scroll_to_search_match();
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            Key::Named(NamedKey::Backspace) => {
                // Read current query from the cached snapshot.
                let query = self
                    .mux
                    .as_ref()
                    .and_then(|m| m.pane_snapshot(pane_id))
                    .map(|s| {
                        let mut q = s.search_query.clone();
                        q.pop();
                        q
                    });
                if let Some(q) = query {
                    if let Some(mux) = self.mux.as_mut() {
                        mux.search_set_query(pane_id, q);
                    }
                }
                self.scroll_to_search_match();
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            Key::Character(c) => {
                // Read current query from the cached snapshot.
                let query = self
                    .mux
                    .as_ref()
                    .and_then(|m| m.pane_snapshot(pane_id))
                    .map(|s| {
                        let mut q = s.search_query.clone();
                        q.push_str(c);
                        q
                    });
                if let Some(q) = query {
                    if let Some(mux) = self.mux.as_mut() {
                        mux.search_set_query(pane_id, q);
                    }
                }
                self.scroll_to_search_match();
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            _ => {}
        }

        true
    }

    /// Scroll the viewport to center the focused search match.
    fn scroll_to_search_match(&mut self) {
        let Some(pane_id) = self.active_pane_id() else {
            return;
        };

        // Read match data from the cached snapshot.
        let scroll_info = self
            .mux
            .as_ref()
            .and_then(|m| m.pane_snapshot(pane_id))
            .and_then(|snap| {
                let focused_idx = snap.search_focused? as usize;
                let focused = snap.search_matches.get(focused_idx)?;
                let stable_row = focused.start_row;

                let sb_len = snap.scrollback_len as usize;
                let display_offset = snap.display_offset as usize;
                let lines = snap.cells.len();

                // Compute absolute row from stable row index.
                // stable_row_base is the stable index of viewport row 0.
                // abs_row = stable_row - (stable_row_base - (sb_len - display_offset))
                // But simpler: the viewport starts at absolute row (sb_len - display_offset).
                // If stable_row < stable_row_base, it's above the viewport start.
                // If stable_row >= stable_row_base, offset from viewport start.
                let base = snap.stable_row_base;
                let abs_row = if stable_row >= base {
                    (sb_len - display_offset) + (stable_row - base) as usize
                } else {
                    // Row is in scrollback above the base.
                    (sb_len - display_offset).saturating_sub((base - stable_row) as usize)
                };

                let view_start = sb_len.saturating_sub(display_offset);
                let view_end = view_start + lines;

                if abs_row >= view_start && abs_row < view_end {
                    return None; // Already visible.
                }

                // Center the match in the viewport.
                let target_start = abs_row.saturating_sub(lines / 2);
                let new_offset = sb_len.saturating_sub(target_start);
                let clamped = new_offset.min(sb_len);
                let delta = clamped as isize - display_offset as isize;
                if delta != 0 {
                    Some((pane_id, delta))
                } else {
                    None
                }
            });

        if let Some((pid, delta)) = scroll_info {
            if let Some(mux) = self.mux.as_mut() {
                mux.scroll_display(pid, delta);
            }
        }
    }
}
