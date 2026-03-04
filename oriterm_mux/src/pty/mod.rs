//! Cross-platform PTY abstraction.
//!
//! Provides PTY creation, shell spawning, a background reader thread, a
//! dedicated writer thread, and the message channel for main-thread → PTY
//! communication. Uses `portable-pty` for platform abstraction: `ConPTY`
//! on Windows, `openpty`/`forkpty` on Linux, POSIX PTY on macOS.

mod event_loop;
mod spawn;

#[cfg(unix)]
pub mod signal;

use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

pub use event_loop::PtyEventLoop;
#[allow(
    unused_imports,
    reason = "returned by PtyHandle::wait/try_wait; callers need access"
)]
pub use spawn::ExitStatus;
pub use spawn::{PtyConfig, PtyControl, PtyHandle, spawn_pty};

/// Commands sent from the main thread to the PTY writer thread.
///
/// Delivered via `std::sync::mpsc::channel`. The sender is held by
/// [`PaneNotifier`](crate::pane::PaneNotifier), the receiver by the
/// writer thread spawned via [`spawn_pty_writer`].
#[derive(Debug)]
pub enum Msg {
    /// Raw bytes to write to the PTY (keyboard input, escape responses).
    ///
    /// Sent by `PaneNotifier::notify()` on the main thread, written
    /// immediately by the dedicated writer thread.
    Input(Vec<u8>),
    /// Gracefully stop both the writer and reader threads.
    Shutdown,
}

/// Spawn a dedicated PTY writer thread.
///
/// Blocks on `rx.recv()` and writes immediately on [`Msg::Input`]. On
/// [`Msg::Shutdown`] (or channel close), sets the `shutdown` flag so the
/// reader thread ([`PtyEventLoop`]) can exit its blocking `read()` loop.
///
/// Separating reads and writes onto different threads prevents a deadlock
/// during shell startup: the shell sends DA1 (device attributes query),
/// the VTE parser generates the response via `Event::PtyWrite`, and the
/// main thread enqueues it as `Msg::Input`. If the writer lived on the
/// reader thread, the response would be stuck behind a blocking `read()`
/// that never returns because the shell is waiting for the DA response.
pub fn spawn_pty_writer(
    mut writer: Box<dyn Write + Send>,
    rx: mpsc::Receiver<Msg>,
    shutdown: Arc<AtomicBool>,
) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("pty-writer".into())
        .spawn(move || {
            while let Ok(msg) = rx.recv() {
                match msg {
                    Msg::Input(data) => {
                        if let Err(e) = writer.write_all(&data) {
                            log::warn!("PTY write failed: {e}");
                            break;
                        }
                        // Drain any additional queued writes before flushing.
                        while let Ok(msg) = rx.try_recv() {
                            match msg {
                                Msg::Input(data) => {
                                    if let Err(e) = writer.write_all(&data) {
                                        log::warn!("PTY write failed: {e}");
                                        shutdown.store(true, Ordering::Release);
                                        return;
                                    }
                                }
                                Msg::Shutdown => {
                                    shutdown.store(true, Ordering::Release);
                                    return;
                                }
                            }
                        }
                        let _ = writer.flush();
                    }
                    Msg::Shutdown => break,
                }
            }
            // Channel closed or shutdown received.
            shutdown.store(true, Ordering::Release);
        })
}

#[cfg(test)]
mod tests;
