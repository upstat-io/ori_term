//! Window chrome: action dispatch, event routing, and shared helpers.
//!
//! Handles `WidgetAction::WindowMinimize`, `WindowMaximize`, and
//! `WindowClose` by forwarding to the appropriate winit window operations.
//! Routes mouse and hover events to the chrome widget, and provides shared
//! geometry helpers used by both init and resize.

use winit::event::ElementState;
use winit::event_loop::ActiveEventLoop;

use oriterm_ui::widgets::window_chrome::WindowChromeWidget;
use oriterm_ui::widgets::{Widget, WidgetAction};

use super::App;
use crate::font::UiFontMeasurer;

/// Scale a logical-pixel rect to physical pixels.
#[cfg(target_os = "windows")]
pub(super) fn scale_rect(r: oriterm_ui::geometry::Rect, scale: f32) -> oriterm_ui::geometry::Rect {
    oriterm_ui::geometry::Rect::new(
        r.x() * scale,
        r.y() * scale,
        r.width() * scale,
        r.height() * scale,
    )
}

impl App {
    /// Dispatch a window chrome action to the corresponding window operation.
    ///
    /// Returns `true` if the action was handled (recognized as a chrome action).
    pub(super) fn handle_chrome_action(
        &mut self,
        action: &WidgetAction,
        _event_loop: &ActiveEventLoop,
    ) -> bool {
        match action {
            WidgetAction::WindowMinimize => {
                if let Some(window) = &self.window {
                    window.window().set_minimized(true);
                }
                true
            }
            WidgetAction::WindowMaximize => {
                if let Some(window) = &mut self.window {
                    let maximized = !window.is_maximized();
                    window.window().set_maximized(maximized);
                    window.set_maximized(maximized);
                    if let Some(chrome) = &mut self.chrome {
                        chrome.set_maximized(maximized);
                    }
                    self.dirty = true;
                }
                true
            }
            WidgetAction::WindowClose => {
                self.shutdown(0);
            }
            _ => false,
        }
    }

    /// Check if a mouse event should be handled by the chrome widget.
    ///
    /// Returns `true` if the event was consumed by chrome (click on a
    /// control button), `false` if it should fall through to the grid.
    pub(super) fn try_chrome_mouse(
        &mut self,
        button: winit::event::MouseButton,
        state: ElementState,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        let pos = self.mouse.cursor_pos();
        let (Some(chrome), Some(window)) = (&mut self.chrome, &self.window) else {
            return false;
        };
        if !chrome.is_visible() {
            return false;
        }

        let scale = window.scale_factor().factor() as f32;
        let logical_y = pos.y as f32 / scale;

        // Only intercept events within the caption height.
        if logical_y >= chrome.caption_height() {
            return false;
        }

        let logical_pos = oriterm_ui::geometry::Point::new(pos.x as f32 / scale, logical_y);

        // Check if the click is on a control button.
        let kind = match (button, state) {
            (winit::event::MouseButton::Left, ElementState::Pressed) => {
                oriterm_ui::input::MouseEventKind::Down(oriterm_ui::input::MouseButton::Left)
            }
            (winit::event::MouseButton::Left, ElementState::Released) => {
                oriterm_ui::input::MouseEventKind::Up(oriterm_ui::input::MouseButton::Left)
            }
            _ => return false,
        };

        let event = oriterm_ui::input::MouseEvent {
            kind,
            pos: logical_pos,
            modifiers: super::winit_mods_to_ui(self.modifiers),
        };
        let logical_w = window.size_px().0 as f32 / scale;
        let measurer = self
            .renderer
            .as_ref()
            .map(|r| UiFontMeasurer::new(r.active_ui_collection(), scale));
        let measurer: &dyn oriterm_ui::widgets::TextMeasurer = match &measurer {
            Some(m) => m,
            None => return false,
        };
        let theme = oriterm_ui::theme::UiTheme::dark();
        let ctx = oriterm_ui::widgets::EventCtx {
            measurer,
            bounds: oriterm_ui::geometry::Rect::new(0.0, 0.0, logical_w, chrome.caption_height()),
            is_focused: false,
            focused_widget: None,
            theme: &theme,
        };

        let resp = chrome.handle_mouse(&event, &ctx);
        if resp.response != oriterm_ui::input::EventResponse::Ignored {
            if let Some(action) = &resp.action {
                self.handle_chrome_action(action, event_loop);
            }
            self.dirty = true;
            return true;
        }

        // Caption click that didn't hit a control button — initiate window drag.
        // On Windows, the WndProc subclass handles WM_NCHITTEST → Caption → drag.
        // On macOS/Linux, winit's drag_window() triggers the native drag protocol.
        #[cfg(not(target_os = "windows"))]
        if button == winit::event::MouseButton::Left && state == ElementState::Pressed {
            let _ = window.window().drag_window();
            return true;
        }

        false
    }

    /// Clear chrome hover state when the cursor leaves the window.
    pub(super) fn clear_chrome_hover(&mut self) {
        let Some(chrome) = &mut self.chrome else {
            return;
        };
        let Some(renderer) = &self.renderer else {
            return;
        };
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |w| w.scale_factor().factor() as f32);
        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
        let theme = oriterm_ui::theme::UiTheme::dark();
        let ctx = oriterm_ui::widgets::EventCtx {
            measurer: &measurer,
            bounds: oriterm_ui::geometry::Rect::default(),
            is_focused: false,
            focused_widget: None,
            theme: &theme,
        };
        let resp = chrome.handle_hover(oriterm_ui::input::HoverEvent::Leave, &ctx);
        if resp.response == oriterm_ui::input::EventResponse::RequestRedraw {
            self.dirty = true;
        }
    }

    /// Update chrome hover state from a cursor position (physical pixels).
    pub(super) fn update_chrome_hover(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
        let (Some(chrome), Some(window)) = (&mut self.chrome, &self.window) else {
            return;
        };
        let Some(renderer) = &self.renderer else {
            return;
        };
        let scale = window.scale_factor().factor() as f32;
        let logical =
            oriterm_ui::geometry::Point::new(position.x as f32 / scale, position.y as f32 / scale);
        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
        let theme = oriterm_ui::theme::UiTheme::dark();
        let ctx = oriterm_ui::widgets::EventCtx {
            measurer: &measurer,
            bounds: oriterm_ui::geometry::Rect::default(),
            is_focused: false,
            focused_widget: None,
            theme: &theme,
        };
        let resp = chrome.update_hover(logical, &ctx);
        if resp.response == oriterm_ui::input::EventResponse::RequestRedraw {
            self.dirty = true;
        }
    }

    /// Returns `true` if the cursor position is within the chrome caption area.
    pub(super) fn cursor_in_chrome(&self, position: winit::dpi::PhysicalPosition<f64>) -> bool {
        let (Some(chrome), Some(window)) = (&self.chrome, &self.window) else {
            return false;
        };
        if !chrome.is_visible() {
            return false;
        }
        let scale = window.scale_factor().factor() as f32;
        let logical_y = position.y as f32 / scale;
        logical_y < chrome.caption_height()
    }

    /// Returns `true` if the cursor position is within the tab bar zone.
    ///
    /// The tab bar zone spans from the chrome caption height to
    /// caption height + `TAB_BAR_HEIGHT` (logical pixels).
    pub(super) fn cursor_in_tab_bar(&self, position: winit::dpi::PhysicalPosition<f64>) -> bool {
        let (Some(chrome), Some(window)) = (&self.chrome, &self.window) else {
            return false;
        };
        if !chrome.is_visible() {
            return false;
        }
        let scale = window.scale_factor().factor() as f32;
        let logical_y = position.y as f32 / scale;
        let caption_h = chrome.caption_height();
        logical_y >= caption_h
            && logical_y < caption_h + oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT
    }

    /// Update tab width lock based on cursor position.
    ///
    /// Called from `CursorMoved`. When the cursor enters the tab bar zone
    /// and no lock is held, computes the current tab width and acquires the
    /// lock. When the cursor leaves the tab bar zone, releases the lock.
    pub(super) fn update_tab_bar_hover(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
        let in_tab_bar = self.cursor_in_tab_bar(position);
        let locked = self.tab_width_lock().is_some();

        match (in_tab_bar, locked) {
            // Cursor entered tab bar without a lock — acquire at current width.
            (true, false) => {
                let window_width = self.window.as_ref().map_or(0.0, |w| {
                    let scale = w.scale_factor().factor() as f32;
                    w.size_px().0 as f32 / scale
                });
                let tab_count = usize::from(self.tab.is_some());
                let layout = oriterm_ui::widgets::tab_bar::TabBarLayout::compute(
                    tab_count,
                    window_width,
                    None,
                );
                self.acquire_tab_width_lock(layout.tab_width);
            }
            // Cursor left tab bar — release lock.
            (false, true) => self.release_tab_width_lock(),
            // Already locked in tab bar, or outside without lock — no change.
            (true, true) | (false, false) => {}
        }
    }

    /// Update window resize increments from current cell metrics.
    ///
    /// Called after any change that affects cell dimensions (font size,
    /// DPI, font family) so the window snaps to cell boundaries.
    pub(super) fn update_resize_increments(&self) {
        if !self.config.window.resize_increments {
            return;
        }
        let (Some(renderer), Some(window)) = (&self.renderer, &self.window) else {
            return;
        };
        let cell = renderer.cell_metrics();
        let inc =
            winit::dpi::PhysicalSize::new(cell.width.round() as u32, cell.height.round() as u32);
        window.window().set_resize_increments(Some(inc));
    }

    /// Recompute grid layout from current cell metrics and viewport size.
    ///
    /// Reads cell metrics from the renderer, caption height from chrome,
    /// and updates the terminal grid widget, tab grid, PTY dimensions,
    /// and resize increments. Called after any change to font, DPI, or
    /// window size.
    pub(super) fn sync_grid_layout(&mut self, viewport_w: u32, viewport_h: u32) {
        let (Some(renderer), Some(window)) = (&self.renderer, &self.window) else {
            return;
        };
        let cell = renderer.cell_metrics();
        let scale = window.scale_factor().factor() as f32;

        let caption_height = self
            .chrome
            .as_ref()
            .map_or(0.0, WindowChromeWidget::caption_height);
        let caption_px = (caption_height * scale).round() as u32;
        let grid_h = viewport_h.saturating_sub(caption_px);
        let cols = cell.columns(viewport_w).max(1);
        let rows = cell.rows(grid_h).max(1);

        if let Some(grid) = &mut self.terminal_grid {
            grid.set_cell_metrics(cell.width, cell.height);
            grid.set_grid_size(cols, rows);
            grid.set_bounds(oriterm_ui::geometry::Rect::new(
                0.0,
                caption_height * scale,
                cols as f32 * cell.width,
                rows as f32 * cell.height,
            ));
        }

        if let Some(tab) = &self.tab {
            tab.resize_grid(rows as u16, cols as u16);
            tab.resize_pty(rows as u16, cols as u16);
        }

        self.update_resize_increments();
    }

    /// Handle window resize: reconfigure surface, update chrome layout,
    /// resize grid and PTY.
    pub(super) fn handle_resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        // Window size changed — cached tab width is invalid.
        self.release_tab_width_lock();

        // On Windows, detect DPI changes from WM_DPICHANGED. The snap
        // subclass proc consumes the message before winit sees it, so
        // ScaleFactorChanged never fires — the resize handler is the
        // only reliable place to detect the change.
        #[cfg(target_os = "windows")]
        {
            let dpi_changed = self.window.as_mut().and_then(|w| {
                let new_scale = oriterm_ui::platform_windows::get_current_dpi(w.window())?;
                w.update_scale_factor(new_scale).then_some(new_scale)
            });
            if let Some(new_scale) = dpi_changed {
                self.handle_dpi_change(new_scale);
                // Update SnapData chrome metrics for the new physical DPI.
                let s = new_scale as f32;
                let caption_h = self
                    .chrome
                    .as_ref()
                    .map_or(0.0, WindowChromeWidget::caption_height);
                if let Some(w) = &self.window {
                    oriterm_ui::platform_windows::set_chrome_metrics(
                        w.window(),
                        oriterm_ui::widgets::window_chrome::constants::RESIZE_BORDER_WIDTH * s,
                        caption_h * s,
                    );
                }
            }
        }

        // Resize GPU surface (scoped to release borrows before sync_grid_layout).
        {
            let (Some(gpu), Some(window)) = (&self.gpu, &mut self.window) else {
                return;
            };
            window.resize_surface(size.width, size.height, gpu);
        }

        // Update chrome layout for new window width.
        if let (Some(chrome), Some(window)) = (&mut self.chrome, &self.window) {
            let scale = window.scale_factor().factor() as f32;
            let logical_w = size.width as f32 / scale;
            chrome.set_window_width(logical_w);
        }

        // Update overlay manager viewport for dialog placement.
        if let Some(window) = &self.window {
            let scale = window.scale_factor().factor() as f32;
            let logical_w = size.width as f32 / scale;
            let logical_h = size.height as f32 / scale;
            self.overlays.set_viewport(oriterm_ui::geometry::Rect::new(
                0.0, 0.0, logical_w, logical_h,
            ));
        }

        // Recompute grid dimensions, resize terminal + PTY + increments.
        self.sync_grid_layout(size.width, size.height);

        // Update platform hit test rects on Windows.
        #[cfg(target_os = "windows")]
        if let (Some(chrome), Some(window)) = (&self.chrome, &self.window) {
            let scale = window.scale_factor().factor() as f32;
            oriterm_ui::platform_windows::set_client_rects(
                window.window(),
                chrome
                    .interactive_rects()
                    .iter()
                    .map(|r| scale_rect(*r, scale))
                    .collect(),
            );
        }

        self.url_cache.invalidate();
        self.hovered_url = None; // Segments contain stale absolute rows.
        self.dirty = true;
    }
}
