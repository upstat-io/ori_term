//! Selection, search, and command-zone selection methods for [`Pane`].
//!
//! Extracted from `mod.rs` to keep file sizes under the 500-line limit.

use oriterm_core::{
    SearchState, Selection, SelectionMode, SelectionPoint, Side, StableRowIndex, Term,
};

use crate::mux_event::MuxEventProxy;

use super::Pane;

impl Pane {
    // -- Selection --

    /// Active text selection, if any.
    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    /// Replace the active selection.
    pub fn set_selection(&mut self, selection: Selection) {
        self.selection = Some(selection);
    }

    /// Clear the active selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Update the endpoint of an active selection during drag.
    pub fn update_selection_end(&mut self, end: SelectionPoint) {
        if let Some(sel) = &mut self.selection {
            sel.end = end;
        }
    }

    /// Check whether terminal output has invalidated the selection.
    pub fn check_selection_invalidation(&mut self) {
        if self.selection.is_none() {
            let mut term = self.terminal.lock();
            if term.is_selection_dirty() {
                term.clear_selection_dirty();
            }
            return;
        }
        let mut term = self.terminal.lock();
        if term.is_selection_dirty() {
            term.clear_selection_dirty();
            drop(term);
            self.selection = None;
        }
    }

    // -- Search --

    /// Active search state, if any.
    pub fn search(&self) -> Option<&SearchState> {
        self.search.as_ref()
    }

    /// Mutable access to the active search state.
    pub fn search_mut(&mut self) -> Option<&mut SearchState> {
        self.search.as_mut()
    }

    /// Activate search.
    pub fn open_search(&mut self) {
        if self.search.is_none() {
            self.search = Some(SearchState::new());
        }
    }

    /// Close search.
    pub fn close_search(&mut self) {
        self.search = None;
    }

    /// Whether search is currently active.
    pub fn is_search_active(&self) -> bool {
        self.search.is_some()
    }

    // -- Command zone selection --

    /// Build a selection for the nearest command output zone (non-mutating).
    ///
    /// Returns the selection without storing it on the pane. Used by
    /// `MuxBackend::select_command_output` to return a selection to the caller.
    pub fn command_output_selection(&self) -> Option<Selection> {
        self.build_zone_selection(Term::command_output_range)
    }

    /// Build a selection for the nearest command input zone (non-mutating).
    ///
    /// Returns the selection without storing it on the pane. Used by
    /// `MuxBackend::select_command_input` to return a selection to the caller.
    pub fn command_input_selection(&self) -> Option<Selection> {
        self.build_zone_selection(Term::command_input_range)
    }

    /// Build a line selection from a range-finding function on the terminal.
    fn build_zone_selection(
        &self,
        range_fn: impl FnOnce(&Term<MuxEventProxy>, usize) -> Option<(usize, usize)>,
    ) -> Option<Selection> {
        let term = self.terminal.lock();
        let grid = term.grid();
        let sb_len = grid.scrollback().len();
        let viewport_center = sb_len.saturating_sub(grid.display_offset()) + grid.lines() / 2;
        let (start_row, end_row) = range_fn(&term, viewport_center)?;
        let start_stable = StableRowIndex::from_absolute(grid, start_row);
        let end_stable = StableRowIndex::from_absolute(grid, end_row);
        let anchor = SelectionPoint {
            row: start_stable,
            col: 0,
            side: Side::Left,
        };
        let pivot = SelectionPoint {
            row: end_stable,
            col: usize::MAX,
            side: Side::Right,
        };
        Some(Selection {
            mode: SelectionMode::Line,
            anchor,
            pivot,
            end: anchor,
        })
    }
}
