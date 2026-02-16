//! Platform clipboard access for copy/paste operations.
//!
//! Uses `clipboard-win` on Windows and `arboard` on Linux/macOS.
//! The `ClipboardProvider` trait enables mock implementations for testing.

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

use log::{debug, warn};

use oriterm_core::event::ClipboardType;

/// Platform-agnostic clipboard access.
///
/// Implementations wrap a system clipboard API (or a test mock).
/// Methods take `&mut self` because platform backends may hold mutable state
/// (e.g. an `arboard::Clipboard` context).
pub trait ClipboardProvider {
    /// Read text from this clipboard. Returns `None` on error.
    fn get_text(&mut self) -> Option<String>;

    /// Write text to this clipboard. Returns `true` on success.
    fn set_text(&mut self, text: &str) -> bool;
}

/// System clipboard with optional primary selection (X11/Wayland).
///
/// The `clipboard` field is always available. The `selection` field holds
/// an X11/Wayland primary selection provider on Linux, or `None` on
/// Windows/macOS where primary selection doesn't exist.
pub struct Clipboard {
    clipboard: Box<dyn ClipboardProvider>,
    selection: Option<Box<dyn ClipboardProvider>>,
}

impl Clipboard {
    /// Create a platform-appropriate clipboard.
    ///
    /// Falls back to a no-op provider if the system clipboard is unavailable
    /// (e.g. headless environment without a display server).
    pub fn new() -> Self {
        #[cfg(windows)]
        {
            windows::create()
        }
        #[cfg(not(windows))]
        {
            unix::create()
        }
    }

    /// Create a no-op clipboard for testing and headless operation.
    #[cfg(test)]
    pub fn new_nop() -> Self {
        Self {
            clipboard: Box::new(NopProvider),
            selection: None,
        }
    }

    /// Store text in the specified clipboard.
    ///
    /// Storing to `Selection` when no selection provider is available
    /// (Windows, macOS) is silently ignored.
    pub fn store(&mut self, ty: ClipboardType, text: &str) {
        let provider = match (ty, &mut self.selection) {
            (ClipboardType::Selection, Some(sel)) => sel,
            (ClipboardType::Selection, None) => return,
            _ => &mut self.clipboard,
        };

        if !provider.set_text(text) {
            warn!("unable to store text in clipboard");
        }
    }

    /// Load text from the specified clipboard.
    ///
    /// Loading from `Selection` when no selection provider is available
    /// falls back to the system clipboard (Alacritty convention).
    pub fn load(&mut self, ty: ClipboardType) -> String {
        let provider = match (ty, &mut self.selection) {
            (ClipboardType::Selection, Some(sel)) => sel,
            _ => &mut self.clipboard,
        };

        if let Some(text) = provider.get_text() {
            text
        } else {
            debug!("unable to load text from clipboard");
            String::new()
        }
    }
}

/// No-op clipboard provider for headless environments.
///
/// `set_text` always succeeds (silently discards). `get_text` always
/// returns `None`.
#[cfg(any(test, not(windows)))]
struct NopProvider;

#[cfg(any(test, not(windows)))]
impl ClipboardProvider for NopProvider {
    fn get_text(&mut self) -> Option<String> {
        None
    }

    fn set_text(&mut self, _text: &str) -> bool {
        true
    }
}

#[cfg(test)]
mod tests;
