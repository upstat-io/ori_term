//! Keyboard input dispatch for the application.
//!
//! Routes key events through mark mode, keybinding table lookup, and
//! finally key encoding to the PTY. Also handles IME commit events.

mod action_dispatch;

use winit::event::ElementState;
use winit::keyboard::SmolStr;

use oriterm_ui::input::Key;
use oriterm_ui::overlay::OverlayEventResult;
use oriterm_ui::widgets::WidgetAction;

use super::{App, mark_mode};
use crate::key_encoding::{self, KeyEventType, KeyInput};
use crate::keybindings;

/// IME composition state machine.
///
/// Tracks whether an IME session is active, the current preedit text, and
/// the cursor position within the preedit. Extracted from [`App`] to enable
/// isolated testing of state transitions.
pub(super) struct ImeState {
    /// Whether an IME composition session is currently active.
    pub active: bool,
    /// Current IME preedit (composition) text. Empty = no active preedit.
    pub preedit: String,
    /// Cursor byte offset within the preedit text (from winit).
    pub preedit_cursor: Option<usize>,
}

/// Side effect to perform after an IME state transition.
#[derive(Debug, PartialEq)]
pub(super) enum ImeEffect {
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
    /// Dispatch a keyboard event through overlays, mark mode, keybindings,
    /// or PTY encoding.
    ///
    /// Priority order:
    /// 0. Modal overlay (if active, consumes ALL key events).
    /// 1. Mark mode (if active, consumes all events).
    /// 2. Keybinding table lookup.
    /// 3. Normal key encoding to PTY.
    pub(super) fn handle_keyboard_input(&mut self, event: &winit::event::KeyEvent) {
        // Suppress raw key events during active IME composition.
        // The IME subsystem sends Ime::Commit when done; raw KeyboardInput
        // events during composition are intermediate and must not reach the PTY.
        if self.ime.should_suppress_key() {
            return;
        }

        // Modal overlay: intercept keyboard events before anything else.
        let has_overlays = self
            .focused_ctx()
            .is_some_and(|ctx| !ctx.overlays.is_empty());
        if has_overlays && event.state == ElementState::Pressed {
            if let Some(key) = winit_key_to_ui_key(&event.logical_key) {
                let ui_event = oriterm_ui::input::KeyEvent {
                    key,
                    modifiers: super::winit_mods_to_ui(self.modifiers),
                };
                let scale = self
                    .focused_ctx()
                    .map_or(1.0, |ctx| ctx.window.scale_factor().factor() as f32);
                let measurer = self
                    .renderer
                    .as_ref()
                    .map(|r| crate::font::UiFontMeasurer::new(r.active_ui_collection(), scale));
                let measurer: &dyn oriterm_ui::widgets::TextMeasurer = match &measurer {
                    Some(m) => m,
                    None => return,
                };
                // Borrow split: inline window lookup borrows only self.windows,
                // leaving self.renderer and self.ui_theme available as disjoint borrows.
                let result = {
                    let Some(ctx) = self
                        .focused_window_id
                        .and_then(|id| self.windows.get_mut(&id))
                    else {
                        return;
                    };
                    ctx.overlays
                        .process_key_event(ui_event, measurer, &self.ui_theme, None)
                };
                self.handle_overlay_result(result);
            }
            return;
        }

        // Search mode: consume ALL key events while search is active.
        if self.is_search_active() {
            self.handle_search_key(event);
            return;
        }

        // Mark mode: consume ALL key events (including releases) to prevent
        // leaking input to the PTY while navigating.
        if let Some(pane_id) = self.active_pane_id() {
            if let Some(pane) = self.panes.get_mut(&pane_id) {
                if pane.is_mark_mode() {
                    if event.state == ElementState::Pressed {
                        let action = mark_mode::handle_mark_mode_key(
                            pane,
                            event,
                            self.modifiers,
                            &self.config.behavior.word_delimiters,
                        );
                        match action {
                            mark_mode::MarkAction::Handled => {
                                if let Some(ctx) = self.focused_ctx_mut() {
                                    ctx.dirty = true;
                                }
                            }
                            mark_mode::MarkAction::Exit { copy } => {
                                if copy {
                                    self.copy_selection();
                                }
                                if let Some(ctx) = self.focused_ctx_mut() {
                                    ctx.dirty = true;
                                }
                            }
                            mark_mode::MarkAction::Ignored => {}
                        }
                    }
                    return;
                }
            }
        }

        // Keybinding dispatch: look up the key+modifiers in the binding table.
        if event.state == ElementState::Pressed {
            let mods = self.modifiers.into();
            if let Some(binding_key) = keybindings::key_to_binding_key(&event.logical_key) {
                if let Some(action) = keybindings::find_binding(&self.bindings, &binding_key, mods)
                {
                    // Clone to release the immutable borrow on self.bindings
                    // before calling execute_action which needs &mut self.
                    let action = action.clone();
                    if self.execute_action(&action) {
                        return;
                    }
                }
            }
        }

        // Normal key encoding to PTY.
        self.encode_key_to_pty(event);
    }

    /// Encode a key event and send the result to the PTY.
    fn encode_key_to_pty(&mut self, event: &winit::event::KeyEvent) {
        let Some(pane) = self.active_pane() else {
            return;
        };

        let mode = pane.terminal().lock().mode();

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
            pane.scroll_to_bottom();
            pane.write_input(&bytes);
            self.cursor_blink.reset();
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.dirty = true;
            }
        }
    }

    /// Scroll by one page in the given direction.
    fn execute_scroll(&mut self, up: bool) -> bool {
        if let Some(pane) = self.active_pane() {
            let term = pane.terminal().lock();
            let lines = term.grid().lines() as isize;
            drop(term);
            pane.scroll_display(if up { lines } else { -lines });
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
        true
    }

    /// Dispatch an IME event: preedit, commit, enabled, or disabled.
    pub(super) fn handle_ime_event(&mut self, ime: winit::event::Ime) {
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
    pub(super) fn update_ime_cursor_area(&self) {
        let Some(ctx) = self.focused_ctx() else {
            return;
        };
        let Some(frame) = &ctx.frame else { return };
        let Some(renderer) = &self.renderer else {
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
    pub(super) fn handle_ime_commit(&mut self, text: &str) {
        let Some(pane) = self.active_pane() else {
            return;
        };
        if !text.is_empty() {
            pane.scroll_to_bottom();
            pane.write_input(text.as_bytes());
            self.cursor_blink.reset();
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.dirty = true;
            }
        }
    }

    /// Process the result of routing an event through the overlay manager.
    pub(super) fn handle_overlay_result(&mut self, result: OverlayEventResult) {
        match result {
            OverlayEventResult::Delivered { response, .. } => match response.action {
                Some(WidgetAction::Clicked(_)) => self.confirm_paste(),
                Some(WidgetAction::DismissOverlay(_)) => self.cancel_paste(),
                _ => {
                    if response.response.is_handled() {
                        if let Some(ctx) = self.focused_ctx_mut() {
                            ctx.dirty = true;
                        }
                    }
                }
            },
            OverlayEventResult::Dismissed(_) => self.cancel_paste(),
            OverlayEventResult::Blocked | OverlayEventResult::PassThrough => {}
        }
    }
}

/// Convert a winit logical key to an `oriterm_ui` [`Key`].
///
/// Returns `None` for keys that the UI framework doesn't handle.
fn winit_key_to_ui_key(key: &winit::keyboard::Key) -> Option<Key> {
    use winit::keyboard::{Key as WKey, NamedKey};
    match key {
        WKey::Named(NamedKey::Enter) => Some(Key::Enter),
        WKey::Named(NamedKey::Space) => Some(Key::Space),
        WKey::Named(NamedKey::Escape) => Some(Key::Escape),
        WKey::Named(NamedKey::Tab) => Some(Key::Tab),
        WKey::Named(NamedKey::Backspace) => Some(Key::Backspace),
        WKey::Named(NamedKey::Delete) => Some(Key::Delete),
        WKey::Named(NamedKey::Home) => Some(Key::Home),
        WKey::Named(NamedKey::End) => Some(Key::End),
        WKey::Named(NamedKey::ArrowUp) => Some(Key::ArrowUp),
        WKey::Named(NamedKey::ArrowDown) => Some(Key::ArrowDown),
        WKey::Named(NamedKey::ArrowLeft) => Some(Key::ArrowLeft),
        WKey::Named(NamedKey::ArrowRight) => Some(Key::ArrowRight),
        WKey::Character(s) => s.chars().next().map(Key::Character),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
