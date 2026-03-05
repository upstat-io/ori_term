//! Keyboard input dispatch for the application.
//!
//! Routes key events through mark mode, keybinding table lookup, and
//! finally key encoding to the PTY. Also handles IME commit events.

mod action_dispatch;
pub(super) mod ime;
mod overlay_dispatch;

use winit::event::ElementState;
use winit::keyboard::SmolStr;

use oriterm_ui::input::Key;

use super::{App, mark_mode};
use crate::key_encoding::{self, KeyEventType, KeyInput};
use crate::keybindings;

pub(super) use ime::ImeState;

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
        // Cancel active tab drag on Escape press.
        if event.state == ElementState::Pressed
            && event.logical_key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
            && self.has_tab_drag()
        {
            self.cancel_tab_drag();
            return;
        }

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
                let now = std::time::Instant::now();
                let result = {
                    let Some(ctx) = self
                        .focused_window_id
                        .and_then(|id| self.windows.get_mut(&id))
                    else {
                        return;
                    };
                    let scale = ctx.window.scale_factor().factor() as f32;
                    let Some(renderer) = ctx.renderer.as_ref() else {
                        return;
                    };
                    let measurer =
                        crate::font::UiFontMeasurer::new(renderer.active_ui_collection(), scale);
                    ctx.overlays.process_key_event(
                        ui_event,
                        &measurer,
                        &self.ui_theme,
                        None,
                        &ctx.layer_tree,
                        &mut ctx.layer_animator,
                        now,
                    )
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
        if self.try_dispatch_mark_mode(event) {
            return;
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

    /// Dispatch a key event to mark mode if active.
    ///
    /// Returns `true` if mark mode consumed the event (caller should return).
    fn try_dispatch_mark_mode(&mut self, event: &winit::event::KeyEvent) -> bool {
        let Some(pane_id) = self.active_pane_id() else {
            return false;
        };
        if !self.is_mark_mode(pane_id) {
            return false;
        }
        if event.state == ElementState::Pressed {
            // Build SnapshotGrid from the current snapshot.
            let mux = self.mux.as_mut().expect("mux checked at pane_id");
            if mux.pane_snapshot(pane_id).is_none() || mux.is_pane_snapshot_dirty(pane_id) {
                mux.refresh_pane_snapshot(pane_id);
            }
            let Some(cursor) = self.pane_mark_cursor(pane_id) else {
                return true;
            };
            let selection = self.pane_selection(pane_id).copied();
            let result = {
                let Some(snapshot) = self.mux.as_ref().and_then(|m| m.pane_snapshot(pane_id))
                else {
                    return true;
                };
                let grid = super::snapshot_grid::SnapshotGrid::new(snapshot);
                mark_mode::handle_mark_mode_key(
                    &grid,
                    cursor,
                    selection.as_ref(),
                    event,
                    self.modifiers,
                    &self.config.behavior.word_delimiters,
                )
            };

            // Apply state mutations from the result.
            if let Some(mc) = result.new_cursor {
                self.mark_cursors.insert(pane_id, mc);
            }
            if let Some(sel_update) = result.new_selection {
                match sel_update {
                    mark_mode::SelectionUpdate::Set(sel) => {
                        self.set_pane_selection(pane_id, sel);
                    }
                    mark_mode::SelectionUpdate::Clear => {
                        self.clear_pane_selection(pane_id);
                    }
                }
            }

            match result.action {
                mark_mode::MarkAction::Handled { scroll_delta } => {
                    if let Some(delta) = scroll_delta {
                        if let Some(mux) = self.mux.as_mut() {
                            mux.scroll_display(pane_id, delta);
                        }
                    }
                }
                mark_mode::MarkAction::Exit { copy } => {
                    self.exit_mark_mode(pane_id);
                    if copy {
                        self.copy_selection();
                    }
                }
                mark_mode::MarkAction::Ignored => {}
            }
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.dirty = true;
            }
        }
        true
    }

    /// Encode a key event and send the result to the PTY.
    ///
    /// Works in both embedded mode (local pane) and daemon mode (snapshot
    /// for mode flags, IPC transport for input).
    fn encode_key_to_pty(&mut self, event: &winit::event::KeyEvent) {
        let Some(pane_id) = self.active_pane_id() else {
            return;
        };
        let Some(mode) = self.pane_mode(pane_id) else {
            return;
        };

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
            if let Some(mux) = self.mux.as_mut() {
                mux.scroll_to_bottom(pane_id);
            }
            self.write_pane_input(pane_id, &bytes);
            self.cursor_blink.reset();
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.dirty = true;
            }
        }
    }

    /// Scroll by one page in the given direction.
    fn execute_scroll(&mut self, up: bool) -> bool {
        if let Some(pane_id) = self.active_pane_id() {
            let lines = self
                .mux
                .as_ref()
                .and_then(|m| m.pane_snapshot(pane_id))
                .map_or(24, |s| s.cells.len() as isize);
            let delta = if up { lines } else { -lines };
            if let Some(mux) = self.mux.as_mut() {
                mux.scroll_display(pane_id, delta);
            }
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
        true
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
