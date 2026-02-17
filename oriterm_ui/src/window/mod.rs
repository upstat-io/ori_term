//! Config-driven window creation for frameless Chrome-style CSD.
//!
//! All platforms use frameless windows from day one. Windows are created
//! invisible so the first frame can be rendered before showing, preventing
//! a white flash.

use std::fmt;
use std::sync::Arc;

use winit::event_loop::ActiveEventLoop;
use winit::window::{Icon, Window, WindowAttributes};

use crate::geometry::{Point, Size};

/// Configuration for creating a new window.
///
/// Scale factor is not included — it is a runtime property of the display,
/// not a configuration input. Query `window.scale_factor()` after creation.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    /// Window title.
    pub title: String,
    /// Logical inner size in device-independent pixels.
    pub inner_size: Size,
    /// Enable transparent background (compositor alpha blending).
    pub transparent: bool,
    /// Enable background blur (macOS vibrancy, Windows Acrylic/Mica).
    pub blur: bool,
    /// Window opacity `[0.0, 1.0]`. Values >= 1.0 are fully opaque.
    pub opacity: f32,
    /// Initial window position, or `None` for OS default.
    pub position: Option<Point>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: String::from("oriterm"),
            inner_size: Size::new(1024.0, 768.0),
            transparent: false,
            blur: false,
            opacity: 1.0,
            position: None,
        }
    }
}

/// Errors that can occur during window creation.
#[derive(Debug)]
pub enum WindowError {
    /// The windowing system refused to create the window.
    Creation(winit::error::OsError),
}

impl fmt::Display for WindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Creation(e) => write!(f, "window creation failed: {e}"),
        }
    }
}

impl std::error::Error for WindowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Creation(e) => Some(e),
        }
    }
}

impl From<winit::error::OsError> for WindowError {
    fn from(e: winit::error::OsError) -> Self {
        Self::Creation(e)
    }
}

/// Creates a new frameless window from the given configuration.
///
/// The window is created invisible. Call `window.set_visible(true)` after
/// rendering the first frame to avoid a white flash.
pub fn create_window(
    event_loop: &ActiveEventLoop,
    config: &WindowConfig,
) -> Result<Arc<Window>, WindowError> {
    let attrs = build_window_attributes(config);
    let window = event_loop.create_window(attrs)?;
    Ok(Arc::new(window))
}

/// Builds platform-aware [`WindowAttributes`] from a [`WindowConfig`].
///
/// All platforms share a frameless, initially-invisible window. Per-platform
/// `#[cfg]` blocks add OS-specific attributes.
fn build_window_attributes(config: &WindowConfig) -> WindowAttributes {
    let mut attrs = WindowAttributes::default()
        .with_title(&config.title)
        .with_inner_size(winit::dpi::LogicalSize::new(
            config.inner_size.width(),
            config.inner_size.height(),
        ))
        .with_decorations(false)
        .with_visible(false)
        .with_transparent(config.transparent);

    if let Some(pos) = config.position {
        attrs = attrs.with_position(winit::dpi::LogicalPosition::new(pos.x, pos.y));
    }

    if let Some(icon) = load_icon() {
        attrs = attrs.with_window_icon(Some(icon));
    }

    attrs = apply_platform_attributes(attrs, config);
    attrs
}

/// Loads the embedded application icon (256x256 RGBA, decoded at build time).
///
/// Returns `None` if the icon data is malformed.
fn load_icon() -> Option<Icon> {
    static ICON_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon_rgba.bin"));

    if ICON_DATA.len() < 8 {
        return None;
    }

    let w = u32::from_le_bytes([ICON_DATA[0], ICON_DATA[1], ICON_DATA[2], ICON_DATA[3]]);
    let h = u32::from_le_bytes([ICON_DATA[4], ICON_DATA[5], ICON_DATA[6], ICON_DATA[7]]);
    let rgba = &ICON_DATA[8..];

    let expected_len = (w as usize) * (h as usize) * 4;
    if rgba.len() != expected_len {
        log::warn!(
            "icon RGBA data length mismatch: expected {expected_len}, got {}",
            rgba.len()
        );
        return None;
    }

    Icon::from_rgba(rgba.to_vec(), w, h).ok()
}

/// Applies platform-specific window attributes.
#[cfg(target_os = "windows")]
fn apply_platform_attributes(attrs: WindowAttributes, config: &WindowConfig) -> WindowAttributes {
    use winit::platform::windows::WindowAttributesExtWindows;

    let mut attrs = attrs;
    if config.transparent {
        // DirectComposition requires no redirection bitmap for alpha blending.
        attrs = attrs.with_no_redirection_bitmap(true);
    }
    attrs
}

/// Applies platform-specific window attributes.
#[cfg(target_os = "macos")]
fn apply_platform_attributes(attrs: WindowAttributes, _config: &WindowConfig) -> WindowAttributes {
    use winit::platform::macos::{OptionAsAlt, WindowAttributesExtMacOS};

    attrs
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true)
        .with_option_as_alt(OptionAsAlt::Both)
}

/// Applies platform-specific window attributes.
#[cfg(target_os = "linux")]
fn apply_platform_attributes(attrs: WindowAttributes, _config: &WindowConfig) -> WindowAttributes {
    use winit::platform::x11::WindowAttributesExtX11;

    // WM_CLASS for X11 window managers (used for taskbar grouping, rules).
    attrs.with_name("oriterm", "oriterm")
}

/// Applies platform-specific window attributes (fallback for other platforms).
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn apply_platform_attributes(attrs: WindowAttributes, _config: &WindowConfig) -> WindowAttributes {
    attrs
}
