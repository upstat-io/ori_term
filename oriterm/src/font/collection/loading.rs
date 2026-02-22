//! Font loading: discovery → raw bytes → `FontSet`.
//!
//! Bridges platform font discovery with the `FontCollection` validation pipeline.

use std::sync::Arc;

use super::super::FontError;
use super::super::discovery::{self, FontOrigin};

/// Raw font bytes and collection index (pre-validation).
pub(super) struct FontData {
    /// Font file bytes shared via `Arc` for rustybuzz face creation.
    pub(super) data: Arc<Vec<u8>>,
    /// Face index within a `.ttc` collection (0 for standalone `.ttf`).
    pub(super) index: u32,
}

/// Four style variants plus an ordered fallback chain.
///
/// Constructed by [`FontSet::load`] from discovery results. Passed to
/// [`FontCollection::new`] for validation and metrics computation.
pub struct FontSet {
    /// Human-readable family name.
    pub(super) family_name: String,
    /// Regular face data (always present).
    pub(super) regular: FontData,
    /// Bold face data (if a real bold variant was found).
    pub(super) bold: Option<FontData>,
    /// Italic face data (if a real italic variant was found).
    pub(super) italic: Option<FontData>,
    /// Bold-italic face data (if a real bold-italic variant was found).
    pub(super) bold_italic: Option<FontData>,
    /// Which style slots have real font files.
    #[allow(dead_code, reason = "font fields consumed in later sections")]
    pub(super) has_variant: [bool; 4],
    /// Ordered fallback fonts for missing-glyph coverage.
    pub(super) fallbacks: Vec<FontData>,
}

impl FontSet {
    /// Build a `FontSet` from the embedded `JetBrains` Mono Regular only.
    ///
    /// No system font discovery, no Bold/Italic/BoldItalic variants, no
    /// fallbacks. Produces deterministic output regardless of system fonts —
    /// ideal for visual regression tests.
    #[cfg(test)]
    pub fn embedded() -> Self {
        Self {
            family_name: "JetBrains Mono (embedded)".to_owned(),
            regular: FontData {
                data: Arc::new(discovery::EMBEDDED_FONT_DATA.to_vec()),
                index: 0,
            },
            bold: None,
            italic: None,
            bold_italic: None,
            has_variant: [true, false, false, false],
            fallbacks: Vec::new(),
        }
    }

    /// Load font data from discovery results.
    ///
    /// If `family` is `None`, uses platform defaults (with embedded fallback).
    /// The `weight` parameter is CSS-style (100–900) for the Regular slot.
    pub fn load(family: Option<&str>, weight: u16) -> Result<Self, FontError> {
        let result = discovery::discover_fonts(family, weight);
        Self::from_discovery(&result)
    }

    /// Prepend user-configured fallback fonts before system-discovered fallbacks.
    ///
    /// Each family name is resolved via platform font discovery. Unresolvable
    /// families are logged and skipped. Returns the number of successfully
    /// loaded user fallbacks (for indexing into `FallbackMeta`).
    pub fn prepend_user_fallbacks(&mut self, families: &[&str]) -> usize {
        let mut user_fonts = Vec::new();
        for family in families {
            match discovery::resolve_user_fallback(family) {
                Some(fb) => match std::fs::read(&fb.path) {
                    Ok(bytes) => {
                        log::info!("font: loaded user fallback {family:?}");
                        user_fonts.push(FontData {
                            data: Arc::new(bytes),
                            index: fb.face_index,
                        });
                    }
                    Err(e) => {
                        log::warn!("font: failed to load user fallback {family:?}: {e}");
                    }
                },
                None => {
                    log::warn!("font: user fallback {family:?} not found, skipping");
                }
            }
        }
        let count = user_fonts.len();
        // Prepend user fallbacks: they take priority over system fallbacks.
        user_fonts.append(&mut self.fallbacks);
        self.fallbacks = user_fonts;
        count
    }

    /// Build a `FontSet` from a discovery result.
    pub(crate) fn from_discovery(result: &discovery::DiscoveryResult) -> Result<Self, FontError> {
        let primary = &result.primary;

        let regular = load_font_data(primary, 0)?;

        let bold = try_load_variant(primary, 1, "Bold");
        let italic = try_load_variant(primary, 2, "Italic");
        let bold_italic = try_load_variant(primary, 3, "BoldItalic");

        let fallbacks = result
            .fallbacks
            .iter()
            .filter_map(|fb| {
                let bytes = match std::fs::read(&fb.path) {
                    Ok(b) => b,
                    Err(e) => {
                        log::warn!("font: failed to load fallback {}: {e}", fb.path.display());
                        return None;
                    }
                };
                Some(FontData {
                    data: Arc::new(bytes),
                    index: fb.face_index,
                })
            })
            .collect();

        Ok(Self {
            family_name: primary.family_name.clone(),
            regular,
            bold,
            italic,
            bold_italic,
            has_variant: primary.has_variant,
            fallbacks,
        })
    }
}

/// Try to load a primary variant, logging on failure.
///
/// Returns `None` if the variant has no file or if loading fails (with a warning).
fn try_load_variant(
    primary: &discovery::FamilyDiscovery,
    slot: usize,
    name: &str,
) -> Option<FontData> {
    if !primary.has_variant[slot] {
        return None;
    }
    match load_font_data(primary, slot) {
        Ok(fd) => Some(fd),
        Err(e) => {
            log::warn!("font: failed to load {name} variant: {e}");
            None
        }
    }
}

/// Load font data for a style slot from a discovery result.
fn load_font_data(
    primary: &discovery::FamilyDiscovery,
    slot: usize,
) -> Result<FontData, FontError> {
    let bytes = if let Some(ref path) = primary.paths[slot] {
        std::fs::read(path)?
    } else if primary.origin == FontOrigin::Embedded && slot == 0 {
        discovery::EMBEDDED_FONT_DATA.to_vec()
    } else {
        return Err(FontError::InvalidFont(format!(
            "no font data for slot {slot}"
        )));
    };
    Ok(FontData {
        data: Arc::new(bytes),
        index: primary.face_indices[slot],
    })
}
