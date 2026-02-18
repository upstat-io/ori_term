//! Windows font discovery via DirectWrite API and static path fallback.
//!
//! Primary path: DirectWrite (`dwrote` crate) for accurate font matching.
//! Fallback path: static `C:\Windows\Fonts\` paths for common monospace fonts.

use std::path::PathBuf;

use super::families::{DWRITE_FALLBACK_FAMILIES, DWRITE_FAMILY_NAMES, PRIMARY_FAMILIES};
use super::{
    DiscoveryResult, FallbackDiscovery, FontOrigin, resolve_fallback_chain, try_families_from_specs,
};

/// Resolve a single font variant via DirectWrite by family name, weight, and style.
///
/// Uses `first_matching_font` for best-match selection rather than requiring
/// an exact weight match. DirectWrite picks the closest available weight.
///
/// The `collection` parameter is a shared DirectWrite system font collection,
/// created once per discovery invocation to avoid redundant COM calls.
fn resolve_font_dwrite(
    collection: &dwrote::FontCollection,
    family_name: &str,
    weight: dwrote::FontWeight,
    style: dwrote::FontStyle,
) -> Option<PathBuf> {
    let family = collection.font_family_by_name(family_name).ok().flatten()?;
    let font = family
        .first_matching_font(weight, dwrote::FontStretch::Normal, style)
        .ok()?;
    let face = font.create_font_face();
    let files = face.files().ok()?;
    let file = files.first()?;
    file.font_file_path().ok()
}

/// Resolve all four variant paths for a family via DirectWrite.
///
/// Returns `None` if the family doesn't exist (Regular not found). Bold/Italic/
/// Bold-Italic paths are filtered: if DirectWrite returns the same file as Regular
/// (fuzzy fallback), the variant is treated as unavailable.
///
/// `weight` is CSS-style (100–900) for the Regular slot. Bold is derived as
/// `min(weight + 300, 900)` per the CSS "bolder" algorithm.
fn resolve_family_dwrite(
    collection: &dwrote::FontCollection,
    family_name: &str,
    weight: u16,
) -> Option<[Option<PathBuf>; 4]> {
    let regular_weight = dwrote::FontWeight::from_u32(u32::from(weight));
    let bold_weight = dwrote::FontWeight::from_u32(u32::from(weight.saturating_add(300).min(900)));

    let regular = resolve_font_dwrite(
        collection,
        family_name,
        regular_weight,
        dwrote::FontStyle::Normal,
    )?;

    let bold = resolve_font_dwrite(
        collection,
        family_name,
        bold_weight,
        dwrote::FontStyle::Normal,
    )
    .filter(|p| *p != regular);

    let italic = resolve_font_dwrite(
        collection,
        family_name,
        regular_weight,
        dwrote::FontStyle::Italic,
    )
    .filter(|p| *p != regular);

    let bold_italic = resolve_font_dwrite(
        collection,
        family_name,
        bold_weight,
        dwrote::FontStyle::Italic,
    )
    .filter(|p| *p != regular);

    log::info!(
        "font discovery (dwrite): {family_name:?} weight={weight} → \
         regular={}, bold={}, italic={}, bold_italic={}",
        regular.display(),
        bold.as_ref().map_or("none", |p| p.to_str().unwrap_or("?")),
        italic
            .as_ref()
            .map_or("none", |p| p.to_str().unwrap_or("?")),
        bold_italic
            .as_ref()
            .map_or("none", |p| p.to_str().unwrap_or("?")),
    );

    Some([Some(regular), bold, italic, bold_italic])
}

/// Resolve fallback fonts via DirectWrite, then augment with static paths.
fn resolve_fallbacks_dwrite(collection: &dwrote::FontCollection) -> Vec<FallbackDiscovery> {
    let mut fallbacks = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // DirectWrite fallback families.
    for name in DWRITE_FALLBACK_FAMILIES {
        if let Some(path) = resolve_font_dwrite(
            collection,
            name,
            dwrote::FontWeight::Regular,
            dwrote::FontStyle::Normal,
        ) {
            if seen.insert(path.clone()) {
                log::debug!(
                    "font discovery: dwrite fallback {name:?} → {}",
                    path.display()
                );
                fallbacks.push(FallbackDiscovery {
                    path,
                    face_index: 0,
                    origin: FontOrigin::DirectWrite,
                });
            }
        }
    }

    // Static fallback paths (deduplicated against DirectWrite results).
    let lookup = |filename: &str| -> Option<PathBuf> {
        let path = PathBuf::from(filename);
        if path.exists() && !seen.contains(&path) {
            Some(path)
        } else {
            None
        }
    };
    fallbacks.extend(resolve_fallback_chain(&lookup, FontOrigin::DirectoryScan));

    fallbacks
}

/// Try to find a user-specified family via DirectWrite, falling back to
/// static path if DirectWrite doesn't find it.
pub(super) fn try_user_family(name: &str, weight: u16) -> Option<DiscoveryResult> {
    log::debug!("font discovery: creating DirectWrite system font collection (user family)");
    let collection = dwrote::FontCollection::system();

    // Try DirectWrite first.
    if let Some(paths) = resolve_family_dwrite(&collection, name, weight) {
        let primary = super::family_from_paths(name, paths, FontOrigin::UserConfig);
        let fallbacks = resolve_fallbacks_dwrite(&collection);
        return Some(DiscoveryResult { primary, fallbacks });
    }

    // Try as an absolute path or filename in C:\Windows\Fonts\.
    let path = if std::path::Path::new(name).is_absolute() {
        PathBuf::from(name)
    } else {
        PathBuf::from(r"C:\Windows\Fonts").join(name)
    };
    if path.exists() {
        let primary =
            super::family_from_paths(name, [Some(path), None, None, None], FontOrigin::UserConfig);
        let fallbacks = resolve_fallbacks_dwrite(&collection);
        return Some(DiscoveryResult { primary, fallbacks });
    }

    None
}

/// Try platform default families in priority order.
///
/// Tries DirectWrite resolution first (using `DWRITE_FAMILY_NAMES`), then
/// falls back to static path scanning (using `PRIMARY_FAMILIES`).
pub(super) fn try_platform_defaults(weight: u16) -> Option<DiscoveryResult> {
    log::debug!("font discovery: creating DirectWrite system font collection (platform defaults)");
    let collection = dwrote::FontCollection::system();

    // DirectWrite first.
    for name in DWRITE_FAMILY_NAMES {
        if let Some(paths) = resolve_family_dwrite(&collection, name, weight) {
            let primary = super::family_from_paths(name, paths, FontOrigin::DirectWrite);
            let fallbacks = resolve_fallbacks_dwrite(&collection);
            return Some(DiscoveryResult { primary, fallbacks });
        }
    }

    // Static path fallback.
    let lookup = |filename: &str| -> Option<PathBuf> {
        let path = PathBuf::from(filename);
        if path.exists() { Some(path) } else { None }
    };
    let primary = try_families_from_specs(PRIMARY_FAMILIES, &lookup, FontOrigin::DirectoryScan)?;
    let fallbacks = resolve_fallbacks_dwrite(&collection);
    Some(DiscoveryResult { primary, fallbacks })
}

/// Resolve a user-configured fallback font name to a path.
#[allow(dead_code, reason = "font discovery consumed in later sections")]
pub(super) fn resolve_user_fallback(family: &str) -> Option<FallbackDiscovery> {
    log::debug!("font discovery: creating DirectWrite system font collection (user fallback)");
    let collection = dwrote::FontCollection::system();

    // Try DirectWrite.
    if let Some(path) = resolve_font_dwrite(
        &collection,
        family,
        dwrote::FontWeight::Regular,
        dwrote::FontStyle::Normal,
    ) {
        return Some(FallbackDiscovery {
            path,
            face_index: 0,
            origin: FontOrigin::UserConfig,
        });
    }

    // Try as path.
    let path = if std::path::Path::new(family).is_absolute() {
        PathBuf::from(family)
    } else {
        PathBuf::from(r"C:\Windows\Fonts").join(family)
    };
    if path.exists() {
        return Some(FallbackDiscovery {
            path,
            face_index: 0,
            origin: FontOrigin::UserConfig,
        });
    }

    None
}
