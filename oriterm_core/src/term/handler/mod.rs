//! VTE handler implementation for `Term<T>`.
//!
//! Implements `vte::ansi::Handler` to process escape sequences, control
//! characters, and printable input. Each method delegates to the
//! appropriate grid/cursor/mode operation.

use vte::ansi::{CharsetIndex, Handler};

use crate::event::{Event, EventListener};

use super::Term;

impl<T: EventListener> Handler for Term<T> {
    /// Print a character to the terminal.
    ///
    /// Translates through the active charset, then writes via `grid.put_char`.
    #[inline]
    fn input(&mut self, c: char) {
        let c = self.charset.translate(c);
        self.grid_mut().put_char(c);
    }

    /// Move cursor left by one column, clearing the wrap-pending state.
    fn backspace(&mut self) {
        self.grid_mut().backspace();
    }

    /// Advance cursor to the next tab stop (or end of line).
    fn put_tab(&mut self, count: u16) {
        let grid = self.grid_mut();
        for _ in 0..count {
            grid.tab();
        }
    }

    /// Move cursor down one line, scrolling if at the bottom of the scroll
    /// region.
    #[inline]
    fn linefeed(&mut self) {
        self.grid_mut().linefeed();
    }

    /// Move cursor to column 0.
    #[inline]
    fn carriage_return(&mut self) {
        self.grid_mut().carriage_return();
    }

    /// Ring the bell — send `Event::Bell` to the listener.
    #[inline]
    fn bell(&mut self) {
        self.event_listener.send_event(Event::Bell);
    }

    /// SUB: treated as a space character per ECMA-48.
    fn substitute(&mut self) {
        self.input(' ');
    }

    /// Switch the active charset slot (SO → G1, SI → G0).
    #[inline]
    fn set_active_charset(&mut self, index: CharsetIndex) {
        self.charset.set_active(index);
    }
}

#[cfg(test)]
mod tests;
