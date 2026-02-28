//! Window lifecycle: create, close, and exit.
//!
//! Coordinates OS window creation/destruction with the mux layer.
//! All windows share a single GPU device, renderer, and font collection.

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use oriterm_ui::window::WindowConfig;

use super::App;
use super::window_context::WindowContext;
use crate::widgets::terminal_grid::TerminalGridWidget;
use crate::window::TermWindow;

impl App {
    /// Create a new terminal window.
    ///
    /// Reuses the existing GPU device, renderer, and mux. Creates a new winit
    /// window with its own surface, chrome/tab bar widgets, and mux window.
    /// An initial tab with one pane is spawned in the new window.
    ///
    /// Returns the winit [`WindowId`] of the new window, or `None` on failure.
    pub(super) fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Option<WindowId> {
        let gpu = self.gpu.as_ref()?;
        let renderer = self.renderer.as_ref()?;
        let mux = self.mux.as_mut()?;

        // Build window config from user settings.
        let opacity = self.config.window.effective_opacity();
        let window_config = WindowConfig {
            title: "ori".into(),
            transparent: opacity < 1.0,
            blur: self.config.window.blur && opacity < 1.0,
            opacity,
            ..WindowConfig::default()
        };

        // Allocate mux window ID before creating the OS window (needed by TermWindow).
        let mux_window_id = mux.create_window();

        // Create OS window + GPU surface.
        let window = match TermWindow::new(event_loop, &window_config, gpu, mux_window_id) {
            Ok(w) => w,
            Err(e) => {
                log::error!("failed to create window: {e}");
                // Roll back mux window allocation.
                mux.close_window(mux_window_id);
                let mut discard = Vec::new();
                mux.drain_notifications(&mut discard);
                return None;
            }
        };

        // Chrome + tab bar widgets.
        let (chrome_widget, tab_bar_widget, caption_height) = self.create_chrome_widgets(&window);

        // Compute grid dimensions for the new window.
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

        // Create initial tab + pane.
        //
        // Inlined rather than calling `create_initial_tab` because the mux
        // is already stored in `self.mux` (borrow conflict with `&mut self`).
        // In `try_init`, the mux is a local variable so no conflict arises.
        let mux = self.mux.as_mut().expect("checked above");
        let theme = self
            .config
            .colors
            .resolve_theme(crate::platform::theme::system_theme);
        let spawn_config = oriterm_mux::domain::SpawnConfig {
            cols: cols as u16,
            rows: rows as u16,
            scrollback: self.config.terminal.scrollback,
            ..oriterm_mux::domain::SpawnConfig::default()
        };
        let tab_result = mux.create_tab(mux_window_id, &spawn_config, theme, &self.event_proxy);
        match tab_result {
            Ok((_tab_id, pane_id, pane)) => {
                self.apply_palette_to_pane(&pane, theme);
                self.panes.insert(pane_id, pane);
                // Drain setup notifications (not useful at creation time).
                let mut discard = Vec::new();
                let mux = self.mux.as_mut().expect("checked above");
                mux.drain_notifications(&mut discard);
            }
            Err(e) => {
                log::error!("failed to create initial tab for new window: {e}");
                mux.close_window(mux_window_id);
                let mut discard = Vec::new();
                mux.drain_notifications(&mut discard);
                return None;
            }
        }

        // Show window.
        window.set_visible(true);

        let winit_id = window.window_id();
        let ctx = WindowContext::new(window, chrome_widget, tab_bar_widget, grid_widget);
        self.windows.insert(winit_id, ctx);

        // Focus the new window.
        self.focused_window_id = Some(winit_id);
        self.active_window = Some(mux_window_id);

        log::info!(
            "window created: {winit_id:?} → mux {mux_window_id:?}, \
             {w}x{h} px, {cols}x{rows} cells"
        );

        Some(winit_id)
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

        // Drop panes on background threads to avoid blocking the event loop.
        for id in pane_ids {
            if let Some(pane) = self.panes.remove(&id) {
                std::thread::spawn(move || drop(pane));
            }
        }

        // Remove the window context.
        self.windows.remove(&winit_id);

        // If the closed window was focused, focus the next available window.
        if self.focused_window_id == Some(winit_id) {
            self.focused_window_id = self.windows.keys().next().copied();
            self.active_window = self
                .focused_window_id
                .and_then(|id| self.windows.get(&id).map(|ctx| ctx.window.mux_window_id()));
        }

        log::info!(
            "window closed: {winit_id:?}, {} remaining",
            self.windows.len()
        );

        // Drain any mux notifications generated by the close.
        self.pump_close_notifications();
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

        let mut notifications = std::mem::take(&mut self.notification_buf);
        #[allow(
            clippy::iter_with_drain,
            reason = "drain preserves Vec capacity; into_iter drops it"
        )]
        for notification in notifications.drain(..) {
            if let crate::mux_event::MuxNotification::PaneClosed(id) = notification {
                // Panes already removed above — just clean up caches.
                if let Some(pane) = self.panes.remove(&id) {
                    std::thread::spawn(move || drop(pane));
                }
                for ctx in self.windows.values_mut() {
                    ctx.pane_cache.remove(id);
                }
            }
            // Other notifications (WindowClosed, LastWindowClosed, etc.)
            // are handled by the caller or are no-ops during close.
        }
        self.notification_buf = notifications;
    }
}
