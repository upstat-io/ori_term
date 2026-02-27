//! Pane operations: split, focus, cycle, close, resize.
//!
//! App-level methods that bridge keybinding actions to the mux layer.
//! Each operation reads mux state, calls the appropriate mux method,
//! then triggers layout recomputation and resize propagation.

use oriterm_mux::layout::SplitDirection;
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
        self.pane_cache.invalidate_all();
        self.dirty = true;
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
            ..SpawnConfig::default()
        };

        let Some(mux) = &mut self.mux else { return };
        match mux.split_pane(
            tab_id,
            source_pane_id,
            direction,
            &config,
            theme,
            &self.event_proxy,
        ) {
            Ok((new_pane_id, pane)) => {
                // Apply palette to the new pane's terminal.
                {
                    let mut term = pane.terminal().lock();
                    let palette =
                        super::config_reload::build_palette_from_config(&self.config.colors, theme);
                    *term.palette_mut() = palette;
                }
                self.panes.insert(new_pane_id, pane);
                log::info!("split pane: {source_pane_id:?} -> {new_pane_id:?} ({direction:?})");
            }
            Err(e) => {
                log::error!("split pane failed: {e}");
            }
        }
        self.dirty = true;
    }

    /// Move focus to a pane in the given direction.
    pub(super) fn focus_pane_direction(&mut self, direction: Direction) {
        self.unzoom_if_needed();
        let layouts = match self.current_pane_layouts() {
            Some(l) => l,
            None => return,
        };
        let Some(pane_id) = self.active_pane_id() else {
            return;
        };

        if let Some(target) = oriterm_mux::nav::navigate(&layouts, pane_id, direction) {
            self.set_focused_pane(target);
        }
    }

    /// Cycle to the next or previous pane.
    pub(super) fn cycle_pane(&mut self, forward: bool) {
        self.unzoom_if_needed();
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
    /// The mux emits `PaneClosed` and `TabLayoutChanged` notifications
    /// which handle cleanup and layout recomputation in `pump_mux_events`.
    pub(super) fn close_focused_pane(&mut self) {
        let Some(pane_id) = self.active_pane_id() else {
            return;
        };
        let Some(mux) = &mut self.mux else { return };

        let result = mux.close_pane(pane_id);
        log::info!("close pane {pane_id:?}: {result:?}");
        self.dirty = true;
    }

    /// Resize all panes in the active tab to match their computed layouts.
    ///
    /// Called after layout changes (split, close, window resize) to ensure
    /// each pane's terminal grid and PTY match their pixel allocation.
    pub(super) fn resize_all_panes(&self) {
        let Some((layouts, _)) = self.compute_pane_layouts() else {
            // Single pane — sync_grid_layout handles this case.
            return;
        };
        for layout in &layouts {
            if let Some(pane) = self.panes.get(&layout.pane_id) {
                pane.resize_grid(layout.rows, layout.cols);
                pane.resize_pty(layout.rows, layout.cols);
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
        let Some(target) = oriterm_mux::nav::nearest_pane(&layouts, pos.x as f32, pos.y as f32)
        else {
            return;
        };
        let Some(current) = self.active_pane_id() else {
            return;
        };
        if target != current {
            self.set_focused_pane(target);
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
        self.dirty = true;
    }

    /// Reset all split ratios in the active tab to 0.5.
    pub(super) fn equalize_panes(&mut self) {
        self.unzoom_if_needed();
        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };
        let Some(mux) = &mut self.mux else { return };
        mux.equalize_panes(tab_id);
        self.dirty = true;
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

    /// Clear zoom on the active tab if currently zoomed.
    ///
    /// Called at the top of operations that should auto-unzoom (split,
    /// navigate, cycle, resize, equalize).
    fn unzoom_if_needed(&mut self) {
        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };
        let Some(mux) = &mut self.mux else { return };
        mux.unzoom(tab_id);
    }

    /// Set the focused pane in the mux and mark dirty.
    fn set_focused_pane(&mut self, pane_id: PaneId) {
        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };
        let Some(mux) = &mut self.mux else { return };
        mux.set_active_pane(tab_id, pane_id);
        self.pane_cache.invalidate_all();
        self.dirty = true;
    }
}
