//! VTE handler implementation for `Term<T>`.
//!
//! Implements `vte::ansi::Handler` to process escape sequences, control
//! characters, and printable input. Each method delegates to the
//! appropriate grid/cursor/mode operation.

use std::cmp;

use log::debug;
use unicode_width::UnicodeWidthChar;
use vte::ansi::{
    Attr, CharsetIndex, ClearMode, Handler, Hyperlink as VteHyperlink, LineClearMode, Mode,
    NamedMode, PrivateMode, Rgb, TabulationClearMode,
};

use crate::event::{Event, EventListener};
use crate::grid::editing::EraseMode;
use crate::grid::navigation::TabClearMode;
use crate::index::Column;

use super::{Term, TermMode};

mod helpers;
mod modes;
mod osc;
mod sgr;

use helpers::{crate_version_number, mode_report_value, named_private_mode_flag,
    named_private_mode_number};

impl<T: EventListener> Handler for Term<T> {
    // --- Print + Execute (C0 controls) ---

    /// Print a character to the terminal.
    ///
    /// Translates through the active charset, then writes via `grid.put_char`.
    /// In INSERT mode, shifts existing content right before writing.
    #[inline]
    fn input(&mut self, c: char) {
        let c = self.charset.translate(c);
        if self.mode.contains(TermMode::INSERT) {
            let width = UnicodeWidthChar::width(c).unwrap_or(1);
            self.grid_mut().insert_blank(width);
        }
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
    /// region. In LNM mode, also performs a carriage return.
    #[inline]
    fn linefeed(&mut self) {
        let lnm = self.mode.contains(TermMode::LINE_FEED_NEW_LINE);
        let grid = self.grid_mut();
        if lnm {
            grid.next_line();
        } else {
            grid.linefeed();
        }
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

    // --- CSI cursor movement ---

    /// CUP / HVP: absolute cursor positioning.
    ///
    /// In ORIGIN mode, coordinates are relative to the scroll region.
    fn goto(&mut self, line: i32, col: usize) {
        let origin = self.mode.contains(TermMode::ORIGIN);
        let grid = self.grid_mut();
        let region = grid.scroll_region().clone();

        let (offset, max_line) = if origin {
            (region.start, region.end.saturating_sub(1))
        } else {
            (0, grid.lines().saturating_sub(1))
        };

        let line = cmp::max(0, line) as usize;
        let line = cmp::min(line + offset, max_line);
        let col = Column(col.min(grid.cols().saturating_sub(1)));
        grid.move_to(line, col);
    }

    /// VPA: set cursor line (ORIGIN mode aware).
    fn goto_line(&mut self, line: i32) {
        let col = self.grid().cursor().col().0;
        self.goto(line, col);
    }

    /// CHA: set cursor column.
    fn goto_col(&mut self, col: usize) {
        self.grid_mut().move_to_column(Column(col));
    }

    /// CUU: move cursor up.
    fn move_up(&mut self, count: usize) {
        self.grid_mut().move_up(count);
    }

    /// CUD: move cursor down.
    fn move_down(&mut self, count: usize) {
        self.grid_mut().move_down(count);
    }

    /// CUF: move cursor forward (right).
    fn move_forward(&mut self, col: usize) {
        self.grid_mut().move_forward(col);
    }

    /// CUB: move cursor backward (left).
    fn move_backward(&mut self, col: usize) {
        self.grid_mut().move_backward(col);
    }

    /// CNL: move cursor down `count` lines, then to column 0.
    fn move_down_and_cr(&mut self, count: usize) {
        let grid = self.grid_mut();
        grid.move_down(count);
        grid.carriage_return();
    }

    /// CPL: move cursor up `count` lines, then to column 0.
    fn move_up_and_cr(&mut self, count: usize) {
        let grid = self.grid_mut();
        grid.move_up(count);
        grid.carriage_return();
    }

    // --- CSI erase ---

    /// ED: erase in display.
    fn clear_screen(&mut self, mode: ClearMode) {
        let erase = match mode {
            ClearMode::Below => EraseMode::Below,
            ClearMode::Above => EraseMode::Above,
            ClearMode::All => EraseMode::All,
            ClearMode::Saved => EraseMode::Scrollback,
        };
        self.grid_mut().erase_display(erase);
    }

    /// EL: erase in line.
    fn clear_line(&mut self, mode: LineClearMode) {
        let erase = match mode {
            LineClearMode::Right => EraseMode::Below,
            LineClearMode::Left => EraseMode::Above,
            LineClearMode::All => EraseMode::All,
        };
        self.grid_mut().erase_line(erase);
    }

    /// ECH: erase characters (replace with blanks, no shift).
    fn erase_chars(&mut self, count: usize) {
        self.grid_mut().erase_chars(count);
    }

    // --- CSI insert / delete ---

    /// ICH: insert blank characters at cursor.
    fn insert_blank(&mut self, count: usize) {
        self.grid_mut().insert_blank(count);
    }

    /// DCH: delete characters at cursor.
    fn delete_chars(&mut self, count: usize) {
        self.grid_mut().delete_chars(count);
    }

    /// IL: insert blank lines at cursor.
    fn insert_blank_lines(&mut self, count: usize) {
        self.grid_mut().insert_lines(count);
    }

    /// DL: delete lines at cursor.
    fn delete_lines(&mut self, count: usize) {
        self.grid_mut().delete_lines(count);
    }

    // --- CSI scroll ---

    /// SU: scroll up (content moves up, blank lines at bottom).
    fn scroll_up(&mut self, count: usize) {
        self.grid_mut().scroll_up(count);
    }

    /// SD: scroll down (content moves down, blank lines at top).
    fn scroll_down(&mut self, count: usize) {
        self.grid_mut().scroll_down(count);
    }

    /// RI: reverse index (move cursor up, scroll down at top of region).
    fn reverse_index(&mut self) {
        self.grid_mut().reverse_index();
    }

    /// NEL: next line (carriage return + linefeed).
    fn newline(&mut self) {
        self.grid_mut().next_line();
    }

    // --- CSI tab ---

    /// CHT: cursor forward tabulation.
    fn move_forward_tabs(&mut self, count: u16) {
        let grid = self.grid_mut();
        for _ in 0..count {
            grid.tab();
        }
    }

    /// CBT: cursor backward tabulation.
    fn move_backward_tabs(&mut self, count: u16) {
        let grid = self.grid_mut();
        for _ in 0..count {
            grid.tab_backward();
        }
    }

    /// HTS: set horizontal tab stop at current column.
    fn set_horizontal_tabstop(&mut self) {
        self.grid_mut().set_tab_stop();
    }

    /// TBC: clear tab stops.
    fn clear_tabs(&mut self, mode: TabulationClearMode) {
        let clear = match mode {
            TabulationClearMode::Current => TabClearMode::Current,
            TabulationClearMode::All => TabClearMode::All,
        };
        self.grid_mut().clear_tab_stop(clear);
    }

    // --- CSI scroll region + cursor save/restore ---

    /// DECSTBM: set scroll region.
    fn set_scrolling_region(&mut self, top: usize, bottom: Option<usize>) {
        self.grid_mut().set_scroll_region(top, bottom);
        // Setting scroll region always moves cursor to origin.
        self.goto(0, 0);
    }

    /// DECSC / CSI s: save cursor position.
    fn save_cursor_position(&mut self) {
        self.grid_mut().save_cursor();
    }

    /// DECRC / CSI u: restore cursor position.
    fn restore_cursor_position(&mut self) {
        self.grid_mut().restore_cursor();
    }

    // --- CSI mode setting ---

    /// SM: set ANSI mode.
    fn set_mode(&mut self, mode: Mode) {
        match mode {
            Mode::Named(NamedMode::Insert) => self.mode.insert(TermMode::INSERT),
            Mode::Named(NamedMode::LineFeedNewLine) => {
                self.mode.insert(TermMode::LINE_FEED_NEW_LINE);
            }
            Mode::Unknown(n) => debug!("Ignoring unknown mode {n} in SM"),
        }
    }

    /// RM: reset ANSI mode.
    fn unset_mode(&mut self, mode: Mode) {
        match mode {
            Mode::Named(NamedMode::Insert) => self.mode.remove(TermMode::INSERT),
            Mode::Named(NamedMode::LineFeedNewLine) => {
                self.mode.remove(TermMode::LINE_FEED_NEW_LINE);
            }
            Mode::Unknown(n) => debug!("Ignoring unknown mode {n} in RM"),
        }
    }

    /// DECSET: set DEC private mode.
    fn set_private_mode(&mut self, mode: PrivateMode) {
        match mode {
            PrivateMode::Named(m) => self.apply_decset(m),
            PrivateMode::Unknown(n) => debug!("Ignoring unknown private mode {n} in DECSET"),
        }
    }

    /// DECRST: reset DEC private mode.
    fn unset_private_mode(&mut self, mode: PrivateMode) {
        match mode {
            PrivateMode::Named(m) => self.apply_decrst(m),
            PrivateMode::Unknown(n) => debug!("Ignoring unknown private mode {n} in DECRST"),
        }
    }

    /// DECRQM: report ANSI mode status.
    fn report_mode(&mut self, mode: Mode) {
        let (num, value) = match mode {
            Mode::Named(NamedMode::Insert) => {
                (4u16, mode_report_value(self.mode.contains(TermMode::INSERT)))
            }
            Mode::Named(NamedMode::LineFeedNewLine) => {
                (20, mode_report_value(self.mode.contains(TermMode::LINE_FEED_NEW_LINE)))
            }
            Mode::Unknown(n) => (n, 0),
        };
        let response = format!("\x1b[{num};{value}$y");
        self.event_listener.send_event(Event::PtyWrite(response));
    }

    /// DECRQM: report DEC private mode status.
    fn report_private_mode(&mut self, mode: PrivateMode) {
        let (num, value) = match mode {
            PrivateMode::Named(named) => {
                let num = named_private_mode_number(named);
                let flag = named_private_mode_flag(named);
                let value = flag.map_or(0, |f| mode_report_value(self.mode.contains(f)));
                (num, value)
            }
            PrivateMode::Unknown(n) => (n, 0),
        };
        let response = format!("\x1b[?{num};{value}$y");
        self.event_listener.send_event(Event::PtyWrite(response));
    }

    // --- CSI device status ---

    /// DA: device attributes response.
    fn identify_terminal(&mut self, intermediate: Option<char>) {
        match intermediate {
            None => {
                // DA1: report VT220 with ANSI color.
                let response = "\x1b[?6c".to_string();
                self.event_listener.send_event(Event::PtyWrite(response));
            }
            Some('>') => {
                // DA2: terminal type 0, version, conformance level 1.
                let version = crate_version_number();
                let response = format!("\x1b[>0;{version};1c");
                self.event_listener.send_event(Event::PtyWrite(response));
            }
            Some(c) => debug!("Unsupported DA intermediate '{c}'"),
        }
    }

    /// DSR: device status report.
    fn device_status(&mut self, arg: usize) {
        match arg {
            5 => {
                // Terminal OK.
                self.event_listener
                    .send_event(Event::PtyWrite("\x1b[0n".to_string()));
            }
            6 => {
                // Cursor position report (1-based, always absolute).
                let line = self.grid().cursor().line() + 1;
                let col = self.grid().cursor().col().0 + 1;
                let response = format!("\x1b[{line};{col}R");
                self.event_listener.send_event(Event::PtyWrite(response));
            }
            _ => debug!("Unknown device status query: {arg}"),
        }
    }

    /// CSI 18 t: report text area size in characters.
    fn text_area_size_chars(&mut self) {
        let lines = self.grid().lines();
        let cols = self.grid().cols();
        let response = format!("\x1b[8;{lines};{cols}t");
        self.event_listener.send_event(Event::PtyWrite(response));
    }

    // --- Keypad mode ---

    /// DECKPAM: set application keypad mode.
    fn set_keypad_application_mode(&mut self) {
        self.mode.insert(TermMode::APP_KEYPAD);
    }

    /// DECKPNM: reset application keypad mode.
    fn unset_keypad_application_mode(&mut self) {
        self.mode.remove(TermMode::APP_KEYPAD);
    }

    // --- SGR (Select Graphic Rendition) ---

    /// Set a terminal attribute (bold, italic, colors, etc.).
    ///
    /// The VTE parser decodes `CSI n m` parameters into high-level `Attr`
    /// variants. Delegates to [`sgr::apply`] which modifies the cursor
    /// template cell so subsequent characters inherit the attribute.
    #[inline]
    fn terminal_attribute(&mut self, attr: Attr) {
        let template = &mut self.grid_mut().cursor_mut().template;
        sgr::apply(template, &attr);
    }

    // --- OSC (Operating System Commands) ---

    /// OSC 0/2: set window title.
    fn set_title(&mut self, title: Option<String>) {
        self.osc_set_title(title);
    }

    /// Push current title onto the title stack.
    fn push_title(&mut self) {
        self.osc_push_title();
    }

    /// Pop title from the stack and set it.
    fn pop_title(&mut self) {
        self.osc_pop_title();
    }

    /// OSC 4/10/11/12: set a palette color.
    fn set_color(&mut self, index: usize, color: Rgb) {
        self.osc_set_color(index, color);
    }

    /// OSC 104/110/111/112: reset a palette color.
    fn reset_color(&mut self, index: usize) {
        self.osc_reset_color(index);
    }

    /// OSC 4/10/11/12 query: respond with current color.
    fn dynamic_color_sequence(&mut self, prefix: String, index: usize, terminator: &str) {
        self.osc_dynamic_color_sequence(prefix, index, terminator);
    }

    /// OSC 52: store clipboard content.
    fn clipboard_store(&mut self, clipboard: u8, base64: &[u8]) {
        self.osc_clipboard_store(clipboard, base64);
    }

    /// OSC 52: request clipboard content.
    fn clipboard_load(&mut self, clipboard: u8, terminator: &str) {
        self.osc_clipboard_load(clipboard, terminator);
    }

    /// OSC 8: set or clear hyperlink.
    fn set_hyperlink(&mut self, hyperlink: Option<VteHyperlink>) {
        self.osc_set_hyperlink(hyperlink);
    }
}

#[cfg(test)]
mod tests;
