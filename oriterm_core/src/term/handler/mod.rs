//! VTE handler implementation for `Term<T>`.
//!
//! Implements `vte::ansi::Handler` to process escape sequences, control
//! characters, and printable input. Each method delegates to the
//! appropriate grid/cursor/mode operation.

use std::cmp;

use log::debug;
use unicode_width::UnicodeWidthChar;
use vte::ansi::{
    Attr, CharsetIndex, ClearMode, CursorStyle, Handler, Hyperlink as VteHyperlink,
    KeyboardModes, KeyboardModesApplyBehavior, LineClearMode, Mode, ModifyOtherKeys, NamedMode,
    PrivateMode, Rgb, StandardCharset, TabulationClearMode,
};

use crate::event::{Event, EventListener};
use crate::grid::editing::{DisplayEraseMode, LineEraseMode};
use crate::grid::navigation::TabClearMode;
use crate::index::Column;

use super::{Term, TermMode};

mod dcs;
mod esc;
mod helpers;
mod modes;
mod osc;
mod sgr;
mod status;

impl<T: EventListener> Handler for Term<T> {
    // --- Print + Execute (C0 controls) ---

    /// Print a character, translated through the active charset.
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

    /// LF: linefeed (+ CR in LNM mode).
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

    /// ESC (, ESC ), ESC *, ESC +: designate G0–G3 charset.
    #[inline]
    fn configure_charset(&mut self, index: CharsetIndex, charset: StandardCharset) {
        self.charset.set_charset(index, charset);
    }

    /// ESC N / ESC O: single shift (SS2/SS3).
    #[inline]
    fn set_single_shift(&mut self, index: CharsetIndex) {
        self.charset.set_single_shift(index);
    }

    // --- CSI cursor movement ---

    /// CUP / HVP: absolute cursor positioning (ORIGIN-aware).
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
            ClearMode::Below => DisplayEraseMode::Below,
            ClearMode::Above => DisplayEraseMode::Above,
            ClearMode::All => DisplayEraseMode::All,
            ClearMode::Saved => DisplayEraseMode::Scrollback,
        };
        self.grid_mut().erase_display(erase);
    }

    /// EL: erase in line.
    fn clear_line(&mut self, mode: LineClearMode) {
        let erase = match mode {
            LineClearMode::Right => LineEraseMode::Right,
            LineClearMode::Left => LineEraseMode::Left,
            LineClearMode::All => LineEraseMode::All,
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
        self.status_report_mode(mode);
    }

    /// DECRQM: report DEC private mode status.
    fn report_private_mode(&mut self, mode: PrivateMode) {
        self.status_report_private_mode(mode);
    }

    // --- CSI device status ---

    /// DA: device attributes response.
    fn identify_terminal(&mut self, intermediate: Option<char>) {
        self.status_identify_terminal(intermediate);
    }

    /// DSR: device status report.
    fn device_status(&mut self, arg: usize) {
        self.status_device_status(arg);
    }

    /// CSI 18 t: report text area size in characters.
    fn text_area_size_chars(&mut self) {
        self.status_text_area_size_chars();
    }

    // --- ESC sequences (keypad mode, reset) ---

    /// DECKPAM: set application keypad mode.
    fn set_keypad_application_mode(&mut self) {
        self.mode.insert(TermMode::APP_KEYPAD);
    }

    /// DECKPNM: reset application keypad mode.
    fn unset_keypad_application_mode(&mut self) {
        self.mode.remove(TermMode::APP_KEYPAD);
    }

    /// RIS (ESC c): full terminal reset.
    fn reset_state(&mut self) {
        self.esc_reset_state();
    }

    // --- SGR (Select Graphic Rendition) ---

    /// SGR: set a terminal attribute (bold, italic, colors, etc.).
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

    /// OSC 7: set working directory (shell integration).
    fn set_working_directory(&mut self, uri: Option<String>) {
        self.osc_set_working_directory(uri);
    }

    /// OSC 8: set or clear hyperlink.
    fn set_hyperlink(&mut self, hyperlink: Option<VteHyperlink>) {
        self.osc_set_hyperlink(hyperlink);
    }

    // --- DCS + Misc (cursor shape, keyboard protocol) ---

    /// DECSCUSR: set cursor shape and blinking state.
    fn set_cursor_style(&mut self, style: Option<CursorStyle>) {
        self.dcs_set_cursor_style(style);
    }

    /// Set cursor shape (no blinking change).
    fn set_cursor_shape(&mut self, shape: vte::ansi::CursorShape) {
        self.dcs_set_cursor_shape(shape);
    }

    /// CSI > u: push keyboard mode onto Kitty keyboard protocol stack.
    fn push_keyboard_mode(&mut self, mode: KeyboardModes) {
        self.dcs_push_keyboard_mode(mode);
    }

    /// CSI < u: pop keyboard modes from the stack.
    fn pop_keyboard_modes(&mut self, to_pop: u16) {
        self.dcs_pop_keyboard_modes(to_pop);
    }

    /// Apply keyboard mode flags with the given behavior.
    fn set_keyboard_mode(&mut self, mode: KeyboardModes, apply: KeyboardModesApplyBehavior) {
        self.dcs_set_keyboard_mode(mode, apply);
    }

    /// CSI ? u: report current keyboard mode.
    fn report_keyboard_mode(&mut self) {
        self.dcs_report_keyboard_mode();
    }

    /// `XTerm` `modifyOtherKeys`: stub.
    fn set_modify_other_keys(&mut self, mode: ModifyOtherKeys) {
        self.dcs_set_modify_other_keys(mode);
    }

    /// `XTerm` `modifyOtherKeys` report: stub.
    fn report_modify_other_keys(&mut self) {
        self.dcs_report_modify_other_keys();
    }

    /// CSI 14 t: report text area size in pixels.
    fn text_area_size_pixels(&mut self) {
        self.dcs_text_area_size_pixels();
    }
}

#[cfg(test)]
mod tests;
