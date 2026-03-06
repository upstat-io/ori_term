//! Floating pane toggle and focus operations.
//!
//! Extracted from `pane_ops/mod.rs` to keep file sizes under the 500-line limit.

use oriterm_mux::{PaneId, SpawnConfig};

use crate::session::SplitDirection;

use super::super::App;

impl App {
    /// Toggle floating pane: focus topmost if any exist, else spawn a new one.
    pub(in crate::app) fn toggle_floating_pane(&mut self) {
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
    pub(in crate::app) fn toggle_float_tile(&mut self) {
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

    /// Raise a floating pane when it receives focus via click.
    pub(in crate::app) fn raise_if_floating(&mut self, pane_id: PaneId) {
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
