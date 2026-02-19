//! Application struct and winit event loop handler.
//!
//! [`App`] implements winit's [`ApplicationHandler`] to drive the terminal.
//! It wires together the three-phase rendering pipeline (Extract → Prepare →
//! Render), handles window events, and dispatches terminal events from the
//! PTY reader thread.

mod cursor_blink;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::window::WindowId;

use oriterm_core::{Event, TermMode};
use oriterm_ui::window::WindowConfig;

use self::cursor_blink::CursorBlink;
use crate::font::{FontCollection, FontSet, GlyphFormat, HintingMode, SubpixelMode};
use crate::gpu::{GpuRenderer, GpuState, SurfaceError, ViewportSize, extract_frame};
use crate::tab::{Tab, TabId, TermEvent};
use crate::window::TermWindow;

/// Default font size in points (wired from config in Section 13).
const DEFAULT_FONT_SIZE_PT: f32 = 11.0;

/// Default DPI for font rasterization (wired from config in Section 13).
const DEFAULT_DPI: f32 = 96.0;

/// Default font weight (CSS-style 100–900).
const DEFAULT_FONT_WEIGHT: u16 = 400;

/// Default scrollback buffer size in lines.
const DEFAULT_SCROLLBACK: usize = 10_000;

/// Terminal application state and event loop handler.
///
/// Owns all top-level resources: GPU state, renderer, window, and tab.
/// Implements winit's `ApplicationHandler<TermEvent>` to receive both
/// window events and terminal events from the PTY reader thread.
pub(crate) struct App {
    // GPU + rendering (lazy init on Resumed).
    gpu: Option<GpuState>,
    renderer: Option<GpuRenderer>,
    window: Option<TermWindow>,

    // Terminal state (single tab for now; multi-tab in Section 15).
    tab: Option<Tab>,

    // Event loop proxy for creating per-tab EventProxy instances.
    event_proxy: EventLoopProxy<TermEvent>,

    // Redraw coalescing.
    dirty: bool,

    // Cursor blink state (application-level, not terminal-level).
    cursor_blink: CursorBlink,

    // Whether the terminal's CURSOR_BLINKING mode is active.
    // Cached from the last extracted frame to gate blink timer in about_to_wait.
    blinking_active: bool,

    // Configuration.
    window_config: WindowConfig,
}

impl App {
    /// Create a new application instance.
    ///
    /// All GPU/window/tab state is `None` until [`resumed`] is called by
    /// the event loop (lazy initialization pattern from winit docs).
    pub(crate) fn new(event_proxy: EventLoopProxy<TermEvent>, window_config: WindowConfig) -> Self {
        Self {
            gpu: None,
            renderer: None,
            window: None,
            tab: None,
            event_proxy,
            dirty: false,
            cursor_blink: CursorBlink::new(),
            blinking_active: false,
            window_config,
        }
    }

    /// Run the one-shot startup sequence: window → GPU → fonts → renderer → tab.
    ///
    /// Returns `Err` with a displayable message on any failure. The caller
    /// logs the error and exits the event loop.
    fn try_init(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn std::error::Error>> {
        let t_start = std::time::Instant::now();

        // 1. Create window (invisible) for GPU surface capability probing.
        let window_arc = oriterm_ui::window::create_window(event_loop, &self.window_config)?;
        let t_window = t_start.elapsed();

        // 2. Spawn font discovery on a background thread (no GPU dependency).
        let font_weight = DEFAULT_FONT_WEIGHT;
        let font_size_pt = DEFAULT_FONT_SIZE_PT;
        let font_dpi = DEFAULT_DPI;
        let font_handle = std::thread::Builder::new()
            .name("font-discovery".into())
            .spawn(
                move || -> Result<(FontCollection, std::time::Duration), crate::font::FontError> {
                    let t0 = std::time::Instant::now();
                    let font_set = FontSet::load(None, font_weight)?;
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
        let gpu = GpuState::new(&window_arc, self.window_config.transparent)?;
        let t_gpu = t_gpu_start.elapsed();

        // 4. Wrap the same window into TermWindow (creates surface, applies effects).
        let window = TermWindow::from_window(window_arc, &self.window_config, &gpu)?;

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

        // 8. Spawn the terminal tab (PTY + VTE + Term).
        let t_tab_start = std::time::Instant::now();
        let tab_id = TabId::next();
        let tab = Tab::new(
            tab_id,
            rows as u16,
            cols as u16,
            DEFAULT_SCROLLBACK,
            self.event_proxy.clone(),
        )?;
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
            DEFAULT_FONT_SIZE_PT,
        );

        // Show window before storing — winit won't deliver RedrawRequested
        // to an invisible window, so we must be visible first.
        window.set_visible(true);

        self.gpu = Some(gpu);
        self.renderer = Some(renderer);
        self.window = Some(window);
        self.tab = Some(tab);
        self.dirty = true;
        Ok(())
    }

    /// Execute the three-phase rendering pipeline: Extract → Prepare → Render.
    fn handle_redraw(&mut self) {
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

            let mut frame = extract_frame(tab.terminal(), viewport, cell);

            // Cache blinking mode for about_to_wait gating.
            // Reset blink phase on false→true transition so the
            // cursor starts visible when blinking is first enabled.
            let blinking_now = frame.content.mode.contains(TermMode::CURSOR_BLINKING);
            if blinking_now && !self.blinking_active {
                self.cursor_blink.reset();
            }
            self.blinking_active = blinking_now;

            // Apply cursor blink: hide cursor during the "off" phase
            // when the terminal has requested blinking.
            if blinking_now && !self.cursor_blink.is_visible() {
                frame.content.cursor.visible = false;
            }

            renderer.prepare(&frame, gpu);
            log::trace!(
                "frame: cells={} bg_inst={} glyph_inst={} cursor_inst={}",
                frame.content.cells.len(),
                renderer.prepared().backgrounds.len(),
                renderer.prepared().glyphs.len(),
                renderer.prepared().cursors.len(),
            );
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

impl ApplicationHandler<TermEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        if let Err(e) = self.try_init(event_loop) {
            log::error!("startup failed: {e}");
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                if let Some(gpu) = &self.gpu {
                    gpu.save_pipeline_cache();
                }
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let (Some(gpu), Some(window), Some(renderer)) =
                    (&self.gpu, &mut self.window, &self.renderer)
                {
                    window.resize_surface(size.width, size.height, gpu);

                    let cell = renderer.cell_metrics();
                    let cols = cell.columns(size.width).max(1);
                    let rows = cell.rows(size.height).max(1);

                    if let Some(tab) = &self.tab {
                        tab.resize(rows as u16, cols as u16);
                    }

                    self.dirty = true;
                }
            }

            WindowEvent::RedrawRequested => self.handle_redraw(),

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                if let Some(text) = &event.text {
                    if let Some(tab) = &self.tab {
                        tab.write_input(text.as_bytes());
                        self.cursor_blink.reset();
                        self.dirty = true;
                    }
                }
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(window) = &mut self.window {
                    if window.update_scale_factor(scale_factor) {
                        // Re-evaluate hinting and subpixel mode for the new scale.
                        let hinting = HintingMode::from_scale_factor(scale_factor);
                        let format = SubpixelMode::from_scale_factor(scale_factor).glyph_format();
                        if let (Some(renderer), Some(gpu)) = (&mut self.renderer, &self.gpu) {
                            renderer.set_hinting_mode(hinting, gpu);
                            renderer.set_glyph_format(format, gpu);
                        }
                        self.dirty = true;
                    }
                }
            }

            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: TermEvent) {
        let TermEvent::Terminal { tab_id: _, event } = event;
        match event {
            Event::Wakeup => {
                self.dirty = true;
            }
            Event::Bell => {
                if let Some(tab) = &mut self.tab {
                    tab.set_bell();
                }
            }
            Event::Title(title) => {
                if let Some(tab) = &mut self.tab {
                    tab.set_title(title);
                }
            }
            Event::ResetTitle => {
                if let Some(tab) = &mut self.tab {
                    tab.set_title(String::new());
                }
            }
            Event::PtyWrite(s) => {
                if let Some(tab) = &self.tab {
                    tab.write_input(s.as_bytes());
                }
            }
            Event::ChildExit(code) => {
                log::info!("child process exited with code {code}");
                if let Some(gpu) = &self.gpu {
                    gpu.save_pipeline_cache();
                }
                event_loop.exit();
            }
            _ => {
                log::debug!("unhandled terminal event: {event:?}");
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Drive cursor blink timer only when blinking is active.
        if self.blinking_active && self.cursor_blink.update() {
            self.dirty = true;
        }

        if self.dirty {
            log::debug!("about_to_wait: dirty, requesting redraw");
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            self.dirty = false;
        }

        // Schedule wakeup for the next blink toggle so the event loop
        // doesn't sleep past it. When blinking is inactive, the default
        // ControlFlow::Wait lets the event loop sleep indefinitely.
        if self.blinking_active {
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.cursor_blink.next_toggle()));
        }
    }
}
