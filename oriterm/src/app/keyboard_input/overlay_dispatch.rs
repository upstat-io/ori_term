//! Overlay and context menu event dispatch.

use oriterm_ui::overlay::OverlayEventResult;
use oriterm_ui::widgets::WidgetAction;

use super::super::{App, context_menu, mark_mode};

impl App {
    /// Process the result of routing an event through the overlay manager.
    pub(in crate::app) fn handle_overlay_result(&mut self, result: OverlayEventResult) {
        match result {
            OverlayEventResult::Delivered { response, .. } => match response.action {
                Some(WidgetAction::Clicked(_)) => self.confirm_paste(),
                Some(WidgetAction::DismissOverlay(_)) => {
                    self.dismiss_context_menu();
                    self.cancel_paste();
                }
                Some(WidgetAction::Selected { index, .. }) => {
                    self.dispatch_context_action(index);
                }
                _ => {
                    if response.response.is_handled() {
                        if let Some(ctx) = self.focused_ctx_mut() {
                            ctx.dirty = true;
                        }
                    }
                }
            },
            OverlayEventResult::Dismissed(_) => {
                self.dismiss_context_menu();
                self.cancel_paste();
            }
            OverlayEventResult::Blocked | OverlayEventResult::PassThrough => {}
        }
    }

    /// Dispatch a context menu selection by index.
    fn dispatch_context_action(&mut self, index: usize) {
        // Resolve the action from the context menu state.
        let action = self
            .focused_ctx()
            .and_then(|ctx| ctx.context_menu.as_ref())
            .and_then(|cm| cm.resolve(index))
            .cloned();

        // Dismiss the menu overlay.
        self.dismiss_context_menu();

        let Some(action) = action else {
            return;
        };

        match action {
            context_menu::ContextAction::SelectScheme(name) => {
                if let Some(scheme) = crate::scheme::resolve_scheme(&name) {
                    let palette = crate::scheme::palette_from_scheme(&scheme);
                    let theme = self
                        .config
                        .colors
                        .resolve_theme(crate::platform::theme::system_theme);
                    // Apply to all panes via MuxBackend.
                    if let Some(mux) = self.mux.as_mut() {
                        for pane_id in mux.pane_ids() {
                            mux.set_pane_theme(pane_id, theme, palette.clone());
                        }
                    }
                    self.config.colors.scheme = name;
                    log::info!("dropdown menu: switched to scheme '{}'", scheme.name);
                }
            }
            context_menu::ContextAction::Settings => {
                log::debug!("settings action not yet implemented");
            }
            context_menu::ContextAction::CloseTab(idx) => {
                self.close_tab_at_index(idx);
            }
            context_menu::ContextAction::DuplicateTab(_idx) => {
                // Duplicate creates a new tab in the same window (inherits CWD
                // from the active pane — same as the keyboard shortcut).
                if let Some(win_id) = self.active_window {
                    self.new_tab_in_window(win_id);
                }
            }
            context_menu::ContextAction::MoveToNewWindow(idx) => {
                self.move_tab_to_new_window_deferred(idx);
            }
            context_menu::ContextAction::Copy => {
                self.copy_selection();
            }
            context_menu::ContextAction::Paste => {
                self.paste_from_clipboard();
            }
            context_menu::ContextAction::SelectAll => {
                if let Some(pane_id) = self.active_pane_id() {
                    // Build SnapshotGrid for select_all.
                    let mux = self.mux.as_mut().expect("mux checked at pane_id");
                    if mux.pane_snapshot(pane_id).is_none() || mux.is_pane_snapshot_dirty(pane_id) {
                        mux.refresh_pane_snapshot(pane_id);
                    }
                    if let Some(snap) = self.mux.as_ref().and_then(|m| m.pane_snapshot(pane_id)) {
                        let grid = crate::app::snapshot_grid::SnapshotGrid::new(snap);
                        let sel = mark_mode::select_all(&grid);
                        self.set_pane_selection(pane_id, sel);
                    }
                }
            }
            context_menu::ContextAction::NewTab => {
                if let Some(win_id) = self.active_window {
                    self.new_tab_in_window(win_id);
                }
            }
        }

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Clear context menu state and dismiss all overlays.
    fn dismiss_context_menu(&mut self) {
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.context_menu = None;
            ctx.overlays
                .clear_all(&mut ctx.layer_tree, &mut ctx.layer_animator);
            ctx.dirty = true;
        }
    }
}
