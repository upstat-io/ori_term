//! URL hover detection and click handling.
//!
//! Detects implicitly detected URLs (regex-based) and explicit OSC 8 hyperlinks
//! under the mouse cursor when Ctrl is held. On Ctrl+click, opens the URL in the
//! system browser. Provides cursor icon feedback (pointer vs default).
//!
//! All grid data is read from [`PaneSnapshot`] — no `pane.terminal().lock()`.

use winit::dpi::PhysicalPosition;
use winit::window::CursorIcon;

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
    /// (from snapshot `hyperlink_uri`) first, then falls back to implicit URL
    /// detection via the cache operating on snapshot cell data.
    pub(super) fn detect_hover_url(&mut self, pos: PhysicalPosition<f64>) -> HoverResult {
        let no_hit = HoverResult {
            cursor_icon: CursorIcon::Default,
            url: None,
        };

        if !self.modifiers.control_key() {
            return no_hit;
        }

        let Some(pane_id) = self.active_pane_id() else {
            return no_hit;
        };
        let Some(renderer) = &self.renderer else {
            return no_hit;
        };
        let Some(ctx) = self.focused_ctx() else {
            return no_hit;
        };

        let grid_ctx = GridCtx {
            widget: &ctx.terminal_grid,
            cell: renderer.cell_metrics(),
            word_delimiters: &self.config.behavior.word_delimiters,
        };

        let Some((col, line)) = mouse_selection::pixel_to_cell(pos, &grid_ctx) else {
            return no_hit;
        };

        let Some(snapshot) = self.mux.as_ref().and_then(|m| m.pane_snapshot(pane_id)) else {
            return no_hit;
        };

        // Bounds check against snapshot viewport.
        let wire_cell = snapshot.cells.get(line).and_then(|row| row.get(col));
        let Some(wire_cell) = wire_cell else {
            return no_hit;
        };

        // Implicit URL detection via cache, operating on snapshot cells.
        let url_hit = {
            let Some(ctx) = self
                .focused_window_id
                .and_then(|id| self.windows.get_mut(&id))
            else {
                return no_hit;
            };
            ctx.url_cache.url_at_snapshot(snapshot, line, col)
        };

        if let Some(hit) = url_hit {
            return HoverResult {
                cursor_icon: CursorIcon::Pointer,
                url: Some(hit),
            };
        }

        // OSC 8 hyperlink fallback: only used when implicit detection misses.
        if let Some(uri) = &wire_cell.hyperlink_uri {
            return HoverResult {
                cursor_icon: CursorIcon::Pointer,
                url: Some(DetectedUrl {
                    segments: vec![],
                    url: uri.clone(),
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
        let prev_url = self
            .focused_ctx()
            .and_then(|ctx| ctx.hovered_url.as_ref().map(|u| &u.url));
        let new_url = result.url.as_ref().map(|u| &u.url);

        if prev_url != new_url {
            if let Some(ctx) = self.focused_ctx() {
                ctx.window.window().set_cursor(result.cursor_icon);
            }
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.hovered_url = result.url;
                ctx.dirty = true;
            }
        }
    }

    /// Clear the hovered URL state.
    ///
    /// Called when Ctrl is released or cursor leaves the grid.
    pub(super) fn clear_url_hover(&mut self) {
        let is_hovered = self
            .focused_ctx()
            .is_some_and(|ctx| ctx.hovered_url.is_some());
        if is_hovered {
            if let Some(ctx) = self.focused_ctx() {
                ctx.window.window().set_cursor(CursorIcon::Default);
            }
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.hovered_url = None;
                ctx.dirty = true;
            }
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
        let Some(ctx) = self.focused_ctx() else {
            return false;
        };
        let Some(url) = &ctx.hovered_url else {
            return false;
        };
        if let Err(e) = crate::platform::url::open_url(&url.url) {
            log::warn!("failed to open URL: {e}");
        }
        true
    }

    /// Fill `out` with hovered URL segments in viewport-relative coordinates.
    ///
    /// Clears `out` and pushes viewport-mapped segments, reusing the Vec's
    /// existing capacity. Uses snapshot metadata for coordinate conversion.
    pub(super) fn fill_hovered_url_viewport_segments(&self, out: &mut Vec<UrlSegment>) {
        out.clear();

        let Some(ctx) = self.focused_ctx() else {
            return;
        };
        let Some(url) = &ctx.hovered_url else {
            return;
        };
        if url.segments.is_empty() {
            // OSC 8 hyperlink — no implicit segments to render.
            return;
        }

        let pane_id = match self.active_pane_id() {
            Some(id) => id,
            None => return,
        };
        let Some(snapshot) = self.mux.as_ref().and_then(|m| m.pane_snapshot(pane_id)) else {
            return;
        };

        let base = snapshot.stable_row_base as usize;
        let lines = snapshot.cells.len();

        // Convert absolute rows to viewport lines.
        for &(abs_row, start_col, end_col) in &url.segments {
            if abs_row < base {
                continue; // Above visible viewport.
            }
            let vp_line = abs_row - base;
            if vp_line >= lines {
                continue; // Below visible viewport.
            }
            out.push((vp_line, start_col, end_col));
        }
    }
}
