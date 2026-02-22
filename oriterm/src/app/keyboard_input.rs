//! Keyboard input dispatch for the application.
//!
//! Routes key events through mark mode, copy/paste keybindings, and
//! finally key encoding to the PTY. Also handles IME commit events.

use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey, SmolStr};

use super::{App, clipboard_ops, mark_mode};
use crate::key_encoding::{self, KeyEventType, KeyInput};

impl App {
    /// Dispatch a keyboard event through mark mode or key encoding to the PTY.
    ///
    /// Mark mode intercepts all key events when active. Otherwise, reads the
    /// terminal mode, converts winit modifiers to key encoding modifiers,
    /// encodes the key event, and sends the result to the PTY.
    pub(super) fn handle_keyboard_input(&mut self, event: &winit::event::KeyEvent) {
        // Mark mode: consume ALL key events (including releases) to prevent
        // leaking input to the PTY while navigating.
        if let Some(tab) = &mut self.tab {
            if tab.is_mark_mode() {
                if event.state == ElementState::Pressed {
                    let action = mark_mode::handle_mark_mode_key(tab, event, self.modifiers);
                    match action {
                        mark_mode::MarkAction::Handled => {
                            self.dirty = true;
                        }
                        mark_mode::MarkAction::Exit { copy } => {
                            if copy {
                                self.copy_selection();
                            }
                            self.dirty = true;
                        }
                        mark_mode::MarkAction::Ignored => {}
                    }
                }
                return;
            }
        }

        // Ctrl+Shift+M enters mark mode.
        // Match on key+modifiers first, consume both press and release to
        // prevent orphaned release events from leaking to the PTY.
        if self.modifiers.control_key()
            && self.modifiers.shift_key()
            && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyM))
        {
            if event.state == ElementState::Pressed && !event.repeat {
                if let Some(tab) = &mut self.tab {
                    tab.enter_mark_mode();
                    self.dirty = true;
                }
            }
            return;
        }

        // Copy keybindings: Ctrl+Shift+C, smart Ctrl+C, Ctrl+Insert.
        if matches!(
            self.try_copy_keybinding(event, self.modifiers),
            clipboard_ops::CopyAction::Handled,
        ) {
            self.dirty = true;
            return;
        }

        // Paste keybindings: Ctrl+Shift+V, Ctrl+V, Shift+Insert.
        if matches!(
            self.try_paste_keybinding(event, self.modifiers),
            clipboard_ops::PasteAction::Handled,
        ) {
            self.dirty = true;
            return;
        }

        // Normal key encoding to PTY.
        let Some(tab) = &self.tab else { return };

        let mode = tab.terminal().lock().mode();

        let event_type = match (event.state, event.repeat) {
            (ElementState::Released, _) => KeyEventType::Release,
            (ElementState::Pressed, true) => KeyEventType::Repeat,
            (ElementState::Pressed, false) => KeyEventType::Press,
        };

        let bytes = key_encoding::encode_key(&KeyInput {
            key: &event.logical_key,
            mods: self.modifiers.into(),
            mode,
            text: event.text.as_ref().map(SmolStr::as_str),
            location: event.location,
            event_type,
        });

        if !bytes.is_empty() {
            tab.scroll_to_bottom();
            tab.write_input(&bytes);
            self.cursor_blink.reset();
            self.dirty = true;
        }
    }

    /// Handle IME commit: send committed text directly to the PTY.
    pub(super) fn handle_ime_commit(&mut self, text: &str) {
        let Some(tab) = &self.tab else { return };
        if !text.is_empty() {
            tab.scroll_to_bottom();
            tab.write_input(text.as_bytes());
            self.cursor_blink.reset();
            self.dirty = true;
        }
    }
}
