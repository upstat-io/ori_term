//! Terminal window: native window + GPU surface wrapper.
//!
//! [`TermWindow`] follows Chrome's `WindowTreeHost` pattern — a pure window
//! wrapper that owns the native window handle and GPU rendering surface.
//! It does **not** own tabs, content, or terminal state. Those belong in
//! [`App`](crate::App) (the orchestrator).
//!
//! Lifecycle: `TermWindow::new()` creates an invisible frameless window and
//! attaches a wgpu surface. After the first frame is rendered, the caller
//! shows the window via [`TermWindow::set_visible()`] to avoid a white flash.

// TermWindow is fully implemented but not yet called from the event loop
// (added later in Section 05). Suppress dead-code warnings until then.
#![expect(dead_code, reason = "TermWindow used once event loop is wired in Section 05")]

use std::fmt;
use std::sync::Arc;

use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use oriterm_ui::scale::ScaleFactor;
use oriterm_ui::window::WindowConfig;

use crate::gpu::state::GpuState;
use crate::gpu::transparency;

/// Default tint for the acrylic/blur layer (Catppuccin Mocha base).
///
/// The exact color is cosmetic — the terminal's background palette color
/// paints over it each frame. This only shows through during the brief
/// moment before the first frame renders.
const DEFAULT_BLUR_TINT: (u8, u8, u8) = (30, 30, 46);

/// A terminal window: native window + GPU surface.
///
/// Pure window wrapper (Chrome's `WindowTreeHost` tier). Owns the platform
/// window handle and wgpu rendering surface. Does NOT own tabs, content,
/// or any terminal state — the [`App`] maps windows to tabs.
pub(crate) struct TermWindow {
    /// Winit window handle (Arc for wgpu surface lifetime).
    window: Arc<Window>,
    /// wgpu rendering surface bound to this window.
    surface: wgpu::Surface<'static>,
    /// Surface format, size, present mode, alpha mode.
    surface_config: wgpu::SurfaceConfiguration,
    /// Window size in physical pixels `(width, height)`.
    size_px: (u32, u32),
    /// DPI scale factor from the display.
    scale_factor: ScaleFactor,
    /// Whether the window is currently maximized.
    is_maximized: bool,
}

impl TermWindow {
    /// Create a new terminal window with an attached GPU surface.
    ///
    /// The window is created invisible. Call [`set_visible(true)`](Self::set_visible)
    /// after rendering the first frame to prevent a white flash.
    ///
    /// When `config.transparent` and `config.blur` are both true, platform-specific
    /// vibrancy effects (Acrylic on Windows, vibrancy on macOS, compositor blur on
    /// Linux) are applied.
    pub(crate) fn new(
        event_loop: &ActiveEventLoop,
        config: &WindowConfig,
        gpu: &GpuState,
    ) -> Result<Self, WindowCreateError> {
        let window = oriterm_ui::window::create_window(event_loop, config)?;

        let (surface, surface_config) = gpu.create_surface(&window)?;

        let phys_size = window.inner_size();
        let size_px = (phys_size.width, phys_size.height);
        let scale_factor = ScaleFactor::new(window.scale_factor());

        // Apply vibrancy/blur when transparent background is requested.
        if config.transparent && config.blur {
            transparency::apply_transparency(&window, config.opacity, true, DEFAULT_BLUR_TINT);
        }

        Ok(Self {
            window,
            surface,
            surface_config,
            size_px,
            scale_factor,
            is_maximized: false,
        })
    }

    // Accessors

    /// Returns the winit [`WindowId`] for event routing.
    pub(crate) fn window_id(&self) -> WindowId {
        self.window.id()
    }

    /// Returns a reference to the underlying winit [`Window`].
    pub(crate) fn window(&self) -> &Window {
        &self.window
    }

    /// Returns the wgpu rendering surface.
    pub(crate) fn surface(&self) -> &wgpu::Surface<'static> {
        &self.surface
    }

    /// Returns the current surface configuration.
    pub(crate) fn surface_config(&self) -> &wgpu::SurfaceConfiguration {
        &self.surface_config
    }

    /// Returns the window size in physical pixels `(width, height)`.
    pub(crate) fn size_px(&self) -> (u32, u32) {
        self.size_px
    }

    /// Returns the DPI scale factor.
    pub(crate) fn scale_factor(&self) -> ScaleFactor {
        self.scale_factor
    }

    /// Returns whether the window is currently maximized.
    pub(crate) fn is_maximized(&self) -> bool {
        self.is_maximized
    }

    // Predicates

    /// Returns true if the window has a non-zero physical size.
    ///
    /// Windows with zero area (minimized) should skip rendering.
    pub(crate) fn has_surface_area(&self) -> bool {
        self.size_px.0 > 0 && self.size_px.1 > 0
    }

    // Public operations

    /// Reconfigure the surface after a window resize.
    ///
    /// Updates internal size tracking and reconfigures the wgpu surface.
    /// The caller should request a redraw after calling this.
    pub(crate) fn resize_surface(&mut self, width: u32, height: u32, gpu: &GpuState) {
        let w = width.max(1);
        let h = height.max(1);

        self.size_px = (w, h);
        self.surface_config.width = w;
        self.surface_config.height = h;
        self.surface.configure(&gpu.device, &self.surface_config);
    }

    /// Update the DPI scale factor (e.g. when the window moves between monitors).
    ///
    /// Returns `true` if the scale factor actually changed.
    pub(crate) fn update_scale_factor(&mut self, factor: f64) -> bool {
        let new_factor = ScaleFactor::new(factor);
        if self.scale_factor == new_factor {
            return false;
        }
        self.scale_factor = new_factor;
        true
    }

    /// Update the maximized state.
    pub(crate) fn set_maximized(&mut self, maximized: bool) {
        self.is_maximized = maximized;
    }

    /// Show or hide the window.
    ///
    /// Call `set_visible(true)` after rendering the first frame to avoid
    /// a white flash on window creation.
    pub(crate) fn set_visible(&self, visible: bool) {
        self.window.set_visible(visible);
    }

    /// Request the windowing system to schedule a redraw.
    pub(crate) fn request_redraw(&self) {
        self.window.request_redraw();
    }
}

/// Errors that can occur when creating a [`TermWindow`].
#[derive(Debug)]
pub(crate) enum WindowCreateError {
    /// The windowing system refused to create the window.
    Window(oriterm_ui::window::WindowError),
    /// GPU surface creation failed.
    Surface(wgpu::CreateSurfaceError),
}

impl fmt::Display for WindowCreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Window(e) => write!(f, "{e}"),
            Self::Surface(e) => write!(f, "failed to create GPU surface for window: {e}"),
        }
    }
}

impl std::error::Error for WindowCreateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Window(e) => Some(e),
            Self::Surface(e) => Some(e),
        }
    }
}

impl From<oriterm_ui::window::WindowError> for WindowCreateError {
    fn from(e: oriterm_ui::window::WindowError) -> Self {
        Self::Window(e)
    }
}

impl From<wgpu::CreateSurfaceError> for WindowCreateError {
    fn from(e: wgpu::CreateSurfaceError) -> Self {
        Self::Surface(e)
    }
}
