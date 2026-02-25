//! Three-phase rendering pipeline: Extract → Prepare → Render.

use std::cell::Cell;
use std::time::Instant;

use unicode_width::UnicodeWidthChar;

use oriterm_core::{CellFlags, Column, CursorShape, RenderableContent, TermMode};

use oriterm_ui::draw::DrawList;
use oriterm_ui::geometry::Point;
use oriterm_ui::overlay::OverlayManager;
use oriterm_ui::theme::UiTheme;
use oriterm_ui::widgets::status_badge::StatusBadge;
use oriterm_ui::widgets::window_chrome::WindowChromeWidget;
use oriterm_ui::widgets::{DrawCtx, Widget};

use super::App;
use super::mouse_selection::{self, GridCtx};
use crate::font::UiFontMeasurer;
use crate::gpu::state::GpuState;
use crate::gpu::{
    FrameSearch, FrameSelection, MarkCursorOverride, SurfaceError, ViewportSize, extract_frame,
    extract_frame_into,
};
use crate::widgets::terminal_grid::TerminalGridWidget;

impl App {
    /// Execute the three-phase rendering pipeline: Extract → Prepare → Render.
    #[expect(
        clippy::too_many_lines,
        reason = "linear three-phase pipeline: Extract → Prepare → Render"
    )]
    pub(super) fn handle_redraw(&mut self) {
        log::trace!("RedrawRequested");

        // Compute URL hover segments before the render block (which borrows
        // self.renderer mutably). Take the Vec from the previous frame to
        // reuse its capacity, avoiding a per-frame allocation.
        let mut url_segments = self
            .frame
            .as_mut()
            .map_or_else(Vec::new, |f| std::mem::take(&mut f.hovered_url_segments));
        self.fill_hovered_url_viewport_segments(&mut url_segments);

        let render_result = {
            let Some(gpu) = self.gpu.as_ref() else {
                log::warn!("redraw: no gpu");
                return;
            };
            let Some(renderer) = self.renderer.as_mut() else {
                log::warn!("redraw: no renderer");
                return;
            };
            let Some(window) = self.window.as_ref() else {
                log::warn!("redraw: no window");
                return;
            };
            let Some(tab) = self.tab.as_ref() else {
                log::warn!("redraw: no tab");
                return;
            };

            if !window.has_surface_area() {
                log::warn!("redraw: no surface area");
                return;
            }

            let (w, h) = window.size_px();
            let viewport = ViewportSize::new(w, h);
            let cell = renderer.cell_metrics();

            // Reuse the FrameInput allocation across frames. First frame
            // does a fresh allocation; subsequent frames refill in place.
            let frame = match &mut self.frame {
                Some(existing) => {
                    extract_frame_into(tab.terminal(), existing, viewport, cell);
                    existing
                }
                slot @ None => {
                    *slot = Some(extract_frame(tab.terminal(), viewport, cell));
                    slot.as_mut().expect("just assigned")
                }
            };

            // Set window opacity from config (extract phase doesn't have
            // access to config — opacity is a window concern, not terminal state).
            frame.palette.opacity = self.config.window.effective_opacity();

            // IME preedit: overlay composition text at the cursor position
            // (underlined) so it flows through the normal shaping pipeline.
            if !self.ime.preedit.is_empty() {
                let cols = frame.columns();
                overlay_preedit_cells(&self.ime.preedit, &mut frame.content, cols);
            }

            // Mark-mode cursor override: hollow block at the mark position.
            frame.mark_cursor = tab.mark_cursor().and_then(|mc| {
                let (line, col) = mc.to_viewport(frame.content.stable_row_base, frame.rows())?;
                Some(MarkCursorOverride {
                    line,
                    column: Column(col),
                    shape: CursorShape::HollowBlock,
                })
            });

            // Snapshot selection and search for rendering (Tab owns both, not Term).
            let base = frame.content.stable_row_base;
            frame.selection = tab.selection().map(|sel| FrameSelection::new(sel, base));
            frame.search = tab.search().map(|s| FrameSearch::new(s, base));

            // Compute hovered cell for hyperlink underline rendering.
            if let Some(grid_widget) = self.terminal_grid.as_ref() {
                let ctx = GridCtx {
                    widget: grid_widget,
                    cell,
                    word_delimiters: &self.config.behavior.word_delimiters,
                };
                frame.hovered_cell = mouse_selection::pixel_to_cell(self.mouse.cursor_pos(), &ctx)
                    .map(|(col, line)| (line, col));
            }

            // Implicit URL hover: viewport-relative segments computed above.
            // The Vec was taken from the previous frame to reuse capacity.
            frame.hovered_url_segments = url_segments;

            // Cache blinking mode; reset on false→true transition.
            let blinking_now = frame.content.mode.contains(TermMode::CURSOR_BLINKING);
            if blinking_now && !self.blinking_active {
                self.cursor_blink.reset();
            }
            self.blinking_active = blinking_now;

            // Cursor blink: "off" phase hides cursor for prepare phase.
            let cursor_blink_visible = !blinking_now || self.cursor_blink.is_visible();

            // Grid origin from layout bounds. When the layout engine
            // positions the grid (e.g. below a tab bar), this shifts all
            // cell rendering. Both bounds and cell metrics are in physical
            // pixels; the viewport (screen_size uniform) is also physical,
            // so the shader maps physical positions to NDC correctly.
            let origin = self
                .terminal_grid
                .as_ref()
                .and_then(TerminalGridWidget::bounds)
                .map_or((0.0, 0.0), |b| (b.x(), b.y()));

            renderer.prepare(frame, gpu, origin, cursor_blink_visible);

            // Draw window chrome into the UI rect layer. Chrome widget
            // draws in logical pixels; scale converts to physical pixels
            // for the GPU pipeline (screen_size uniform is physical).
            let scale = window.scale_factor().factor() as f32;
            let logical_w = (w as f32 / scale).round() as u32;
            let chrome_animating = Self::draw_chrome(
                self.chrome.as_ref(),
                renderer,
                &mut self.chrome_draw_list,
                logical_w,
                scale,
                &self.ui_theme,
            );
            if chrome_animating {
                self.dirty = true;
            }

            // Draw tab bar below the chrome caption. Tab bar contains text
            // (tab titles), so uses the text-aware draw list conversion.
            let caption_h = self
                .chrome
                .as_ref()
                .map_or(0.0, WindowChromeWidget::caption_height);
            if Self::draw_tab_bar(
                self.tab_bar.as_ref(),
                renderer,
                &mut self.chrome_draw_list,
                logical_w as f32,
                caption_h,
                scale,
                gpu,
                &self.ui_theme,
            ) {
                self.dirty = true;
            }

            // Draw modal overlays (e.g. paste confirmation dialog).
            let logical_size = (logical_w as f32, h as f32 / scale);
            if Self::draw_overlays(
                &mut self.overlays,
                renderer,
                &mut self.chrome_draw_list,
                logical_size,
                scale,
                gpu,
                &self.ui_theme,
            ) {
                self.dirty = true;
            }

            // Draw search bar overlay when search is active.
            if let Some(search) = frame.search.as_ref() {
                // Position below all chrome (caption + tab bar).
                let chrome_h = caption_h
                    + if self.tab_bar.is_some() {
                        oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT
                    } else {
                        0.0
                    };
                Self::draw_search_bar(
                    search,
                    renderer,
                    &mut self.chrome_draw_list,
                    logical_w as f32,
                    chrome_h,
                    scale,
                    gpu,
                );
            }

            renderer.render_to_surface(gpu, window.surface())
        };

        self.handle_render_result(render_result);

        // Keep the IME candidate window positioned at the terminal cursor.
        // Called every frame (not just during preedit) so Windows knows the
        // cursor area before composition starts — otherwise the candidate
        // popup defaults to the bottom-right corner (Alacritty pattern).
        self.update_ime_cursor_area();
    }

    /// Handle the result of a render pass, recovering from surface loss.
    fn handle_render_result(&mut self, result: Result<(), SurfaceError>) {
        match result {
            Ok(()) => log::trace!("render ok"),
            Err(SurfaceError::Lost) => {
                log::warn!("surface lost, reconfiguring");
                if let (Some(window), Some(gpu)) = (self.window.as_mut(), self.gpu.as_ref()) {
                    let (w, h) = window.size_px();
                    window.resize_surface(w, h, gpu);
                }
            }
            Err(e) => log::error!("render error: {e}"),
        }
    }

    /// Draw window chrome into the renderer's UI rect layer.
    ///
    /// Chrome widget coordinates are in logical pixels. The `scale` factor
    /// converts logical draw list positions to physical pixels for the GPU
    /// pipeline (`screen_size` uniform is physical).
    ///
    /// Returns `true` if chrome has running animations that need continued
    /// redraws. The `draw_list` is cleared and reused across frames to
    /// avoid per-frame allocation.
    #[expect(
        clippy::too_many_arguments,
        reason = "chrome drawing: widget, renderer, draw list, viewport, scale, theme"
    )]
    fn draw_chrome(
        chrome: Option<&WindowChromeWidget>,
        renderer: &mut crate::gpu::GpuRenderer,
        draw_list: &mut DrawList,
        logical_width: u32,
        scale: f32,
        theme: &UiTheme,
    ) -> bool {
        let Some(chrome) = chrome else {
            return false;
        };
        if !chrome.is_visible() {
            return false;
        }

        // Build draw list with real measurer (immutable borrow on renderer
        // ends after chrome.draw — NLL lets the mutable append follow).
        draw_list.clear();
        let animations_running = Cell::new(false);
        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
        let caption_h = chrome.caption_height();
        let bounds = oriterm_ui::geometry::Rect::new(0.0, 0.0, logical_width as f32, caption_h);

        let mut ctx = DrawCtx {
            measurer: &measurer,
            draw_list,
            bounds,
            focused_widget: None,
            now: Instant::now(),
            animations_running: &animations_running,
            theme,
        };
        chrome.draw(&mut ctx);
        let animating = animations_running.get();

        // Chrome uses geometric symbols only — no text context needed.
        renderer.append_ui_draw_list(draw_list, scale);
        animating
    }

    /// Draw the tab bar below the window chrome caption.
    ///
    /// Tab bar coordinates are in logical pixels, positioned at `y = caption_h`.
    /// Uses [`append_ui_draw_list_with_text`](crate::gpu::GpuRenderer::append_ui_draw_list_with_text)
    /// because tab titles are rendered as shaped text.
    ///
    /// Returns `true` if the tab bar has running animations (e.g. bell pulse).
    #[expect(
        clippy::too_many_arguments,
        reason = "tab bar drawing: widget, renderer, draw list, viewport, caption offset, scale, GPU, theme"
    )]
    fn draw_tab_bar(
        tab_bar: Option<&oriterm_ui::widgets::tab_bar::TabBarWidget>,
        renderer: &mut crate::gpu::GpuRenderer,
        draw_list: &mut DrawList,
        logical_width: f32,
        caption_h: f32,
        scale: f32,
        gpu: &GpuState,
        theme: &UiTheme,
    ) -> bool {
        let Some(tab_bar) = tab_bar else {
            return false;
        };
        if tab_bar.tab_count() == 0 {
            return false;
        }

        let tab_bar_h = oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT;
        let bounds = oriterm_ui::geometry::Rect::new(0.0, caption_h, logical_width, tab_bar_h);

        draw_list.clear();
        let animations_running = Cell::new(false);
        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);

        let mut ctx = DrawCtx {
            measurer: &measurer,
            draw_list,
            bounds,
            focused_widget: None,
            now: Instant::now(),
            animations_running: &animations_running,
            theme,
        };
        tab_bar.draw(&mut ctx);
        let animating = animations_running.get();

        // Tab bar contains text — use text-aware conversion to rasterize
        // tab title glyphs into the UI overlay layer.
        renderer.append_ui_draw_list_with_text(draw_list, scale, gpu);
        animating
    }

    /// Draw modal overlays (e.g. paste confirmation dialog).
    ///
    /// Overlay coordinates are in logical pixels. The `scale` factor converts
    /// to physical pixels for the GPU pipeline. Reuses the chrome draw list
    /// buffer to avoid extra allocation.
    ///
    /// Uses [`append_ui_draw_list_with_text`](crate::gpu::GpuRenderer::append_ui_draw_list_with_text)
    /// so dialog text (title, message, button labels) renders with real glyphs.
    ///
    /// Returns `true` if overlays have running animations. Returns `false`
    /// immediately if no overlays are active.
    #[expect(
        clippy::too_many_arguments,
        reason = "overlay drawing: manager, renderer, draw list, viewport, scale, GPU, theme"
    )]
    fn draw_overlays(
        overlays: &mut OverlayManager,
        renderer: &mut crate::gpu::GpuRenderer,
        draw_list: &mut DrawList,
        logical_size: (f32, f32),
        scale: f32,
        gpu: &GpuState,
        theme: &UiTheme,
    ) -> bool {
        if overlays.is_empty() {
            return false;
        }

        // Build draw list with real measurer (immutable borrow on renderer
        // ends after draw_overlays — NLL lets the mutable append follow).
        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);

        overlays.layout_overlays(&measurer, theme);

        draw_list.clear();
        let animations_running = Cell::new(false);
        let bounds = oriterm_ui::geometry::Rect::new(0.0, 0.0, logical_size.0, logical_size.1);
        let mut ctx = DrawCtx {
            measurer: &measurer,
            draw_list,
            bounds,
            focused_widget: None,
            now: Instant::now(),
            animations_running: &animations_running,
            theme,
        };
        overlays.draw_overlays(&mut ctx);
        let animating = animations_running.get();

        // Overlays contain text — use text-aware conversion to rasterize
        // and emit glyph instances for titles, messages, and button labels.
        renderer.append_ui_draw_list_with_text(draw_list, scale, gpu);
        animating
    }

    /// Draw the search bar overlay above the grid area.
    ///
    /// Shows the current query and match count ("N of M") as a floating
    /// [`StatusBadge`]. Coordinates are in logical pixels; `scale` converts
    /// to physical pixels for the GPU pipeline.
    #[expect(
        clippy::too_many_arguments,
        reason = "search bar drawing: search state, renderer, draw list, viewport, caption, scale, GPU"
    )]
    fn draw_search_bar(
        search: &FrameSearch,
        renderer: &mut crate::gpu::GpuRenderer,
        draw_list: &mut DrawList,
        logical_width: f32,
        caption_h: f32,
        scale: f32,
        gpu: &GpuState,
    ) {
        let query = search.query();
        let text = if query.is_empty() {
            String::from("Search: ")
        } else if search.match_count() == 0 {
            format!("Search: {query}  No matches")
        } else {
            format!(
                "Search: {query}  {} of {}",
                search.focused_display(),
                search.match_count()
            )
        };

        let badge = StatusBadge::new(&text);

        // Shape text and measure badge (immutable borrow on renderer ends
        // after shape — NLL lets the mutable append follow).
        let max_text_w = logical_width * 0.4;
        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
        let (w, _h) = badge.measure(&measurer, max_text_w);

        // Position: top-right of grid area, inset from edges.
        let margin = 8.0;
        let pos = Point::new(logical_width - w - margin, caption_h + margin);

        draw_list.clear();
        let _ = badge.draw(draw_list, &measurer, pos, max_text_w);

        renderer.append_ui_draw_list_with_text(draw_list, scale, gpu);
    }
}

/// Overlay IME preedit characters into the renderable content at the cursor.
///
/// Replaces cells at the cursor position with preedit characters, adding
/// [`CellFlags::UNDERLINE`] to visually distinguish composition text from
/// committed text. Wide (CJK) characters occupy two cells; the spacer cell
/// gets [`CellFlags::WIDE_CHAR_SPACER`]. Characters beyond the grid width
/// are clipped.
///
/// The content's cursor visibility is set to `false` so the prepare phase
/// does not emit a cursor on top of the preedit text.
pub(super) fn overlay_preedit_cells(preedit: &str, content: &mut RenderableContent, cols: usize) {
    if content.cells.is_empty() || cols == 0 {
        return;
    }

    let line = content.cursor.line;
    let start_col = content.cursor.column.0;

    // Hide the terminal cursor while preedit is active.
    content.cursor.visible = false;

    let mut col = start_col;
    for ch in preedit.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w == 0 {
            continue;
        }
        if col >= cols {
            break;
        }

        let idx = line * cols + col;
        if idx >= content.cells.len() {
            break;
        }

        // Preserve the cell's colors but replace character and add underline.
        let cell = &mut content.cells[idx];
        cell.ch = ch;
        cell.flags = (cell.flags
            - CellFlags::WIDE_CHAR
            - CellFlags::WIDE_CHAR_SPACER
            - CellFlags::LEADING_WIDE_CHAR_SPACER)
            | CellFlags::UNDERLINE;
        cell.zerowidth.clear();

        if w == 2 {
            cell.flags |= CellFlags::WIDE_CHAR;
            // Mark the next cell as a spacer for the wide character.
            if col + 1 < cols {
                let spacer_idx = idx + 1;
                if spacer_idx < content.cells.len() {
                    let spacer = &mut content.cells[spacer_idx];
                    spacer.ch = ' ';
                    spacer.flags = CellFlags::WIDE_CHAR_SPACER;
                    spacer.zerowidth.clear();
                }
            }
        }

        col += w;
    }
}
