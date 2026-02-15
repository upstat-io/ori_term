//! Terminal event types and listener traits.
//!
//! Events flow outward from the terminal state machine to the UI layer.
//! The `EventListener` trait decouples `Term<T>` from any specific UI
//! framework — tests use `VoidListener`, the real app routes events
//! through winit's event loop proxy.

use std::fmt;
use std::sync::Arc;

use crate::color::Rgb;

/// Which system clipboard to target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardType {
    /// OS clipboard (Ctrl+C / Ctrl+V).
    Clipboard,
    /// X11 primary selection (middle-click paste).
    Selection,
}

/// Terminal events that flow outward to the UI layer.
///
/// Produced by VTE handler methods on `Term<T>`. The attached
/// `EventListener` receives these via `send_event`.
#[derive(Clone)]
pub enum Event {
    /// New content available — trigger a redraw.
    Wakeup,
    /// BEL character received.
    Bell,
    /// Window title changed (OSC 0/2).
    Title(String),
    /// Window title reset to default.
    ResetTitle,
    /// OSC 52 clipboard store request.
    ClipboardStore(ClipboardType, String),
    /// OSC 52 clipboard load request.
    ///
    /// The closure formats the clipboard text into the response escape
    /// sequence that gets written back to the PTY.
    ClipboardLoad(ClipboardType, Arc<dyn Fn(&str) -> String + Send + Sync>),
    /// OSC 4/10/11 color query response.
    ///
    /// The closure formats the RGB color into the response escape sequence.
    ColorRequest(usize, Arc<dyn Fn(Rgb) -> String + Send + Sync>),
    /// Response bytes to write back to PTY (DA, DSR, DECRPM, etc.).
    PtyWrite(String),
    /// Cursor blink state toggled via DECSET/DECRST.
    CursorBlinkingChange,
    /// Mouse cursor shape may need update (e.g. hover over hyperlink).
    MouseCursorDirty,
    /// Child process exited with the given status code.
    ChildExit(i32),
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wakeup => write!(f, "Wakeup"),
            Self::Bell => write!(f, "Bell"),
            Self::Title(t) => write!(f, "Title({t})"),
            Self::ResetTitle => write!(f, "ResetTitle"),
            Self::ClipboardStore(ty, text) => write!(f, "ClipboardStore({ty:?}, {text})"),
            Self::ClipboardLoad(ty, _) => write!(f, "ClipboardLoad({ty:?})"),
            Self::ColorRequest(idx, _) => write!(f, "ColorRequest({idx})"),
            Self::PtyWrite(text) => write!(f, "PtyWrite({text})"),
            Self::CursorBlinkingChange => write!(f, "CursorBlinkingChange"),
            Self::MouseCursorDirty => write!(f, "MouseCursorDirty"),
            Self::ChildExit(code) => write!(f, "ChildExit({code})"),
        }
    }
}

/// Receives terminal events from `Term<T>`.
///
/// The default implementation is a no-op, so `VoidListener` needs no
/// method body. Real implementations route events to the UI event loop.
///
/// Bound: `Send + 'static` because the PTY reader thread may fire events
/// from a background thread.
pub trait EventListener: Send + 'static {
    /// Handle a terminal event. Default: no-op.
    fn send_event(&self, _event: Event) {}
}

/// No-op event listener for tests and headless operation.
pub struct VoidListener;

impl EventListener for VoidListener {}

#[cfg(test)]
mod tests;
