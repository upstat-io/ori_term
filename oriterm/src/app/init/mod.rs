//! One-shot application startup: window → GPU → mux → fonts → renderer → tab.

use winit::event_loop::ActiveEventLoop;

use oriterm_mux::domain::SpawnConfig;
use oriterm_ui::widgets::window_chrome::WindowChromeWidget;
use oriterm_ui::window::WindowConfig;

use super::window_context::WindowContext;
use super::{App, DEFAULT_DPI};
use crate::app::config_reload;
use crate::font::{FontCollection, FontSet, GlyphFormat, HintingMode};
use crate::gpu::{GpuRenderer, GpuState};
use crate::mux::InProcessMux;
use crate::widgets::terminal_grid::TerminalGridWidget;
use crate::window::TermWindow;

impl App {
    /// Run the one-shot startup sequence: window → GPU → fonts → renderer → tab.
    ///
    /// Returns `Err` with a displayable message on any failure. The caller
    /// logs the error and exits the event loop.
    pub(super) fn try_init(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let t_start = std::time::Instant::now();

        // Build UI window config from the user's config.
        let opacity = self.config.window.effective_opacity();
        let window_config = WindowConfig {
            title: "ori".into(),
            transparent: opacity < 1.0,
            blur: self.config.window.blur && opacity < 1.0,
            opacity,
            ..WindowConfig::default()
        };

        // 1. Create window (invisible) for GPU surface capability probing.
        let window_arc = oriterm_ui::window::create_window(event_loop, &window_config)?;
        let t_window = t_start.elapsed();

        // 2. Spawn font discovery on a background thread (no GPU dependency).
        let font_handle = self.spawn_font_discovery()?;

        // 3. Init GPU on main thread (requires window Arc, runs concurrently with fonts).
        let t_gpu_start = std::time::Instant::now();
        let gpu = GpuState::new(&window_arc, window_config.transparent)?;
        let t_gpu = t_gpu_start.elapsed();

        // 4. Create mux infrastructure early (allocators only — no window/GPU deps).
        //    The mux window ID is needed by TermWindow for the mux↔winit mapping.
        let mut mux = InProcessMux::new();
        let mux_window_id = mux.create_window();

        // 5. Wrap the same window into TermWindow (creates surface, applies effects).
        let window = TermWindow::from_window(window_arc, &window_config, &gpu, mux_window_id)?;

        // 6. Join font thread (GPU init + surface setup ran concurrently).
        let (mut font_collection, user_fb_count, t_fonts) = match font_handle.join() {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => return Err("font discovery thread panicked".into()),
        };

        // 6b. Rescale fonts to physical DPI so glyph bitmaps match the
        // physical surface resolution. At 1.5x scaling: 96 * 1.5 = 144 DPI,
        // producing glyphs that are 1.5x larger in pixels — exactly matching
        // the physical surface. Cell metrics become physical pixels.
        let scale = window.scale_factor().factor();
        let physical_dpi = DEFAULT_DPI * scale as f32;
        font_collection.set_size(self.config.font.size, physical_dpi);

        // 6c. Adjust hinting and subpixel mode for the actual display scale factor.
        // Config overrides take priority over auto-detection.
        let hinting = config_reload::resolve_hinting(&self.config.font, scale);
        font_collection.set_hinting(hinting);
        let subpixel_format =
            config_reload::resolve_subpixel_mode(&self.config.font, scale).glyph_format();
        font_collection.set_format(subpixel_format);

        // 6d. Apply font config: features, per-fallback metadata, codepoint map.
        config_reload::apply_font_config(&mut font_collection, &self.config.font, user_fb_count);

        // 7. Create GPU renderer (pipelines, atlas, pre-cached ASCII glyphs).
        let t_renderer_start = std::time::Instant::now();
        let renderer = GpuRenderer::new(&gpu, font_collection);
        let t_renderer = t_renderer_start.elapsed();

        // 8. Create chrome + tab bar widgets and apply platform effects.
        let (w, h) = window.size_px();
        let (chrome_widget, tab_bar_widget, caption_height) = self.create_chrome_widgets(&window);

        // 11. Compute grid dimensions from viewport, offset by chrome height.
        // Chrome = caption bar + tab bar. Cell metrics are in physical pixels
        // (rasterized at physical DPI), so divide physical viewport by physical
        // cell size directly.
        let tab_bar_h = oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT;
        let cell = renderer.cell_metrics();
        let scale = window.scale_factor().factor() as f32;
        let origin_y = super::chrome::grid_origin_y(caption_height + tab_bar_h, scale);
        let chrome_px = origin_y as u32;
        let grid_h = h.saturating_sub(chrome_px);
        let cols = cell.columns(w).max(1);
        let rows = cell.rows(grid_h).max(1);

        // 12. Create grid widget with cell metrics and initial grid size.
        // Bounds are in physical pixels to match the physical viewport.
        let grid_widget = TerminalGridWidget::new(cell.width, cell.height, cols, rows);
        grid_widget.set_bounds(oriterm_ui::geometry::Rect::new(
            0.0,
            origin_y,
            cols as f32 * cell.width,
            rows as f32 * cell.height,
        ));

        // 13. Create initial tab + pane in the pre-allocated mux window.
        let t_mux_start = std::time::Instant::now();
        self.create_initial_tab(&mut mux, mux_window_id, rows as u16, cols as u16)?;
        let t_mux = t_mux_start.elapsed();

        let t_total = t_start.elapsed();
        log::info!(
            "app: startup — window={t_window:?} gpu={t_gpu:?} fonts={t_fonts:?} \
             renderer={t_renderer:?} mux={t_mux:?} total={t_total:?}",
        );
        log::info!(
            "app: initialized — {w}x{h} px, {cols} cols × {rows} rows, \
             caption={caption_height}px, font={} {:.1}pt",
            renderer.family_name(),
            self.config.font.size,
        );

        // Render a clear frame with the theme's background color before
        // showing the window. This prevents the white/gray flash that occurs
        // when the compositor displays an uninitialized framebuffer.
        let theme = self
            .config
            .colors
            .resolve_theme(crate::platform::theme::system_theme);
        let palette = config_reload::build_palette_from_config(&self.config.colors, theme);
        gpu.clear_surface(window.surface(), palette.background(), opacity);

        // Show window — winit won't deliver RedrawRequested to an invisible
        // window, so we must be visible before storing state.
        window.set_visible(true);

        let winit_id = window.window_id();
        let ctx = WindowContext::new(window, chrome_widget, tab_bar_widget, grid_widget);
        self.gpu = Some(gpu);
        self.renderer = Some(renderer);
        self.windows.insert(winit_id, ctx);
        self.focused_window_id = Some(winit_id);
        self.mux = Some(mux);
        self.active_window = Some(mux_window_id);
        Ok(())
    }

    /// Spawn font discovery on a background thread.
    #[expect(
        clippy::type_complexity,
        reason = "thread join handle with font discovery result — not worth a type alias"
    )]
    fn spawn_font_discovery(
        &self,
    ) -> Result<
        std::thread::JoinHandle<
            Result<(FontCollection, usize, std::time::Duration), crate::font::FontError>,
        >,
        Box<dyn std::error::Error>,
    > {
        let font_weight = self.config.font.effective_weight();
        let font_size_pt = self.config.font.size;
        let font_config = self.config.font.clone();
        let font_dpi = DEFAULT_DPI;

        std::thread::Builder::new()
            .name("font-discovery".into())
            .spawn(move || {
                let t0 = std::time::Instant::now();
                let mut font_set = FontSet::load(font_config.family.as_deref(), font_weight)?;

                // Prepend user-configured fallback fonts.
                let user_fb_families: Vec<&str> = font_config
                    .fallback
                    .iter()
                    .map(|f| f.family.as_str())
                    .collect();
                let user_fb_count = font_set.prepend_user_fallbacks(&user_fb_families);

                // Default to Full hinting + Alpha format; adjusted after window
                // creation once the actual display scale factor is known.
                let fc = FontCollection::new(
                    font_set,
                    font_size_pt,
                    font_dpi,
                    GlyphFormat::Alpha,
                    font_weight,
                    HintingMode::Full,
                )?;
                Ok((fc, user_fb_count, t0.elapsed()))
            })
            .map_err(|e| -> Box<dyn std::error::Error> {
                format!("failed to spawn font discovery thread: {e}").into()
            })
    }

    /// Create chrome and tab bar widgets, and apply platform window effects.
    ///
    /// Returns `(chrome_widget, tab_bar_widget, caption_height)`.
    pub(super) fn create_chrome_widgets(
        &self,
        window: &TermWindow,
    ) -> (
        WindowChromeWidget,
        oriterm_ui::widgets::tab_bar::TabBarWidget,
        f32,
    ) {
        let (w, _) = window.size_px();
        let logical_w = w as f32 / window.scale_factor().factor() as f32;
        let chrome_widget = WindowChromeWidget::with_theme("ori", logical_w, &self.ui_theme);
        let caption_height = chrome_widget.caption_height();

        // Enable Aero Snap on Windows (installs WndProc subclass).
        // All values are in physical pixels — the subclass proc works in
        // the physical coordinate space of WM_NCHITTEST cursor positions.
        #[cfg(target_os = "windows")]
        {
            let s = window.scale_factor().factor() as f32;
            oriterm_ui::platform_windows::enable_snap(
                window.window(),
                oriterm_ui::widgets::window_chrome::constants::RESIZE_BORDER_WIDTH * s,
                caption_height * s,
            );
            oriterm_ui::platform_windows::set_client_rects(
                window.window(),
                chrome_widget
                    .interactive_rects()
                    .iter()
                    .map(|r| super::chrome::scale_rect(*r, s))
                    .collect(),
            );
        }

        let mut tab_bar_widget =
            oriterm_ui::widgets::tab_bar::TabBarWidget::with_theme(logical_w, &self.ui_theme);
        tab_bar_widget.set_tabs(vec![oriterm_ui::widgets::tab_bar::TabEntry::new("")]);

        (chrome_widget, tab_bar_widget, caption_height)
    }

    /// Create an initial tab with one pane in the given mux window.
    ///
    /// The mux and window must already be created. The pane is stored in
    /// `self.panes`. Setup notifications are drained and discarded.
    pub(super) fn create_initial_tab(
        &mut self,
        mux: &mut InProcessMux,
        window_id: oriterm_mux::WindowId,
        rows: u16,
        cols: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let theme = self
            .config
            .colors
            .resolve_theme(crate::platform::theme::system_theme);

        let config = SpawnConfig {
            cols,
            rows,
            scrollback: self.config.terminal.scrollback,
            ..SpawnConfig::default()
        };

        let (_tab_id, pane_id, pane) =
            mux.create_tab(window_id, &config, theme, &self.event_proxy)?;

        // Apply color scheme + user overrides to the pane's terminal palette.
        self.apply_palette_to_pane(&pane, theme);

        self.panes.insert(pane_id, pane);

        // Drain setup notifications (not useful at init time).
        let mut discard = Vec::new();
        mux.drain_notifications(&mut discard);

        Ok(())
    }
}
