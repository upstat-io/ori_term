//! IME (Input Method Editor) composition state and event handling.

use super::super::App;

/// IME composition state machine.
///
/// Tracks whether an IME session is active, the current preedit text, and
/// the cursor position within the preedit. Extracted from [`App`] to enable
/// isolated testing of state transitions.
pub(in crate::app) struct ImeState {
    /// Whether an IME composition session is currently active.
    pub active: bool,
    /// Current IME preedit (composition) text. Empty = no active preedit.
    pub preedit: String,
    /// Cursor byte offset within the preedit text (from winit).
    pub preedit_cursor: Option<usize>,
}

/// Side effect to perform after an IME state transition.
#[derive(Debug, PartialEq)]
pub(in crate::app) enum ImeEffect {
    /// State updated, request a redraw.
    Redraw,
    /// Preedit changed, update IME cursor area and redraw.
    UpdateCursorArea,
    /// Text committed, send to PTY.
    Commit(String),
}

impl ImeState {
    /// Create a new IME state with no active composition.
    pub fn new() -> Self {
        Self {
            active: false,
            preedit: String::new(),
            preedit_cursor: None,
        }
    }

    /// Whether raw key events should be suppressed (IME is composing).
    pub fn should_suppress_key(&self) -> bool {
        self.active && !self.preedit.is_empty()
    }

    /// Process an IME event, updating internal state and returning the
    /// side effect for App to perform.
    pub fn handle_event(&mut self, ime: winit::event::Ime) -> ImeEffect {
        match ime {
            winit::event::Ime::Enabled => {
                self.active = true;
                ImeEffect::Redraw
            }
            winit::event::Ime::Preedit(text, cursor) => {
                self.preedit = text;
                self.preedit_cursor = cursor.map(|(start, _)| start);
                ImeEffect::UpdateCursorArea
            }
            winit::event::Ime::Commit(text) => {
                self.preedit.clear();
                self.preedit_cursor = None;
                self.active = false;
                ImeEffect::Commit(text)
            }
            winit::event::Ime::Disabled => {
                self.active = false;
                self.preedit.clear();
                self.preedit_cursor = None;
                ImeEffect::Redraw
            }
        }
    }
}

impl App {
    /// Dispatch an IME event: preedit, commit, enabled, or disabled.
    pub(in crate::app) fn handle_ime_event(&mut self, ime: winit::event::Ime) {
        match self.ime.handle_event(ime) {
            ImeEffect::Redraw => {
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            ImeEffect::UpdateCursorArea => {
                self.update_ime_cursor_area();
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            ImeEffect::Commit(text) => {
                self.handle_ime_commit(&text);
            }
        }
    }

    /// Update the IME candidate window position to match the terminal cursor.
    ///
    /// Reads cursor position from the extracted frame rather than re-locking
    /// the terminal — this is both cheaper (no mutex acquisition) and more
    /// correct (matches the visual frame just rendered). Before the first
    /// frame, `ctx.frame` is `None` and we skip the update.
    ///
    /// Uses 2× cell width for the exclusion zone (Alacritty convention —
    /// avoids tight exclusion on right edge).
    pub(in crate::app) fn update_ime_cursor_area(&self) {
        let Some(ctx) = self.focused_ctx() else {
            return;
        };
        let Some(frame) = &ctx.frame else { return };
        let Some(renderer) = ctx.renderer.as_ref() else {
            return;
        };
        let Some(bounds) = ctx.terminal_grid.bounds() else {
            return;
        };

        let metrics = renderer.cell_metrics();
        let cursor_line = frame.content.cursor.line;
        let cursor_col = frame.content.cursor.column.0;

        // Pixel position of the cursor cell, relative to the window.
        let x = f64::from(bounds.x()) + cursor_col as f64 * f64::from(metrics.width);
        let y = f64::from(bounds.y()) + cursor_line as f64 * f64::from(metrics.height);

        // Exclusion zone: 2× cell width, 1× cell height.
        let w = f64::from(metrics.width) * 2.0;
        let h = f64::from(metrics.height);

        ctx.window.window().set_ime_cursor_area(
            winit::dpi::PhysicalPosition::new(x, y),
            winit::dpi::PhysicalSize::new(w, h),
        );
    }

    /// Handle IME commit: send committed text directly to the PTY.
    pub(in crate::app) fn handle_ime_commit(&mut self, text: &str) {
        let Some(pane_id) = self.active_pane_id() else {
            return;
        };
        if !text.is_empty() {
            if let Some(mux) = self.mux.as_mut() {
                mux.scroll_to_bottom(pane_id);
            }
            self.write_pane_input(pane_id, text.as_bytes());
            self.cursor_blink.reset();
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.dirty = true;
            }
        }
    }
}
