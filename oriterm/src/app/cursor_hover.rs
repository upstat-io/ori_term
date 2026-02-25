//! URL hover detection and click handling.
//!
//! Detects implicitly detected URLs (regex-based) and explicit OSC 8 hyperlinks
//! under the mouse cursor when Ctrl is held. On Ctrl+click, opens the URL in the
//! system browser. Provides cursor icon feedback (pointer vs default).

use winit::dpi::PhysicalPosition;
use winit::window::CursorIcon;

use oriterm_core::Column;

use super::App;
use super::mouse_selection::{self, GridCtx};
use crate::url_detect::{DetectedUrl, UrlSegment};

/// Result of hover URL detection at the current cursor position.
pub(super) struct HoverResult {
    /// Cursor icon to display.
    pub cursor_icon: CursorIcon,
    /// The hovered URL, if any.
    pub url: Option<DetectedUrl>,
}

impl App {
    /// Detect URL under the cursor when Ctrl is held.
    ///
    /// Converts the pixel position to a grid cell, checks for OSC 8 hyperlinks
    /// first, then falls back to implicit URL detection via the cache.
    pub(super) fn detect_hover_url(&mut self, pos: PhysicalPosition<f64>) -> HoverResult {
        let no_hit = HoverResult {
            cursor_icon: CursorIcon::Default,
            url: None,
        };

        if !self.modifiers.control_key() {
            return no_hit;
        }

        let (Some(tab), Some(grid_widget), Some(renderer)) =
            (&self.tab, &self.terminal_grid, &self.renderer)
        else {
            return no_hit;
        };

        let ctx = GridCtx {
            widget: grid_widget,
            cell: renderer.cell_metrics(),
            word_delimiters: &self.config.behavior.word_delimiters,
        };

        let Some((col, line)) = mouse_selection::pixel_to_cell(pos, &ctx) else {
            return no_hit;
        };

        let term = tab.terminal().lock();
        let grid = term.grid();
        let abs_row = grid.scrollback().len() + line - grid.display_offset();

        let Some(row) = grid.absolute_row(abs_row) else {
            return no_hit;
        };

        if col >= row.cols() {
            return no_hit;
        }

        // Implicit URL detection first — it joins wrapped lines and finds
        // complete URLs even when the emitting program truncated them.
        let url_hit = self.url_cache.url_at(grid, abs_row, col);

        // OSC 8 hyperlink fallback: only used when implicit detection misses.
        let osc8_url = if url_hit.is_none() {
            row[Column(col)].hyperlink().map(|h| h.uri.clone())
        } else {
            None
        };
        drop(term);

        if let Some(hit) = url_hit {
            return HoverResult {
                cursor_icon: CursorIcon::Pointer,
                url: Some(hit),
            };
        }

        if let Some(uri) = osc8_url {
            return HoverResult {
                cursor_icon: CursorIcon::Pointer,
                url: Some(DetectedUrl {
                    segments: vec![],
                    url: uri,
                }),
            };
        }

        no_hit
    }

    /// Update hover state on cursor move.
    ///
    /// Called from the `CursorMoved` event handler. Updates the hovered URL,
    /// cursor icon, and requests a redraw if the hover state changed.
    pub(super) fn update_url_hover(&mut self, position: PhysicalPosition<f64>) {
        let result = self.detect_hover_url(position);
        let prev_url = self.hovered_url.as_ref().map(|u| &u.url);
        let new_url = result.url.as_ref().map(|u| &u.url);

        if prev_url != new_url {
            self.hovered_url = result.url;
            if let Some(window) = &self.window {
                window.window().set_cursor(result.cursor_icon);
            }
            self.dirty = true;
        }
    }

    /// Clear the hovered URL state.
    ///
    /// Called when Ctrl is released or cursor leaves the grid.
    pub(super) fn clear_url_hover(&mut self) {
        if self.hovered_url.is_some() {
            self.hovered_url = None;
            if let Some(window) = &self.window {
                window.window().set_cursor(CursorIcon::Default);
            }
            self.dirty = true;
        }
    }

    /// Handle Ctrl+click on a hovered URL.
    ///
    /// Opens the URL in the system browser if one is currently hovered.
    /// Returns `true` if the click was consumed (URL opened).
    pub(super) fn try_open_hovered_url(&self) -> bool {
        if !self.modifiers.control_key() {
            return false;
        }
        let Some(url) = &self.hovered_url else {
            return false;
        };
        if let Err(e) = crate::platform::url::open_url(&url.url) {
            log::warn!("failed to open URL: {e}");
        }
        true
    }

    /// Convert hovered URL segments to viewport-relative coordinates.
    ///
    /// Used by the frame extraction to pass hover info to the renderer.
    /// Returns empty vec if no URL is hovered or segments can't be mapped.
    pub(super) fn hovered_url_viewport_segments(&self) -> Vec<UrlSegment> {
        let Some(url) = &self.hovered_url else {
            return Vec::new();
        };
        if url.segments.is_empty() {
            // OSC 8 hyperlink — no implicit segments to render.
            return Vec::new();
        }
        let Some(tab) = &self.tab else {
            return Vec::new();
        };
        let term = tab.terminal().lock();
        let grid = term.grid();
        let sb_len = grid.scrollback().len();
        let display_offset = grid.display_offset();

        // Convert absolute rows to viewport lines.
        let mut viewport_segments = Vec::new();
        for &(abs_row, start_col, end_col) in &url.segments {
            // viewport_line = abs_row - (scrollback_len - display_offset)
            let base = sb_len.saturating_sub(display_offset);
            if abs_row < base {
                continue; // Above visible viewport.
            }
            let vp_line = abs_row - base;
            if vp_line >= grid.lines() {
                continue; // Below visible viewport.
            }
            viewport_segments.push((vp_line, start_col, end_col));
        }
        viewport_segments
    }
}
