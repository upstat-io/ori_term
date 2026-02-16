//! Cross-platform system theme detection.
//!
//! Detects the operating system's dark/light mode preference:
//! - **Windows**: Reads `AppsUseLightTheme` from the registry.
//! - **Linux**: Queries `org.freedesktop.appearance.color-scheme` via D-Bus,
//!   with fallback to `GTK_THEME` environment variable.
//! - **macOS**: Queries `AppleInterfaceStyle` via `defaults read`.
//!
//! Reference: `WezTerm` appearance detection, Ghostty color scheme sync.

// Theme detection is wired into app startup in a later section.
// In test builds, tests exercise system_theme() and helpers so dead_code
// doesn't fire — making #![expect(dead_code)] produce an unfulfilled-lint
// warning on some platforms.
#![allow(dead_code, reason = "theme detection wired into app startup later")]

use oriterm_core::Theme;

/// Detect the system's color theme preference.
///
/// Returns [`Theme::Dark`] or [`Theme::Light`] based on platform queries.
/// Falls back to [`Theme::Unknown`] if detection fails or is unsupported.
pub fn system_theme() -> Theme {
    platform_theme()
}

/// Windows: read `AppsUseLightTheme` from the registry.
///
/// `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize`
/// — value 0 means dark mode, value 1 means light mode.
#[cfg(windows)]
fn platform_theme() -> Theme {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW,
    };

    let subkey: Vec<u16> = OsStr::new(
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
    )
    .encode_wide()
    .chain(Some(0))
    .collect();

    let value_name: Vec<u16> = OsStr::new("AppsUseLightTheme")
        .encode_wide()
        .chain(Some(0))
        .collect();

    let mut data: u32 = 0;
    let mut size: u32 = size_of::<u32>() as u32;

    // SAFETY: `RegGetValueW` is a standard Win32 API. We pass valid
    // null-terminated wide strings, a correctly sized output buffer,
    // and restrict the result type to `REG_DWORD` via `RRF_RT_REG_DWORD`.
    #[allow(unsafe_code)]
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value_name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut data).cast(),
            &raw mut size,
        )
    };

    // ERROR_SUCCESS = 0.
    if status != 0 {
        return Theme::Unknown;
    }

    match data {
        0 => Theme::Dark,
        1 => Theme::Light,
        _ => Theme::Unknown,
    }
}

/// macOS: query `AppleInterfaceStyle` via `defaults read`.
///
/// Returns "Dark" if dark mode is active; exits with error if light mode
/// (the key is absent in light mode). This is the standard approach used
/// by many CLI tools and terminal emulators.
#[cfg(target_os = "macos")]
fn platform_theme() -> Theme {
    use std::process::Command;

    let output = Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().eq_ignore_ascii_case("dark") {
                Theme::Dark
            } else {
                Theme::Unknown
            }
        }
        // Key absent → light mode (macOS removes the key in light mode).
        Ok(_) => Theme::Light,
        Err(_) => Theme::Unknown,
    }
}

/// Linux: query the XDG Desktop Portal for color scheme preference.
///
/// Primary: `org.freedesktop.portal.Settings` D-Bus interface.
/// Fallback: `GTK_THEME` environment variable (substring match for "dark").
#[cfg(target_os = "linux")]
fn platform_theme() -> Theme {
    dbus_color_scheme().unwrap_or_else(gtk_theme_fallback)
}

/// Query `org.freedesktop.appearance.color-scheme` via `dbus-send`.
///
/// Returns `None` if D-Bus is unavailable or the query fails.
/// Values: 1 = dark, 2 = light, 0 = no preference.
#[cfg(target_os = "linux")]
fn dbus_color_scheme() -> Option<Theme> {
    use std::process::Command;

    let output = Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply=literal",
            "--dest=org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings.Read",
            "string:org.freedesktop.appearance",
            "string:color-scheme",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_dbus_color_scheme(&stdout)
}

/// Parse the `dbus-send --print-reply=literal` output for color-scheme.
///
/// The output format is a nested variant: `variant    variant       uint32 N`
/// where N is the color scheme value.
#[cfg(target_os = "linux")]
fn parse_dbus_color_scheme(output: &str) -> Option<Theme> {
    // Extract the last integer from the output.
    let value: u32 = output
        .split_whitespace()
        .rev()
        .find_map(|token| token.parse().ok())?;

    match value {
        1 => Some(Theme::Dark),
        2 => Some(Theme::Light),
        _ => Some(Theme::Unknown),
    }
}

/// Fallback: check `GTK_THEME` environment variable for "dark" substring.
///
/// Common values: "Adwaita:dark", "Adwaita-dark", "Breeze-Dark".
#[cfg(target_os = "linux")]
fn gtk_theme_fallback() -> Theme {
    let val = std::env::var("GTK_THEME").ok();
    theme_from_gtk_name(val.as_deref())
}

/// Classify a GTK theme name as dark or light.
///
/// Returns [`Theme::Dark`] if the name contains "dark" (case-insensitive),
/// [`Theme::Light`] if it's a known non-dark name, or [`Theme::Unknown`] if
/// the value is absent.
#[cfg(target_os = "linux")]
fn theme_from_gtk_name(name: Option<&str>) -> Theme {
    match name {
        Some(val) if val.to_ascii_lowercase().contains("dark") => Theme::Dark,
        Some(_) => Theme::Light,
        None => Theme::Unknown,
    }
}

/// Fallback for unsupported platforms.
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn platform_theme() -> Theme {
    Theme::Unknown
}

#[cfg(test)]
mod tests;
