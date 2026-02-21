//! Copy and clipboard operations for the application.
//!
//! Implements copy triggers (keybindings), clipboard writes from selection
//! content, and OSC 52 clipboard integration.

use winit::event::ElementState;
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

use oriterm_core::event::ClipboardType;
use oriterm_core::selection::extract_text;

use super::App;

/// Result of a copy keybinding check.
pub(super) enum CopyAction {
    /// The event was a copy keybinding and was handled.
    Handled,
    /// The event was not a copy keybinding.
    NotCopy,
}

impl App {
    /// Copy the active selection to the system clipboard.
    ///
    /// Extracts text from the current tab's selection, stores it in the
    /// clipboard. Returns `true` if text was copied.
    ///
    /// Borrow pattern: the immutable borrow of `self.tab` (for grid + selection)
    /// is confined to the inner block so `self.clipboard` can be mutated after.
    pub(crate) fn copy_selection(&mut self) -> bool {
        let text = {
            let Some(tab) = &self.tab else {
                return false;
            };
            let Some(sel) = tab.selection() else {
                return false;
            };
            let term = tab.terminal().lock();
            extract_text(term.grid(), sel)
        };
        if text.is_empty() {
            return false;
        }
        self.clipboard.store(ClipboardType::Clipboard, &text);
        log::debug!("copied {} bytes to clipboard", text.len());
        true
    }

    /// Copy the active selection to the X11/Wayland primary selection.
    ///
    /// Called on mouse release after a drag selection. On Windows/macOS this
    /// is a no-op (the clipboard module silently ignores `Selection` stores
    /// when no primary selection provider is available).
    pub(crate) fn copy_selection_to_primary(&mut self) {
        let text = {
            let Some(tab) = &self.tab else { return };
            let Some(sel) = tab.selection() else { return };
            let term = tab.terminal().lock();
            extract_text(term.grid(), sel)
        };
        if !text.is_empty() {
            self.clipboard.store(ClipboardType::Selection, &text);
        }
    }

    /// Try to handle a key event as a copy keybinding.
    ///
    /// Recognizes:
    /// - **Ctrl+Shift+C** — copy selection (if any)
    /// - **Ctrl+C** (smart) — copy if selection exists, otherwise not handled
    ///   (falls through to PTY encoding which sends `\x03`)
    /// - **Ctrl+Insert** — copy selection (if any)
    ///
    /// Returns `Handled` if the event was consumed, `NotCopy` if it should
    /// continue through the normal dispatch chain.
    pub(super) fn try_copy_keybinding(
        &mut self,
        event: &winit::event::KeyEvent,
        modifiers: ModifiersState,
    ) -> CopyAction {
        if event.state != ElementState::Pressed {
            return CopyAction::NotCopy;
        }

        let ctrl = modifiers.control_key();
        let shift = modifiers.shift_key();

        match event.physical_key {
            // Ctrl+Shift+C — always a copy keybinding.
            PhysicalKey::Code(KeyCode::KeyC) if ctrl && shift => {
                self.copy_selection();
                CopyAction::Handled
            }
            // Ctrl+C (no shift) — smart: copy if selection, else fall through to PTY.
            PhysicalKey::Code(KeyCode::KeyC) if ctrl && !shift => {
                let has_selection = self.tab.as_ref().is_some_and(|t| t.selection().is_some());
                if has_selection {
                    self.copy_selection();
                    CopyAction::Handled
                } else {
                    CopyAction::NotCopy
                }
            }
            // Ctrl+Insert — copy selection.
            PhysicalKey::Code(KeyCode::Insert) if ctrl => {
                self.copy_selection();
                CopyAction::Handled
            }
            _ => CopyAction::NotCopy,
        }
    }
}
