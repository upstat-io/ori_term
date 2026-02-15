//! ESC sequence handler implementations.
//!
//! Handles RIS (full reset). Methods are called by the
//! `vte::ansi::Handler` trait impl on `Term<T>`.

use log::debug;

use crate::event::{Event, EventListener};

use super::super::{CharsetState, Term, TermMode};

impl<T: EventListener> Term<T> {
    /// RIS (ESC c): full terminal reset.
    ///
    /// Resets both grids, mode flags, charset, palette, title, cursor shape,
    /// and keyboard mode stacks to initial state.
    pub(super) fn esc_reset_state(&mut self) {
        debug!("RIS: full terminal reset");

        // If in alt screen, swap back to primary first so the grid references
        // are correct after reset.
        if self.mode.contains(TermMode::ALT_SCREEN) {
            self.swap_alt();
        }

        self.grid_mut().reset();
        self.alt_grid.reset();
        self.mode = TermMode::default();
        self.charset = CharsetState::default();
        self.palette = crate::color::Palette::default();
        self.cursor_shape = crate::grid::CursorShape::default();
        self.title.clear();
        self.title_stack.clear();
        self.cwd = None;
        self.keyboard_mode_stack.clear();
        self.inactive_keyboard_mode_stack.clear();

        self.event_listener.send_event(Event::ResetTitle);
    }
}
