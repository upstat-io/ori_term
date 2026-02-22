//! Font discovery — finding font files on disk across platforms.
//!
//! Pure discovery: resolves family names and style variants to file paths.
//! No font loading, rasterizing, or caching happens here.
//!
//! # Strategy
//!
//! 1. If the user specified a family name in config, try that first.
//! 2. Try platform-specific discovery (DirectWrite on Windows, directory
//!    scanning on Linux/macOS) against the default priority list.
//! 3. Fall back to the embedded `JetBrains` Mono regular as a last resort.

mod families;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::collections::HashMap;
use std::path::PathBuf;

use families::{FALLBACK_FONTS, FamilySpec, UI_FONT_FAMILIES};

/// Embedded `JetBrains` Mono regular — guaranteed fallback that ships with the binary.
pub(crate) const EMBEDDED_FONT_DATA: &[u8] =
    include_bytes!("../../../fonts/JetBrainsMono-Regular.ttf");

/// Where the font was discovered (for logging and diagnostics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontOrigin {
    /// Windows DirectWrite API.
    #[cfg(target_os = "windows")]
    DirectWrite,
    /// Filesystem directory scan.
    DirectoryScan,
    /// User-specified path or family in config.
    UserConfig,
    /// Built into the binary (`include_bytes!`).
    Embedded,
}

/// Discovery result for a font family — all four style slots.
///
/// Slots: Regular (0), Bold (1), Italic (2), Bold-Italic (3).
#[derive(Debug)]
pub struct FamilyDiscovery {
    /// Human-readable family name.
    pub family_name: String,
    /// File paths for each style slot (`None` = needs synthesis).
    pub paths: [Option<PathBuf>; 4],
    /// Whether each slot has a real file (vs. needing synthesis from Regular).
    pub has_variant: [bool; 4],
    /// Face index within a `.ttc` collection file (0 for standalone `.ttf`).
    pub face_indices: [u32; 4],
    /// How this family was found.
    pub origin: FontOrigin,
}

/// A single fallback font for missing-glyph coverage.
#[derive(Debug)]
pub struct FallbackDiscovery {
    /// Path to the font file.
    pub path: PathBuf,
    /// Face index within a `.ttc` collection file.
    pub face_index: u32,
    /// How this fallback was found.
    #[allow(dead_code, reason = "font discovery consumed in later sections")]
    pub origin: FontOrigin,
}

/// Complete discovery result: primary family plus ordered fallback chain.
#[derive(Debug)]
pub struct DiscoveryResult {
    /// The primary font family (Regular + optional Bold/Italic/BoldItalic).
    pub primary: FamilyDiscovery,
    /// Ordered fallback fonts for missing-glyph coverage.
    pub fallbacks: Vec<FallbackDiscovery>,
}

/// Discover fonts for the terminal, trying multiple sources in priority order.
///
/// 1. If `family_override` is provided, try to find that family first.
/// 2. Try platform defaults in priority order.
/// 3. Fall back to the embedded `JetBrains` Mono regular.
///
/// The `weight` parameter (CSS-style, 100–900) controls the Regular weight;
/// Bold is derived as `min(weight + 300, 900)`.
///
/// This function always succeeds — the embedded fallback guarantees a result.
pub fn discover_fonts(family_override: Option<&str>, weight: u16) -> DiscoveryResult {
    // Build the font index once for directory-scanning platforms.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let font_index = {
        #[cfg(target_os = "linux")]
        {
            linux::build_font_index()
        }
        #[cfg(target_os = "macos")]
        {
            macos::build_font_index()
        }
    };

    // Try user-specified family first.
    if let Some(name) = family_override {
        let result = {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            {
                try_user_family_with_index(name, weight, &font_index)
            }
            #[cfg(target_os = "windows")]
            {
                try_user_family(name, weight)
            }
        };
        if let Some(result) = result {
            log::info!("font discovery: using user-specified family {name:?}");
            return result;
        }
        log::warn!("font discovery: user-specified family {name:?} not found, trying defaults");
    }

    // Try platform defaults.
    let result = {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            try_platform_defaults_with_index(weight, &font_index)
        }
        #[cfg(target_os = "windows")]
        {
            try_platform_defaults(weight)
        }
    };
    if let Some(result) = result {
        return result;
    }

    // Embedded fallback — always available.
    log::info!("font discovery: using embedded `JetBrains` Mono");
    DiscoveryResult {
        primary: embedded_family(),
        fallbacks: Vec::new(),
    }
}

/// Discover UI fonts (proportional sans-serif) for tab bar, labels, and overlays.
///
/// Tries platform-specific UI font families in priority order. If no UI font
/// is found, falls back to the terminal font discovery result (the terminal's
/// monospace font is better than nothing).
///
/// This function always succeeds — the embedded fallback guarantees a result.
#[allow(dead_code, reason = "wired when GpuRenderer loads UI fonts")]
pub fn discover_ui_fonts() -> DiscoveryResult {
    // Build the font index once for directory-scanning platforms.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let font_index = {
        #[cfg(target_os = "linux")]
        {
            linux::build_font_index()
        }
        #[cfg(target_os = "macos")]
        {
            macos::build_font_index()
        }
    };

    let result = {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            try_ui_fonts_with_index(&font_index)
        }
        #[cfg(target_os = "windows")]
        {
            try_ui_fonts()
        }
    };
    if let Some(result) = result {
        return result;
    }

    // Fall back to terminal font discovery.
    log::info!("UI font discovery: no UI font found, falling back to terminal font");
    discover_fonts(None, 400)
}

/// Try UI font families using a pre-built font index (Linux/macOS).
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn try_ui_fonts_with_index(index: &HashMap<String, PathBuf>) -> Option<DiscoveryResult> {
    let lookup = |name: &str| index.get(name).cloned();
    let primary = try_families_from_specs(UI_FONT_FAMILIES, &lookup, FontOrigin::DirectoryScan)?;
    let fallbacks = Vec::new(); // UI text doesn't need glyph fallbacks.
    log::info!(
        "UI font discovery: found {:?} (origin={:?})",
        primary.family_name,
        primary.origin,
    );
    Some(DiscoveryResult { primary, fallbacks })
}

/// Try UI font families via DirectWrite (Windows).
#[cfg(target_os = "windows")]
fn try_ui_fonts() -> Option<DiscoveryResult> {
    let lookup = |name: &str| {
        let path = PathBuf::from(name);
        path.exists().then_some(path)
    };
    let primary = try_families_from_specs(UI_FONT_FAMILIES, &lookup, FontOrigin::DirectoryScan)?;
    let fallbacks = Vec::new();
    log::info!(
        "UI font discovery: found {:?} (origin={:?})",
        primary.family_name,
        primary.origin,
    );
    Some(DiscoveryResult { primary, fallbacks })
}

/// Resolve a user-configured fallback font name to a path.
///
/// Accepts either a family name (resolved via platform APIs or directory scan)
/// or an absolute file path. Returns `None` if the font cannot be found.
///
/// On Linux/macOS, builds a font index once and passes it to the platform
/// resolver. Windows uses DirectWrite and does not need an index.
pub fn resolve_user_fallback(family: &str) -> Option<FallbackDiscovery> {
    #[cfg(target_os = "windows")]
    {
        windows::resolve_user_fallback(family)
    }
    #[cfg(target_os = "linux")]
    {
        let index = linux::build_font_index();
        linux::resolve_user_fallback(family, &index)
    }
    #[cfg(target_os = "macos")]
    {
        let index = macos::build_font_index();
        macos::resolve_user_fallback(family, &index)
    }
}

/// Build a `FamilyDiscovery` for the embedded `JetBrains` Mono regular.
///
/// All paths are `None` because the font data is compiled into the binary.
/// Only the Regular slot is marked as available; Bold/Italic/Bold-Italic
/// must be synthesized by the renderer.
fn embedded_family() -> FamilyDiscovery {
    FamilyDiscovery {
        family_name: "`JetBrains` Mono (embedded)".to_owned(),
        paths: [None, None, None, None],
        has_variant: [true, false, false, false],
        face_indices: [0; 4],
        origin: FontOrigin::Embedded,
    }
}

/// Try to find a user-specified family name via platform discovery (Windows).
#[cfg(target_os = "windows")]
fn try_user_family(name: &str, weight: u16) -> Option<DiscoveryResult> {
    windows::try_user_family(name, weight)
}

/// Try to find a user-specified family name using a pre-built font index (Linux/macOS).
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn try_user_family_with_index(
    name: &str,
    weight: u16,
    index: &HashMap<String, PathBuf>,
) -> Option<DiscoveryResult> {
    #[cfg(target_os = "linux")]
    {
        linux::try_user_family(name, weight, index)
    }
    #[cfg(target_os = "macos")]
    {
        macos::try_user_family(name, weight, index)
    }
}

/// Try platform default families in priority order (Windows).
#[cfg(target_os = "windows")]
fn try_platform_defaults(weight: u16) -> Option<DiscoveryResult> {
    windows::try_platform_defaults(weight)
}

/// Try platform default families using a pre-built font index (Linux/macOS).
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn try_platform_defaults_with_index(
    weight: u16,
    index: &HashMap<String, PathBuf>,
) -> Option<DiscoveryResult> {
    #[cfg(target_os = "linux")]
    {
        linux::try_platform_defaults(weight, index)
    }
    #[cfg(target_os = "macos")]
    {
        macos::try_platform_defaults(weight, index)
    }
}

// Shared helpers used by platform modules.

/// Build a `FamilyDiscovery` from resolved paths, marking variants as available
/// only when a distinct file exists for that slot.
fn family_from_paths(
    name: &str,
    paths: [Option<PathBuf>; 4],
    origin: FontOrigin,
) -> FamilyDiscovery {
    let has_variant = [
        paths[0].is_some(),
        paths[1].is_some(),
        paths[2].is_some(),
        paths[3].is_some(),
    ];
    FamilyDiscovery {
        family_name: name.to_owned(),
        paths,
        has_variant,
        face_indices: [0; 4],
        origin,
    }
}

/// Resolve fallback fonts from the platform's fallback list using a lookup function.
fn resolve_fallback_chain(
    lookup: &dyn Fn(&str) -> Option<PathBuf>,
    origin: FontOrigin,
) -> Vec<FallbackDiscovery> {
    let mut fallbacks = Vec::new();
    for spec in FALLBACK_FONTS {
        for filename in spec.filenames {
            if let Some(path) = lookup(filename) {
                log::debug!(
                    "font discovery: fallback {:?} → {}",
                    spec.name,
                    path.display()
                );
                fallbacks.push(FallbackDiscovery {
                    path,
                    face_index: 0,
                    origin,
                });
                break;
            }
        }
    }
    fallbacks
}

/// Try to find a family from the priority list using a lookup function.
///
/// Returns the first family where at least the Regular variant is found.
fn try_families_from_specs(
    specs: &[FamilySpec],
    lookup: &dyn Fn(&str) -> Option<PathBuf>,
    origin: FontOrigin,
) -> Option<FamilyDiscovery> {
    for spec in specs {
        let regular = find_first_match(spec.regular, lookup);
        if regular.is_none() {
            continue;
        }

        let bold = find_first_match(spec.bold, lookup);
        let italic = find_first_match(spec.italic, lookup);
        let bold_italic = find_first_match(spec.bold_italic, lookup);

        log::info!(
            "font discovery: found {:?} (origin={:?}, bold={}, italic={}, bold_italic={})",
            spec.name,
            origin,
            bold.is_some(),
            italic.is_some(),
            bold_italic.is_some(),
        );

        return Some(family_from_paths(
            spec.name,
            [regular, bold, italic, bold_italic],
            origin,
        ));
    }
    None
}

/// Try candidate filenames in order, returning the first match.
fn find_first_match(
    candidates: &[&str],
    lookup: &dyn Fn(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    candidates.iter().find_map(|name| lookup(name))
}

#[cfg(test)]
mod tests;
