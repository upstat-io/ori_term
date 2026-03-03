//! Multi-pane rendering: compute pane layouts and render all panes.
//!
//! When a tab has more than one pane (split or floating), this module takes
//! over from the single-pane fast path. Each pane is extracted, prepared at
//! its layout-computed pixel offset, and instances accumulate into one shared
//! `PreparedFrame` for a single GPU submission.

use oriterm_core::{Column, CursorShape, TermMode};
use oriterm_mux::layout::{DividerLayout, LayoutDescriptor, PaneLayout, Rect, compute_all};

use super::App;
use super::mouse_selection::{self, GridCtx};
use crate::gpu::{
    FrameSearch, FrameSelection, MarkCursorOverride, ViewportSize, extract_frame,
    extract_frame_from_snapshot, extract_frame_into,
};

impl App {
    /// Compute pane layouts for the active tab.
    ///
    /// Returns `None` if the tab has a single pane (use the fast path).
    /// Returns `Some((pane_layouts, divider_layouts))` when multi-pane.
    pub(in crate::app) fn compute_pane_layouts(
        &self,
    ) -> Option<(Vec<PaneLayout>, Vec<DividerLayout>)> {
        let mux = self.mux.as_ref()?;
        let win_id = self.active_window?;
        let win = mux.session().get_window(win_id)?;
        let tab_id = win.active_tab()?;
        let tab = mux.session().get_tab(tab_id)?;

        let is_zoomed = tab.zoomed_pane().is_some();

        if !is_zoomed && tab.tree().pane_count() <= 1 && tab.floating().is_empty() {
            return None;
        }

        let ctx = self.focused_ctx()?;
        let bounds = ctx.terminal_grid.bounds()?;
        let renderer = self.renderer.as_ref()?;
        let cell = renderer.cell_metrics();

        // Zoomed: single pane fills the entire available area.
        if let Some(zoomed_id) = tab.zoomed_pane() {
            let avail = Rect {
                x: bounds.x(),
                y: bounds.y(),
                width: bounds.width(),
                height: bounds.height(),
            };
            let cols = (avail.width / cell.width).floor() as u16;
            let rows = (avail.height / cell.height).floor() as u16;
            let snapped_w = cols as f32 * cell.width;
            let snapped_h = rows as f32 * cell.height;
            return Some((
                vec![PaneLayout {
                    pane_id: zoomed_id,
                    pixel_rect: Rect {
                        x: avail.x,
                        y: avail.y,
                        width: snapped_w,
                        height: snapped_h,
                    },
                    cols: cols.max(1),
                    rows: rows.max(1),
                    is_focused: true,
                    is_floating: false,
                }],
                vec![],
            ));
        }

        let desc = LayoutDescriptor {
            available: Rect {
                x: bounds.x(),
                y: bounds.y(),
                width: bounds.width(),
                height: bounds.height(),
            },
            cell_width: cell.width,
            cell_height: cell.height,
            divider_px: self.config.pane.divider_px,
            min_pane_cells: self.config.pane.min_cells,
        };

        let (panes, dividers) = compute_all(tab.tree(), tab.floating(), tab.active_pane(), &desc);
        Some((panes, dividers))
    }

    /// Execute the multi-pane rendering pipeline.
    ///
    /// Iterates all pane layouts, extracts and prepares each pane at its
    /// pixel offset, then appends dividers and a focus border. Chrome, tab
    /// bar, overlays, and search bar are drawn after all panes. Instances
    /// accumulate in a single `PreparedFrame` for one GPU submission.
    #[expect(
        clippy::too_many_lines,
        reason = "linear multi-pane pipeline: begin → per-pane extract+prepare → dividers → border → chrome → render"
    )]
    pub(super) fn handle_redraw_multi_pane(
        &mut self,
        layouts: &[PaneLayout],
        dividers: &[DividerLayout],
        mut url_segments: Vec<crate::url_detect::UrlSegment>,
    ) {
        let (render_result, blinking_now) = {
            let Some(gpu) = self.gpu.as_ref() else {
                log::warn!("redraw multi: no gpu");
                return;
            };
            let Some(renderer) = self.renderer.as_mut() else {
                log::warn!("redraw multi: no renderer");
                return;
            };
            let Some(ctx) = self
                .focused_window_id
                .and_then(|id| self.windows.get_mut(&id))
            else {
                log::warn!("redraw multi: no window");
                return;
            };

            if !ctx.window.has_surface_area() {
                return;
            }

            let (w, h) = ctx.window.size_px();
            let viewport = ViewportSize::new(w, h);
            let cell = renderer.cell_metrics();
            let bg = ctx
                .frame
                .as_ref()
                .map_or(oriterm_core::Rgb { r: 0, g: 0, b: 0 }, |f| {
                    f.palette.background
                });
            let opacity = f64::from(self.config.window.effective_opacity());

            renderer.begin_multi_pane_frame(viewport, bg, opacity);

            let dim_inactive = self.config.pane.dim_inactive;
            let inactive_opacity = self.config.pane.effective_inactive_opacity();
            let cursor_blink_visible = !self.blinking_active || self.cursor_blink.is_visible();

            let mut focused_rect = None;
            let mut focused_base = 0u64;
            let mut blinking_now = self.blinking_active;
            let daemon_mode = self.mux.as_ref().is_some_and(|m| m.is_daemon_mode());

            for layout in layouts {
                let pane_id = layout.pane_id;

                // Dirty check: daemon mode checks snapshot dirty flag;
                // embedded mode checks local grid dirty + cache.
                let is_cached = ctx.pane_cache.is_cached(pane_id, layout);
                let dirty = if daemon_mode {
                    let snap_dirty = self
                        .mux
                        .as_ref()
                        .is_some_and(|m| m.is_pane_snapshot_dirty(pane_id));
                    layout.is_focused || snap_dirty || !is_cached
                } else {
                    let grid_dirty = self
                        .mux
                        .as_ref()
                        .and_then(|m| m.pane(pane_id))
                        .is_some_and(oriterm_mux::Pane::grid_dirty);
                    layout.is_focused || grid_dirty || !is_cached
                };

                if dirty {
                    let pane_viewport = ViewportSize::new(
                        layout.pixel_rect.width as u32,
                        layout.pixel_rect.height as u32,
                    );

                    // Extract phase: daemon mode uses snapshot; embedded
                    // mode locks the local terminal.
                    if daemon_mode {
                        let mux = self.mux.as_mut().expect("mux checked");
                        if mux.is_pane_snapshot_dirty(pane_id) {
                            mux.refresh_pane_snapshot(pane_id);
                        }
                        if let Some(snapshot) = mux.pane_snapshot(pane_id) {
                            ctx.frame =
                                Some(extract_frame_from_snapshot(snapshot, pane_viewport, cell));
                        } else {
                            log::warn!("multi-pane: no snapshot for pane {pane_id:?}");
                            continue;
                        }
                        mux.clear_pane_snapshot_dirty(pane_id);
                    } else {
                        let Some(pane) = self.mux.as_ref().and_then(|m| m.pane(pane_id)) else {
                            log::warn!("multi-pane: pane {pane_id:?} not found");
                            continue;
                        };
                        pane.clear_grid_dirty();
                        match &mut ctx.frame {
                            Some(existing) => {
                                extract_frame_into(pane.terminal(), existing, pane_viewport, cell);
                            }
                            slot @ None => {
                                *slot = Some(extract_frame(pane.terminal(), pane_viewport, cell));
                            }
                        }
                    }

                    let frame = ctx.frame.as_mut().expect("frame just assigned");

                    frame.palette.opacity = self.config.window.effective_opacity();

                    if layout.is_focused && !self.ime.preedit.is_empty() {
                        let cols = frame.columns();
                        super::preedit::overlay_preedit_cells(
                            &self.ime.preedit,
                            &mut frame.content,
                            cols,
                        );
                    }

                    // Pane-level annotations (selection, search, mark cursor)
                    // are only available in embedded mode.
                    if !daemon_mode {
                        if let Some(pane) = self.mux.as_ref().and_then(|m| m.pane(pane_id)) {
                            frame.mark_cursor = if layout.is_focused {
                                pane.mark_cursor().and_then(|mc| {
                                    let (line, col) = mc
                                        .to_viewport(frame.content.stable_row_base, frame.rows())?;
                                    Some(MarkCursorOverride {
                                        line,
                                        column: Column(col),
                                        shape: CursorShape::HollowBlock,
                                    })
                                })
                            } else {
                                None
                            };
                            let base = frame.content.stable_row_base;
                            frame.selection =
                                pane.selection().map(|sel| FrameSelection::new(sel, base));
                            frame.search = pane.search().map(|s| FrameSearch::new(s, base));
                        }
                    }

                    if layout.is_focused {
                        let cell_metrics = renderer.cell_metrics();
                        let grid_ctx = GridCtx {
                            widget: &ctx.terminal_grid,
                            cell: cell_metrics,
                            word_delimiters: &self.config.behavior.word_delimiters,
                        };
                        frame.hovered_cell =
                            mouse_selection::pixel_to_cell(self.mouse.cursor_pos(), &grid_ctx)
                                .map(|(col, line)| (line, col));
                        frame.hovered_url_segments = std::mem::take(&mut url_segments);
                    } else {
                        frame.hovered_cell = None;
                        frame.hovered_url_segments.clear();
                    }

                    // Visual prompt markers: clear if disabled.
                    if !self.config.behavior.prompt_markers {
                        frame.prompt_marker_rows.clear();
                    }

                    if layout.is_focused {
                        focused_base = frame.content.stable_row_base;
                        blinking_now = frame.content.mode.contains(TermMode::CURSOR_BLINKING);
                    }

                    frame.fg_dim = if layout.is_focused || !dim_inactive {
                        1.0
                    } else {
                        inactive_opacity
                    };

                    let origin = (layout.pixel_rect.x, layout.pixel_rect.y);
                    let pane_cursor_visible = cursor_blink_visible && layout.is_focused;

                    let cached = ctx
                        .pane_cache
                        .get_or_prepare(pane_id, layout, true, |target| {
                            renderer.prepare_pane_into(
                                frame,
                                gpu,
                                origin,
                                pane_cursor_visible,
                                target,
                            );
                        });
                    renderer.prepared.extend_from(cached);
                } else {
                    // Cache hit — merge cached instances without extraction.
                    let cached = ctx
                        .pane_cache
                        .get_cached(pane_id)
                        .expect("is_cached verified");
                    renderer.prepared.extend_from(cached);
                }

                if layout.is_focused {
                    focused_rect = Some(layout.pixel_rect);
                }
            }

            // Restore focused pane's search for the search bar (embedded mode only).
            if !daemon_mode {
                if let Some(focused) = layouts.iter().find(|l| l.is_focused) {
                    if let Some(pane) = self.mux.as_ref().and_then(|m| m.pane(focused.pane_id)) {
                        if let Some(frame) = ctx.frame.as_mut() {
                            frame.search = pane.search().map(|s| FrameSearch::new(s, focused_base));
                        }
                    }
                }
            }

            // Dividers between split panes.
            let renderer = self.renderer.as_mut().expect("renderer checked");
            let divider_color = self.config.pane.effective_divider_color();
            renderer.append_dividers(dividers, divider_color);

            // Floating pane decorations (shadow + border).
            let accent_color = self.config.pane.effective_focus_border_color();
            for layout in layouts.iter().filter(|l| l.is_floating) {
                renderer.append_floating_decoration(&layout.pixel_rect, accent_color);
            }

            // Focus border on active pane (only when multiple panes visible).
            if layouts.len() > 1 {
                if let Some(rect) = &focused_rect {
                    renderer.append_focus_border(rect, accent_color);
                }
            }

            // Chrome, tab bar, overlays, search bar (shared with single-pane path).
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

            // Search bar from focused pane.
            if let Some(frame) = ctx.frame.as_ref() {
                if let Some(search) = frame.search.as_ref() {
                    let chrome_h =
                        caption_h + oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT;
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

        self.update_ime_cursor_area();
    }
}
