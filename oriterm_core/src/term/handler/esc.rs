//! ESC sequence handler implementations.
//!
//! Handles RIS (full reset). Methods are called by the
//! `vte::ansi::Handler` trait impl on `Term<T>`.

use log::debug;

use crate::event::{Event, EventListener};

use super::super::{CharsetState, PromptState, Term, TermMode};

impl<T: EventListener> Term<T> {
    /// RIS (ESC c): full terminal reset.
    ///
    /// Resets both grids, mode flags, charset, palette, title, cursor shape,
    /// and keyboard mode stacks to initial state.
    pub(super) fn esc_reset_state(&mut self) {
        debug!("RIS: full terminal reset");

        self.selection_dirty = true;

        // Clear alt-screen flag without swapping — both grids are reset
        // immediately after, so cursor save/restore and dirty marking from
        // swap_alt() would be wasted work.
        self.mode.remove(TermMode::ALT_SCREEN);

        self.grid_mut().reset();
        self.alt_grid.reset();
        self.mode = TermMode::default();
        self.charset = CharsetState::default();
        self.palette = crate::color::Palette::for_theme(self.theme);
        self.cursor_shape = crate::grid::CursorShape::default();
        self.title.clear();
        self.icon_name.clear();
        self.title_stack.clear();
        self.cwd = None;
        self.keyboard_mode_stack.clear();
        self.inactive_keyboard_mode_stack.clear();

        // Shell integration state.
        self.prompt_state = PromptState::None;
        self.prompt_mark_pending = false;
        self.prompt_markers.clear();
        self.command_start_mark_pending = false;
        self.output_start_mark_pending = false;
        self.pending_notifications.clear();
        self.command_start = None;
        self.last_command_duration = None;
        self.has_explicit_title = false;
        self.title_dirty = true;
        self.saved_private_modes.clear();

        self.event_listener.send_event(Event::ResetTitle);
    }
}
