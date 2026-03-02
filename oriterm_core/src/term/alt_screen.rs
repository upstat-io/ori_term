//! Alt screen swap operations.
//!
//! Modes 47 (legacy), 1047 (clear on enter), and 1049 (save/restore cursor)
//! each use a different swap variant. All toggle `ALT_SCREEN`, swap keyboard
//! mode stacks, and mark all lines dirty.

use crate::event::EventListener;

use super::{Term, TermMode};

impl<T: EventListener> Term<T> {
    /// Switch between primary and alternate screen (mode 1049).
    ///
    /// Saves/restores cursor, toggles `TermMode::ALT_SCREEN`, swaps keyboard
    /// mode stacks, and marks all lines dirty. Also marks selection as dirty
    /// since screen content changes completely.
    pub fn swap_alt(&mut self) {
        self.selection_dirty = true;
        if self.mode.contains(TermMode::ALT_SCREEN) {
            // Switching back to primary: save alt cursor, restore primary cursor.
            self.alt_grid.save_cursor();
            self.grid.restore_cursor();
        } else {
            // Switching to alt: save primary cursor, restore alt cursor.
            self.grid.save_cursor();
            self.alt_grid.restore_cursor();
        }

        self.toggle_alt_common();
    }

    /// Switch alt screen without saving/restoring cursor (mode 47).
    ///
    /// Toggles `ALT_SCREEN`, swaps keyboard mode stacks, and marks all
    /// lines dirty. Does NOT save or restore the cursor position.
    pub fn swap_alt_no_cursor(&mut self) {
        self.selection_dirty = true;
        self.toggle_alt_common();
    }

    /// Switch to alt screen, clearing it on enter (mode 1047).
    ///
    /// When entering alt screen: clears the alt grid, then swaps.
    /// Does NOT save or restore the cursor position.
    pub fn swap_alt_clear(&mut self) {
        self.selection_dirty = true;
        // Clear the alt grid before entering.
        self.alt_grid.reset();
        self.toggle_alt_common();
    }

    /// Common alt screen toggle: flip flag, swap keyboard stacks, mark dirty.
    fn toggle_alt_common(&mut self) {
        self.mode.toggle(TermMode::ALT_SCREEN);
        std::mem::swap(
            &mut self.keyboard_mode_stack,
            &mut self.inactive_keyboard_mode_stack,
        );
        self.grid_mut().dirty_mut().mark_all();
    }
}
