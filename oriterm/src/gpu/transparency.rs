//! Platform-specific window transparency and compositor effects.
//!
//! Applies blur/vibrancy effects when the terminal has sub-1.0 opacity:
//! - **Windows**: Acrylic blur via `DwmSetWindowAttribute` (Windows 11),
//!   using the `window-vibrancy` crate. Falls back to opaque on Win10
//!   without DWM composition.
//! - **macOS**: `NSVisualEffectView` vibrancy via `window-vibrancy`.
//! - **Linux**: Compositor-driven blur via `winit::Window::set_blur()`.
//!   Requires a compositor (Picom, `KWin`, Mutter, Sway). Falls back to
//!   opaque when no compositor is running.

// Transparency helpers are fully implemented but not yet called from the event
// loop (added in Section 05). Suppress dead-code warnings until then.
#![expect(dead_code, reason = "transparency infrastructure used in Section 05")]

use winit::window::Window;

/// Apply platform-specific transparency effects to a window.
///
/// Does nothing when `opacity >= 1.0`. When `blur` is true and the platform
/// supports it, enables frosted glass / vibrancy behind transparent areas.
pub fn apply_transparency(window: &Window, opacity: f32, blur: bool) {
    if opacity >= 1.0 {
        return;
    }

    if blur {
        apply_blur(window, opacity);
    }
}

/// Apply platform-specific blur effects.
#[cfg(target_os = "windows")]
fn apply_blur(window: &Window, opacity: f32) {
    let alpha = (opacity * 255.0) as u8;
    // Tinted dark background with the requested opacity.
    let color = Some((30_u8, 30, 46, alpha));
    match window_vibrancy::apply_acrylic(window, color) {
        Ok(()) => log::info!("transparency: acrylic applied (alpha={alpha})"),
        Err(e) => log::warn!("transparency: acrylic failed: {e}"),
    }
}

#[cfg(target_os = "macos")]
fn apply_blur(window: &Window, _opacity: f32) {
    match window_vibrancy::apply_vibrancy(
        window,
        window_vibrancy::NSVisualEffectMaterial::UnderWindowBackground,
        None,
        None,
    ) {
        Ok(()) => log::info!("transparency: macOS vibrancy applied"),
        Err(e) => log::warn!("transparency: macOS vibrancy failed: {e}"),
    }
}

#[cfg(target_os = "linux")]
fn apply_blur(window: &Window, _opacity: f32) {
    window.set_blur(true);
    log::info!("transparency: compositor blur enabled");
}

// Fallback for other platforms (WASM, etc.).
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn apply_blur(_window: &Window, _opacity: f32) {
    log::debug!("transparency: blur not supported on this platform");
}
