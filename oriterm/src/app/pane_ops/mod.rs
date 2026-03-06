//! Pane operations: split, focus, cycle, close, resize.
//!
//! App-level methods that bridge keybinding actions to session/mux.
//! Layout mutations (zoom, split tree, floating layer) are applied
//! to the local session. Only pane spawn/close/resize go through mux.

mod helpers;

use oriterm_mux::{PaneId, SpawnConfig};

use crate::session::SplitDirection;
use crate::session::nav::Direction;

use super::App;
use crate::keybindings::Action;

/// Per-keypress ratio adjustment for keyboard resize (5%).
const RESIZE_STEP: f32 = 0.05;

impl App {
    /// Dispatch a pane-related keybinding action.
    pub(super) fn execute_pane_action(&mut self, action: &Action) {
        match action {
            Action::SplitRight => self.split_pane(SplitDirection::Vertical),
            Action::SplitDown => self.split_pane(SplitDirection::Horizontal),
            Action::FocusPaneUp => self.focus_pane_direction(Direction::Up),
            Action::FocusPaneDown => self.focus_pane_direction(Direction::Down),
            Action::FocusPaneLeft => self.focus_pane_direction(Direction::Left),
            Action::FocusPaneRight => self.focus_pane_direction(Direction::Right),
            Action::NextPane => self.cycle_pane(true),
            Action::PrevPane => self.cycle_pane(false),
            Action::ClosePane => self.close_focused_pane(),
            Action::ResizePaneUp => self.resize_pane_toward(Direction::Up),
            Action::ResizePaneDown => self.resize_pane_toward(Direction::Down),
            Action::ResizePaneLeft => self.resize_pane_toward(Direction::Left),
            Action::ResizePaneRight => self.resize_pane_toward(Direction::Right),
            Action::EqualizePanes => self.equalize_panes(),
            Action::ToggleZoom => self.toggle_zoom(),
            Action::ToggleFloatingPane => self.toggle_floating_pane(),
            Action::ToggleFloatTile => self.toggle_float_tile(),
            Action::UndoSplit => self.undo_split(),
            Action::RedoSplit => self.redo_split(),
            _ => {}
        }
    }

    /// Toggle zoom on the focused pane.
    ///
    /// When zoomed, the focused pane fills the entire tab area and all
    /// other panes and dividers are hidden. Pressing again restores the
    /// full split layout.
    pub(super) fn toggle_zoom(&mut self) {
        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };
        if let Some(tab) = self.session.get_tab_mut(tab_id) {
            if tab.zoomed_pane().is_some() {
                tab.set_zoomed_pane(None);
            } else {
                tab.set_zoomed_pane(Some(tab.active_pane()));
            }
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.pane_cache.invalidate_all();
            ctx.dirty = true;
        }
        self.sync_tab_bar_from_mux();
    }

    /// Split the focused pane in the given direction.
    ///
    /// Spawns a new pane via the mux, then updates the local split tree.
    pub(super) fn split_pane(&mut self, direction: SplitDirection) {
        self.unzoom_if_needed();
        let (tab_id, source_pane_id) = match self.active_pane_context() {
            Some(ctx) => ctx,
            None => return,
        };

        let (approx_rows, approx_cols) = self.estimate_split_size(source_pane_id, direction);

        let theme = self
            .config
            .colors
            .resolve_theme(crate::platform::theme::system_theme);

        let config = SpawnConfig {
            cols: approx_cols,
            rows: approx_rows,
            scrollback: self.config.terminal.scrollback,
            shell_integration: self.config.behavior.shell_integration,
            ..SpawnConfig::default()
        };

        let palette =
            crate::app::config_reload::build_palette_from_config(&self.config.colors, theme);

        let Some(mux) = &mut self.mux else { return };
        let new_pane_id = match mux.spawn_pane(&config, theme) {
            Ok(pid) => {
                mux.set_pane_theme(pid, theme, palette);
                log::info!("split pane: {source_pane_id:?} -> {pid:?} ({direction:?})");
                pid
            }
            Err(e) => {
                log::error!("split pane failed: {e}");
                return;
            }
        };

        // Local tree split.
        if let Some(tab) = self.session.get_tab_mut(tab_id) {
            let new_tree = tab
                .tree()
                .split_at(source_pane_id, direction, new_pane_id, 0.5);
            tab.set_tree(new_tree);
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Move focus to a pane in the given direction.
    pub(super) fn focus_pane_direction(&mut self, direction: Direction) {
        if self.unzoom_if_needed() {
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.pane_cache.invalidate_all();
            }
            self.resize_all_panes();
        }
        let layouts = match self.current_pane_layouts() {
            Some(l) => l,
            None => return,
        };
        let Some(pane_id) = self.active_pane_id() else {
            return;
        };

        if let Some(target) = crate::session::nav::navigate_wrap(&layouts, pane_id, direction) {
            self.set_focused_pane(target);
        }
    }

    /// Cycle to the next or previous pane.
    pub(super) fn cycle_pane(&mut self, forward: bool) {
        if self.unzoom_if_needed() {
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.pane_cache.invalidate_all();
            }
            self.resize_all_panes();
        }
        let layouts = match self.current_pane_layouts() {
            Some(l) => l,
            None => return,
        };
        let Some(pane_id) = self.active_pane_id() else {
            return;
        };

        if let Some(target) = crate::session::nav::cycle(&layouts, pane_id, forward) {
            self.set_focused_pane(target);
        }
    }

    /// Close the focused pane.
    ///
    /// If this is the last pane in the last tab, takes the same `shutdown()`
    /// path as `WindowEvent::CloseRequested` (the window close button). A
    /// future confirmation dialog only needs to gate `shutdown()`.
    pub(super) fn close_focused_pane(&mut self) {
        let Some(pane_id) = self.active_pane_id() else {
            return;
        };

        // Last pane? Same path as the close button.
        if self.session.is_last_pane(pane_id) {
            self.exit_app();
        }

        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };

        // Close the pane via mux (unregisters, emits PaneClosed for cleanup).
        if let Some(mux) = &mut self.mux {
            mux.close_pane(pane_id);
        }
        log::info!("close pane {pane_id:?}");

        // Update local session: remove pane from tree/floating.
        let tab_empty = if let Some(tab) = self.session.get_tab_mut(tab_id) {
            if tab.is_floating(pane_id) {
                let new_layer = tab.floating().remove(pane_id);
                tab.set_floating(new_layer);
            } else if let Some(new_tree) = tab.tree().remove(pane_id) {
                tab.replace_layout(new_tree);
            } else {
                // Pane not found in tree or floating — already removed.
            }
            // Reassign active pane if the closed pane was active.
            if tab.active_pane() == pane_id {
                tab.set_active_pane(tab.tree().first_pane());
            }
            tab.all_panes().is_empty()
        } else {
            false
        };

        if tab_empty {
            // Tab has no panes left — remove it.
            let win_id = self.session.window_for_tab(tab_id);
            self.session.remove_tab(tab_id);
            if let Some(wid) = win_id {
                if let Some(win) = self.session.get_window_mut(wid) {
                    win.remove_tab(tab_id);
                }
                let window_empty = self
                    .session
                    .get_window(wid)
                    .is_some_and(|w| w.tabs().is_empty());
                if window_empty {
                    self.close_empty_session_window(wid);
                    return;
                }
            }
        }

        self.sync_tab_bar_from_mux();
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.pane_cache.invalidate_all();
            ctx.cached_dividers = None;
            ctx.dirty = true;
        }
        self.resize_all_panes();
    }

    /// Resize all panes in the active tab to match their computed layouts.
    ///
    /// Called after layout changes (split, close, window resize) to ensure
    /// each pane's terminal grid and PTY match their pixel allocation.
    /// For single-pane tabs, resizes the active pane to fill the full grid
    /// area (the layout engine returns `None` for single-pane tabs).
    pub(super) fn resize_all_panes(&mut self) {
        let Some((layouts, _)) = self.compute_pane_layouts() else {
            // Single pane — resize it to fill the full grid area.
            self.resize_single_pane();
            return;
        };
        let Some(mux) = self.mux.as_mut() else { return };
        for layout in &layouts {
            mux.resize_pane_grid(layout.pane_id, layout.rows, layout.cols);
        }
    }

    /// Resize the single active pane to fill the full grid area.
    ///
    /// Computes rows/cols from the grid bounds and cell metrics, matching
    /// the same calculation `sync_grid_layout` uses during window resize.
    fn resize_single_pane(&mut self) {
        let Some(ctx) = self.focused_ctx() else {
            return;
        };
        let Some(bounds) = ctx.terminal_grid.bounds() else {
            return;
        };
        let Some(renderer) = ctx.renderer.as_ref() else {
            return;
        };
        let cell = renderer.cell_metrics();
        let cols = cell.columns(bounds.width() as u32).max(1) as u16;
        let rows = cell.rows(bounds.height() as u32).max(1) as u16;
        if let Some(pane_id) = self.active_pane_id() {
            if let Some(mux) = self.mux.as_mut() {
                mux.resize_pane_grid(pane_id, rows, cols);
            }
        }
    }

    /// Switch focus to the pane under the mouse cursor.
    ///
    /// Called on any mouse button press. In single-pane tabs this is a no-op
    /// (layouts return `None`). In multi-pane tabs, hit-tests the cursor
    /// position against pane rects — floating panes take priority.
    /// Does not consume the click; the caller continues with normal handling.
    pub(super) fn try_pane_focus_click(&mut self) {
        let layouts = match self.current_pane_layouts() {
            Some(l) => l,
            None => return,
        };
        let pos = self.mouse.cursor_pos();
        let Some(target) = crate::session::nav::nearest_pane(&layouts, pos.x as f32, pos.y as f32)
        else {
            return;
        };
        let Some(current) = self.active_pane_id() else {
            return;
        };
        if target != current {
            self.set_focused_pane(target);
            self.raise_if_floating(target);
        }
    }

    /// Resize the focused pane by pushing the nearest split border.
    ///
    /// Translates a navigation direction to axis + side + delta:
    /// - Right: push vertical border right (pane in first, +step)
    /// - Left: push vertical border left (pane in second, −step)
    /// - Down: push horizontal border down (pane in first, +step)
    /// - Up: push horizontal border up (pane in second, −step)
    pub(super) fn resize_pane_toward(&mut self, direction: Direction) {
        self.unzoom_if_needed();
        let Some((tab_id, pane_id)) = self.active_pane_context() else {
            return;
        };
        let (axis, pane_in_first, delta) = match direction {
            Direction::Right => (SplitDirection::Vertical, true, RESIZE_STEP),
            Direction::Left => (SplitDirection::Vertical, false, -RESIZE_STEP),
            Direction::Down => (SplitDirection::Horizontal, true, RESIZE_STEP),
            Direction::Up => (SplitDirection::Horizontal, false, -RESIZE_STEP),
        };
        if let Some(tab) = self.session.get_tab_mut(tab_id) {
            if let Some(new_tree) =
                tab.tree()
                    .try_resize_toward(pane_id, axis, pane_in_first, delta)
            {
                tab.set_tree(new_tree);
            }
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Reset all split ratios in the active tab to 0.5.
    pub(super) fn equalize_panes(&mut self) {
        self.unzoom_if_needed();
        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };
        if let Some(tab) = self.session.get_tab_mut(tab_id) {
            let new_tree = tab.tree().equalize();
            if new_tree != *tab.tree() {
                tab.set_tree(new_tree);
            }
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Toggle floating pane: focus topmost if any exist, else spawn a new one.
    pub(super) fn toggle_floating_pane(&mut self) {
        self.unzoom_if_needed();
        let Some((tab_id, active)) = self.active_pane_context() else {
            return;
        };

        // Read local session to decide focus target.
        let focus_target = {
            let Some(tab) = self.session.get_tab(tab_id) else {
                return;
            };
            if tab.floating().is_empty() {
                None
            } else if tab.is_floating(active) {
                // Active is floating — focus first tiled pane.
                Some(tab.tree().first_pane())
            } else {
                // Active is tiled — focus topmost floating pane.
                tab.floating().panes().last().map(|fp| fp.pane_id)
            }
        };

        if let Some(target) = focus_target {
            self.set_focused_pane(target);
            return;
        }

        // No floating panes — spawn a new one.
        let Some(available) = self.grid_available_rect() else {
            return;
        };

        let theme = self
            .config
            .colors
            .resolve_theme(crate::platform::theme::system_theme);

        let config = SpawnConfig {
            cols: 80,
            rows: 24,
            scrollback: self.config.terminal.scrollback,
            shell_integration: self.config.behavior.shell_integration,
            ..SpawnConfig::default()
        };

        let palette =
            crate::app::config_reload::build_palette_from_config(&self.config.colors, theme);

        let Some(mux) = &mut self.mux else { return };
        let new_pane_id = match mux.spawn_pane(&config, theme) {
            Ok(pid) => {
                mux.set_pane_theme(pid, theme, palette);
                log::info!("spawn floating pane: {pid:?}");
                pid
            }
            Err(e) => {
                log::error!("spawn floating pane failed: {e}");
                return;
            }
        };

        // Local floating pane add.
        if let Some(tab) = self.session.get_tab_mut(tab_id) {
            let next_z = tab
                .floating()
                .panes()
                .iter()
                .map(|p| p.z_order)
                .max()
                .unwrap_or(0)
                + 1;
            let fp = crate::session::FloatingPane::centered(new_pane_id, &available, next_z);
            let new_layer = tab.floating().add(fp);
            tab.set_floating(new_layer);
            tab.set_active_pane(new_pane_id);
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.pane_cache.invalidate_all();
            ctx.dirty = true;
        }
    }

    /// Toggle the focused pane between floating and tiled.
    pub(super) fn toggle_float_tile(&mut self) {
        self.unzoom_if_needed();
        let Some((tab_id, pane_id)) = self.active_pane_context() else {
            return;
        };

        let is_floating = {
            let Some(tab) = self.session.get_tab(tab_id) else {
                return;
            };
            tab.is_floating(pane_id)
        };

        if is_floating {
            let Some(tab) = self.session.get_tab_mut(tab_id) else {
                return;
            };
            if !tab.floating().contains(pane_id) {
                return;
            }
            let new_layer = tab.floating().remove(pane_id);
            tab.set_floating(new_layer);
            let anchor = tab.tree().first_pane();
            let new_tree = tab
                .tree()
                .split_at(anchor, SplitDirection::Vertical, pane_id, 0.5);
            tab.set_tree(new_tree);
            tab.set_active_pane(pane_id);
        } else {
            let Some(avail) = self.grid_available_rect() else {
                return;
            };
            let Some(tab) = self.session.get_tab_mut(tab_id) else {
                return;
            };
            if !tab.tree().contains(pane_id) {
                return;
            }
            let Some(new_tree) = tab.tree().remove(pane_id) else {
                return;
            };
            tab.set_tree(new_tree);
            let next_z = tab
                .floating()
                .panes()
                .iter()
                .map(|p| p.z_order)
                .max()
                .unwrap_or(0)
                + 1;
            let fp = crate::session::FloatingPane::centered(pane_id, &avail, next_z);
            let new_layer = tab.floating().add(fp);
            tab.set_floating(new_layer);
            tab.set_active_pane(pane_id);
        }

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.pane_cache.invalidate_all();
            ctx.dirty = true;
        }
    }

    /// Undo the last split tree mutation.
    pub(super) fn undo_split(&mut self) {
        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };
        let live_panes = self.live_pane_ids(tab_id);
        let applied = self
            .session
            .get_tab_mut(tab_id)
            .is_some_and(|tab| tab.undo_tree(&live_panes));
        if applied {
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.pane_cache.invalidate_all();
                ctx.dirty = true;
            }
        }
    }

    /// Redo the last undone split tree mutation.
    pub(super) fn redo_split(&mut self) {
        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };
        let live_panes = self.live_pane_ids(tab_id);
        let applied = self
            .session
            .get_tab_mut(tab_id)
            .is_some_and(|tab| tab.redo_tree(&live_panes));
        if applied {
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.pane_cache.invalidate_all();
                ctx.dirty = true;
            }
        }
    }

    /// Raise a floating pane when it receives focus via click.
    pub(super) fn raise_if_floating(&mut self, pane_id: PaneId) {
        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };
        if let Some(tab) = self.session.get_tab_mut(tab_id) {
            if tab.is_floating(pane_id) {
                let new_layer = tab.floating().raise(pane_id);
                tab.set_floating(new_layer);
            }
        }
    }
}
