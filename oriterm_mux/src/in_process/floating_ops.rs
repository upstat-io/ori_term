//! Floating pane operations for `InProcessMux`.
//!
//! Spawn, move, resize, raise, and tile/float transitions for floating panes.
//! Separated from the main CRUD operations to keep `mod.rs` under 500 lines.

use std::io;
use std::sync::Arc;

use crate::domain::SpawnConfig;
use crate::layout::Rect;
use crate::layout::floating::FloatingPane;
use crate::{PaneId, TabId};
use oriterm_core::Theme;

use super::InProcessMux;
use crate::mux_event::MuxNotification;
use crate::pane::Pane;

impl InProcessMux {
    /// Spawn a new floating pane centered in the available area.
    ///
    /// Returns `(PaneId, Pane)` — the caller stores the `Pane` in its own map.
    #[expect(
        clippy::too_many_arguments,
        reason = "floating spawn requires tab + config + theme + proxy + available rect"
    )]
    pub fn spawn_floating_pane(
        &mut self,
        tab_id: TabId,
        config: &SpawnConfig,
        theme: Theme,
        wakeup: &Arc<dyn Fn() + Send + Sync>,
        available: &Rect,
    ) -> io::Result<(PaneId, Pane)> {
        let (pane_id, pane) = self.spawn_pane(tab_id, config, theme, wakeup)?;

        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            self.pane_registry.unregister(pane_id);
            return Err(io::Error::other("tab not found after spawn"));
        };

        let next_z = tab
            .floating()
            .panes()
            .iter()
            .map(|p| p.z_order)
            .max()
            .unwrap_or(0)
            + 1;
        let fp = FloatingPane::centered(pane_id, available, next_z);
        let new_layer = tab.floating().add(fp);
        tab.set_floating(new_layer);
        tab.set_active_pane(pane_id);

        self.notifications
            .push(MuxNotification::TabLayoutChanged(tab_id));

        Ok((pane_id, pane))
    }

    /// Move a tiled pane into the floating layer.
    ///
    /// Removes the pane from the split tree and inserts it as a centered
    /// floating pane. Fails silently if the pane is the last tiled pane
    /// (must keep at least one in the tree).
    pub fn move_pane_to_floating(
        &mut self,
        tab_id: TabId,
        pane_id: PaneId,
        available: &Rect,
    ) -> bool {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return false;
        };

        // Don't allow floating a pane that isn't in the tiled tree
        // (already floating or not present), nor the last tiled pane.
        if !tab.tree().contains(pane_id) {
            return false;
        }
        let Some(new_tree) = tab.tree().remove(pane_id) else {
            return false;
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
        let fp = FloatingPane::centered(pane_id, available, next_z);
        let new_layer = tab.floating().add(fp);
        tab.set_floating(new_layer);
        tab.set_active_pane(pane_id);

        self.notifications
            .push(MuxNotification::TabLayoutChanged(tab_id));
        true
    }

    /// Move a floating pane back into the tiled split tree.
    ///
    /// Removes the pane from the floating layer and inserts it as a sibling
    /// of the given tiled pane (or the first tiled pane if none specified).
    pub fn move_pane_to_tiled(&mut self, tab_id: TabId, pane_id: PaneId) -> bool {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return false;
        };

        if !tab.floating().contains(pane_id) {
            return false;
        }

        let new_layer = tab.floating().remove(pane_id);
        tab.set_floating(new_layer);

        // Split next to the first tiled pane.
        let anchor = tab.tree().first_pane();
        let new_tree = tab.tree().split_at(
            anchor,
            crate::layout::SplitDirection::Vertical,
            pane_id,
            0.5,
        );
        tab.set_tree(new_tree);
        tab.set_active_pane(pane_id);

        self.notifications
            .push(MuxNotification::TabLayoutChanged(tab_id));
        true
    }

    /// Move a floating pane to a new position.
    ///
    /// Uses in-place mutation and emits `FloatingPaneChanged` (lightweight,
    /// no PTY resize) instead of `TabLayoutChanged`.
    pub fn move_floating_pane(&mut self, tab_id: TabId, pane_id: PaneId, x: f32, y: f32) {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return;
        };
        tab.floating_mut().move_pane_mut(pane_id, x, y);
        self.notifications
            .push(MuxNotification::FloatingPaneChanged(tab_id));
    }

    /// Resize a floating pane to new dimensions.
    ///
    /// Uses in-place mutation and emits `FloatingPaneChanged` (lightweight,
    /// no PTY resize) instead of `TabLayoutChanged`. PTY resize is deferred
    /// to drag finish.
    pub fn resize_floating_pane(
        &mut self,
        tab_id: TabId,
        pane_id: PaneId,
        width: f32,
        height: f32,
    ) {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return;
        };
        tab.floating_mut().resize_pane_mut(pane_id, width, height);
        self.notifications
            .push(MuxNotification::FloatingPaneChanged(tab_id));
    }

    /// Resize and move a floating pane in one call with one notification.
    ///
    /// Used during edge/corner resize drags where both position and size
    /// change. Avoids emitting two separate notifications per mouse move.
    pub fn set_floating_pane_rect(&mut self, tab_id: TabId, pane_id: PaneId, rect: Rect) {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return;
        };
        tab.floating_mut().set_pane_rect_mut(pane_id, rect);
        self.notifications
            .push(MuxNotification::FloatingPaneChanged(tab_id));
    }

    /// Bring a floating pane to the front (highest z-order).
    pub fn raise_floating_pane(&mut self, tab_id: TabId, pane_id: PaneId) {
        let Some(tab) = self.session.get_tab_mut(tab_id) else {
            return;
        };
        let new_layer = tab.floating().raise(pane_id);
        tab.set_floating(new_layer);
        self.notifications
            .push(MuxNotification::FloatingPaneChanged(tab_id));
    }
}
