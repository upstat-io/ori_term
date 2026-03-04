//! Three-phase rendering pipeline: Extract → Prepare → Render.

mod multi_pane;
pub(in crate::app) mod preedit;
mod search_bar;

use std::cell::Cell;
use std::time::Instant;

use oriterm_core::{Column, CursorShape, TermMode};

use oriterm_ui::draw::DrawList;
use oriterm_ui::overlay::OverlayManager;
use oriterm_ui::theme::UiTheme;
use oriterm_ui::widgets::window_chrome::WindowChromeWidget;
use oriterm_ui::widgets::{DrawCtx, Widget};

use super::App;
use super::mouse_selection::{self, GridCtx};
use crate::font::UiFontMeasurer;
use crate::gpu::state::GpuState;
use crate::gpu::{
    FrameSearch, FrameSelection, MarkCursorOverride, SurfaceError, ViewportSize,
    extract_frame_from_snapshot, extract_frame_from_snapshot_into,
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
        // self.renderer mutably). Take the Vec from the previous frame to
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
        // `&self`, which conflicts with the `&mut self.renderer` inside the
        // block. `PaneId` is `Copy`, so the borrow ends here.
        let Some(pane_id) = self.active_pane_id() else {
            log::warn!("redraw: no active pane");
            return;
        };

        // Copy selection before the render block (where self.renderer is
        // mutably borrowed, preventing immutable self borrows).
        let pane_sel = self.pane_selection(pane_id).copied();
        let pane_mc = self.pane_mark_cursor(pane_id);

        let (render_result, blinking_now) = {
            let Some(gpu) = self.gpu.as_ref() else {
                log::warn!("redraw: no gpu");
                return;
            };
            let Some(renderer) = self.renderer.as_mut() else {
                log::warn!("redraw: no renderer");
                return;
            };
            let Some(ctx) = self
                .focused_window_id
                .and_then(|id| self.windows.get_mut(&id))
            else {
                log::warn!("redraw: no window");
                return;
            };
            if !ctx.window.has_surface_area() {
                log::warn!("redraw: no surface area");
                return;
            }

            let (w, h) = ctx.window.size_px();
            let viewport = ViewportSize::new(w, h);
            let cell = renderer.cell_metrics();
            let mux = self.mux.as_mut().expect("mux checked at pane_id");

            // Extract phase: always snapshot-based (both embedded and daemon).
            let snap_is_none = mux.pane_snapshot(pane_id).is_none();
            let snap_dirty = mux.is_pane_snapshot_dirty(pane_id);
            let snap_needed = snap_is_none || snap_dirty;
            log::info!(
                "[DIAG] redraw: pane={pane_id:?} snap_none={snap_is_none} snap_dirty={snap_dirty} snap_needed={snap_needed}"
            );
            if snap_needed {
                let snap_start = Instant::now();
                mux.refresh_pane_snapshot(pane_id);
                let snap_elapsed = snap_start.elapsed();
                if snap_elapsed.as_millis() > 5 {
                    log::warn!("[DIAG] refresh_pane_snapshot took {:?}", snap_elapsed);
                }
            }
            let Some(snapshot) = mux.pane_snapshot(pane_id) else {
                log::warn!("redraw: no snapshot for pane {pane_id:?}");
                // Keep trying on subsequent ticks. Without this, a transient
                // snapshot miss can clear `dirty` and stall visible updates.
                ctx.dirty = true;
                return;
            };
            match &mut ctx.frame {
                Some(existing) => {
                    extract_frame_from_snapshot_into(snapshot, existing, viewport, cell);
                }
                slot @ None => {
                    *slot = Some(extract_frame_from_snapshot(snapshot, viewport, cell));
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

            renderer.prepare(frame, gpu, origin, cursor_blink_visible);

            // Draw window chrome into the UI rect layer. Chrome widget
            // draws in logical pixels; scale converts to physical pixels
            // for the GPU pipeline (screen_size uniform is physical).
            let scale = ctx.window.scale_factor().factor() as f32;
            let logical_w = (w as f32 / scale).round() as u32;
            let chrome_animating = Self::draw_chrome(
                Some(&ctx.chrome),
                renderer,
                &mut ctx.chrome_draw_list,
                logical_w,
                scale,
                &self.ui_theme,
            );
            if chrome_animating {
                ctx.dirty = true;
            }

            // Draw tab bar below the chrome caption. Tab bar contains text
            // (tab titles), so uses the text-aware draw list conversion.
            let caption_h = ctx.chrome.caption_height();
            if Self::draw_tab_bar(
                Some(&ctx.tab_bar),
                renderer,
                &mut ctx.chrome_draw_list,
                logical_w as f32,
                caption_h,
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
                let chrome_h = caption_h + oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT;
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

            let result = renderer.render_to_surface(gpu, ctx.window.surface());
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
        renderer.append_ui_draw_list(draw_list, scale, 1.0);
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
        renderer.append_ui_draw_list_with_text(draw_list, scale, 1.0, gpu);
        animating
    }

    /// Draw overlays (active + dismissing) with per-overlay compositor opacity.
    ///
    /// Each overlay is drawn individually so its compositor layer opacity
    /// can be applied independently (e.g. during simultaneous fade-in/fade-out).
    /// Modal dim rects are emitted before their content overlay.
    ///
    /// Returns `true` if overlays have running animations (fade-in/fade-out).
    #[expect(
        clippy::too_many_arguments,
        reason = "overlay drawing: manager, renderer, draw list, viewport, scale, GPU, tree, theme"
    )]
    fn draw_overlays(
        overlays: &mut OverlayManager,
        renderer: &mut crate::gpu::GpuRenderer,
        draw_list: &mut DrawList,
        logical_size: (f32, f32),
        scale: f32,
        gpu: &GpuState,
        tree: &oriterm_ui::compositor::layer_tree::LayerTree,
        theme: &UiTheme,
    ) -> bool {
        let count = overlays.draw_count();
        if count == 0 {
            return false;
        }

        let bounds = oriterm_ui::geometry::Rect::new(0.0, 0.0, logical_size.0, logical_size.1);
        let animations_running = Cell::new(false);
        let mut animating = false;

        // Layout + draw phase: measurer borrows renderer immutably, then
        // drops before the mutable append_ui_draw_list_with_text call.
        // We collect (opacity) per overlay, then append after the borrow ends.
        {
            let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
            overlays.layout_overlays(&measurer, theme);
        }

        for i in 0..count {
            draw_list.clear();
            // Re-create measurer per iteration — cheap (no allocation), and
            // the immutable borrow drops before the mutable append below.
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
            let opacity = overlays.draw_overlay_at(i, &mut ctx, tree);

            // If opacity is < 1.0 an animation is running.
            if opacity < 1.0 - f32::EPSILON {
                animating = true;
            }

            // measurer (immutable borrow on renderer) is dropped here by NLL.
            // Overlays write to the overlay tier (draws 10–13) so their
            // backgrounds render ON TOP of chrome text (draws 7–9).
            renderer.append_overlay_draw_list_with_text(draw_list, scale, opacity, gpu);
        }

        animating || animations_running.get()
    }
}
