//! Window chrome: action dispatch, event routing, and shared helpers.
//!
//! Handles `WidgetAction::WindowMinimize`, `WindowMaximize`, and
//! `WindowClose` by forwarding to the appropriate winit window operations.
//! Routes mouse and hover events to the chrome widget, and provides shared
//! geometry helpers used by both init and resize.

use winit::event::ElementState;
use winit::event_loop::ActiveEventLoop;

#[cfg(target_os = "windows")]
use oriterm_ui::widgets::window_chrome::WindowChromeWidget;
use oriterm_ui::widgets::{Widget, WidgetAction};

use super::App;
use super::redraw::NullMeasurer;

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
                if let Some(gpu) = &self.gpu {
                    gpu.save_pipeline_cache_async();
                }
                // Exit immediately. wgpu Device::drop() calls
                // vkDeviceWaitIdle() which blocks for seconds — the OS
                // reclaims all GPU resources on process exit anyway.
                std::process::exit(0);
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
            modifiers: oriterm_ui::input::Modifiers::NONE,
        };
        let logical_w = window.size_px().0 as f32 / scale;
        let measurer = NullMeasurer;
        let theme = oriterm_ui::theme::UiTheme::dark();
        let ctx = oriterm_ui::widgets::EventCtx {
            measurer: &measurer,
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
        false
    }

    /// Clear chrome hover state when the cursor leaves the window.
    pub(super) fn clear_chrome_hover(&mut self) {
        let Some(chrome) = &mut self.chrome else {
            return;
        };
        let measurer = NullMeasurer;
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
        let scale = window.scale_factor().factor() as f32;
        let logical =
            oriterm_ui::geometry::Point::new(position.x as f32 / scale, position.y as f32 / scale);
        let measurer = NullMeasurer;
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

    /// Handle window resize: reconfigure surface, update chrome layout,
    /// resize grid and PTY.
    pub(super) fn handle_resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
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

        let (Some(gpu), Some(window), Some(renderer)) =
            (&self.gpu, &mut self.window, &self.renderer)
        else {
            return;
        };

        window.resize_surface(size.width, size.height, gpu);
        let cell = renderer.cell_metrics();
        let scale = window.scale_factor().factor() as f32;

        // Snap resize to cell boundaries when configured.
        if self.config.window.resize_increments {
            let inc = winit::dpi::PhysicalSize::new(
                cell.width.round() as u32,
                cell.height.round() as u32,
            );
            window.window().set_resize_increments(Some(inc));
        }

        // Update chrome layout for new window width.
        let caption_height = if let Some(chrome) = &mut self.chrome {
            let logical_w = size.width as f32 / scale;
            chrome.set_window_width(logical_w);
            chrome.caption_height()
        } else {
            0.0
        };

        // Grid viewport excludes caption height. Cell metrics are in physical
        // pixels (rasterized at physical DPI), so use physical dimensions.
        let caption_px = (caption_height * scale).round() as u32;
        let grid_h = size.height.saturating_sub(caption_px);
        let cols = cell.columns(size.width).max(1);
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

        // Update platform hit test rects on Windows.
        #[cfg(target_os = "windows")]
        if let Some(chrome) = &self.chrome {
            oriterm_ui::platform_windows::set_client_rects(
                window.window(),
                chrome
                    .interactive_rects()
                    .iter()
                    .map(|r| scale_rect(*r, scale))
                    .collect(),
            );
        }

        let (r, c) = (rows as u16, cols as u16);
        if let Some(tab) = &self.tab {
            // Grid and PTY resize together so the shell always knows the
            // current dimensions. Desynchronized resize (throttled PTY)
            // causes the shell to write content at stale dimensions,
            // producing duplicate/ghost content after reflow.
            tab.resize_grid(r, c);
            tab.resize_pty(r, c);
        }

        self.dirty = true;
    }
}
