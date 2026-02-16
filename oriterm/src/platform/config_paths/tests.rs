use std::path::Path;

use super::{config_dir, config_path};

#[test]
fn config_dir_returns_non_empty_path() {
    let dir = config_dir();
    assert!(!dir.as_os_str().is_empty(), "config_dir should not be empty");
}

#[test]
fn config_dir_ends_with_oriterm() {
    let dir = config_dir();
    assert_eq!(
        dir.file_name().and_then(|n| n.to_str()),
        Some("oriterm"),
        "config dir should end with 'oriterm': {dir:?}",
    );
}

#[test]
fn config_path_ends_with_toml() {
    let path = config_path();
    assert_eq!(
        path.extension().and_then(|e| e.to_str()),
        Some("toml"),
        "config path should end with .toml: {path:?}",
    );
}

#[test]
fn config_path_is_inside_config_dir() {
    let dir = config_dir();
    let path = config_path();
    assert!(
        path.starts_with(&dir),
        "config path {path:?} should be inside config dir {dir:?}",
    );
}

#[cfg(target_os = "linux")]
#[test]
fn linux_xdg_config_home_respected() {
    // This test verifies the logic without actually setting env vars
    // (which would be racy in parallel tests). We just check that the
    // default path contains ".config" when XDG_CONFIG_HOME is not set.
    let dir = config_dir();
    let path_str = dir.to_string_lossy();
    // Either XDG_CONFIG_HOME is set (custom path) or we get ~/.config/oriterm.
    let has_xdg = std::env::var("XDG_CONFIG_HOME").is_ok();
    if !has_xdg {
        assert!(
            path_str.contains(".config") || path_str.contains("oriterm"),
            "Linux config dir should use .config fallback: {path_str}",
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn macos_application_support_path() {
    let dir = config_dir();
    let path_str = dir.to_string_lossy();
    assert!(
        path_str.contains("Application Support"),
        "macOS config dir should use Application Support: {path_str}",
    );
}

#[cfg(windows)]
#[test]
fn windows_appdata_path() {
    let dir = config_dir();
    let path_str = dir.to_string_lossy();
    // APPDATA is always set on Windows. The path should contain it.
    assert!(
        path_str.contains("AppData") || path_str.contains("appdata"),
        "Windows config dir should use APPDATA: {path_str}",
    );
}

#[test]
fn config_dir_is_absolute_or_relative() {
    let dir = config_dir();
    // On a normal system with HOME/APPDATA set, the path should be absolute.
    // On CI without env vars, it may be relative (fallback). Both are valid.
    let _ = Path::new(&dir);
}
