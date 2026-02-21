//! Three-phase rendering pipeline: Extract → Prepare → Render.

use oriterm_core::{Column, CursorShape, TermMode};

use super::App;
use crate::gpu::{FrameSelection, SurfaceError, ViewportSize, extract_frame, extract_frame_into};
use crate::widgets::terminal_grid::TerminalGridWidget;

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

            // Override cursor for mark mode: show a hollow block at the mark
            // cursor position so the user can see where they are navigating.
            if let Some(mc) = tab.mark_cursor() {
                if let Some((line, col)) =
                    mc.to_viewport(frame.content.stable_row_base, frame.rows())
                {
                    frame.content.cursor.line = line;
                    frame.content.cursor.column = Column(col);
                    frame.content.cursor.shape = CursorShape::HollowBlock;
                    frame.content.cursor.visible = true;
                }
            }

            // Snapshot selection for rendering. The selection lives on Tab
            // (not inside Term), so we build the FrameSelection after the
            // terminal lock is released, using the stable_row_base from
            // the extracted content.
            frame.selection = tab
                .selection()
                .map(|sel| FrameSelection::new(sel, frame.content.stable_row_base));

            // Cache blinking mode for about_to_wait gating.
            // Reset blink phase on false→true transition so the
            // cursor starts visible when blinking is first enabled.
            let blinking_now = frame.content.mode.contains(TermMode::CURSOR_BLINKING);
            if blinking_now && !self.blinking_active {
                self.cursor_blink.reset();
            }
            self.blinking_active = blinking_now;

            // Cursor blink: the "off" phase hides the cursor. This flag is
            // passed to the Prepare phase which gates cursor emission —
            // the extracted frame is never mutated between Extract and Prepare.
            let cursor_blink_visible = !blinking_now || self.cursor_blink.is_visible();

            // Grid origin from layout bounds. When the layout engine
            // positions the grid (e.g. below a tab bar), this shifts all
            // cell rendering.
            let origin = self
                .terminal_grid
                .as_ref()
                .and_then(TerminalGridWidget::bounds)
                .map_or((0.0, 0.0), |b| (b.x(), b.y()));

            renderer.prepare(frame, gpu, origin, cursor_blink_visible);
            renderer.render_to_surface(gpu, window.surface())
        };

        match render_result {
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
}
