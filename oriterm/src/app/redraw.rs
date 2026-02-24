//! Three-phase rendering pipeline: Extract → Prepare → Render.

use std::cell::Cell;
use std::time::Instant;

use unicode_width::UnicodeWidthChar;

use oriterm_core::{CellFlags, Column, CursorShape, RenderableContent, TermMode};

use oriterm_ui::draw::DrawList;
use oriterm_ui::theme::UiTheme;
use oriterm_ui::widgets::window_chrome::WindowChromeWidget;
use oriterm_ui::widgets::{DrawCtx, Widget};

use super::App;
use super::mouse_selection::{self, GridCtx};
use crate::gpu::{
    FrameSelection, MarkCursorOverride, SurfaceError, ViewportSize, extract_frame,
    extract_frame_into,
};
use crate::widgets::terminal_grid::TerminalGridWidget;

/// Stub text measurer for chrome drawing (no text measurement needed for
/// geometric symbols, but the trait is required by `DrawCtx`).
pub(super) struct NullMeasurer;

impl oriterm_ui::widgets::TextMeasurer for NullMeasurer {
    fn measure(
        &self,
        _text: &str,
        _style: &oriterm_ui::text::TextStyle,
        _max_width: f32,
    ) -> oriterm_ui::text::TextMetrics {
        oriterm_ui::text::TextMetrics {
            width: 0.0,
            height: 0.0,
            line_count: 0,
        }
    }

    fn shape(
        &self,
        _text: &str,
        _style: &oriterm_ui::text::TextStyle,
        _max_width: f32,
    ) -> oriterm_ui::text::ShapedText {
        oriterm_ui::text::ShapedText {
            glyphs: Vec::new(),
            width: 0.0,
            height: 0.0,
            baseline: 0.0,
        }
    }
}

impl App {
    /// Execute the three-phase rendering pipeline: Extract → Prepare → Render.
    pub(super) fn handle_redraw(&mut self) {
        log::trace!("RedrawRequested");
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
                frame.preedit.clone_from(&self.ime.preedit);
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

            // Snapshot selection for rendering (Tab owns selection, not Term).
            frame.selection = tab
                .selection()
                .map(|sel| FrameSelection::new(sel, frame.content.stable_row_base));

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
            );
            if chrome_animating {
                self.dirty = true;
            }

            renderer.render_to_surface(gpu, window.surface())
        };

        self.handle_render_result(render_result);

        // Keep the IME candidate window positioned at the cursor while
        // composition is active. Called after rendering to avoid borrow
        // conflicts with the renderer's mutable borrow.
        if !self.ime.preedit.is_empty() {
            self.update_ime_cursor_area();
        }
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
    fn draw_chrome(
        chrome: Option<&WindowChromeWidget>,
        renderer: &mut crate::gpu::GpuRenderer,
        draw_list: &mut DrawList,
        logical_width: u32,
        scale: f32,
    ) -> bool {
        let Some(chrome) = chrome else {
            return false;
        };
        if !chrome.is_visible() {
            return false;
        }

        draw_list.clear();
        let animations_running = Cell::new(false);
        let measurer = NullMeasurer;
        let theme = UiTheme::dark();
        let caption_h = chrome.caption_height();
        let bounds = oriterm_ui::geometry::Rect::new(0.0, 0.0, logical_width as f32, caption_h);

        let mut ctx = DrawCtx {
            measurer: &measurer,
            draw_list,
            bounds,
            focused_widget: None,
            now: Instant::now(),
            animations_running: &animations_running,
            theme: &theme,
        };
        chrome.draw(&mut ctx);

        renderer.append_ui_draw_list(draw_list, scale);
        animations_running.get()
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
