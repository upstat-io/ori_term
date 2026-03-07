//! Three-phase rendering pipeline: Extract → Prepare → Render.

mod draw_helpers;
mod multi_pane;
pub(in crate::app) mod preedit;
mod search_bar;

use oriterm_core::{Column, CursorShape, TermMode};

use super::App;
use super::mouse_selection::{self, GridCtx};
use crate::gpu::{
    FrameSearch, FrameSelection, MarkCursorOverride, SurfaceError, ViewportSize,
    extract_frame_from_snapshot, extract_frame_from_snapshot_into, snapshot_palette,
};

impl App {
    /// Execute the three-phase rendering pipeline: Extract → Prepare → Render.
    #[expect(
        clippy::too_many_lines,
        reason = "linear three-phase pipeline: Extract → Prepare → Render"
    )]
    pub(super) fn handle_redraw(&mut self) {
        log::trace!("RedrawRequested");

        // Compute URL hover segments before the render block (which borrows
        // ctx.renderer mutably). Take the Vec from the previous frame to
        // reuse its capacity, avoiding a per-frame allocation.
        let mut url_segments = self
            .focused_ctx_mut()
            .and_then(|ctx| ctx.frame.as_mut())
            .map_or_else(Vec::new, |f| std::mem::take(&mut f.hovered_url_segments));
        self.fill_hovered_url_viewport_segments(&mut url_segments);

        // Multi-pane check: if the active tab has splits, dispatch to the
        // multi-pane renderer which iterates all panes in one GPU frame.
        if let Some((layouts, dividers)) = self.compute_pane_layouts() {
            self.handle_redraw_multi_pane(&layouts, &dividers, url_segments);
            return;
        }

        // Resolve pane ID before the render block: `active_pane_id()` borrows
        // `&self`, which conflicts with the `&mut ctx.renderer` inside the
        // block. `PaneId` is `Copy`, so the borrow ends here.
        let Some(pane_id) = self.active_pane_id() else {
            log::warn!("redraw: no active pane");
            return;
        };

        // Copy selection before the render block (where ctx.renderer is
        // mutably borrowed, preventing immutable self borrows).
        let pane_sel = self.pane_selection(pane_id).copied();
        let pane_mc = self.pane_mark_cursor(pane_id);

        let (render_result, blinking_now) = {
            let Some(gpu) = self.gpu.as_ref() else {
                log::warn!("redraw: no gpu");
                return;
            };
            let Some(pipelines) = self.pipelines.as_ref() else {
                log::warn!("redraw: no pipelines");
                return;
            };
            let Some(ctx) = self
                .focused_window_id
                .and_then(|id| self.windows.get_mut(&id))
            else {
                log::warn!("redraw: no window");
                return;
            };
            let Some(renderer) = ctx.renderer.as_mut() else {
                log::warn!("redraw: no renderer");
                return;
            };
            if !ctx.window.has_surface_area() {
                log::warn!("redraw: no surface area");
                return;
            }

            let (w, h) = ctx.window.size_px();
            let viewport = ViewportSize::new(w, h);
            let cell = renderer.cell_metrics();
            let Some(mux) = self.mux.as_mut() else {
                return;
            };

            // Extract phase: refresh snapshot if needed.
            // Detect tab switch / tear-off: when the rendered pane changes,
            // force a refresh to flush stale `renderable_cache` entries left
            // by the previous `swap_renderable_content` cycle.
            let pane_changed = ctx.last_rendered_pane != Some(pane_id);
            ctx.last_rendered_pane = Some(pane_id);
            let snap_is_none = mux.pane_snapshot(pane_id).is_none();
            let snap_dirty = mux.is_pane_snapshot_dirty(pane_id);
            let content_changed = snap_is_none || snap_dirty || pane_changed;
            if content_changed {
                mux.refresh_pane_snapshot(pane_id);
            }

            // Fast path (embedded): swap RenderableContent directly from
            // the terminal, bypassing the WireCell round-trip. Only attempt
            // when content was refreshed — stale cache entries from prior
            // tab switches would contaminate the frame otherwise.
            let swapped = content_changed
                && ctx
                    .frame
                    .as_mut()
                    .is_some_and(|f| mux.swap_renderable_content(pane_id, &mut f.content));

            let Some(snapshot) = mux.pane_snapshot(pane_id) else {
                log::warn!("redraw: no snapshot for pane {pane_id:?}");
                ctx.dirty = true;
                return;
            };
            if swapped {
                let frame = ctx.frame.as_mut().expect("frame exists when swapped");
                frame.viewport = viewport;
                frame.cell_size = cell;
                frame.palette = snapshot_palette(snapshot);
                frame.selection = None;
                frame.search = None;
                frame.hovered_cell = None;
                frame.hovered_url_segments.clear();
                frame.mark_cursor = None;
                frame.fg_dim = 1.0;
                frame.prompt_marker_rows.clear();
            } else {
                match &mut ctx.frame {
                    Some(existing) => {
                        extract_frame_from_snapshot_into(snapshot, existing, viewport, cell);
                    }
                    slot @ None => {
                        *slot = Some(extract_frame_from_snapshot(snapshot, viewport, cell));
                    }
                }
            }
            mux.clear_pane_snapshot_dirty(pane_id);

            let frame = ctx.frame.as_mut().expect("frame just assigned");

            // Set window opacity from config (extract phase doesn't have
            // access to config — opacity is a window concern, not terminal state).
            frame.palette.opacity = self.config.window.effective_opacity();

            // IME preedit: overlay composition text at the cursor position
            // (underlined) so it flows through the normal shaping pipeline.
            if !self.ime.preedit.is_empty() {
                let cols = frame.columns();
                preedit::overlay_preedit_cells(&self.ime.preedit, &mut frame.content, cols);
            }

            // Annotate frame with pane-level state (mark cursor, search)
            // and client-side selection from App state.
            let base = frame.content.stable_row_base;
            // Mark cursor from App state (copied before render block).
            frame.mark_cursor = pane_mc.and_then(|mc| {
                let (line, col) = mc.to_viewport(frame.content.stable_row_base, frame.rows())?;
                Some(MarkCursorOverride {
                    line,
                    column: Column(col),
                    shape: CursorShape::HollowBlock,
                })
            });
            // Search from snapshot.
            {
                let mux = self.mux.as_ref().expect("mux checked");
                frame.search = mux
                    .pane_snapshot(pane_id)
                    .and_then(FrameSearch::from_snapshot);
            }
            // Selection lives on App, not Pane (copied before render block).
            frame.selection = pane_sel.map(|sel| FrameSelection::new(&sel, base));

            // Compute hovered cell for hyperlink underline rendering.
            let cell_metrics = renderer.cell_metrics();
            let hovered_cell = {
                let grid_ctx = GridCtx {
                    widget: &ctx.terminal_grid,
                    cell: cell_metrics,
                    word_delimiters: &self.config.behavior.word_delimiters,
                };
                mouse_selection::pixel_to_cell(self.mouse.cursor_pos(), &grid_ctx)
                    .map(|(col, line)| (line, col))
            };
            frame.hovered_cell = hovered_cell;

            // Implicit URL hover: viewport-relative segments computed above.
            // The Vec was taken from the previous frame to reuse capacity.
            frame.hovered_url_segments = url_segments;

            // Visual prompt markers: clear extracted rows if the feature is disabled.
            if !self.config.behavior.prompt_markers {
                frame.prompt_marker_rows.clear();
            }

            // Capture blinking mode for post-render update. Timer reset
            // and state mutation are deferred to after GPU submission so that
            // the render block stays free of blink-state side effects.
            let blinking_now = frame.content.mode.contains(TermMode::CURSOR_BLINKING);

            // On false→true transition, force cursor visible this frame (the
            // timer reset hasn't happened yet, so is_visible() may be stale).
            let cursor_blink_visible =
                !blinking_now || !self.blinking_active || self.cursor_blink.is_visible();

            // Grid origin from layout bounds. When the layout engine
            // positions the grid (e.g. below a tab bar), this shifts all
            // cell rendering. Both bounds and cell metrics are in physical
            // pixels; the viewport (screen_size uniform) is also physical,
            // so the shader maps physical positions to NDC correctly.
            let origin = ctx
                .terminal_grid
                .bounds()
                .map_or((0.0, 0.0), |b| (b.x(), b.y()));

            renderer.prepare(
                frame,
                gpu,
                pipelines,
                origin,
                cursor_blink_visible,
                content_changed,
            );

            // Draw tab bar (unified chrome bar). Tab bar contains text
            // (tab titles), so uses the text-aware draw list conversion.
            let scale = ctx.window.scale_factor().factor() as f32;
            let logical_w = (w as f32 / scale).round() as u32;
            if Self::draw_tab_bar(
                Some(&ctx.tab_bar),
                renderer,
                &mut ctx.chrome_draw_list,
                logical_w as f32,
                scale,
                gpu,
                &self.ui_theme,
            ) {
                ctx.dirty = true;
            }

            // Draw overlays with per-overlay compositor opacity.
            let logical_size = (logical_w as f32, h as f32 / scale);
            if Self::draw_overlays(
                &mut ctx.overlays,
                renderer,
                &mut ctx.chrome_draw_list,
                logical_size,
                scale,
                gpu,
                &ctx.layer_tree,
                &self.ui_theme,
            ) {
                ctx.dirty = true;
            }

            // Draw search bar overlay when search is active.
            if let Some(search) = frame.search.as_ref() {
                // Position below all chrome (caption + tab bar).
                let chrome_h = oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT;
                Self::draw_search_bar(
                    search,
                    renderer,
                    &mut ctx.chrome_draw_list,
                    &mut ctx.search_bar_buf,
                    logical_w as f32,
                    chrome_h,
                    scale,
                    gpu,
                );
            }

            let result = renderer.render_to_surface(gpu, pipelines, ctx.window.surface());
            (result, blinking_now)
        };

        self.handle_render_result(render_result);

        // Update blink state after rendering (no state mutation during render).
        if blinking_now && !self.blinking_active {
            self.cursor_blink.reset();
        }
        self.blinking_active = blinking_now;

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
                let Some(gpu) = self.gpu.as_ref() else { return };
                if let Some(ctx) = self
                    .focused_window_id
                    .and_then(|id| self.windows.get_mut(&id))
                {
                    let (w, h) = ctx.window.size_px();
                    ctx.window.resize_surface(w, h, gpu);
                }
            }
            Err(e) => log::error!("render error: {e}"),
        }
    }
}
