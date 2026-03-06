//! Private helpers for pane operations.
//!
//! Extracted from `mod.rs` to keep the module under the 500-line limit.

use oriterm_mux::PaneId;

use crate::session::{Rect, SplitDirection};

use crate::session::TabId;

use super::super::App;

impl App {
    /// Resolve `(tab_id, active_pane_id)` for the current active tab.
    pub(in crate::app) fn active_pane_context(&self) -> Option<(TabId, PaneId)> {
        let win_id = self.active_window?;
        let win = self.session.get_window(win_id)?;
        let tab_id = win.active_tab()?;
        let tab = self.session.get_tab(tab_id)?;
        Some((tab_id, tab.active_pane()))
    }

    /// Estimate rows/cols for a new pane after splitting.
    ///
    /// Uses half the source pane's dimensions in the split direction.
    /// The actual size is refined when layout computes the real rects.
    pub(super) fn estimate_split_size(
        &self,
        source: PaneId,
        direction: SplitDirection,
    ) -> (u16, u16) {
        let Some(snapshot) = self.mux.as_ref().and_then(|m| m.pane_snapshot(source)) else {
            return (24, 80);
        };
        let rows = snapshot.cells.len() as u16;
        let cols = snapshot.cols;

        match direction {
            SplitDirection::Horizontal => (rows / 2, cols),
            SplitDirection::Vertical => (rows, cols / 2),
        }
    }

    /// Compute pane layouts for the current tab (flat list for navigation).
    pub(super) fn current_pane_layouts(&self) -> Option<Vec<crate::session::PaneLayout>> {
        self.compute_pane_layouts().map(|(layouts, _)| layouts)
    }

    /// Collect all live pane IDs for a given tab.
    pub(super) fn live_pane_ids(&self, tab_id: TabId) -> std::collections::HashSet<PaneId> {
        let Some(tab) = self.session.get_tab(tab_id) else {
            return std::collections::HashSet::new();
        };
        tab.all_panes().into_iter().collect()
    }

    /// Clear zoom on the active tab if currently zoomed.
    ///
    /// Returns `true` if zoom was actually cleared. Callers that don't
    /// mutate layout (focus, cycle) must handle the layout change
    /// themselves when this returns `true`.
    pub(super) fn unzoom_if_needed(&mut self) -> bool {
        let Some((tab_id, _)) = self.active_pane_context() else {
            return false;
        };
        let was_zoomed = self
            .session
            .get_tab(tab_id)
            .is_some_and(|t| t.zoomed_pane().is_some());
        if was_zoomed {
            if let Some(tab) = self.session.get_tab_mut(tab_id) {
                tab.set_zoomed_pane(None);
            }
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.cached_dividers = None;
            }
            self.sync_tab_bar_from_mux();
        }
        was_zoomed
    }

    /// Get the available grid area as a mux layout `Rect`.
    pub(in crate::app) fn grid_available_rect(&self) -> Option<Rect> {
        let bounds = self.focused_ctx()?.terminal_grid.bounds()?;
        Some(Rect {
            x: bounds.x(),
            y: bounds.y(),
            width: bounds.width(),
            height: bounds.height(),
        })
    }

    /// Set the focused pane and mark dirty.
    pub(super) fn set_focused_pane(&mut self, pane_id: PaneId) {
        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };
        if let Some(tab) = self.session.get_tab_mut(tab_id) {
            tab.set_active_pane(pane_id);
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.pane_cache.invalidate_all();
            ctx.dirty = true;
        }
    }
}
