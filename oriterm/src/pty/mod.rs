//! Cross-platform PTY abstraction.
//!
//! Provides PTY creation, shell spawning, a background reader thread, and the
//! message channel for main-thread → PTY-thread communication.
//! Uses `portable-pty` for platform abstraction: `ConPTY` on Windows,
//! `openpty`/`forkpty` on Linux, POSIX PTY on macOS.

mod event_loop;
mod spawn;

#[cfg(unix)]
pub mod signal;

pub(crate) use event_loop::PtyEventLoop;
#[allow(unused_imports, reason = "returned by PtyHandle::wait/try_wait; callers need access")]
pub use spawn::ExitStatus;
pub use spawn::{PtyConfig, PtyControl, PtyHandle, spawn_pty};

/// Commands sent from the main thread to the PTY reader thread.
///
/// Delivered via `std::sync::mpsc::channel`. The sender is held by
/// [`Notifier`](crate::tab::Notifier), the receiver by
/// [`PtyEventLoop`].
#[derive(Debug)]
pub enum Msg {
    /// Bytes to write to the PTY (keyboard input, escape responses).
    Input(Vec<u8>),
    /// Resize the PTY and terminal grid.
    Resize {
        /// New row count.
        rows: u16,
        /// New column count.
        cols: u16,
    },
    /// Gracefully stop the reader thread.
    Shutdown,
}

#[cfg(test)]
mod tests;
