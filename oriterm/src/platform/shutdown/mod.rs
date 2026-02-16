//! Cross-platform shutdown signal handling.
//!
//! Registers handlers for termination signals so the application can
//! perform clean shutdown (restore terminal state, kill child processes):
//! - **Unix**: `SIGTERM` and `SIGINT` via `signal-hook`.
//! - **Windows**: `SetConsoleCtrlHandler` for `CTRL_C_EVENT` and
//!   `CTRL_CLOSE_EVENT`.
//!
//! Call [`init`] once at startup, then poll [`should_shutdown`] from
//! the event loop.

// Shutdown signal infrastructure is wired into the event loop in Section 04.
// In test builds, tests exercise init/should_shutdown so dead_code doesn't
// fire — making #![expect(dead_code)] produce an unfulfilled-lint warning.
#![allow(dead_code, reason = "shutdown signals used in Section 04")]

use std::io;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global shutdown flag, set by signal handlers.
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Initialization state to ensure single registration.
static INIT: OnceLock<Result<(), String>> = OnceLock::new();

/// Register platform-specific shutdown signal handlers.
///
/// Safe to call multiple times; only the first call registers handlers.
pub fn init() -> io::Result<()> {
    let result = INIT.get_or_init(|| platform_init().map_err(|e| e.to_string()));
    match result {
        Ok(()) => Ok(()),
        Err(msg) => Err(io::Error::other(msg.clone())),
    }
}

/// Check whether a shutdown signal has been received.
///
/// Returns `true` if `SIGTERM`, `SIGINT`, or the Windows console control
/// event has fired since the last check. Non-destructive: repeated calls
/// return `true` once the flag is set.
pub fn should_shutdown() -> bool {
    SHUTDOWN.load(Ordering::Relaxed)
}

/// Unix: register `SIGTERM` and `SIGINT` via `signal-hook`.
///
/// Uses `signal_hook::low_level::register` to set the global `SHUTDOWN`
/// flag directly from the signal handler. `AtomicBool::store` with
/// `Ordering::Relaxed` is async-signal-safe.
#[cfg(unix)]
fn platform_init() -> io::Result<()> {
    use signal_hook::consts::{SIGINT, SIGTERM};

    // SAFETY: The closure only calls AtomicBool::store (async-signal-safe).
    // signal_hook::low_level::register requires the closure to be signal-safe.
    #[allow(unsafe_code)]
    unsafe {
        signal_hook::low_level::register(SIGINT, || {
            SHUTDOWN.store(true, Ordering::Relaxed);
        })?;
        signal_hook::low_level::register(SIGTERM, || {
            SHUTDOWN.store(true, Ordering::Relaxed);
        })?;
    }

    Ok(())
}

/// Windows: `SetConsoleCtrlHandler` for `CTRL_C_EVENT` and `CTRL_CLOSE_EVENT`.
#[cfg(windows)]
fn platform_init() -> io::Result<()> {
    use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;

    // SAFETY: SetConsoleCtrlHandler is a standard Win32 API. The handler
    // function has the correct signature and only sets an atomic flag.
    #[allow(unsafe_code)]
    let ok = unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), 1) };

    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Windows console control handler callback.
///
/// Called by the OS on Ctrl+C, console close, logoff, or shutdown events.
/// Sets the global shutdown flag and returns `TRUE` to prevent the default
/// handler (which would terminate the process immediately).
#[cfg(windows)]
#[allow(unsafe_code)]
unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> i32 {
    use windows_sys::Win32::System::Console::{CTRL_CLOSE_EVENT, CTRL_C_EVENT};

    if ctrl_type == CTRL_C_EVENT || ctrl_type == CTRL_CLOSE_EVENT {
        SHUTDOWN.store(true, Ordering::Relaxed);
        1 // TRUE — we handled it
    } else {
        0 // FALSE — let the next handler deal with it
    }
}

#[cfg(test)]
mod tests;
