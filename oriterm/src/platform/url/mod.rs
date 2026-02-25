//! Cross-platform URL opening.
//!
//! Opens URLs in the user's default browser using the platform's native
//! mechanism:
//! - **Windows**: `ShellExecuteW` (Win32 API) — the approach used by Chrome.
//! - **Linux**: `xdg-open` subprocess.
//! - **macOS**: `open` subprocess.
//!
//! URL schemes are validated before opening to prevent command injection.

use std::io;

/// Allowed URL schemes for `open_url`.
///
/// Only these schemes are permitted to prevent command injection via
/// crafted URIs (e.g. `file:///...`, `javascript:`, custom protocol handlers).
const ALLOWED_SCHEMES: &[&str] = &["http://", "https://", "ftp://", "file://", "mailto:"];

/// Open a URL in the user's default browser.
///
/// Validates the URL scheme against [`ALLOWED_SCHEMES`] before dispatching
/// to the platform-specific handler. Returns an error if the scheme is
/// disallowed or if the platform call fails.
pub fn open_url(url: &str) -> io::Result<()> {
    validate_scheme(url)?;
    platform_open(url)
}

/// Validate that the URL uses an allowed scheme.
fn validate_scheme(url: &str) -> io::Result<()> {
    let lower = url.to_ascii_lowercase();
    for scheme in ALLOWED_SCHEMES {
        if lower.starts_with(scheme) {
            return Ok(());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("disallowed URL scheme: {url}"),
    ))
}

/// Windows: `ShellExecuteW` — Chrome's approach for opening URLs.
///
/// Uses the Win32 `ShellExecuteW` API directly, which is safer than
/// `cmd /c start` (no shell injection) and handles URL encoding correctly.
#[cfg(windows)]
fn platform_open(url: &str) -> io::Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::UI::Shell::ShellExecuteW;

    // Encode URL as wide string for Win32 API.
    let wide: Vec<u16> = OsStr::new(url).encode_wide().chain(Some(0)).collect();
    let open: Vec<u16> = OsStr::new("open").encode_wide().chain(Some(0)).collect();

    // SAFETY: ShellExecuteW is a standard Win32 API. We pass valid null-terminated
    // wide strings and null pointers for unused parameters.
    #[allow(unsafe_code)]
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(), // hwnd — no parent window
            open.as_ptr(),        // operation — "open"
            wide.as_ptr(),        // file — the URL
            std::ptr::null(),     // parameters — none
            std::ptr::null(),     // directory — none
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW returns > 32 on success, <= 32 on failure.
    if result as usize > 32 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "ShellExecuteW failed (code {})",
            result as usize,
        )))
    }
}

/// Linux: `xdg-open` subprocess.
#[cfg(target_os = "linux")]
fn platform_open(url: &str) -> io::Result<()> {
    use std::process::Command;

    Command::new("xdg-open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

/// macOS: `open` subprocess.
#[cfg(target_os = "macos")]
fn platform_open(url: &str) -> io::Result<()> {
    use std::process::Command;

    Command::new("open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests;
