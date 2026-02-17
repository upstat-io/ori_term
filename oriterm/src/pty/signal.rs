//! SIGCHLD signal handling for child process exit detection.
//!
//! On Unix, child process exits are signaled via SIGCHLD. This module
//! registers an async-signal-safe handler that sets an atomic flag,
//! which can be polled from the event loop to detect child exits.
//!
//! Call [`init`] once at startup, then [`check`] periodically to detect
//! whether any child process has exited since the last check.

use std::io;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use signal_hook::consts::SIGCHLD;

/// Initialization result: either the flag on success or an error message.
enum InitState {
    Ok(Arc<AtomicBool>),
    Err(String),
}

static STATE: OnceLock<InitState> = OnceLock::new();

/// Register the SIGCHLD signal handler.
///
/// Safe to call multiple times; only the first call registers the handler.
/// Must be called before [`check`] returns meaningful results.
pub fn init() -> io::Result<()> {
    let state = STATE.get_or_init(|| {
        let flag = Arc::new(AtomicBool::new(false));
        match signal_hook::flag::register(SIGCHLD, Arc::clone(&flag)) {
            Ok(_) => InitState::Ok(flag),
            Err(e) => InitState::Err(e.to_string()),
        }
    });
    match state {
        InitState::Ok(_) => Ok(()),
        InitState::Err(msg) => Err(io::Error::other(msg.clone())),
    }
}

/// Check whether SIGCHLD was received since the last check.
///
/// Returns `true` if one or more child processes have exited. The flag
/// is cleared atomically (test-and-clear), so consecutive calls without
/// an intervening signal return `false`.
pub fn check() -> bool {
    matches!(STATE.get(), Some(InitState::Ok(f)) if f.swap(false, Ordering::Relaxed))
}
