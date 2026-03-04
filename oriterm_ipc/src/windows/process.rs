//! Windows process validation and daemon probing.

use std::path::Path;

use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Pipes::WaitNamedPipeW;
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

/// Check whether the daemon named pipe exists and has an available instance.
///
/// Uses `WaitNamedPipeW` instead of actually connecting — this avoids
/// consuming a pipe instance (which would race with the real client
/// connection that follows immediately after).
pub fn probe_daemon(path: &Path) -> bool {
    let wide = super::pipe_name::to_wide_string(path);
    // SAFETY: `WaitNamedPipeW` is a well-documented Win32 API.
    // Timeout of 1ms: effectively a non-blocking existence check.
    unsafe { WaitNamedPipeW(wide.as_ptr(), 1) != 0 }
}

/// Check whether a PID refers to a running process.
///
/// Uses `OpenProcess` with `PROCESS_QUERY_LIMITED_INFORMATION` to check
/// existence without requiring elevated privileges.
pub fn validate_pid(pid: u32) -> bool {
    // SAFETY: `OpenProcess` is a well-documented Win32 API.
    // We request minimal rights and immediately close the handle.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };

    if handle.is_null() {
        return false;
    }

    // SAFETY: we got a valid handle from `OpenProcess`.
    unsafe { CloseHandle(handle) };
    true
}

/// The daemon binary name on this platform.
pub fn daemon_binary_name() -> &'static str {
    "oriterm-mux.exe"
}
