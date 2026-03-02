//! DECSET/DECRST private mode dispatch.
//!
//! Extracted from the Handler impl to keep `handler/mod.rs` under the
//! 500-line limit. Each method maps `NamedPrivateMode` variants to
//! `TermMode` flag changes and side effects (events, screen swaps).

use log::debug;
use vte::ansi::{NamedPrivateMode, PrivateMode};

use crate::event::{Event, EventListener};
use crate::term::{Term, TermMode};

use super::helpers::named_private_mode_flag;

impl<T: EventListener> Term<T> {
    /// Apply DECSET (set private mode).
    pub(super) fn apply_decset(&mut self, named: NamedPrivateMode) {
        match named {
            NamedPrivateMode::CursorKeys => self.mode.insert(TermMode::APP_CURSOR),
            NamedPrivateMode::Origin => {
                self.mode.insert(TermMode::ORIGIN);
                self.goto_origin_aware(0, 0);
            }
            NamedPrivateMode::LineWrap => self.mode.insert(TermMode::LINE_WRAP),
            NamedPrivateMode::BlinkingCursor => {
                self.mode.insert(TermMode::CURSOR_BLINKING);
                self.event_listener.send_event(Event::CursorBlinkingChange);
            }
            NamedPrivateMode::ShowCursor => self.mode.insert(TermMode::SHOW_CURSOR),
            NamedPrivateMode::X10Mouse => {
                self.mode.remove(TermMode::ANY_MOUSE);
                self.mode.insert(TermMode::MOUSE_X10);
                self.event_listener.send_event(Event::MouseCursorDirty);
            }
            NamedPrivateMode::ReportMouseClicks => {
                self.mode.remove(TermMode::ANY_MOUSE);
                self.mode.insert(TermMode::MOUSE_REPORT_CLICK);
                self.event_listener.send_event(Event::MouseCursorDirty);
            }
            NamedPrivateMode::ReportCellMouseMotion => {
                self.mode.remove(TermMode::ANY_MOUSE);
                self.mode.insert(TermMode::MOUSE_DRAG);
                self.event_listener.send_event(Event::MouseCursorDirty);
            }
            NamedPrivateMode::ReportAllMouseMotion => {
                self.mode.remove(TermMode::ANY_MOUSE);
                self.mode.insert(TermMode::MOUSE_MOTION);
                self.event_listener.send_event(Event::MouseCursorDirty);
            }
            NamedPrivateMode::ReportFocusInOut => self.mode.insert(TermMode::FOCUS_IN_OUT),
            NamedPrivateMode::Utf8Mouse => {
                self.mode.remove(TermMode::ANY_MOUSE_ENCODING);
                self.mode.insert(TermMode::MOUSE_UTF8);
            }
            NamedPrivateMode::SgrMouse => {
                self.mode.remove(TermMode::ANY_MOUSE_ENCODING);
                self.mode.insert(TermMode::MOUSE_SGR);
            }
            NamedPrivateMode::UrxvtMouse => {
                self.mode.remove(TermMode::ANY_MOUSE_ENCODING);
                self.mode.insert(TermMode::MOUSE_URXVT);
            }
            NamedPrivateMode::UrgencyHints => self.mode.insert(TermMode::URGENCY_HINTS),
            NamedPrivateMode::ReverseWraparound => {
                self.mode.insert(TermMode::REVERSE_WRAP);
            }
            NamedPrivateMode::AltScreen => {
                if !self.mode.contains(TermMode::ALT_SCREEN) {
                    self.swap_alt_no_cursor();
                }
            }
            NamedPrivateMode::AltScreenOpt => {
                if !self.mode.contains(TermMode::ALT_SCREEN) {
                    self.swap_alt_clear();
                }
            }
            NamedPrivateMode::SaveCursor => self.grid_mut().save_cursor(),
            NamedPrivateMode::SwapScreenAndSetRestoreCursor => {
                if !self.mode.contains(TermMode::ALT_SCREEN) {
                    self.swap_alt();
                }
            }
            NamedPrivateMode::BracketedPaste => self.mode.insert(TermMode::BRACKETED_PASTE),
            NamedPrivateMode::SyncUpdate => self.mode.insert(TermMode::SYNC_UPDATE),
            NamedPrivateMode::AlternateScroll => {
                self.mode.insert(TermMode::ALTERNATE_SCROLL);
            }
            NamedPrivateMode::ColumnMode => {
                debug!("Ignoring DECSET for unimplemented mode {named:?}");
            }
        }
    }

    /// Apply DECRST (reset private mode).
    pub(super) fn apply_decrst(&mut self, named: NamedPrivateMode) {
        match named {
            NamedPrivateMode::CursorKeys => self.mode.remove(TermMode::APP_CURSOR),
            NamedPrivateMode::Origin => {
                self.mode.remove(TermMode::ORIGIN);
                self.goto_origin_aware(0, 0);
            }
            NamedPrivateMode::LineWrap => self.mode.remove(TermMode::LINE_WRAP),
            NamedPrivateMode::BlinkingCursor => {
                self.mode.remove(TermMode::CURSOR_BLINKING);
                self.event_listener.send_event(Event::CursorBlinkingChange);
            }
            NamedPrivateMode::ShowCursor => self.mode.remove(TermMode::SHOW_CURSOR),
            NamedPrivateMode::X10Mouse => {
                self.mode.remove(TermMode::MOUSE_X10);
                self.event_listener.send_event(Event::MouseCursorDirty);
            }
            NamedPrivateMode::ReportMouseClicks => {
                self.mode.remove(TermMode::MOUSE_REPORT_CLICK);
                self.event_listener.send_event(Event::MouseCursorDirty);
            }
            NamedPrivateMode::ReportCellMouseMotion => {
                self.mode.remove(TermMode::MOUSE_DRAG);
                self.event_listener.send_event(Event::MouseCursorDirty);
            }
            NamedPrivateMode::ReportAllMouseMotion => {
                self.mode.remove(TermMode::MOUSE_MOTION);
                self.event_listener.send_event(Event::MouseCursorDirty);
            }
            NamedPrivateMode::ReportFocusInOut => self.mode.remove(TermMode::FOCUS_IN_OUT),
            NamedPrivateMode::Utf8Mouse => self.mode.remove(TermMode::MOUSE_UTF8),
            NamedPrivateMode::SgrMouse => self.mode.remove(TermMode::MOUSE_SGR),
            NamedPrivateMode::UrxvtMouse => self.mode.remove(TermMode::MOUSE_URXVT),
            NamedPrivateMode::UrgencyHints => self.mode.remove(TermMode::URGENCY_HINTS),
            NamedPrivateMode::ReverseWraparound => {
                self.mode.remove(TermMode::REVERSE_WRAP);
            }
            NamedPrivateMode::AltScreen | NamedPrivateMode::AltScreenOpt => {
                if self.mode.contains(TermMode::ALT_SCREEN) {
                    self.swap_alt_no_cursor();
                }
            }
            NamedPrivateMode::SaveCursor => self.grid_mut().restore_cursor(),
            NamedPrivateMode::SwapScreenAndSetRestoreCursor => {
                if self.mode.contains(TermMode::ALT_SCREEN) {
                    self.swap_alt();
                }
            }
            NamedPrivateMode::BracketedPaste => self.mode.remove(TermMode::BRACKETED_PASTE),
            NamedPrivateMode::SyncUpdate => self.mode.remove(TermMode::SYNC_UPDATE),
            NamedPrivateMode::AlternateScroll => {
                self.mode.remove(TermMode::ALTERNATE_SCROLL);
            }
            NamedPrivateMode::ColumnMode => {
                debug!("Ignoring DECRST for unimplemented mode {named:?}");
            }
        }
    }

    /// XTSAVE: save current state of listed private modes.
    pub(super) fn apply_xtsave(&mut self, modes: &[u16]) {
        for &num in modes {
            let pm = PrivateMode::new(num);
            let is_set = match pm {
                PrivateMode::Named(named) => {
                    named_private_mode_flag(named).is_some_and(|flag| self.mode.contains(flag))
                }
                PrivateMode::Unknown(_) => false,
            };
            self.saved_private_modes.insert(num, is_set);
        }
    }

    /// XTRESTORE: restore previously saved private mode values.
    pub(super) fn apply_xtrestore(&mut self, modes: &[u16]) {
        for &num in modes {
            if let Some(&saved) = self.saved_private_modes.get(&num) {
                let pm = PrivateMode::new(num);
                if let PrivateMode::Named(named) = pm {
                    if saved {
                        self.apply_decset(named);
                    } else {
                        self.apply_decrst(named);
                    }
                }
            }
        }
    }
}
