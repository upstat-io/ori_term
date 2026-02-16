//! DECSET/DECRST private mode dispatch.
//!
//! Extracted from the Handler impl to keep `handler/mod.rs` under the
//! 500-line limit. Each method maps `NamedPrivateMode` variants to
//! `TermMode` flag changes and side effects (events, screen swaps).

use log::debug;
use vte::ansi::NamedPrivateMode;

use crate::event::{Event, EventListener};
use crate::term::{Term, TermMode};

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
            NamedPrivateMode::ReportMouseClicks => {
                self.mode.insert(TermMode::MOUSE_REPORT_CLICK);
                self.event_listener.send_event(Event::MouseCursorDirty);
            }
            NamedPrivateMode::ReportCellMouseMotion => {
                self.mode.insert(TermMode::MOUSE_DRAG);
                self.event_listener.send_event(Event::MouseCursorDirty);
            }
            NamedPrivateMode::ReportAllMouseMotion => {
                self.mode.insert(TermMode::MOUSE_MOTION);
                self.event_listener.send_event(Event::MouseCursorDirty);
            }
            NamedPrivateMode::ReportFocusInOut => self.mode.insert(TermMode::FOCUS_IN_OUT),
            NamedPrivateMode::Utf8Mouse => self.mode.insert(TermMode::MOUSE_UTF8),
            NamedPrivateMode::SgrMouse => self.mode.insert(TermMode::MOUSE_SGR),
            NamedPrivateMode::UrgencyHints => self.mode.insert(TermMode::URGENCY_HINTS),
            NamedPrivateMode::SwapScreenAndSetRestoreCursor => {
                if !self.mode.contains(TermMode::ALT_SCREEN) {
                    self.swap_alt();
                }
            }
            NamedPrivateMode::BracketedPaste => self.mode.insert(TermMode::BRACKETED_PASTE),
            NamedPrivateMode::SyncUpdate => self.mode.insert(TermMode::SYNC_UPDATE),
            NamedPrivateMode::ColumnMode | NamedPrivateMode::AlternateScroll => {
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
            NamedPrivateMode::UrgencyHints => self.mode.remove(TermMode::URGENCY_HINTS),
            NamedPrivateMode::SwapScreenAndSetRestoreCursor => {
                if self.mode.contains(TermMode::ALT_SCREEN) {
                    self.swap_alt();
                }
            }
            NamedPrivateMode::BracketedPaste => self.mode.remove(TermMode::BRACKETED_PASTE),
            NamedPrivateMode::SyncUpdate => self.mode.remove(TermMode::SYNC_UPDATE),
            NamedPrivateMode::ColumnMode | NamedPrivateMode::AlternateScroll => {
                debug!("Ignoring DECRST for unimplemented mode {named:?}");
            }
        }
    }
}
