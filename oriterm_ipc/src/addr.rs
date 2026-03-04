//! Platform-appropriate IPC address and PID file path computation.

use std::path::PathBuf;

/// Compute the default IPC address.
///
/// - **Unix**: `$XDG_RUNTIME_DIR/oriterm-mux.sock` or `/tmp/oriterm-$USER/mux.sock`.
/// - **Windows**: `\\.\pipe\oriterm-mux-{USERNAME}`.
pub fn ipc_addr() -> PathBuf {
    platform_ipc_addr()
}

/// Compute the default PID file path.
///
/// - **Unix**: `$XDG_RUNTIME_DIR/oriterm-mux.pid` or `/tmp/oriterm-$USER/mux.pid`.
/// - **Windows**: `%LOCALAPPDATA%\oriterm\mux.pid` or `%TEMP%\oriterm\mux.pid`.
pub fn pid_file_path() -> PathBuf {
    platform_pid_file_path()
}

#[cfg(unix)]
fn platform_ipc_addr() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir).join("oriterm-mux.sock")
    } else {
        let user = std::env::var("USER").unwrap_or_else(|_| String::from("unknown"));
        PathBuf::from(format!("/tmp/oriterm-{user}")).join("mux.sock")
    }
}

#[cfg(windows)]
fn platform_ipc_addr() -> PathBuf {
    super::windows::pipe_name::pipe_name()
}

#[cfg(unix)]
fn platform_pid_file_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir).join("oriterm-mux.pid")
    } else {
        let user = std::env::var("USER").unwrap_or_else(|_| String::from("unknown"));
        PathBuf::from(format!("/tmp/oriterm-{user}")).join("mux.pid")
    }
}

#[cfg(windows)]
fn platform_pid_file_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("TEMP"))
        .unwrap_or_else(|_| String::from(r"C:\Temp"));
    PathBuf::from(base).join("oriterm").join("mux.pid")
}
