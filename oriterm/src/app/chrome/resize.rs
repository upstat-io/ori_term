//! Window resize handling.
//!
//! Extracted from `chrome/mod.rs` to keep file sizes under the 500-line limit.

use winit::window::WindowId;

use super::grid_origin_y;
use crate::app::App;

impl App {
    /// Update window resize increments from current cell metrics.
    ///
    /// Called after any change that affects cell dimensions (font size,
    /// DPI, font family) so the window snaps to cell boundaries.
    pub(in crate::app) fn update_resize_increments(&self, winit_id: WindowId) {
        if !self.config.window.resize_increments {
            return;
        }
        let Some(ctx) = self.windows.get(&winit_id) else {
            return;
        };
        let Some(renderer) = ctx.renderer.as_ref() else {
            return;
        };
        let cell = renderer.cell_metrics();
        let inc =
            winit::dpi::PhysicalSize::new(cell.width.round() as u32, cell.height.round() as u32);
        ctx.window.window().set_resize_increments(Some(inc));
    }

    /// Recompute grid layout from current cell metrics and viewport size.
    ///
    /// Reads cell metrics from the renderer, chrome height (caption + tab bar)
    /// from widgets, and updates the terminal grid widget, tab grid, PTY
    /// dimensions, and resize increments. Called after any change to font,
    /// DPI, or window size.
    ///
    /// `winit_id` identifies which window to recompute. Widget updates and
    /// cache invalidation target only this window.
    pub(in crate::app) fn sync_grid_layout(
        &mut self,
        winit_id: WindowId,
        viewport_w: u32,
        viewport_h: u32,
    ) {
        let Some(ctx) = self.windows.get(&winit_id) else {
            return;
        };
        let Some(renderer) = ctx.renderer.as_ref() else {
            return;
        };
        let cell = renderer.cell_metrics();
        let scale = ctx.window.scale_factor().factor() as f32;

        let caption_height = ctx.chrome.caption_height();
        let tab_bar_h = oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT;
        let chrome_height = caption_height + tab_bar_h;
        let origin_y = grid_origin_y(chrome_height, scale);
        let chrome_px = origin_y as u32;
        let grid_h = viewport_h.saturating_sub(chrome_px);
        let cols = cell.columns(viewport_w).max(1);
        let rows = cell.rows(grid_h).max(1);

        // Reborrow mutably now that immutable reads are done.
        let ctx = self.windows.get_mut(&winit_id).expect("checked above");
        ctx.terminal_grid.set_cell_metrics(cell.width, cell.height);
        ctx.terminal_grid.set_grid_size(cols, rows);
        ctx.terminal_grid
            .set_bounds(oriterm_ui::geometry::Rect::new(
                0.0,
                origin_y,
                cols as f32 * cell.width,
                rows as f32 * cell.height,
            ));

        // Resize the active pane in this specific window (not the globally
        // focused one). Multi-pane layouts are recomputed by resize_all_panes.
        if let Some(pane_id) = self.active_pane_id_for_window(winit_id) {
            if let Some(mux) = self.mux.as_mut() {
                mux.resize_pane_grid(pane_id, rows as u16, cols as u16);
            }
        }
        self.resize_all_panes();
        if let Some(ctx) = self.windows.get_mut(&winit_id) {
            ctx.cached_dividers = None;
        }

        self.update_resize_increments(winit_id);
    }

    /// Handle window resize: reconfigure surface, update chrome layout,
    /// resize grid and PTY.
    ///
    /// `winit_id` identifies which window was resized. All operations
    /// (surface reconfigure, widget layout, grid recomputation) target
    /// only this window.
    pub(in crate::app) fn handle_resize(
        &mut self,
        winit_id: WindowId,
        size: winit::dpi::PhysicalSize<u32>,
    ) {
        // Window size changed — cached tab width is invalid.
        self.release_tab_width_lock();

        // On Windows, detect DPI changes from WM_DPICHANGED. The snap
        // subclass proc consumes the message before winit sees it, so
        // ScaleFactorChanged never fires — the resize handler is the
        // only reliable place to detect the change.
        #[cfg(target_os = "windows")]
        {
            let dpi_changed = self.windows.get_mut(&winit_id).and_then(|ctx| {
                let new_scale = oriterm_ui::platform_windows::get_current_dpi(ctx.window.window())?;
                ctx.window
                    .update_scale_factor(new_scale)
                    .then_some(new_scale)
            });
            if let Some(new_scale) = dpi_changed {
                self.handle_dpi_change(winit_id, new_scale);
                // Update SnapData chrome metrics for the new physical DPI.
                let s = new_scale as f32;
                let caption_h = self
                    .windows
                    .get(&winit_id)
                    .map_or(0.0, |ctx| ctx.chrome.caption_height());
                if let Some(ctx) = self.windows.get(&winit_id) {
                    oriterm_ui::platform_windows::set_chrome_metrics(
                        ctx.window.window(),
                        oriterm_ui::widgets::window_chrome::constants::RESIZE_BORDER_WIDTH * s,
                        caption_h * s,
                    );
                }
            }
        }

        // Resize GPU surface (scoped to release borrows before sync_grid_layout).
        {
            let Some(gpu) = &self.gpu else { return };
            let Some(ctx) = self.windows.get_mut(&winit_id) else {
                return;
            };
            ctx.window.resize_surface(size.width, size.height, gpu);
        }

        // Update chrome and tab bar layout for new window width.
        if let Some(ctx) = self.windows.get_mut(&winit_id) {
            let scale = ctx.window.scale_factor().factor() as f32;
            let logical_w = size.width as f32 / scale;
            ctx.chrome.set_window_width(logical_w);
            ctx.tab_bar.set_window_width(logical_w);
        }

        // Update overlay manager viewport for dialog placement.
        if let Some(ctx) = self.windows.get_mut(&winit_id) {
            let scale = ctx.window.scale_factor().factor() as f32;
            let logical_w = size.width as f32 / scale;
            let logical_h = size.height as f32 / scale;
            ctx.overlays.set_viewport(oriterm_ui::geometry::Rect::new(
                0.0, 0.0, logical_w, logical_h,
            ));
        }

        // Recompute grid dimensions, resize terminal + PTY + increments.
        self.sync_grid_layout(winit_id, size.width, size.height);

        // Update platform hit test rects on Windows.
        #[cfg(target_os = "windows")]
        if let Some(ctx) = self.windows.get(&winit_id) {
            let scale = ctx.window.scale_factor().factor() as f32;
            oriterm_ui::platform_windows::set_client_rects(
                ctx.window.window(),
                ctx.chrome
                    .interactive_rects()
                    .iter()
                    .map(|r| super::scale_rect(*r, scale))
                    .collect(),
            );
        }

        if let Some(ctx) = self.windows.get_mut(&winit_id) {
            ctx.url_cache.invalidate();
            ctx.hovered_url = None; // Segments contain stale absolute rows.
            ctx.dirty = true;
        }
    }
}
