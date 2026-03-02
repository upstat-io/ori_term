//! Pane operations: split, focus, cycle, close, resize.
//!
//! App-level methods that bridge keybinding actions to the mux layer.
//! Each operation reads mux state, calls the appropriate mux method,
//! then triggers layout recomputation and resize propagation.

use oriterm_mux::layout::{Rect, SplitDirection};
use oriterm_mux::nav::Direction;
use oriterm_mux::{PaneId, SpawnConfig, TabId};

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
        let Some(mux) = &mut self.mux else { return };
        mux.toggle_zoom(tab_id);
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.pane_cache.invalidate_all();
            ctx.dirty = true;
        }
        self.sync_tab_bar_from_mux();
    }

    /// Split the focused pane in the given direction.
    ///
    /// Spawns a new pane via the mux, which updates the split tree.
    /// Layout recomputation and pane resize happen on the next
    /// `TabLayoutChanged` notification (emitted by the mux).
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

        let Some(mux) = &mut self.mux else { return };
        match mux.split_pane(
            tab_id,
            source_pane_id,
            direction,
            &config,
            theme,
            &self.mux_wakeup,
        ) {
            Ok((new_pane_id, pane)) => {
                self.apply_palette_to_pane(&pane, theme);
                self.panes.insert(new_pane_id, pane);
                log::info!("split pane: {source_pane_id:?} -> {new_pane_id:?} ({direction:?})");
            }
            Err(e) => {
                log::error!("split pane failed: {e}");
            }
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

        if let Some(target) = oriterm_mux::nav::navigate_wrap(&layouts, pane_id, direction) {
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

        if let Some(target) = oriterm_mux::nav::cycle(&layouts, pane_id, forward) {
            self.set_focused_pane(target);
        }
    }

    /// Close the focused pane.
    ///
    /// If this is the last pane in the last tab, takes the same `shutdown()`
    /// path as `WindowEvent::CloseRequested` (the window close button). A
    /// future confirmation dialog only needs to gate `shutdown()`.
    ///
    /// For non-last panes, the mux emits `PaneClosed` and `TabLayoutChanged`
    /// notifications which handle cleanup in `pump_mux_events`.
    pub(super) fn close_focused_pane(&mut self) {
        let Some(pane_id) = self.active_pane_id() else {
            return;
        };
        let Some(mux) = &mut self.mux else { return };

        // Last pane? Same path as the close button.
        if mux.is_last_pane(pane_id) {
            self.exit_app();
        }
        let result = mux.close_pane(pane_id);
        log::info!("close pane {pane_id:?}: {result:?}");
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Resize all panes in the active tab to match their computed layouts.
    ///
    /// Called after layout changes (split, close, window resize) to ensure
    /// each pane's terminal grid and PTY match their pixel allocation.
    /// For single-pane tabs, resizes the active pane to fill the full grid
    /// area (the layout engine returns `None` for single-pane tabs).
    pub(super) fn resize_all_panes(&self) {
        let Some((layouts, _)) = self.compute_pane_layouts() else {
            // Single pane — resize it to fill the full grid area.
            self.resize_single_pane();
            return;
        };
        for layout in &layouts {
            if let Some(pane) = self.panes.get(&layout.pane_id) {
                pane.resize_grid(layout.rows, layout.cols);
                pane.resize_pty(layout.rows, layout.cols);
            }
        }
    }

    /// Resize the single active pane to fill the full grid area.
    ///
    /// Computes rows/cols from the grid bounds and cell metrics, matching
    /// the same calculation `sync_grid_layout` uses during window resize.
    fn resize_single_pane(&self) {
        let Some(ctx) = self.focused_ctx() else {
            return;
        };
        let Some(bounds) = ctx.terminal_grid.bounds() else {
            return;
        };
        let Some(renderer) = &self.renderer else {
            return;
        };
        let cell = renderer.cell_metrics();
        let cols = cell.columns(bounds.width() as u32).max(1) as u16;
        let rows = cell.rows(bounds.height() as u32).max(1) as u16;
        if let Some(pane) = self.active_pane() {
            pane.resize_grid(rows, cols);
            pane.resize_pty(rows, cols);
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
        let Some(target) = oriterm_mux::nav::nearest_pane(&layouts, pos.x as f32, pos.y as f32)
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
        let Some(mux) = &mut self.mux else { return };
        mux.resize_pane(tab_id, pane_id, axis, pane_in_first, delta);
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
        let Some(mux) = &mut self.mux else { return };
        mux.equalize_panes(tab_id);
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

        // Single borrow: decide whether to focus an existing pane or spawn new.
        let focus_target = {
            let Some(mux) = self.mux.as_ref() else { return };
            let Some(tab) = mux.session().get_tab(tab_id) else {
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

        let Some(mux) = &mut self.mux else { return };
        match mux.spawn_floating_pane(tab_id, &config, theme, &self.mux_wakeup, &available) {
            Ok((new_pane_id, pane)) => {
                self.apply_palette_to_pane(&pane, theme);
                self.panes.insert(new_pane_id, pane);
                log::info!("spawn floating pane: {new_pane_id:?}");
            }
            Err(e) => {
                log::error!("spawn floating pane failed: {e}");
            }
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
            let Some(mux) = self.mux.as_ref() else { return };
            let Some(tab) = mux.session().get_tab(tab_id) else {
                return;
            };
            tab.is_floating(pane_id)
        };

        // Compute available rect before borrowing mux mutably.
        let available = self.grid_available_rect();

        let Some(mux) = &mut self.mux else { return };
        if is_floating {
            mux.move_pane_to_tiled(tab_id, pane_id);
        } else if let Some(ref avail) = available {
            mux.move_pane_to_floating(tab_id, pane_id, avail);
        } else {
            return;
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
        let Some(mux) = &mut self.mux else { return };
        if mux.undo_split(tab_id, &live_panes) {
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
        let Some(mux) = &mut self.mux else { return };
        if mux.redo_split(tab_id, &live_panes) {
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
        let is_floating = {
            let Some(mux) = self.mux.as_ref() else { return };
            let Some(tab) = mux.session().get_tab(tab_id) else {
                return;
            };
            tab.is_floating(pane_id)
        };
        if is_floating {
            let Some(mux) = &mut self.mux else { return };
            mux.raise_floating_pane(tab_id, pane_id);
        }
    }

    // -- Private helpers --

    /// Resolve `(tab_id, active_pane_id)` for the current active tab.
    pub(super) fn active_pane_context(&self) -> Option<(TabId, PaneId)> {
        let mux = self.mux.as_ref()?;
        let win_id = self.active_window?;
        let tab_id = mux.active_tab_id(win_id)?;
        let tab = mux.session().get_tab(tab_id)?;
        Some((tab_id, tab.active_pane()))
    }

    /// Estimate rows/cols for a new pane after splitting.
    ///
    /// Uses half the source pane's dimensions in the split direction.
    /// The actual size is refined when layout computes the real rects.
    fn estimate_split_size(&self, source: PaneId, direction: SplitDirection) -> (u16, u16) {
        let Some(pane) = self.panes.get(&source) else {
            return (24, 80);
        };
        let term = pane.terminal().lock();
        let grid = term.grid();
        let rows = grid.lines() as u16;
        let cols = grid.cols() as u16;
        drop(term);

        match direction {
            SplitDirection::Horizontal => (rows / 2, cols),
            SplitDirection::Vertical => (rows, cols / 2),
        }
    }

    /// Compute pane layouts for the current tab (flat list for navigation).
    fn current_pane_layouts(&self) -> Option<Vec<oriterm_mux::layout::PaneLayout>> {
        self.compute_pane_layouts().map(|(layouts, _)| layouts)
    }

    /// Collect all live pane IDs for a given tab.
    fn live_pane_ids(&self, tab_id: TabId) -> std::collections::HashSet<PaneId> {
        let Some(mux) = self.mux.as_ref() else {
            return std::collections::HashSet::new();
        };
        let Some(tab) = mux.session().get_tab(tab_id) else {
            return std::collections::HashSet::new();
        };
        tab.all_panes().into_iter().collect()
    }

    /// Clear zoom on the active tab if currently zoomed.
    ///
    /// Returns `true` if zoom was actually cleared. Uses `unzoom_silent`
    /// so callers that will emit their own `TabLayoutChanged` (split,
    /// resize, equalize) avoid a duplicate notification. Callers that
    /// don't mutate layout (focus, cycle) must handle the layout change
    /// themselves when this returns `true`.
    fn unzoom_if_needed(&mut self) -> bool {
        let Some((tab_id, _)) = self.active_pane_context() else {
            return false;
        };
        let Some(mux) = &mut self.mux else {
            return false;
        };
        let Some(tab) = mux.session().get_tab(tab_id) else {
            return false;
        };
        let was_zoomed = tab.zoomed_pane().is_some();
        if was_zoomed {
            mux.unzoom_silent(tab_id);
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.cached_dividers = None;
            }
            self.sync_tab_bar_from_mux();
        }
        was_zoomed
    }

    /// Get the available grid area as a mux layout `Rect`.
    pub(super) fn grid_available_rect(&self) -> Option<Rect> {
        let bounds = self.focused_ctx()?.terminal_grid.bounds()?;
        Some(Rect {
            x: bounds.x(),
            y: bounds.y(),
            width: bounds.width(),
            height: bounds.height(),
        })
    }

    /// Set the focused pane in the mux and mark dirty.
    fn set_focused_pane(&mut self, pane_id: PaneId) {
        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };
        let Some(mux) = &mut self.mux else { return };
        mux.set_active_pane(tab_id, pane_id);
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.pane_cache.invalidate_all();
            ctx.dirty = true;
        }
    }
}
