//! Platform-specific configuration directory resolution.
//!
//! Follows platform conventions for where applications store config files:
//! - **Windows**: `%APPDATA%\oriterm\` (e.g. `C:\Users\X\AppData\Roaming\oriterm\`)
//! - **Linux**: `$XDG_CONFIG_HOME/oriterm/` (fallback: `~/.config/oriterm/`)
//! - **macOS**: `~/Library/Application Support/oriterm/`

// Config path infrastructure is wired into the config loader in Section 13.
#![expect(dead_code, reason = "platform config paths used in Section 13")]

use std::io;
use std::path::PathBuf;

/// Returns the platform-specific configuration directory for oriterm.
///
/// This is a pure path computation — it does not create the directory.
/// Call [`ensure_config_dir`] to create it if needed.
pub fn config_dir() -> PathBuf {
    platform_config_dir()
}

/// Returns the path to the main configuration file.
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// Ensures the config directory exists, creating it if needed.
///
/// Returns the directory path on success, or an I/O error if creation fails.
pub fn ensure_config_dir() -> io::Result<PathBuf> {
    let dir = platform_config_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Windows: `%APPDATA%\oriterm\`.
#[cfg(windows)]
fn platform_config_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("oriterm");
    }
    // Fallback: current directory (should never happen on a real Windows install).
    log::warn!("APPDATA not set, using current directory for config");
    PathBuf::from(".").join("oriterm")
}

/// Linux: `$XDG_CONFIG_HOME/oriterm/` or `~/.config/oriterm/`.
#[cfg(target_os = "linux")]
fn platform_config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("oriterm");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config").join("oriterm");
    }
    log::warn!("neither XDG_CONFIG_HOME nor HOME set, using current directory for config");
    PathBuf::from(".").join("oriterm")
}

/// macOS: `~/Library/Application Support/oriterm/`.
#[cfg(target_os = "macos")]
fn platform_config_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("oriterm");
    }
    log::warn!("HOME not set, using current directory for config");
    PathBuf::from(".").join("oriterm")
}

#[cfg(test)]
mod tests;
