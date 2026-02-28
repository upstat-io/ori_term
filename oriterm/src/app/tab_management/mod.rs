//! Tab lifecycle — create, close, duplicate, cycle, reorder.
//!
//! All operations go through the mux layer. The mux owns tab state
//! (`MuxTab` with `SplitTree`); the App owns rendering state (tab bar
//! layout, animation offsets) and the actual `Pane` structs.

use std::path::PathBuf;

use oriterm_mux::domain::SpawnConfig;
use oriterm_mux::{TabId, WindowId as MuxWindowId};

use super::App;

impl App {
    /// Create a new tab in the given mux window.
    ///
    /// Inherits CWD from the active pane in the current tab. Applies the
    /// color palette, syncs the tab bar, and clears the width lock.
    pub(super) fn new_tab_in_window(&mut self, window_id: MuxWindowId) {
        let cwd = self.active_pane().and_then(|p| p.cwd().map(PathBuf::from));

        let (rows, cols) = self.current_grid_dims();
        let theme = self
            .config
            .colors
            .resolve_theme(crate::platform::theme::system_theme);

        let config = SpawnConfig {
            cols,
            rows,
            scrollback: self.config.terminal.scrollback,
            cwd,
            ..SpawnConfig::default()
        };

        let Some(mux) = &mut self.mux else { return };
        match mux.create_tab(window_id, &config, theme, &self.event_proxy) {
            Ok((_tab_id, pane_id, pane)) => {
                self.apply_palette_to_pane(&pane, theme);
                self.panes.insert(pane_id, pane);
                log::info!("new tab with pane {pane_id:?} in window {window_id:?}");
            }
            Err(e) => {
                log::error!("new tab failed: {e}");
                return;
            }
        }
        self.release_tab_width_lock();
        self.sync_tab_bar_from_mux();
        self.dirty = true;
    }

    /// Close a tab and all its panes.
    ///
    /// If this was the last tab in the last window, shuts down immediately
    /// (ConPTY-safe: `process::exit` before dropping panes). Otherwise
    /// drops panes on background threads.
    pub(super) fn close_tab(&mut self, tab_id: TabId) {
        let Some(mux) = &mut self.mux else { return };

        // Check before closing: if the session has only one tab total,
        // closing it will leave zero windows. Must exit *before* dropping
        // Pane structs (ConPTY safety on Windows).
        let is_last = mux.session().tab_count() <= 1;

        let pane_ids = mux.close_tab(tab_id);

        if is_last {
            log::info!("last tab closed, shutting down");
            self.shutdown(0);
        }

        // Drop panes on background threads (ConPTY safety).
        for pid in pane_ids {
            if let Some(pane) = self.panes.remove(&pid) {
                self.pane_cache.remove(pid);
                std::thread::spawn(move || drop(pane));
            }
        }

        self.release_tab_width_lock();
        self.sync_tab_bar_from_mux();
        self.dirty = true;
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
            let Some(mux) = self.mux.as_ref() else { return };
            let Some(win_id) = self.active_window else {
                return;
            };
            let Some(win) = mux.session().get_window(win_id) else {
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
        let Some(mux) = &mut self.mux else { return };
        let Some(win_id) = self.active_window else {
            return;
        };
        if mux.cycle_active_tab(win_id, delta).is_none() {
            return;
        }

        // Clear bell badge on the newly active tab.
        if let Some(pane) = self.active_pane_mut() {
            pane.clear_bell();
        }

        self.pane_cache.invalidate_all();
        self.cached_dividers = None;
        self.resize_all_panes();
        self.sync_tab_bar_from_mux();
        self.dirty = true;
    }

    /// Switch to a specific tab by its ID.
    pub(super) fn switch_to_tab(&mut self, tab_id: TabId) {
        let Some(mux) = &mut self.mux else { return };
        let Some(win_id) = self.active_window else {
            return;
        };
        if !mux.switch_active_tab(win_id, tab_id) {
            return;
        }

        if let Some(pane) = self.active_pane_mut() {
            pane.clear_bell();
        }

        self.pane_cache.invalidate_all();
        self.cached_dividers = None;
        self.resize_all_panes();
        self.sync_tab_bar_from_mux();
        self.dirty = true;
    }

    /// Switch to a tab by its index in the active window.
    pub(super) fn switch_to_tab_index(&mut self, index: usize) {
        let Some(mux) = &mut self.mux else { return };
        let Some(win_id) = self.active_window else {
            return;
        };

        let tab_id = {
            let Some(win) = mux.session().get_window(win_id) else {
                return;
            };
            match win.tabs().get(index).copied() {
                Some(id) => id,
                None => return,
            }
        };

        self.switch_to_tab(tab_id);
    }

    /// Reorder a tab within the active window.
    #[allow(dead_code, reason = "wired to drag-and-drop reorder in Section 17")]
    pub(super) fn move_tab(&mut self, from: usize, to: usize) {
        let Some(mux) = &mut self.mux else { return };
        let Some(win_id) = self.active_window else {
            return;
        };
        if !mux.reorder_tab(win_id, from, to) {
            return;
        }
        self.sync_tab_bar_from_mux();
        self.dirty = true;
    }

    // -- Private helpers --

    /// The active tab ID for the active window.
    fn active_tab_id(&self) -> Option<TabId> {
        let mux = self.mux.as_ref()?;
        let win_id = self.active_window?;
        mux.active_tab_id(win_id)
    }

    /// Current grid dimensions (rows, cols) from the grid widget.
    fn current_grid_dims(&self) -> (u16, u16) {
        self.terminal_grid
            .as_ref()
            .map_or((24, 80), |g| (g.rows() as u16, g.cols() as u16))
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
        let Some(win) = mux.session().get_window(win_id) else {
            return;
        };

        // Collect entries without borrowing self mutably.
        let active_idx = win.active_tab_idx();
        let entries: Vec<oriterm_ui::widgets::tab_bar::TabEntry> = win
            .tabs()
            .iter()
            .map(|&tab_id| {
                let tab = mux.session().get_tab(tab_id);
                let pane_id = tab.map(oriterm_mux::session::MuxTab::active_pane);
                let title = pane_id
                    .and_then(|pid| self.panes.get(&pid))
                    .map(|p| p.title().to_owned())
                    .unwrap_or_default();
                let is_zoomed = tab.is_some_and(|t| t.zoomed_pane().is_some());
                let display = if is_zoomed {
                    format!("{title} [Z]")
                } else {
                    title
                };
                oriterm_ui::widgets::tab_bar::TabEntry::new(display)
            })
            .collect();

        if let Some(tab_bar) = &mut self.tab_bar {
            tab_bar.set_tabs(entries);
            tab_bar.set_active_index(active_idx);
        }
    }
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
