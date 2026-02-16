//! Cross-platform system theme detection.
//!
//! Detects the operating system's dark/light mode preference:
//! - **Windows**: Reads `AppsUseLightTheme` from the registry.
//! - **Linux**: Queries `org.freedesktop.appearance.color-scheme` via D-Bus,
//!   with fallback to DE-specific settings via `$XDG_CURRENT_DESKTOP`, then
//!   `GTK_THEME` environment variable.
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
/// Fallback chain:
/// 1. `org.freedesktop.portal.Settings` D-Bus interface.
/// 2. DE-specific queries via `$XDG_CURRENT_DESKTOP` (gsettings, kdeglobals).
/// 3. `GTK_THEME` environment variable (substring match for "dark").
#[cfg(target_os = "linux")]
fn platform_theme() -> Theme {
    dbus_color_scheme()
        .or_else(de_specific_theme)
        .unwrap_or_else(gtk_theme_fallback)
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
        0 => None,
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

/// Detected desktop environment for DE-specific theme queries.
///
/// Used as a fallback when the XDG Desktop Portal is unavailable.
#[cfg(target_os = "linux")]
enum DesktopEnvironment {
    /// GNOME, Unity, Budgie, Pantheon — uses `org.gnome.desktop.interface`.
    Gnome,
    /// KDE Plasma — reads `~/.config/kdeglobals`.
    Kde,
    /// Linux Mint Cinnamon — uses `org.cinnamon.desktop.interface`.
    Cinnamon,
    /// MATE — uses `org.mate.interface`.
    Mate,
    /// Xfce — uses `xfconf-query`.
    Xfce,
}

/// Detect the desktop environment from `XDG_CURRENT_DESKTOP` or
/// `XDG_SESSION_DESKTOP`.
///
/// `XDG_CURRENT_DESKTOP` is a colon-separated list (e.g., `"ubuntu:GNOME"`).
/// Reference: Ghostty `src/os/desktop.zig`.
#[cfg(target_os = "linux")]
fn detect_desktop() -> Option<DesktopEnvironment> {
    if let Ok(val) = std::env::var("XDG_CURRENT_DESKTOP") {
        for segment in val.split(':') {
            if let Some(de) = classify_desktop(segment.trim()) {
                return Some(de);
            }
        }
    }
    std::env::var("XDG_SESSION_DESKTOP")
        .ok()
        .and_then(|val| classify_desktop(val.trim()))
}

/// Map a desktop environment name to a [`DesktopEnvironment`] variant.
///
/// Case-insensitive matching. Returns `None` for unrecognized names.
#[cfg(target_os = "linux")]
fn classify_desktop(name: &str) -> Option<DesktopEnvironment> {
    match name.to_ascii_lowercase().as_str() {
        "gnome" | "gnome-xorg" | "unity" | "budgie" | "pantheon" => {
            Some(DesktopEnvironment::Gnome)
        }
        "kde" | "kde-plasma" => Some(DesktopEnvironment::Kde),
        "x-cinnamon" | "cinnamon" => Some(DesktopEnvironment::Cinnamon),
        "mate" => Some(DesktopEnvironment::Mate),
        "xfce" => Some(DesktopEnvironment::Xfce),
        _ => None,
    }
}

/// Query DE-specific settings for theme preference.
///
/// Dispatches to per-DE detection logic based on `$XDG_CURRENT_DESKTOP`.
#[cfg(target_os = "linux")]
fn de_specific_theme() -> Option<Theme> {
    let de = detect_desktop()?;
    match de {
        DesktopEnvironment::Gnome => gnome_theme(),
        DesktopEnvironment::Kde => kde_theme(),
        DesktopEnvironment::Cinnamon => {
            gsettings_gtk_theme("org.cinnamon.desktop.interface", "gtk-theme")
        }
        DesktopEnvironment::Mate => {
            gsettings_gtk_theme("org.mate.interface", "gtk-theme")
        }
        DesktopEnvironment::Xfce => xfce_theme(),
    }
}

/// GNOME: query `color-scheme` first, then `gtk-theme` as fallback.
///
/// `color-scheme` was added in GNOME 42. Older GNOME uses the GTK theme name.
#[cfg(target_os = "linux")]
fn gnome_theme() -> Option<Theme> {
    if let Some(theme) = gsettings_color_scheme() {
        return Some(theme);
    }
    gsettings_gtk_theme("org.gnome.desktop.interface", "gtk-theme")
}

/// Query `org.gnome.desktop.interface color-scheme` via `gsettings`.
#[cfg(target_os = "linux")]
fn gsettings_color_scheme() -> Option<Theme> {
    use std::process::Command;

    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_gsettings_color_scheme(&stdout)
}

/// Parse `gsettings` color-scheme output.
///
/// Values: `'prefer-dark'`, `'prefer-light'`, `'default'` (no preference).
/// `'default'` returns `None` to defer to the next fallback.
#[cfg(target_os = "linux")]
fn parse_gsettings_color_scheme(output: &str) -> Option<Theme> {
    let val = output.trim().trim_matches('\'');
    match val {
        "prefer-dark" => Some(Theme::Dark),
        "prefer-light" => Some(Theme::Light),
        _ => None,
    }
}

/// Query a GTK theme name from a `gsettings` schema and classify it.
#[cfg(target_os = "linux")]
fn gsettings_gtk_theme(schema: &str, key: &str) -> Option<Theme> {
    use std::process::Command;

    let output = Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let name = stdout.trim().trim_matches('\'');
    if name.is_empty() {
        return None;
    }
    Some(theme_from_gtk_name(Some(name)))
}

/// KDE Plasma: read `~/.config/kdeglobals` for color scheme name.
#[cfg(target_os = "linux")]
fn kde_theme() -> Option<Theme> {
    let home = std::env::var("HOME").ok()?;
    let path = std::path::Path::new(&home).join(".config/kdeglobals");
    let content = std::fs::read_to_string(path).ok()?;
    parse_kdeglobals(&content)
}

/// Parse KDE's `kdeglobals` INI file for the `ColorScheme` value.
///
/// Looks for `ColorScheme=<name>` in the `[General]` section.
#[cfg(target_os = "linux")]
fn parse_kdeglobals(content: &str) -> Option<Theme> {
    let mut in_general = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_general = trimmed.eq_ignore_ascii_case("[general]");
            continue;
        }
        if in_general {
            if let Some(val) = trimmed.strip_prefix("ColorScheme=") {
                return if val.to_ascii_lowercase().contains("dark") {
                    Some(Theme::Dark)
                } else {
                    Some(Theme::Light)
                };
            }
        }
    }
    None
}

/// Xfce: query theme name via `xfconf-query`.
#[cfg(target_os = "linux")]
fn xfce_theme() -> Option<Theme> {
    use std::process::Command;

    let output = Command::new("xfconf-query")
        .args(["-c", "xsettings", "-p", "/Net/ThemeName"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let name = stdout.trim();
    if name.is_empty() {
        return None;
    }
    Some(theme_from_gtk_name(Some(name)))
}

/// Fallback for unsupported platforms.
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn platform_theme() -> Theme {
    Theme::Unknown
}

#[cfg(test)]
mod tests;
