//! Windows clipboard provider using `clipboard-win`.
//!
//! Stateless — `clipboard-win` exposes free functions that access the
//! Win32 clipboard directly, no persistent context needed.

use super::{Clipboard, ClipboardProvider};

/// Windows clipboard via `clipboard-win` free functions.
struct WindowsProvider;

impl ClipboardProvider for WindowsProvider {
    fn get_text(&mut self) -> Option<String> {
        clipboard_win::get_clipboard_string().ok()
    }

    fn set_text(&mut self, text: &str) -> bool {
        clipboard_win::set_clipboard_string(text).is_ok()
    }
}

/// Create a Windows clipboard (system clipboard only, no primary selection).
pub(super) fn create() -> Clipboard {
    Clipboard {
        clipboard: Box::new(WindowsProvider),
        selection: None,
    }
}
