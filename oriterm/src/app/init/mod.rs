//! One-shot application startup: window → GPU → fonts → renderer → tab.

use winit::event_loop::ActiveEventLoop;

use oriterm_ui::window::WindowConfig;

use super::{App, DEFAULT_DPI};
use crate::font::{FontCollection, FontSet, GlyphFormat, HintingMode, SubpixelMode};
use crate::gpu::{GpuRenderer, GpuState};
use crate::tab::{Tab, TabId};
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
        let font_weight = self.config.font.effective_weight();
        let font_size_pt = self.config.font.size;
        let font_family = self.config.font.family.clone();
        let font_dpi = DEFAULT_DPI;
        let font_handle = std::thread::Builder::new()
            .name("font-discovery".into())
            .spawn(
                move || -> Result<(FontCollection, std::time::Duration), crate::font::FontError> {
                    let t0 = std::time::Instant::now();
                    let font_set = FontSet::load(font_family.as_deref(), font_weight)?;
                    // Default to Full hinting; adjusted after window creation
                    // once the actual display scale factor is known.
                    let fc = FontCollection::new(
                        font_set,
                        font_size_pt,
                        font_dpi,
                        GlyphFormat::Alpha,
                        font_weight,
                        HintingMode::Full,
                    )?;
                    Ok((fc, t0.elapsed()))
                },
            )
            .map_err(|e| -> Box<dyn std::error::Error> {
                format!("failed to spawn font discovery thread: {e}").into()
            })?;

        // 3. Init GPU on main thread (requires window Arc, runs concurrently with fonts).
        let t_gpu_start = std::time::Instant::now();
        let gpu = GpuState::new(&window_arc, window_config.transparent)?;
        let t_gpu = t_gpu_start.elapsed();

        // 4. Wrap the same window into TermWindow (creates surface, applies effects).
        let window = TermWindow::from_window(window_arc, &window_config, &gpu)?;

        // 5. Join font thread (GPU init + surface setup ran concurrently).
        let (mut font_collection, t_fonts) = match font_handle.join() {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => return Err("font discovery thread panicked".into()),
        };

        // 5b. Adjust hinting and subpixel mode for the actual display scale factor.
        // The font thread used Full hinting + Alpha format as defaults; HiDPI
        // displays (2x+) disable hinting and subpixel rendering.
        let scale = window.scale_factor().factor();
        let hinting = HintingMode::from_scale_factor(scale);
        font_collection.set_hinting(hinting);
        let subpixel_format = SubpixelMode::from_scale_factor(scale).glyph_format();
        font_collection.set_format(subpixel_format);

        // 6. Create GPU renderer (pipelines, atlas, pre-cached ASCII glyphs).
        let t_renderer_start = std::time::Instant::now();
        let renderer = GpuRenderer::new(&gpu, font_collection);
        let t_renderer = t_renderer_start.elapsed();

        // 7. Compute grid dimensions from viewport and cell metrics.
        let (w, h) = window.size_px();
        let cell = renderer.cell_metrics();
        let cols = cell.columns(w).max(1);
        let rows = cell.rows(h).max(1);

        // 8. Create grid widget with cell metrics and initial grid size.
        let grid_widget = TerminalGridWidget::new(cell.width, cell.height, cols, rows);
        grid_widget.set_bounds(oriterm_ui::geometry::Rect::new(
            0.0,
            0.0,
            cols as f32 * cell.width,
            rows as f32 * cell.height,
        ));

        // 9. Spawn the terminal tab (PTY + VTE + Term).
        let t_tab_start = std::time::Instant::now();
        let tab_id = TabId::next();
        let theme = self
            .config
            .colors
            .resolve_theme(crate::platform::theme::system_theme);
        let tab_cfg = crate::tab::TabConfig {
            rows: rows as u16,
            cols: cols as u16,
            scrollback: self.config.terminal.scrollback,
            theme,
        };
        let tab = Tab::new(tab_id, &tab_cfg, self.event_proxy.clone())?;
        let t_tab = t_tab_start.elapsed();

        let t_total = t_start.elapsed();
        log::info!(
            "app: startup — window={t_window:?} gpu={t_gpu:?} fonts={t_fonts:?} \
             renderer={t_renderer:?} tab={t_tab:?} total={t_total:?}",
        );
        log::info!(
            "app: initialized — {w}x{h} px, {cols} cols × {rows} rows, \
             font={} {:.1}pt",
            renderer.family_name(),
            font_size_pt,
        );

        // Show window before storing — winit won't deliver RedrawRequested
        // to an invisible window, so we must be visible first.
        window.set_visible(true);

        self.gpu = Some(gpu);
        self.renderer = Some(renderer);
        self.window = Some(window);
        self.tab = Some(tab);
        self.terminal_grid = Some(grid_widget);
        self.dirty = true;
        Ok(())
    }
}
