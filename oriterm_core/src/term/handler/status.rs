//! CSI device status and mode reporting handlers.
//!
//! DA (device attributes), DSR (device status report), DECRQM (mode report),
//! and CSI t (text area size). Methods are called by the `vte::ansi::Handler`
//! trait impl on `Term<T>`.
//!
//! All methods take `&mut self` because the `Handler` trait requires it,
//! even though these only read state and send events.

use log::debug;
use vte::ansi::{Mode, NamedMode, PrivateMode};

use crate::event::{Event, EventListener};
use crate::term::{Term, TermMode};

use super::helpers::{crate_version_number, mode_report_value, named_private_mode_flag,
    named_private_mode_number};

#[expect(clippy::needless_pass_by_ref_mut, reason = "Handler trait requires &mut self")]
impl<T: EventListener> Term<T> {
    /// DECRQM: report ANSI mode status.
    pub(super) fn status_report_mode(&mut self, mode: Mode) {
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
    pub(super) fn status_report_private_mode(&mut self, mode: PrivateMode) {
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

    /// DA: device attributes response.
    pub(super) fn status_identify_terminal(&mut self, intermediate: Option<char>) {
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
    pub(super) fn status_device_status(&mut self, arg: usize) {
        match arg {
            5 => {
                self.event_listener
                    .send_event(Event::PtyWrite("\x1b[0n".to_string()));
            }
            6 => {
                let line = self.grid().cursor().line() + 1;
                let col = self.grid().cursor().col().0 + 1;
                let response = format!("\x1b[{line};{col}R");
                self.event_listener.send_event(Event::PtyWrite(response));
            }
            _ => debug!("Unknown device status query: {arg}"),
        }
    }

    /// CSI 18 t: report text area size in characters.
    pub(super) fn status_text_area_size_chars(&mut self) {
        let lines = self.grid().lines();
        let cols = self.grid().cols();
        let response = format!("\x1b[8;{lines};{cols}t");
        self.event_listener.send_event(Event::PtyWrite(response));
    }
}
