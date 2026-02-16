//! Unix clipboard provider using `arboard`.
//!
//! On Linux, supports both the system clipboard and X11/Wayland primary
//! selection (middle-click paste). On macOS, only the system clipboard
//! is available.

use log::warn;

use super::{Clipboard, ClipboardProvider, NopProvider};

/// System clipboard provider via `arboard`.
struct ArboardClipboard {
    ctx: arboard::Clipboard,
}

impl ClipboardProvider for ArboardClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.ctx.get_text().ok()
    }

    fn set_text(&mut self, text: &str) -> bool {
        self.ctx.set_text(text.to_owned()).is_ok()
    }
}

/// X11/Wayland primary selection provider via `arboard`.
///
/// Uses `GetExtLinux`/`SetExtLinux` extension traits to access the
/// primary selection rather than the system clipboard.
#[cfg(target_os = "linux")]
struct ArboardSelection {
    ctx: arboard::Clipboard,
}

#[cfg(target_os = "linux")]
impl ClipboardProvider for ArboardSelection {
    fn get_text(&mut self) -> Option<String> {
        use arboard::{GetExtLinux, LinuxClipboardKind};
        self.ctx
            .get()
            .clipboard(LinuxClipboardKind::Primary)
            .text()
            .ok()
    }

    fn set_text(&mut self, text: &str) -> bool {
        use arboard::{LinuxClipboardKind, SetExtLinux};
        self.ctx
            .set()
            .clipboard(LinuxClipboardKind::Primary)
            .text(text.to_owned())
            .is_ok()
    }
}

/// Create a Unix clipboard with platform-appropriate providers.
///
/// On Linux, creates both a system clipboard and primary selection provider.
/// On macOS, creates only a system clipboard provider.
/// Falls back to `NopProvider` if `arboard::Clipboard::new()` fails.
pub(super) fn create() -> Clipboard {
    let clipboard: Box<dyn ClipboardProvider> = match arboard::Clipboard::new() {
        Ok(ctx) => Box::new(ArboardClipboard { ctx }),
        Err(e) => {
            warn!("failed to create clipboard context: {e}, clipboard disabled");
            Box::new(NopProvider)
        }
    };

    #[cfg(target_os = "linux")]
    let selection: Option<Box<dyn ClipboardProvider>> = match arboard::Clipboard::new() {
        Ok(ctx) => Some(Box::new(ArboardSelection { ctx })),
        Err(e) => {
            warn!("failed to create primary selection context: {e}");
            None
        }
    };

    #[cfg(not(target_os = "linux"))]
    let selection: Option<Box<dyn ClipboardProvider>> = None;

    Clipboard {
        clipboard,
        selection,
    }
}
