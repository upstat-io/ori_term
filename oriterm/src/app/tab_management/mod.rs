//! Tab lifecycle — create, close, duplicate, cycle, reorder.
//!
//! All operations go through the mux layer. The mux owns tab state
//! (`MuxTab` with `SplitTree`); the App owns rendering state (tab bar
//! layout, animation offsets) and the actual `Pane` structs.

use std::path::PathBuf;

use winit::window::WindowId;

use oriterm_mux::domain::SpawnConfig;
use oriterm_mux::{TabId, WindowId as MuxWindowId};

use super::App;

impl App {
    /// Create a new tab in the given mux window.
    ///
    /// Inherits CWD from the active pane in the current tab. Applies the
    /// color palette and clears the width lock. Tab bar sync happens via
    /// the `WindowTabsChanged` notification from the mux.
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
            shell_integration: self.config.behavior.shell_integration,
            cwd,
            ..SpawnConfig::default()
        };

        let Some(mux) = &mut self.mux else { return };
        match mux.create_tab(window_id, &config, theme) {
            Ok((_tab_id, pane_id)) => {
                if let Some(pane) = mux.pane(pane_id) {
                    super::apply_palette(&self.config, pane, theme);
                }
                log::info!("new tab with pane {pane_id:?} in window {window_id:?}");
            }
            Err(e) => {
                log::error!("new tab failed: {e}");
                return;
            }
        }
        self.release_tab_width_lock();
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Close a tab and all its panes.
    ///
    /// If this was the last tab in the last window, shuts down immediately
    /// (ConPTY-safe: `process::exit` before dropping panes). Otherwise
    /// pane cleanup happens via `PaneClosed` notifications in `pump_mux_events`.
    pub(super) fn close_tab(&mut self, tab_id: TabId) {
        // Capture slide animation data before the mutable borrow of mux.
        let slide_info = self.capture_close_slide_info(tab_id);

        let Some(mux) = &mut self.mux else { return };

        // Check before closing: if the session has only one tab total,
        // closing it will leave zero windows. Must exit *before* dropping
        // Pane structs (ConPTY safety on Windows).
        let is_last = mux.session().tab_count() <= 1;

        // Pane cleanup is deferred to `PaneClosed` notifications in
        // `pump_mux_events` — the returned IDs are intentionally unused here.
        let _pane_ids = mux.close_tab(tab_id);

        if is_last {
            log::info!("last tab closed, shutting down");
            self.exit_app();
        }

        // Width lock is NOT released here. It persists for Chrome-style
        // rapid-close targeting (close button stays under cursor). The lock
        // is released when the cursor leaves the tab bar, a new tab is
        // created, or a drag finishes/cancels.

        // Sync tab bar immediately so slide animation has correct tab count.
        self.sync_tab_bar_from_mux();

        // Start slide animation for displaced tabs (skip if last tab).
        if !is_last {
            if let Some((closed_idx, tab_width)) = slide_info {
                self.start_tab_close_slide(closed_idx, tab_width);
            }
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

    /// Move a tab to a different window.
    ///
    /// Preserves the tab's panes and split layout. If the source window
    /// becomes empty, it is closed. Panes in the moved tab are resized to
    /// fit the destination window dimensions.
    pub(super) fn move_tab_to_window(&mut self, tab_id: TabId, dest_window: MuxWindowId) {
        let Some(mux) = &mut self.mux else { return };
        if !mux.move_tab_to_window(tab_id, dest_window) {
            return;
        }

        // Mux notifications (WindowTabsChanged, WindowClosed, TabLayoutChanged)
        // are processed in the normal pump_mux_events cycle. Sync both windows'
        // tab bars immediately so the UI doesn't lag.
        self.release_tab_width_lock();
        self.sync_tab_bar_from_mux();

        // Resize panes in the moved tab to fit the destination window.
        self.resize_all_panes();

        // Mark the destination window dirty.
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.pane_cache.invalidate_all();
            ctx.cached_dividers = None;
            ctx.dirty = true;
        }
    }

    /// Defers a move-tab-to-new-window request.
    ///
    /// Resolves the tab index to a `TabId` and stores it for processing
    /// in `about_to_wait` where `ActiveEventLoop` is available.
    pub(super) fn move_tab_to_new_window_deferred(&mut self, tab_index: usize) {
        self.pending_move_tab_to_window = Some(tab_index);
    }

    /// Move a tab to a new window.
    ///
    /// In embedded mode: creates a new OS window in this process, moves
    /// the tab there. In daemon mode: creates a new mux window via the
    /// daemon, moves the tab, then spawns a new `oriterm` process with
    /// `--connect` + `--window` to render it.
    ///
    /// Refuses if the tab is the last tab in the last window.
    #[allow(dead_code, reason = "superseded by tear_off_tab in Section 17.2")]
    pub(super) fn move_tab_to_new_window(
        &mut self,
        tab_id: TabId,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) {
        // Refuse if this is the last tab in the entire session.
        let is_last = self
            .mux
            .as_ref()
            .is_some_and(|m| m.session().tab_count() <= 1);
        if is_last {
            log::warn!("move_tab_to_new_window: refused — last tab in session");
            return;
        }

        let is_daemon = self.mux.as_ref().is_some_and(|m| m.is_daemon_mode());

        if is_daemon {
            self.move_tab_to_new_window_daemon(tab_id);
        } else {
            self.move_tab_to_new_window_embedded(tab_id, event_loop);
        }
    }

    /// Daemon-mode: create window via daemon, move tab, spawn new process.
    fn move_tab_to_new_window_daemon(&mut self, tab_id: TabId) {
        let Some(mux) = &mut self.mux else { return };

        // Create a new empty window in the daemon.
        let new_window_id = mux.create_window();
        if new_window_id.raw() == 0 {
            log::error!("move_tab_to_new_window_daemon: failed to create window");
            return;
        }

        // Move the tab to the new window.
        if !mux.move_tab_to_window(tab_id, new_window_id) {
            log::error!("move_tab_to_new_window_daemon: failed to move tab");
            return;
        }

        // Spawn a new oriterm process to render the new window.
        // It connects to the same daemon socket and claims the window ID.
        #[cfg(unix)]
        {
            let socket_path = oriterm_mux::server::socket_path();
            match std::process::Command::new(std::env::current_exe().unwrap_or_default())
                .arg("--connect")
                .arg(&socket_path)
                .arg("--window")
                .arg(new_window_id.raw().to_string())
                .spawn()
            {
                Ok(child) => {
                    log::info!(
                        "spawned new window process (pid={}) for {new_window_id}",
                        child.id()
                    );
                }
                Err(e) => {
                    log::error!("failed to spawn new window process: {e}");
                }
            }
        }

        // Sync tab bars for the source window.
        self.release_tab_width_lock();
        self.sync_tab_bar_from_mux();
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Embedded-mode: create in-process window, move tab there.
    fn move_tab_to_new_window_embedded(
        &mut self,
        tab_id: TabId,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) {
        // Create new window (GPU, surface, chrome, initial tab).
        let Some(new_winit_id) = self.create_window(event_loop) else {
            return;
        };

        // The new window got a fresh initial tab. Find the mux window ID,
        // then move the requested tab there BEFORE closing the initial tab.
        let Some(ctx) = self.windows.get(&new_winit_id) else {
            return;
        };
        let new_mux_id = ctx.window.mux_window_id();

        // Capture the initial tab ID before moving (the move changes active tab).
        let initial_tab = self.mux.as_ref().and_then(|m| m.active_tab_id(new_mux_id));

        // Move the requested tab to the new window (now has 2 tabs).
        self.move_tab_to_window(tab_id, new_mux_id);

        // Close the initial (empty) tab that `create_window` spawned
        // (window now has 1 tab — the moved one).
        if let Some(initial) = initial_tab {
            if let Some(mux) = &mut self.mux {
                let pane_ids = mux.close_tab(initial);
                for pid in pane_ids {
                    if let Some(pane) = mux.remove_pane(pid) {
                        std::thread::spawn(move || drop(pane));
                    }
                }
            }
        }

        // Sync tab bars: old window lost a tab, new window gained one.
        self.sync_tab_bar_from_mux();
        self.sync_tab_bar_for_window(new_winit_id);
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Reorder a tab within the active window (with animation).
    #[allow(
        dead_code,
        reason = "used by keybinding-driven reorder; drag uses reorder_tab_silent"
    )]
    pub(super) fn move_tab(&mut self, from: usize, to: usize) {
        // Capture tab width before the mutable mux borrow.
        let tab_width = self
            .focused_ctx()
            .map_or(0.0, |ctx| ctx.tab_bar.layout().tab_width);

        let Some(mux) = &mut self.mux else { return };
        let Some(win_id) = self.active_window else {
            return;
        };

        if !mux.reorder_tab(win_id, from, to) {
            return;
        }
        self.sync_tab_bar_from_mux();

        // Start slide animation for displaced tabs.
        self.start_tab_reorder_slide(from, to, tab_width);

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    // -- Private helpers --

    /// Captures the tab index and width for a close slide animation.
    ///
    /// Returns `None` if the tab or window context cannot be resolved.
    fn capture_close_slide_info(&self, tab_id: TabId) -> Option<(usize, f32)> {
        let mux = self.mux.as_ref()?;
        let win_id = self.active_window?;
        let win = mux.session().get_window(win_id)?;
        let idx = win.tabs().iter().position(|&id| id == tab_id)?;
        let tab_width = self.focused_ctx()?.tab_bar.layout().tab_width;
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
        let mux = self.mux.as_ref()?;
        let win_id = self.active_window?;
        mux.active_tab_id(win_id)
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
        let Some(win) = mux.session().get_window(win_id) else {
            return;
        };

        let (entries, active_idx) = build_tab_entries(mux.as_ref(), win);

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
        let mux_wid = {
            let Some(ctx) = self.windows.get(&winit_id) else {
                return;
            };
            ctx.window.mux_window_id()
        };
        let Some(mux) = self.mux.as_ref() else {
            return;
        };
        let Some(win) = mux.session().get_window(mux_wid) else {
            return;
        };

        let (entries, active_idx) = build_tab_entries(mux.as_ref(), win);

        if let Some(ctx) = self.windows.get_mut(&winit_id) {
            ctx.tab_bar.set_tabs(entries);
            ctx.tab_bar.set_active_index(active_idx);
        }
    }
}

/// Build tab bar entries from a mux window's tab list.
///
/// Returns `(entries, active_tab_index)`. Shared by both
/// `sync_tab_bar_from_mux` and `sync_tab_bar_for_window`.
fn build_tab_entries(
    mux: &dyn oriterm_mux::backend::MuxBackend,
    win: &oriterm_mux::session::MuxWindow,
) -> (Vec<oriterm_ui::widgets::tab_bar::TabEntry>, usize) {
    let active_idx = win.active_tab_idx();
    let entries = win
        .tabs()
        .iter()
        .map(|&tab_id| {
            let tab = mux.session().get_tab(tab_id);
            let pane_id = tab.map(oriterm_mux::session::MuxTab::active_pane);
            let pane = pane_id.and_then(|pid| mux.pane(pid));
            let mut title = pane
                .map(|p| p.effective_title().to_owned())
                .unwrap_or_default();
            let icon = pane
                .and_then(|p| p.icon_name())
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
