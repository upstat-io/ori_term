//! Tab lifecycle — create, close, duplicate, cycle, reorder.
//!
//! All operations go through the mux layer (flat pane server). The GUI
//! session owns tab/window/layout state; the App owns rendering state
//! (tab bar layout, animation offsets).

mod move_ops;

use std::path::PathBuf;

use winit::window::WindowId;

use oriterm_mux::domain::SpawnConfig;

use crate::session::{TabId, WindowId as SessionWindowId};

use super::App;

impl App {
    /// Create a new tab in the given window.
    ///
    /// Inherits CWD from the active pane in the current tab. Spawns a
    /// pane via the mux, then creates a local tab and registers it in
    /// the session.
    pub(super) fn new_tab_in_window(&mut self, window_id: SessionWindowId) {
        let cwd = self
            .active_pane_id()
            .and_then(|id| self.mux.as_ref()?.pane_cwd(id))
            .map(PathBuf::from);

        let (rows, cols) = self.current_grid_dims();
        let theme = self
            .config
            .colors
            .resolve_theme(crate::platform::theme::system_theme);

        let config = SpawnConfig {
            cols,
            rows,
            scrollback: self.config.terminal.scrollback,
            shell_integration: self.config.behavior.shell_integration,
            cwd,
            ..SpawnConfig::default()
        };

        let palette =
            crate::app::config_reload::build_palette_from_config(&self.config.colors, theme);

        let Some(mux) = &mut self.mux else { return };
        let pane_id = match mux.spawn_pane(&config, theme) {
            Ok(pid) => {
                mux.set_pane_theme(pid, theme, palette);
                mux.set_image_config(pid, self.config.terminal.image_config());
                pid
            }
            Err(e) => {
                log::error!("new tab failed: {e}");
                return;
            }
        };

        // Local tab creation.
        let tab_id = self.session.alloc_tab_id();
        let tab = crate::session::Tab::new(tab_id, pane_id);
        self.session.add_tab(tab);
        if let Some(win) = self.session.get_window_mut(window_id) {
            win.add_tab(tab_id);
        }
        log::info!("new tab {tab_id:?} with pane {pane_id:?} in window {window_id:?}");

        self.release_tab_width_lock();
        self.sync_tab_bar_from_mux();
        if let Some(wid) = self.focused_window_id {
            self.refresh_platform_rects(wid);
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Close a tab and all its panes.
    ///
    /// If this was the last tab in the last window, shuts down immediately
    /// (ConPTY-safe: `process::exit` before dropping panes). If this was the
    /// last tab in a non-last window, the empty window is closed too.
    /// Otherwise pane cleanup happens via `PaneClosed` notifications in
    /// `pump_mux_events`.
    pub(super) fn close_tab(&mut self, tab_id: TabId) {
        // Capture slide animation data before mutations.
        let slide_info = self.capture_close_slide_info(tab_id);

        let is_last = self.session.tab_count() <= 1;
        let owner_window = self.session.window_for_tab(tab_id);

        // Collect pane IDs from local session before removing the tab.
        let pane_ids: Vec<oriterm_mux::PaneId> = self
            .session
            .get_tab(tab_id)
            .map(crate::session::Tab::all_panes)
            .unwrap_or_default();

        // Close each pane through the mux (unregisters from pane registry,
        // emits PaneClosed for cleanup in pump_mux_events).
        if let Some(mux) = &mut self.mux {
            for &pid in &pane_ids {
                mux.close_pane(pid);
            }
        }

        // Remove tab from local session.
        self.session.remove_tab(tab_id);
        if let Some(wid) = owner_window {
            if let Some(win) = self.session.get_window_mut(wid) {
                win.remove_tab(tab_id);
            }
        }

        if is_last {
            log::info!("last tab closed, shutting down");
            self.exit_app();
        }

        // If the owning window is now empty (last tab in a non-last window),
        // close it. This handles torn-off windows and multi-window setups.
        if let Some(win_id) = owner_window {
            let window_empty = self
                .session
                .get_window(win_id)
                .is_some_and(|w| w.tabs().is_empty());
            if window_empty {
                self.close_empty_session_window(win_id);
                return;
            }
        }

        self.sync_tab_bar_from_mux();
        if let Some(wid) = self.focused_window_id {
            self.refresh_platform_rects(wid);
        }

        // Start slide animation for displaced tabs (skip if last tab).
        if let Some((closed_idx, tab_width)) = slide_info {
            self.start_tab_close_slide(closed_idx, tab_width);
        }

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Close the currently active tab.
    pub(super) fn close_active_tab(&mut self) {
        let Some(tab_id) = self.active_tab_id() else {
            return;
        };
        self.close_tab(tab_id);
    }

    /// Close the tab at a specific index in the active window.
    ///
    /// Used by tab bar close-button clicks. Resolves the tab ID from the
    /// index and delegates to `close_tab`.
    pub(super) fn close_tab_at_index(&mut self, index: usize) {
        let tab_id = {
            let Some(win_id) = self.active_window else {
                return;
            };
            let Some(win) = self.session.get_window(win_id) else {
                return;
            };
            match win.tabs().get(index).copied() {
                Some(id) => id,
                None => return,
            }
        };
        self.close_tab(tab_id);
    }

    /// Duplicate the active tab (new shell in the same CWD).
    pub(super) fn duplicate_active_tab(&mut self) {
        let Some(window_id) = self.active_window else {
            return;
        };
        self.new_tab_in_window(window_id);
    }

    /// Cycle to the next or previous tab in the active window.
    pub(super) fn cycle_tab(&mut self, delta: isize) {
        let Some(win_id) = self.active_window else {
            return;
        };
        let cycled = {
            let Some(win) = self.session.get_window_mut(win_id) else {
                return;
            };
            let count = win.tabs().len();
            if count == 0 {
                return;
            }
            let current = win.active_tab_idx();
            let new_idx = (current as isize + delta).rem_euclid(count as isize) as usize;
            if new_idx == current {
                return;
            }
            win.set_active_tab_idx(new_idx);
            true
        };
        if !cycled {
            return;
        }

        // Clear bell badge on the newly active pane.
        if let Some(id) = self.active_pane_id() {
            if let Some(mux) = self.mux.as_mut() {
                mux.clear_bell(id);
            }
        }

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.pane_cache.invalidate_all();
            ctx.cached_dividers = None;
            ctx.dirty = true;
        }
        self.resize_all_panes();
        self.sync_tab_bar_from_mux();
    }

    /// Switch to a specific tab by its ID.
    pub(super) fn switch_to_tab(&mut self, tab_id: TabId) {
        let Some(win_id) = self.active_window else {
            return;
        };
        {
            let Some(win) = self.session.get_window_mut(win_id) else {
                return;
            };
            let Some(idx) = win.tabs().iter().position(|&id| id == tab_id) else {
                return;
            };
            win.set_active_tab_idx(idx);
        }

        if let Some(id) = self.active_pane_id() {
            if let Some(mux) = self.mux.as_mut() {
                mux.clear_bell(id);
            }
        }

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.pane_cache.invalidate_all();
            ctx.cached_dividers = None;
            ctx.dirty = true;
        }
        self.resize_all_panes();
        self.sync_tab_bar_from_mux();
    }

    /// Switch to a tab by its index in the active window.
    pub(super) fn switch_to_tab_index(&mut self, index: usize) {
        let Some(win_id) = self.active_window else {
            return;
        };
        let tab_id = {
            let Some(win) = self.session.get_window(win_id) else {
                return;
            };
            match win.tabs().get(index).copied() {
                Some(id) => id,
                None => return,
            }
        };
        self.switch_to_tab(tab_id);
    }

    // -- Private helpers --

    /// Captures the tab index and width for a close slide animation.
    ///
    /// Returns `None` if the tab or window context cannot be resolved.
    fn capture_close_slide_info(&self, tab_id: TabId) -> Option<(usize, f32)> {
        let win_id = self.active_window?;
        let win = self.session.get_window(win_id)?;
        let idx = win.tabs().iter().position(|&id| id == tab_id)?;
        let tab_width = self.focused_ctx()?.tab_bar.layout().base_tab_width();
        Some((idx, tab_width))
    }

    /// Starts a close-slide animation and syncs offsets to the widget.
    fn start_tab_close_slide(&mut self, closed_idx: usize, tab_width: f32) {
        use oriterm_ui::widgets::tab_bar::slide::SlideContext;

        let now = std::time::Instant::now();
        let Some(ctx) = self.focused_ctx_mut() else {
            return;
        };
        let tab_count = ctx.tab_bar.tab_count();
        let mut cx = SlideContext {
            tree: &mut ctx.layer_tree,
            animator: &mut ctx.layer_animator,
            now,
        };
        ctx.tab_slide
            .start_close_slide(closed_idx, tab_width, tab_count, &mut cx);
        ctx.tab_slide
            .sync_to_widget(tab_count, &ctx.layer_tree, &mut ctx.tab_bar);
    }

    /// Starts a reorder-slide animation and syncs offsets to the widget.
    pub(super) fn start_tab_reorder_slide(&mut self, from: usize, to: usize, tab_width: f32) {
        use oriterm_ui::widgets::tab_bar::slide::SlideContext;

        let now = std::time::Instant::now();
        let Some(ctx) = self.focused_ctx_mut() else {
            return;
        };
        let tab_count = ctx.tab_bar.tab_count();
        let mut cx = SlideContext {
            tree: &mut ctx.layer_tree,
            animator: &mut ctx.layer_animator,
            now,
        };
        ctx.tab_slide
            .start_reorder_slide(from, to, tab_width, &mut cx);
        ctx.tab_slide
            .sync_to_widget(tab_count, &ctx.layer_tree, &mut ctx.tab_bar);
    }

    /// The active tab ID for the active window.
    fn active_tab_id(&self) -> Option<TabId> {
        let win_id = self.active_window?;
        self.session.get_window(win_id)?.active_tab()
    }

    /// Current grid dimensions (rows, cols) from the grid widget.
    fn current_grid_dims(&self) -> (u16, u16) {
        self.focused_ctx().map_or((24, 80), |ctx| {
            (
                ctx.terminal_grid.rows() as u16,
                ctx.terminal_grid.cols() as u16,
            )
        })
    }

    /// Rebuild the tab bar entries from the mux's window state.
    ///
    /// Reads all tabs in the active window, builds `TabEntry` list with
    /// titles from each tab's active pane, and sets the active index.
    pub(super) fn sync_tab_bar_from_mux(&mut self) {
        let Some(mux) = self.mux.as_ref() else { return };
        let Some(win_id) = self.active_window else {
            return;
        };
        let Some(win) = self.session.get_window(win_id) else {
            return;
        };

        let (entries, active_idx) = build_tab_entries(mux.as_ref(), &self.session, win);

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.tab_bar.set_tabs(entries);
            ctx.tab_bar.set_active_index(active_idx);
        }
    }

    /// Rebuild the tab bar for a specific window by its winit ID.
    ///
    /// Like [`sync_tab_bar_from_mux`] but targets a specific window instead
    /// of the active window. Used by tear-off/merge when both source and
    /// destination windows need their tab bars updated.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(super) fn sync_tab_bar_for_window(&mut self, winit_id: WindowId) {
        let session_wid = {
            let Some(ctx) = self.windows.get(&winit_id) else {
                return;
            };
            ctx.window.session_window_id()
        };
        let Some(mux) = self.mux.as_ref() else {
            return;
        };
        let Some(win) = self.session.get_window(session_wid) else {
            return;
        };

        let (entries, active_idx) = build_tab_entries(mux.as_ref(), &self.session, win);

        if let Some(ctx) = self.windows.get_mut(&winit_id) {
            ctx.tab_bar.set_tabs(entries);
            ctx.tab_bar.set_active_index(active_idx);
        }
    }
}

/// Build tab bar entries from a session window's tab list.
///
/// Returns `(entries, active_tab_index)`. Shared by both
/// `sync_tab_bar_from_mux` and `sync_tab_bar_for_window`.
fn build_tab_entries(
    mux: &dyn oriterm_mux::backend::MuxBackend,
    session: &crate::session::SessionRegistry,
    win: &crate::session::Window,
) -> (Vec<oriterm_ui::widgets::tab_bar::TabEntry>, usize) {
    let active_idx = win.active_tab_idx();
    let entries = win
        .tabs()
        .iter()
        .map(|&tab_id| {
            let tab = session.get_tab(tab_id);
            let pane_id = tab.map(crate::session::Tab::active_pane);
            let snapshot = pane_id.and_then(|pid| mux.pane_snapshot(pid));
            let mut title = snapshot.map(|s| s.title.clone()).unwrap_or_default();
            let icon = snapshot
                .and_then(|s| s.icon_name.as_deref())
                .and_then(oriterm_ui::widgets::tab_bar::extract_emoji_icon);
            // Strip leading emoji from title when it matches the icon
            // (OSC 0 sets both title and icon_name to the same string).
            if let Some(oriterm_ui::widgets::tab_bar::TabIcon::Emoji(ref e)) = icon {
                let stripped = title
                    .strip_prefix(e.as_str())
                    .map(|r| r.trim_start().to_owned());
                if let Some(s) = stripped {
                    title = s;
                }
            }
            let is_zoomed = tab.is_some_and(|t| t.zoomed_pane().is_some());
            let display = if is_zoomed {
                format!("{title} [Z]")
            } else {
                title
            };
            oriterm_ui::widgets::tab_bar::TabEntry::new(display).with_icon(icon)
        })
        .collect();
    (entries, active_idx)
}

/// Wrapping index arithmetic for tab cycling.
#[cfg(test)]
fn wrap_index(current: usize, delta: isize, count: usize) -> usize {
    let c = count as isize;
    let next = (current as isize + delta).rem_euclid(c);
    next as usize
}

#[cfg(test)]
mod tests;
