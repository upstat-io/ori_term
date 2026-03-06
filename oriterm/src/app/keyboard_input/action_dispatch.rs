//! Keybinding action dispatch table.
//!
//! Maps [`Action`] variants to application state mutations. Extracted from
//! `keyboard_input/mod.rs` to keep the parent under the 500-line limit.

use crate::keybindings::Action;

use super::super::App;

impl App {
    /// Execute a keybinding action. Returns `true` if the event was consumed.
    ///
    /// `SmartCopy` returns `false` when no selection exists (fall through to PTY
    /// so Ctrl+C sends SIGINT). Other actions always consume the event.
    #[expect(
        clippy::too_many_lines,
        reason = "action dispatch table — inherently one arm per variant"
    )]
    pub(in crate::app) fn execute_action(&mut self, action: &Action) -> bool {
        match action {
            Action::Copy => {
                self.copy_selection();
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
                true
            }
            Action::Paste | Action::SmartPaste => {
                self.paste_from_clipboard();
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
                true
            }
            Action::SmartCopy => {
                let has_sel = self
                    .active_pane_id()
                    .and_then(|id| self.pane_selection(id))
                    .is_some();
                if has_sel {
                    self.copy_selection();
                    if let Some(ctx) = self.focused_ctx_mut() {
                        ctx.dirty = true;
                    }
                    true
                } else {
                    false
                }
            }
            Action::ScrollPageUp => self.execute_scroll(true),
            Action::ScrollPageDown => self.execute_scroll(false),
            Action::ScrollToTop => {
                if let Some(pane_id) = self.active_pane_id() {
                    if let Some(mux) = self.mux.as_mut() {
                        mux.scroll_display(pane_id, isize::MAX);
                    }
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
                true
            }
            Action::ScrollToBottom => {
                if let Some(pane_id) = self.active_pane_id() {
                    if let Some(mux) = self.mux.as_mut() {
                        mux.scroll_to_bottom(pane_id);
                    }
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
                true
            }
            Action::ReloadConfig => {
                self.apply_config_reload();
                true
            }
            Action::ToggleFullscreen => {
                if let Some(ctx) = self.focused_ctx() {
                    let is_fs = ctx.window.is_fullscreen();
                    ctx.window.set_fullscreen(!is_fs);
                }
                true
            }
            Action::EnterMarkMode => {
                if let Some(pane_id) = self.active_pane_id() {
                    self.enter_mark_mode(pane_id);
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
                true
            }
            Action::SendText(text) => {
                if let Some(pane_id) = self.active_pane_id() {
                    if let Some(mux) = self.mux.as_mut() {
                        mux.scroll_to_bottom(pane_id);
                    }
                    self.write_pane_input(pane_id, text.as_bytes());
                    self.cursor_blink.reset();
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
                true
            }
            Action::OpenSearch => {
                self.open_search();
                true
            }
            // Pane splitting and navigation (delegated to pane_ops).
            Action::SplitRight
            | Action::SplitDown
            | Action::FocusPaneUp
            | Action::FocusPaneDown
            | Action::FocusPaneLeft
            | Action::FocusPaneRight
            | Action::NextPane
            | Action::PrevPane
            | Action::ClosePane
            | Action::ResizePaneUp
            | Action::ResizePaneDown
            | Action::ResizePaneLeft
            | Action::ResizePaneRight
            | Action::EqualizePanes
            | Action::ToggleZoom
            | Action::ToggleFloatingPane
            | Action::ToggleFloatTile
            | Action::UndoSplit
            | Action::RedoSplit => {
                self.execute_pane_action(action);
                true
            }
            // Tab management (Section 32.1).
            Action::NewTab => {
                if let Some(win_id) = self.active_window {
                    self.new_tab_in_window(win_id);
                }
                true
            }
            Action::CloseTab => {
                self.close_active_tab();
                true
            }
            Action::NextTab => {
                self.cycle_tab(1);
                true
            }
            Action::PrevTab => {
                self.cycle_tab(-1);
                true
            }
            Action::DuplicateTab => {
                self.duplicate_active_tab();
                true
            }
            Action::NewWindow => {
                let _ = self
                    .event_proxy
                    .send_event(crate::event::TermEvent::CreateWindow);
                true
            }
            Action::PreviousPrompt => {
                if let Some(pane_id) = self.active_pane_id() {
                    if let Some(mux) = self.mux.as_mut() {
                        mux.scroll_to_previous_prompt(pane_id);
                    }
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
                true
            }
            Action::NextPrompt => {
                if let Some(pane_id) = self.active_pane_id() {
                    if let Some(mux) = self.mux.as_mut() {
                        mux.scroll_to_next_prompt(pane_id);
                    }
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
                true
            }
            Action::SelectCommandOutput => {
                if let Some(pane_id) = self.active_pane_id() {
                    if let Some(sel) = self
                        .mux
                        .as_ref()
                        .and_then(|m| m.select_command_output(pane_id))
                    {
                        self.set_pane_selection(pane_id, sel);
                    }
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
                true
            }
            Action::SelectCommandInput => {
                if let Some(pane_id) = self.active_pane_id() {
                    if let Some(sel) = self
                        .mux
                        .as_ref()
                        .and_then(|m| m.select_command_input(pane_id))
                    {
                        self.set_pane_selection(pane_id, sel);
                    }
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
                true
            }
            Action::MoveTabToNewWindow => {
                // Resolve the active tab index and defer to `about_to_wait`
                // where `ActiveEventLoop` is available.
                let idx = self.active_window.and_then(|wid| {
                    let win = self.session.get_window(wid)?;
                    Some(win.active_tab_idx())
                });
                if let Some(i) = idx {
                    self.move_tab_to_new_window_deferred(i);
                }
                true
            }
            // Actions for future sections — consume the event but log a stub.
            Action::ZoomIn | Action::ZoomOut | Action::ZoomReset => {
                log::debug!("keybinding action not yet implemented: {action:?}");
                true
            }
            Action::None => true,
        }
    }
}
