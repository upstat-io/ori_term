//! Window lifecycle: create, close, and exit.
//!
//! Coordinates OS window creation/destruction with the mux layer.
//! All windows share a single GPU device and pipeline set; each window
//! owns its own renderer, font collection, and glyph atlases.

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use oriterm_mux::WindowId as MuxWindowId;
use oriterm_ui::window::WindowConfig;

use super::App;
use super::window_context::WindowContext;
use crate::widgets::terminal_grid::TerminalGridWidget;
use crate::window::TermWindow;

impl App {
    /// Create a new terminal window with an initial tab and pane.
    ///
    /// Reuses the existing GPU device, pipelines, and mux. Creates a new winit
    /// window with its own surface, renderer, chrome/tab bar widgets, and mux
    /// window. An initial tab with one pane is spawned in the new window.
    ///
    /// Returns the winit [`WindowId`] of the new window, or `None` on failure.
    pub(super) fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Option<WindowId> {
        let (winit_id, mux_window_id) = self.create_window_bare(event_loop)?;

        // Extract geometry from the new window's per-window renderer
        // (scoped to release the borrow before mux operations).
        let (cols, rows) = {
            let ctx = self.windows.get(&winit_id)?;
            let renderer = ctx.renderer.as_ref()?;
            let (w, h) = ctx.window.size_px();
            let cell = renderer.cell_metrics();
            let scale = ctx.window.scale_factor().factor() as f32;
            let tab_bar_h = oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT;
            let caption_h = ctx.chrome.caption_height();
            let origin_y = super::chrome::grid_origin_y(caption_h + tab_bar_h, scale);
            let chrome_px = origin_y as u32;
            let grid_h = h.saturating_sub(chrome_px);
            let cols = cell.columns(w).max(1);
            let rows = cell.rows(grid_h).max(1);
            (cols, rows)
        };

        let mux = self.mux.as_mut()?;
        let theme = self
            .config
            .colors
            .resolve_theme(crate::platform::theme::system_theme);
        let spawn_config = oriterm_mux::domain::SpawnConfig {
            cols: cols as u16,
            rows: rows as u16,
            scrollback: self.config.terminal.scrollback,
            shell_integration: self.config.behavior.shell_integration,
            ..oriterm_mux::domain::SpawnConfig::default()
        };
        let palette =
            crate::app::config_reload::build_palette_from_config(&self.config.colors, theme);
        let tab_result = mux.create_tab(mux_window_id, &spawn_config, theme);
        match tab_result {
            Ok((_tab_id, pane_id)) => {
                mux.set_pane_theme(pane_id, theme, palette);
                mux.discard_notifications();
            }
            Err(e) => {
                log::error!("failed to create initial tab for new window: {e}");
                mux.close_window(mux_window_id);
                mux.discard_notifications();
                self.windows.remove(&winit_id);
                return None;
            }
        }

        // Clear frame and show.
        let palette =
            crate::app::config_reload::build_palette_from_config(&self.config.colors, theme);
        let opacity = self.config.window.effective_opacity();
        if let Some(gpu) = self.gpu.as_ref() {
            if let Some(ctx) = self.windows.get(&winit_id) {
                gpu.clear_surface(ctx.window.surface(), palette.background(), opacity);
            }
        }
        if let Some(ctx) = self.windows.get(&winit_id) {
            ctx.window.set_visible(true);
        }

        // Focus the new window.
        self.focused_window_id = Some(winit_id);
        self.active_window = Some(mux_window_id);

        log::info!("window created: {winit_id:?} → mux {mux_window_id:?}");

        Some(winit_id)
    }

    /// Create an OS window without spawning any tabs.
    ///
    /// Allocates a mux window ID, creates the OS window + GPU surface,
    /// per-window renderer, chrome/tab bar widgets, and grid widget. The
    /// window starts hidden. The caller is responsible for moving or
    /// creating tabs, clearing the surface, and showing the window.
    ///
    /// Returns `(winit_id, mux_window_id)` or `None` on failure.
    pub(super) fn create_window_bare(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Option<(WindowId, MuxWindowId)> {
        let gpu = self.gpu.as_ref()?;
        let pipelines = self.pipelines.as_ref()?;
        let font_set = self.font_set.as_ref()?.clone();
        let mux = self.mux.as_mut()?;

        let opacity = self.config.window.effective_opacity();
        let window_config = WindowConfig {
            title: "ori".into(),
            transparent: opacity < 1.0,
            blur: self.config.window.blur && opacity < 1.0,
            opacity,
            ..WindowConfig::default()
        };

        let mux_window_id = match mux.create_window() {
            Ok(id) => id,
            Err(e) => {
                log::error!("failed to create mux window: {e}");
                return None;
            }
        };

        // Tell the daemon this client renders the new window.
        if mux.is_daemon_mode() {
            if let Err(e) = mux.claim_window(mux_window_id) {
                log::error!("failed to claim mux window {mux_window_id}: {e}");
                mux.close_window(mux_window_id);
                mux.discard_notifications();
                return None;
            }
        }

        let window = match TermWindow::new(event_loop, &window_config, gpu, mux_window_id) {
            Ok(w) => w,
            Err(e) => {
                log::error!("failed to create window: {e}");
                // mux borrow from above ended (NLL); re-borrow for cleanup.
                if let Some(mux) = self.mux.as_mut() {
                    mux.close_window(mux_window_id);
                    mux.discard_notifications();
                }
                return None;
            }
        };

        // Chrome + tab bar widgets.
        let (chrome_widget, tab_bar_widget, caption_height) = self.create_chrome_widgets(&window);

        let Some(renderer) = self.create_window_renderer(&window, gpu, pipelines, font_set) else {
            if let Some(mux) = self.mux.as_mut() {
                mux.close_window(mux_window_id);
                mux.discard_notifications();
            }
            return None;
        };

        // Compute grid dimensions from per-window cell metrics.
        let (w, h) = window.size_px();
        let cell = renderer.cell_metrics();
        let scale = window.scale_factor().factor() as f32;
        let tab_bar_h = oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT;
        let origin_y = super::chrome::grid_origin_y(caption_height + tab_bar_h, scale);
        let chrome_px = origin_y as u32;
        let grid_h = h.saturating_sub(chrome_px);
        let cols = cell.columns(w).max(1);
        let rows = cell.rows(grid_h).max(1);

        // Terminal grid widget.
        let grid_widget = TerminalGridWidget::new(cell.width, cell.height, cols, rows);
        grid_widget.set_bounds(oriterm_ui::geometry::Rect::new(
            0.0,
            origin_y,
            cols as f32 * cell.width,
            rows as f32 * cell.height,
        ));

        let winit_id = window.window_id();
        let ctx = WindowContext::new(
            window,
            chrome_widget,
            tab_bar_widget,
            grid_widget,
            Some(renderer),
        );
        self.windows.insert(winit_id, ctx);

        log::info!(
            "bare window created: {winit_id:?} → mux {mux_window_id:?}, \
             {w}x{h} px, {cols}x{rows} cells"
        );

        Some((winit_id, mux_window_id))
    }

    /// Build a per-window renderer for the given window's DPI and font config.
    fn create_window_renderer(
        &self,
        window: &TermWindow,
        gpu: &crate::gpu::GpuState,
        pipelines: &crate::gpu::GpuPipelines,
        font_set: crate::font::FontSet,
    ) -> Option<crate::gpu::WindowRenderer> {
        let scale = window.scale_factor().factor() as f32;
        let physical_dpi = super::DEFAULT_DPI * scale;
        let hinting = super::config_reload::resolve_hinting(&self.config.font, f64::from(scale));
        let format =
            super::config_reload::resolve_subpixel_mode(&self.config.font, f64::from(scale))
                .glyph_format();
        let weight = self.config.font.effective_weight();

        let mut font_collection = match crate::font::FontCollection::new(
            font_set,
            self.config.font.size,
            physical_dpi,
            format,
            weight,
            hinting,
        ) {
            Ok(fc) => fc,
            Err(e) => {
                log::error!("failed to create font collection for new window: {e}");
                return None;
            }
        };
        super::config_reload::apply_font_config(
            &mut font_collection,
            &self.config.font,
            self.user_fb_count,
        );

        // UI font from cached FontSet (no re-discovery per window).
        let ui_fc = self.ui_font_set.as_ref().and_then(|fs| {
            crate::font::FontCollection::new(fs.clone(), 11.0, physical_dpi, format, 400, hinting)
                .ok()
        });

        Some(crate::gpu::WindowRenderer::new(
            gpu,
            pipelines,
            font_collection,
            ui_fc,
        ))
    }

    /// Close a single window.
    ///
    /// If this is the last window, calls [`exit_app`](Self::exit_app) which
    /// terminates the process (`ConPTY`-safe: must exit before dropping panes).
    /// Otherwise, removes the window and drops its panes on background threads.
    pub(super) fn close_window(&mut self, winit_id: WindowId, _event_loop: &ActiveEventLoop) {
        // Look up the mux window ID for this OS window.
        let Some(ctx) = self.windows.get(&winit_id) else {
            log::warn!("close_window: unknown winit id {winit_id:?}");
            return;
        };
        let mux_window_id = ctx.window.mux_window_id();

        // If this is the last window, exit the process immediately.
        // ConPTY safety: process::exit() must run before pane destructors.
        if self.windows.len() <= 1 {
            self.exit_app();
        }

        // Close the mux window — returns pane IDs to clean up.
        let pane_ids = if let Some(mux) = &mut self.mux {
            mux.close_window(mux_window_id)
        } else {
            Vec::new()
        };

        // Clean up pane resources (PTY kill + background drop in embedded mode).
        if let Some(mux) = &mut self.mux {
            for id in pane_ids {
                mux.cleanup_closed_pane(id);
            }
        }

        // Remove the window context.
        self.windows.remove(&winit_id);

        // If the closed window was focused, focus the next available window.
        self.transfer_focus_from(winit_id);

        log::info!(
            "window closed: {winit_id:?}, {} remaining",
            self.windows.len()
        );

        // Drain any mux notifications generated by the close.
        self.pump_close_notifications();
    }

    /// Close a window whose last tab was just closed.
    ///
    /// Resolves the mux window ID to a winit window ID. If this is the last
    /// OS window, exits the process. Otherwise closes the mux window (which
    /// has no tabs/panes left) and removes the OS window.
    pub(super) fn close_empty_mux_window(&mut self, mux_window_id: MuxWindowId) {
        // If this is the only OS window, exit.
        if self.windows.len() <= 1 {
            self.exit_app();
        }

        // Find the winit window that renders this mux window.
        let winit_id = self
            .windows
            .iter()
            .find(|(_, ctx)| ctx.window.mux_window_id() == mux_window_id)
            .map(|(&id, _)| id);

        let Some(winit_id) = winit_id else {
            // No OS window for this mux window (daemon mode, rendered by
            // another process). Close the mux window to avoid orphans.
            if let Some(mux) = &mut self.mux {
                mux.close_window(mux_window_id);
                mux.discard_notifications();
            }
            return;
        };

        // Close the (empty) mux window.
        if let Some(mux) = &mut self.mux {
            mux.close_window(mux_window_id);
            mux.discard_notifications();
        }

        // Remove the OS window.
        self.windows.remove(&winit_id);

        self.transfer_focus_from(winit_id);

        log::info!(
            "empty window closed: {winit_id:?} (mux {mux_window_id:?}), {} remaining",
            self.windows.len()
        );
    }

    /// Remove a window from the App without closing mux resources.
    ///
    /// Used by tear-off merge: the mux tab was already moved out, so the
    /// window's mux state is empty. This removes the OS window and context
    /// without touching the mux layer.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(super) fn remove_empty_window(&mut self, winit_id: WindowId) {
        // Close the mux window to clean up the empty container.
        if let Some(ctx) = self.windows.get(&winit_id) {
            let mux_wid = ctx.window.mux_window_id();
            if let Some(mux) = &mut self.mux {
                mux.close_window(mux_wid);
                mux.discard_notifications();
            }
        }

        self.windows.remove(&winit_id);

        // If the removed window was focused, pick another.
        self.transfer_focus_from(winit_id);

        log::info!(
            "empty window removed: {winit_id:?}, {} remaining",
            self.windows.len()
        );
    }

    /// Terminate the application.
    ///
    /// Saves GPU pipeline cache and exits the process. This method does not
    /// return. `ConPTY` safety: `process::exit()` runs before any pane
    /// destructors, preventing deadlocks with the `ConPTY` API.
    pub(super) fn exit_app(&self) -> ! {
        if let Some(gpu) = &self.gpu {
            gpu.save_pipeline_cache_async();
        }
        log::info!("exit_app: shutting down");
        std::process::exit(0)
    }

    /// Drain mux notifications generated by window close operations.
    ///
    /// Handles `PaneClosed` and `WindowClosed` notifications that arise from
    /// `mux.close_window()`. Separated from the main `pump_mux_events` to
    /// avoid re-entrancy issues during `close_window`.
    fn pump_close_notifications(&mut self) {
        let Some(mux) = &mut self.mux else { return };
        mux.drain_notifications(&mut self.notification_buf);
        if self.notification_buf.is_empty() {
            return;
        }

        self.with_drained_notifications(|this, notification| {
            if let oriterm_mux::mux_event::MuxNotification::PaneClosed(id) = notification {
                // Clean up backend resources and caches.
                if let Some(mux) = this.mux.as_mut() {
                    mux.cleanup_closed_pane(id);
                }
                for ctx in this.windows.values_mut() {
                    ctx.pane_cache.remove(id);
                }
            }
            // Other notifications (WindowClosed, LastWindowClosed, etc.)
            // are handled by the caller or are no-ops during close.
        });
    }
}
