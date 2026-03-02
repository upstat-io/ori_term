//! Search bar overlay and keyboard dispatch.
//!
//! Manages the search lifecycle: open, close, query input, match
//! navigation, and viewport scrolling to the focused match.

use winit::event::ElementState;
use winit::keyboard::{Key, NamedKey};

use super::App;
use oriterm_mux::pane::Pane;

impl App {
    /// Open the search bar for the active pane.
    pub(super) fn open_search(&mut self) {
        if let Some(pane) = self.active_pane_mut() {
            pane.open_search();
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Close the search bar and clear search state.
    pub(super) fn close_search(&mut self) {
        if let Some(pane) = self.active_pane_mut() {
            pane.close_search();
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Whether search mode is active.
    pub(super) fn is_search_active(&self) -> bool {
        self.active_pane().is_some_and(Pane::is_search_active)
    }

    /// Dispatch a key event while search is active.
    ///
    /// Returns `true` if the event was consumed by search.
    pub(super) fn handle_search_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return true; // Consume releases to prevent leaking to PTY.
        }

        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.close_search();
            }
            Key::Named(NamedKey::Enter) => {
                let shift = self.modifiers.shift_key();
                if let Some(pane) = self.active_pane_mut() {
                    if let Some(search) = pane.search_mut() {
                        if shift {
                            search.prev_match();
                        } else {
                            search.next_match();
                        }
                    }
                }
                self.scroll_to_search_match();
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            Key::Named(NamedKey::Backspace) => {
                if let Some(pane) = self.active_pane_mut() {
                    let grid_ref = pane.terminal().clone();
                    if let Some(search) = pane.search_mut() {
                        let mut q = search.query().to_string();
                        q.pop();
                        let term = grid_ref.lock();
                        search.set_query(q, term.grid());
                    }
                }
                self.scroll_to_search_match();
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            Key::Character(c) => {
                if let Some(pane) = self.active_pane_mut() {
                    let grid_ref = pane.terminal().clone();
                    if let Some(search) = pane.search_mut() {
                        let mut q = search.query().to_string();
                        q.push_str(c);
                        let term = grid_ref.lock();
                        search.set_query(q, term.grid());
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
    fn scroll_to_search_match(&self) {
        let Some(pane) = self.active_pane() else {
            return;
        };
        let Some(search) = pane.search() else { return };
        let Some(focused) = search.focused_match() else {
            return;
        };

        let stable_row = focused.start_row;
        let term = pane.terminal().lock();
        let grid = term.grid();

        let Some(abs_row) = stable_row.to_absolute(grid) else {
            return;
        };

        let sb_len = grid.scrollback().len();
        let lines = grid.lines();
        let current_offset = grid.display_offset();

        // Viewport shows rows from (sb_len - offset) to (sb_len - offset + lines - 1).
        let view_start = sb_len.saturating_sub(current_offset);
        let view_end = view_start + lines;

        if abs_row >= view_start && abs_row < view_end {
            return; // Already visible.
        }

        // Center the match in the viewport.
        let target_start = abs_row.saturating_sub(lines / 2);
        let new_offset = sb_len.saturating_sub(target_start);
        let clamped = new_offset.min(sb_len);
        drop(term);

        // Scroll to the computed offset.
        let delta = clamped as isize - current_offset as isize;
        if delta != 0 {
            pane.scroll_display(delta);
        }
    }
}
