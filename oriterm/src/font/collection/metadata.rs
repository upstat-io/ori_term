//! Fallback metadata, cap-height normalization, and OpenType feature helpers.
//!
//! Extracted from `collection/mod.rs` to keep the main module under the
//! 500-line limit. All items are internal to the collection module.

use super::super::FaceIdx;
use super::face::{AxisInfo, clamp_to_axis, has_axis};
use crate::font::SyntheticFlags;

/// Weight axis tag.
const WGHT: &[u8; 4] = b"wght";
/// Slant axis tag.
const SLNT: &[u8; 4] = b"slnt";
/// Italic axis tag.
const ITAL: &[u8; 4] = b"ital";

/// Minimum font size in pixels (prevents degenerate scaling).
pub(super) const MIN_FONT_SIZE: f32 = 2.0;

/// Maximum font size in pixels (prevents absurd scaling).
pub(super) const MAX_FONT_SIZE: f32 = 200.0;

/// Per-fallback metadata for cap-height normalization and feature overrides.
///
/// Each entry in `fallback_meta` corresponds 1:1 to the matching entry in
/// `fallbacks`. System-discovered fallbacks get auto-computed `scale_factor`
/// with default features; user-configured fallbacks can override features
/// and add a `size_offset`.
pub(super) struct FallbackMeta {
    /// Cap-height normalization: `primary_cap_height / fallback_cap_height`.
    ///
    /// Ensures glyphs from different fonts appear at visually consistent sizes.
    /// A value of 1.0 means the fallback already matches the primary.
    pub scale_factor: f32,
    /// User-configured size adjustment in points (0.0 if unset).
    pub size_offset: f32,
    /// Per-fallback OpenType feature overrides.
    ///
    /// When `Some`, these features replace collection-wide defaults for this
    /// fallback. When `None`, collection defaults apply.
    pub features: Option<Vec<rustybuzz::Feature>>,
}

/// Default OpenType features: standard ligatures + contextual alternates.
///
/// These are the features most users expect from a terminal font.
pub(super) fn default_features() -> Vec<rustybuzz::Feature> {
    parse_features(&["liga", "calt"])
}

/// Parse feature tag strings into rustybuzz features.
///
/// Each string follows rustybuzz's `Feature::from_str` format:
/// - `"liga"` — enable standard ligatures
/// - `"-liga"` — disable standard ligatures
/// - `"+dlig"` — enable discretionary ligatures
/// - `"kern=0"` — disable kerning
///
/// Invalid tags are logged and skipped.
pub(super) fn parse_features(tags: &[&str]) -> Vec<rustybuzz::Feature> {
    tags.iter()
        .filter_map(|tag| match tag.parse::<rustybuzz::Feature>() {
            Ok(f) => Some(f),
            Err(e) => {
                log::warn!("font: invalid OpenType feature '{tag}': {e}");
                None
            }
        })
        .collect()
}

/// Compute effective font size for a face index with cap-height normalization.
///
/// Primary faces return `base_size` unchanged. Fallback faces are scaled by
/// their cap-height ratio: `base_size * scale_factor + size_offset`, clamped
/// to `[MIN_FONT_SIZE, MAX_FONT_SIZE]`.
pub(super) fn effective_size_for(
    face_idx: FaceIdx,
    base_size: f32,
    fallback_meta: &[FallbackMeta],
) -> f32 {
    if let Some(fb_i) = face_idx.fallback_index() {
        if let Some(meta) = fallback_meta.get(fb_i) {
            return (base_size * meta.scale_factor + meta.size_offset)
                .clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        }
    }
    base_size
}

/// Inline storage for ≤2 variation axis settings (wght + slnt/ital).
///
/// Avoids heap allocation for the common case. Variable fonts set at most
/// one weight axis and one slant/italic axis per face.
pub(super) struct VarSettings {
    entries: [(&'static str, f32); 2],
    len: u8,
}

impl VarSettings {
    /// Empty settings (no variation axes).
    const fn new() -> Self {
        Self {
            entries: [("", 0.0); 2],
            len: 0,
        }
    }

    /// Add a variation setting. Max 2 entries (wght + slnt/ital).
    fn push(&mut self, tag: &'static str, value: f32) {
        debug_assert!(
            (self.len as usize) < 2,
            "VarSettings overflow: max 2 entries"
        );
        self.entries[self.len as usize] = (tag, value);
        self.len += 1;
    }
}

impl std::ops::Deref for VarSettings {
    type Target = [(&'static str, f32)];

    fn deref(&self) -> &Self::Target {
        &self.entries[..self.len as usize]
    }
}

/// Computed variation settings and synthetic flag suppression.
///
/// When a font has variable axes that cover a requested style (e.g. `wght`
/// for bold, `slnt`/`ital` for italic), the axis is set and the corresponding
/// synthetic flag is suppressed — the real axis replaces outline manipulation.
pub(super) struct FaceVariationResult {
    /// Variation settings to pass to swash (rasterization) and rustybuzz (shaping).
    pub settings: VarSettings,
    /// Synthetic flags to suppress because real axes handle them.
    pub suppress_synthetic: SyntheticFlags,
}

/// Compute variation axis settings and synthetic suppression for a face.
///
/// Primary faces get weight/slant/italic axes set based on their style slot
/// (Regular=0, Bold=1, Italic=2, `BoldItalic`=3) and any synthetic flags from
/// resolution. Fallback faces return empty settings (no variation).
///
/// When the font has a real axis that covers the requested style, the
/// corresponding synthetic flag is added to `suppress_synthetic` so callers
/// can subtract it from the rasterization key's synthetic flags.
pub(super) fn face_variations(
    face_idx: FaceIdx,
    synthetic: SyntheticFlags,
    weight: u16,
    axes: &[AxisInfo],
) -> FaceVariationResult {
    if face_idx.is_fallback() || axes.is_empty() {
        return FaceVariationResult {
            settings: VarSettings::new(),
            suppress_synthetic: SyntheticFlags::NONE,
        };
    }

    let mut settings = VarSettings::new();
    let mut suppress = SyntheticFlags::NONE;
    let i = face_idx.as_usize();

    // Weight axis: Bold/BoldItalic slots (1, 3) or synthetic BOLD.
    let wants_bold = i == 1 || i == 3 || synthetic.contains(SyntheticFlags::BOLD);
    if has_axis(axes, *WGHT) {
        let target = if wants_bold {
            (weight as f32 + 300.0).min(900.0)
        } else {
            weight as f32
        };
        settings.push("wght", clamp_to_axis(axes, *WGHT, target));
        if synthetic.contains(SyntheticFlags::BOLD) {
            suppress |= SyntheticFlags::BOLD;
        }
    }

    // Slant/Italic axes: Italic/BoldItalic slots (2, 3) or synthetic ITALIC.
    let wants_italic = i == 2 || i == 3 || synthetic.contains(SyntheticFlags::ITALIC);
    if wants_italic {
        if has_axis(axes, *SLNT) {
            settings.push("slnt", clamp_to_axis(axes, *SLNT, -12.0));
            if synthetic.contains(SyntheticFlags::ITALIC) {
                suppress |= SyntheticFlags::ITALIC;
            }
        } else if has_axis(axes, *ITAL) {
            settings.push("ital", clamp_to_axis(axes, *ITAL, 1.0));
            if synthetic.contains(SyntheticFlags::ITALIC) {
                suppress |= SyntheticFlags::ITALIC;
            }
        } else {
            // No slant or italic axis — synthesis remains active.
        }
    }

    FaceVariationResult {
        settings,
        suppress_synthetic: suppress,
    }
}
