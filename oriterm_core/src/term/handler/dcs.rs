//! DCS + misc handler implementations.
//!
//! Handles DECSCUSR (cursor shape), Kitty keyboard protocol, and
//! stub responses for unimplemented DCS sequences. Methods are called
//! by the `vte::ansi::Handler` trait impl on `Term<T>`.

use log::debug;
use vte::ansi::{CursorStyle, KeyboardModes, KeyboardModesApplyBehavior, ModifyOtherKeys};

use crate::event::{Event, EventListener};
use crate::grid::CursorShape;
use crate::term::{KEYBOARD_MODE_STACK_MAX_DEPTH, Term, TermMode};

impl<T: EventListener> Term<T> {
    /// DECSCUSR: set cursor shape and blinking.
    ///
    /// `None` means reset to default (Block + no explicit blinking).
    pub(super) fn dcs_set_cursor_style(&mut self, style: Option<CursorStyle>) {
        if let Some(style) = style {
            self.cursor_shape = CursorShape::from(style.shape);
            self.mode.set(TermMode::CURSOR_BLINKING, style.blinking);
        } else {
            // Reset to default: block cursor, implementation-default blinking.
            self.cursor_shape = CursorShape::default();
            self.mode.remove(TermMode::CURSOR_BLINKING);
        }
        self.event_listener.send_event(Event::CursorBlinkingChange);
    }

    /// Set only the cursor shape (no blinking change).
    pub(super) fn dcs_set_cursor_shape(&mut self, shape: vte::ansi::CursorShape) {
        self.cursor_shape = CursorShape::from(shape);
    }

    /// Push a keyboard mode onto the Kitty keyboard protocol stack.
    ///
    /// If the stack exceeds [`KEYBOARD_MODE_STACK_MAX_DEPTH`], the oldest
    /// entry is removed. After pushing, the mode flags are applied.
    pub(super) fn dcs_push_keyboard_mode(&mut self, mode: KeyboardModes) {
        if self.keyboard_mode_stack.len() >= KEYBOARD_MODE_STACK_MAX_DEPTH {
            self.keyboard_mode_stack.pop_front();
        }
        self.keyboard_mode_stack.push_back(mode);
        self.dcs_set_keyboard_mode(mode, KeyboardModesApplyBehavior::Replace);
    }

    /// Pop `to_pop` keyboard modes from the stack, reloading the active mode.
    pub(super) fn dcs_pop_keyboard_modes(&mut self, to_pop: u16) {
        let new_len = self
            .keyboard_mode_stack
            .len()
            .saturating_sub(to_pop as usize);
        self.keyboard_mode_stack.truncate(new_len);

        let mode = self
            .keyboard_mode_stack
            .back()
            .copied()
            .unwrap_or(KeyboardModes::NO_MODE);
        self.dcs_set_keyboard_mode(mode, KeyboardModesApplyBehavior::Replace);
    }

    /// Apply keyboard mode flags with the given behavior (replace/union/difference).
    pub(super) fn dcs_set_keyboard_mode(
        &mut self,
        mode: KeyboardModes,
        apply: KeyboardModesApplyBehavior,
    ) {
        let active = self.mode & TermMode::KITTY_KEYBOARD_PROTOCOL;
        self.mode &= !TermMode::KITTY_KEYBOARD_PROTOCOL;

        let new_mode = TermMode::from(mode);
        let applied = match apply {
            KeyboardModesApplyBehavior::Replace => new_mode,
            KeyboardModesApplyBehavior::Union => active.union(new_mode),
            KeyboardModesApplyBehavior::Difference => active.difference(new_mode),
        };

        self.mode |= applied;
    }

    /// Report the current keyboard mode to the PTY (`CSI ? <mode> u`).
    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "Handler trait requires &mut self"
    )]
    pub(super) fn dcs_report_keyboard_mode(&mut self) {
        let bits = self
            .keyboard_mode_stack
            .back()
            .copied()
            .unwrap_or(KeyboardModes::NO_MODE)
            .bits();
        let response = format!("\x1b[?{bits}u");
        self.event_listener.send_event(Event::PtyWrite(response));
    }

    /// `XTerm` `modifyOtherKeys`: stub implementation.
    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "Handler trait requires &mut self"
    )]
    pub(super) fn dcs_set_modify_other_keys(&mut self, mode: ModifyOtherKeys) {
        debug!("Ignoring modifyOtherKeys: {mode:?}");
    }

    /// `XTerm` `modifyOtherKeys` report: stub implementation.
    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "Handler trait requires &mut self"
    )]
    pub(super) fn dcs_report_modify_other_keys(&mut self) {
        // Report mode 0 (disabled) since we don't implement modifyOtherKeys.
        let response = "\x1b[>4;0m".to_string();
        self.event_listener.send_event(Event::PtyWrite(response));
    }

    /// CSI 14 t: report text area size in pixels (stub).
    ///
    /// Real implementation requires window dimensions from the GUI layer.
    /// Returns 0x0 until wired to the actual window.
    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "Handler trait requires &mut self"
    )]
    pub(super) fn dcs_text_area_size_pixels(&mut self) {
        debug!("text_area_size_pixels: no window yet, reporting 0x0");
        let response = "\x1b[4;0;0t".to_string();
        self.event_listener.send_event(Event::PtyWrite(response));
    }
}
