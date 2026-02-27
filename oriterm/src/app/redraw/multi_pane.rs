//! Multi-pane rendering: compute pane layouts and render all panes.
//!
//! When a tab has more than one pane (split or floating), this module takes
//! over from the single-pane fast path. Each pane is extracted, prepared at
//! its layout-computed pixel offset, and instances accumulate into one shared
//! `PreparedFrame` for a single GPU submission.

use oriterm_core::{Column, CursorShape, TermMode};
use oriterm_mux::layout::{DividerLayout, LayoutDescriptor, PaneLayout, compute_all};

use oriterm_ui::widgets::window_chrome::WindowChromeWidget;

use super::App;
use super::mouse_selection::{self, GridCtx};
use crate::gpu::{
    FrameSearch, FrameSelection, MarkCursorOverride, ViewportSize, extract_frame,
    extract_frame_into,
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

        if tab.tree().pane_count() <= 1 && tab.floating().is_empty() {
            return None;
        }

        let grid_widget = self.terminal_grid.as_ref()?;
        let bounds = grid_widget.bounds()?;
        let renderer = self.renderer.as_ref()?;
        let cell = renderer.cell_metrics();

        let desc = LayoutDescriptor {
            available: oriterm_mux::layout::Rect {
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
        url_segments: &[crate::url_detect::UrlSegment],
    ) {
        let render_result = {
            let Some(gpu) = self.gpu.as_ref() else {
                log::warn!("redraw multi: no gpu");
                return;
            };
            let Some(renderer) = self.renderer.as_mut() else {
                log::warn!("redraw multi: no renderer");
                return;
            };
            let Some(window) = self.window.as_ref() else {
                log::warn!("redraw multi: no window");
                return;
            };

            if !window.has_surface_area() {
                return;
            }

            let (w, h) = window.size_px();
            let viewport = ViewportSize::new(w, h);
            let cell = renderer.cell_metrics();
            let bg = self
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

            for layout in layouts {
                let pane_id = layout.pane_id;
                let Some(pane) = self.panes.get(&pane_id) else {
                    log::warn!("multi-pane: pane {:?} not found", pane_id);
                    continue;
                };

                // Focused pane always re-prepares (cursor blink, hover change
                // per-frame). Unfocused panes only re-prepare on new PTY output
                // or missing cache entry.
                let is_cached = self.pane_cache.is_cached(pane_id, layout);
                let dirty = layout.is_focused || pane.grid_dirty() || !is_cached;

                if dirty {
                    pane.clear_grid_dirty();

                    let pane_viewport = ViewportSize::new(
                        layout.pixel_rect.width as u32,
                        layout.pixel_rect.height as u32,
                    );

                    let frame = match &mut self.frame {
                        Some(existing) => {
                            extract_frame_into(pane.terminal(), existing, pane_viewport, cell);
                            existing
                        }
                        slot @ None => {
                            *slot = Some(extract_frame(pane.terminal(), pane_viewport, cell));
                            slot.as_mut().expect("just assigned")
                        }
                    };

                    frame.palette.opacity = self.config.window.effective_opacity();

                    if layout.is_focused && !self.ime.preedit.is_empty() {
                        let cols = frame.columns();
                        super::overlay_preedit_cells(&self.ime.preedit, &mut frame.content, cols);
                    }

                    frame.mark_cursor = if layout.is_focused {
                        pane.mark_cursor().and_then(|mc| {
                            let (line, col) =
                                mc.to_viewport(frame.content.stable_row_base, frame.rows())?;
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
                    frame.selection = pane.selection().map(|sel| FrameSelection::new(sel, base));
                    frame.search = pane.search().map(|s| FrameSearch::new(s, base));

                    if layout.is_focused {
                        if let Some(grid_widget) = self.terminal_grid.as_ref() {
                            let ctx = GridCtx {
                                widget: grid_widget,
                                cell,
                                word_delimiters: &self.config.behavior.word_delimiters,
                            };
                            frame.hovered_cell =
                                mouse_selection::pixel_to_cell(self.mouse.cursor_pos(), &ctx)
                                    .map(|(col, line)| (line, col));
                        }
                        frame.hovered_url_segments = url_segments.to_vec();
                    } else {
                        frame.hovered_cell = None;
                        frame.hovered_url_segments.clear();
                    }

                    if layout.is_focused {
                        // Save stable row base for search bar restoration after the loop.
                        focused_base = frame.content.stable_row_base;

                        // Cache blinking mode from focused pane.
                        let blinking_now = frame.content.mode.contains(TermMode::CURSOR_BLINKING);
                        if blinking_now && !self.blinking_active {
                            self.cursor_blink.reset();
                        }
                        self.blinking_active = blinking_now;
                    }

                    frame.fg_dim = if layout.is_focused || !dim_inactive {
                        1.0
                    } else {
                        inactive_opacity
                    };

                    let origin = (layout.pixel_rect.x, layout.pixel_rect.y);
                    let pane_cursor_visible = cursor_blink_visible && layout.is_focused;

                    // Prepare into per-pane cache, then merge into aggregate frame.
                    let renderer = self.renderer.as_mut().expect("renderer checked");
                    let cache = &mut self.pane_cache;
                    let cached = cache.get_or_prepare(pane_id, layout, true, |target| {
                        renderer.prepare_pane_into(frame, gpu, origin, pane_cursor_visible, target);
                    });
                    renderer.prepared.extend_from(cached);
                } else {
                    // Cache hit — merge cached instances without extraction.
                    let cached = self
                        .pane_cache
                        .get_cached(pane_id)
                        .expect("is_cached verified");
                    let renderer = self.renderer.as_mut().expect("renderer checked");
                    renderer.prepared.extend_from(cached);
                }

                if layout.is_focused {
                    focused_rect = Some(layout.pixel_rect);
                }
            }

            // Restore focused pane's search for the search bar — `frame.search`
            // may have been overwritten by a non-focused dirty pane later in
            // layout order.
            if let Some(focused) = layouts.iter().find(|l| l.is_focused) {
                if let Some(pane) = self.panes.get(&focused.pane_id) {
                    if let Some(frame) = self.frame.as_mut() {
                        frame.search = pane.search().map(|s| FrameSearch::new(s, focused_base));
                    }
                }
            }

            // Dividers between split panes.
            let renderer = self.renderer.as_mut().expect("renderer checked");
            let divider_color = oriterm_core::Rgb {
                r: 80,
                g: 80,
                b: 80,
            };
            renderer.append_dividers(dividers, divider_color);

            // Focus border on active pane (only when multiple panes visible).
            if layouts.len() > 1 {
                if let Some(rect) = &focused_rect {
                    let accent_color = oriterm_core::Rgb {
                        r: 100,
                        g: 149,
                        b: 237,
                    };
                    renderer.append_focus_border(rect, accent_color);
                }
            }

            // Chrome, tab bar, overlays, search bar (shared with single-pane path).
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

            // Search bar from focused pane.
            if let Some(frame) = self.frame.as_ref() {
                if let Some(search) = frame.search.as_ref() {
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
            }

            renderer.render_to_surface(gpu, window.surface())
        };

        self.handle_render_result(render_result);
        self.update_ime_cursor_area();
    }
}
