//! Linux font discovery via recursive directory scanning.
//!
//! Builds a filename → path index by scanning standard font directories once,
//! then resolves families and fallbacks by filename lookup.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::families::PRIMARY_FAMILIES;
use super::{
    DiscoveryResult, FallbackDiscovery, FontOrigin, resolve_fallback_chain, try_families_from_specs,
};

/// Standard font directories on Linux, in priority order.
///
/// User fonts take precedence over system fonts so personal installations
/// override distribution-provided versions.
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::with_capacity(3);
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/fonts"));
    }
    dirs.push(PathBuf::from("/usr/share/fonts"));
    dirs.push(PathBuf::from("/usr/local/share/fonts"));
    dirs
}

/// Build a filename → full path index by scanning all font directories once.
///
/// First-seen wins: if the same filename exists in multiple directories, the
/// one from the highest-priority directory (user before system) is kept.
pub(crate) fn build_font_index() -> HashMap<String, PathBuf> {
    let mut index = HashMap::new();
    for dir in font_dirs() {
        index_font_dir(&dir, &mut index);
    }
    index
}

/// Recursively index a font directory, mapping filenames to full paths.
fn index_font_dir(dir: &Path, index: &mut HashMap<String, PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            index_font_dir(&path, index);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // First-seen wins — don't overwrite user fonts with system fonts.
            index.entry(name.to_owned()).or_insert(path);
        } else {
            // Non-UTF-8 filename — skip.
        }
    }
}

/// Try to find a user-specified family by scanning for filenames matching
/// common naming conventions.
pub(super) fn try_user_family(name: &str, _weight: u16) -> Option<DiscoveryResult> {
    let index = build_font_index();
    let lookup = |filename: &str| -> Option<PathBuf> { index.get(filename).cloned() };

    // Try the name as a filename directly.
    if let Some(path) = index.get(name) {
        let primary = super::family_from_paths(
            name,
            [Some(path.clone()), None, None, None],
            FontOrigin::UserConfig,
        );
        let fallbacks = resolve_fallback_chain(&lookup, FontOrigin::DirectoryScan);
        return Some(DiscoveryResult { primary, fallbacks });
    }

    // Try common naming patterns: "FontName-Regular.ttf", "FontName-Regular.otf".
    for ext in &["ttf", "otf"] {
        let candidate = format!("{name}-Regular.{ext}");
        if let Some(path) = index.get(&candidate) {
            let bold = index.get(&format!("{name}-Bold.{ext}")).cloned();
            let italic = index.get(&format!("{name}-Italic.{ext}")).cloned();
            let bold_italic = index.get(&format!("{name}-BoldItalic.{ext}")).cloned();

            let primary = super::family_from_paths(
                name,
                [Some(path.clone()), bold, italic, bold_italic],
                FontOrigin::UserConfig,
            );
            let fallbacks = resolve_fallback_chain(&lookup, FontOrigin::DirectoryScan);
            return Some(DiscoveryResult { primary, fallbacks });
        }
    }

    // Try as absolute path.
    let path = PathBuf::from(name);
    if path.is_absolute() && path.exists() {
        let primary =
            super::family_from_paths(name, [Some(path), None, None, None], FontOrigin::UserConfig);
        let fallbacks = resolve_fallback_chain(&lookup, FontOrigin::DirectoryScan);
        return Some(DiscoveryResult { primary, fallbacks });
    }

    None
}

/// Try platform default families in priority order.
pub(super) fn try_platform_defaults(_weight: u16) -> Option<DiscoveryResult> {
    let index = build_font_index();
    let lookup = |filename: &str| -> Option<PathBuf> { index.get(filename).cloned() };

    let primary = try_families_from_specs(PRIMARY_FAMILIES, &lookup, FontOrigin::DirectoryScan)?;
    let fallbacks = resolve_fallback_chain(&lookup, FontOrigin::DirectoryScan);
    Some(DiscoveryResult { primary, fallbacks })
}

/// Resolve a user-configured fallback font name to a path.
pub(super) fn resolve_user_fallback(family: &str) -> Option<FallbackDiscovery> {
    let index = build_font_index();

    // Try as filename in index.
    if let Some(path) = index.get(family) {
        return Some(FallbackDiscovery {
            path: path.clone(),
            face_index: 0,
            origin: FontOrigin::UserConfig,
        });
    }

    // Try as absolute path.
    let path = PathBuf::from(family);
    if path.is_absolute() && path.exists() {
        return Some(FallbackDiscovery {
            path,
            face_index: 0,
            origin: FontOrigin::UserConfig,
        });
    }

    None
}
