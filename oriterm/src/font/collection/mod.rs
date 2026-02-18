//! Font collection: face data, rasterization, metrics, glyph cache.
//!
//! [`FontSet`] loads raw font bytes from discovery results. [`FontCollection`]
//! validates faces, computes cell metrics, resolves characters to glyph IDs,
//! and rasterizes glyphs into bitmaps ready for GPU atlas upload.

mod face;
mod loading;

use std::collections::HashMap;
use std::sync::Arc;

use swash::scale::ScaleContext;

use super::{CellMetrics, FaceIdx, FontError, GlyphFormat, GlyphStyle, RasterKey, ResolvedGlyph, SyntheticFlags};
use face::{build_face, cap_height_px, compute_metrics, glyph_id, rasterize_from_face, FaceData};
pub use loading::FontSet;
#[cfg(test)]
use loading::FontData;

pub use face::size_key;

/// Minimum font size in pixels (prevents degenerate scaling).
const MIN_FONT_SIZE: f32 = 2.0;

/// Maximum font size in pixels (prevents absurd scaling).
const MAX_FONT_SIZE: f32 = 200.0;

/// Per-fallback metadata for cap-height normalization and feature overrides.
///
/// Each entry in `fallback_meta` corresponds 1:1 to the matching entry in
/// `fallbacks`. System-discovered fallbacks get auto-computed `scale_factor`
/// with default features; user-configured fallbacks can override features
/// and add a `size_offset`.
struct FallbackMeta {
    /// Cap-height normalization: `primary_cap_height / fallback_cap_height`.
    ///
    /// Ensures glyphs from different fonts appear at visually consistent sizes.
    /// A value of 1.0 means the fallback already matches the primary.
    scale_factor: f32,
    /// User-configured size adjustment in points (0.0 if unset).
    size_offset: f32,
    /// Per-fallback OpenType feature overrides.
    ///
    /// When `Some`, these features replace collection-wide defaults for this
    /// fallback. When `None`, collection defaults apply.
    features: Option<Vec<rustybuzz::Feature>>,
}

/// A rasterized glyph bitmap ready for atlas upload.
#[derive(Debug, Clone)]
pub struct RasterizedGlyph {
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
    /// Horizontal bearing (pixels from origin to left edge of bitmap).
    pub bearing_x: i32,
    /// Vertical bearing (pixels from baseline to top edge; positive = above).
    pub bearing_y: i32,
    /// Horizontal advance width in pixels.
    #[allow(dead_code, reason = "font fields consumed in later sections")]
    pub advance: f32,
    /// Pixel format of the bitmap data.
    pub format: GlyphFormat,
    /// Raw bitmap bytes. Layout depends on format:
    /// - Alpha: `width * height` bytes (1 byte/pixel).
    /// - SubpixelRgb/Bgr: `width * height * 4` bytes (RGBA per-channel).
    /// - Color: `width * height * 4` bytes (RGBA premultiplied).
    pub bitmap: Vec<u8>,
}

/// Font collection: validated faces, cell metrics, glyph cache, rasterization.
///
/// Owns all font face data and provides the bridge between font discovery
/// and the GPU renderer. Resolves characters to glyph IDs, rasterizes
/// bitmaps, and pre-caches ASCII glyphs.
pub struct FontCollection {
    // Faces
    primary: [Option<FaceData>; 4],
    fallbacks: Vec<FaceData>,
    fallback_meta: Vec<FallbackMeta>,
    // Metrics
    size_px: f32,
    cell_width: f32,
    cell_height: f32,
    baseline: f32,
    #[allow(dead_code, reason = "used for diagnostics and future dynamic fallback loading")]
    cap_height_px: f32,
    // Rasterization
    format: GlyphFormat,
    glyph_cache: HashMap<RasterKey, RasterizedGlyph>,
    scale_context: ScaleContext,
    // Config
    weight: u16,
    family_name: String,
    /// Collection-wide OpenType features applied to all primary faces.
    ///
    /// Default: `["liga", "calt"]` (standard ligatures + contextual alternates).
    features: Vec<rustybuzz::Feature>,
}

impl FontCollection {
    /// Build a font collection from a loaded font set.
    ///
    /// Validates all faces, computes cell metrics from the Regular font,
    /// and pre-caches all printable ASCII glyphs (0x20–0x7E).
    pub fn new(
        font_set: FontSet,
        size_pt: f32,
        dpi: f32,
        format: GlyphFormat,
        weight: u16,
    ) -> Result<Self, FontError> {
        let size_px = size_pt * dpi / 72.0;

        // Validate Regular (required).
        let regular_face = build_face(Arc::clone(&font_set.regular.data), font_set.regular.index)
            .ok_or_else(|| FontError::InvalidFont("Regular font is invalid".into()))?;

        // Compute metrics from Regular.
        let (cell_width, cell_height, baseline) =
            compute_metrics(&font_set.regular.data, font_set.regular.index, size_px);
        let primary_cap =
            cap_height_px(&font_set.regular.data, font_set.regular.index, size_px);

        // Validate optional primary variants.
        let bold = font_set
            .bold
            .as_ref()
            .and_then(|fd| build_face(Arc::clone(&fd.data), fd.index));
        let italic = font_set
            .italic
            .as_ref()
            .and_then(|fd| build_face(Arc::clone(&fd.data), fd.index));
        let bold_italic = font_set
            .bold_italic
            .as_ref()
            .and_then(|fd| build_face(Arc::clone(&fd.data), fd.index));

        // Validate fallbacks and compute cap-height normalization.
        let mut fallbacks = Vec::new();
        let mut fallback_meta = Vec::new();
        for fd in &font_set.fallbacks {
            if let Some(face) = build_face(Arc::clone(&fd.data), fd.index) {
                let fb_cap = cap_height_px(&fd.data, fd.index, size_px);
                let scale_factor = if fb_cap > 0.0 && primary_cap > 0.0 {
                    primary_cap / fb_cap
                } else {
                    1.0
                };
                fallbacks.push(face);
                fallback_meta.push(FallbackMeta {
                    scale_factor,
                    size_offset: 0.0,
                    features: None,
                });
            }
        }

        let mut collection = Self {
            primary: [Some(regular_face), bold, italic, bold_italic],
            fallbacks,
            fallback_meta,
            size_px,
            cell_width,
            cell_height,
            baseline,
            cap_height_px: primary_cap,
            format,
            glyph_cache: HashMap::new(),
            scale_context: ScaleContext::new(),
            weight,
            family_name: font_set.family_name,
            features: default_features(),
        };

        collection.pre_cache_ascii();
        Ok(collection)
    }

    // ── Accessors ──

    /// Cell metrics for the GPU renderer.
    pub fn cell_metrics(&self) -> CellMetrics {
        CellMetrics::new(self.cell_width, self.cell_height, self.baseline)
    }

    /// Font size in pixels.
    pub fn size_px(&self) -> f32 {
        self.size_px
    }

    /// Family name of the primary font.
    pub fn family_name(&self) -> &str {
        &self.family_name
    }

    /// Rasterization format.
    #[allow(dead_code, reason = "font fields consumed in later sections")]
    pub fn format(&self) -> GlyphFormat {
        self.format
    }

    /// Number of cached glyphs.
    #[allow(dead_code, reason = "font fields consumed in later sections")]
    pub fn cache_len(&self) -> usize {
        self.glyph_cache.len()
    }

    /// Effective pixel size for a face, accounting for cap-height normalization.
    ///
    /// Primary faces return `size_px`. Fallback faces are scaled by their
    /// cap-height ratio plus any user-configured `size_offset`.
    pub fn effective_size(&self, face_idx: FaceIdx) -> f32 {
        effective_size_for(face_idx, self.size_px, &self.fallback_meta)
    }

    /// OpenType features for a given face.
    ///
    /// Primary faces (0–3) use collection-wide defaults. Fallback faces use
    /// their per-fallback override if configured, otherwise collection defaults.
    pub fn features_for_face(&self, face_idx: FaceIdx) -> &[rustybuzz::Feature] {
        if let Some(fb_i) = face_idx.fallback_index() {
            if let Some(meta) = self.fallback_meta.get(fb_i) {
                if let Some(ref fb_features) = meta.features {
                    return fb_features;
                }
            }
        }
        &self.features
    }

    /// Create rustybuzz `Face` objects for all loaded faces.
    ///
    /// Returns one entry per face slot (4 primary + N fallbacks). Primary faces
    /// get weight variation applied; fallback faces use font defaults.
    ///
    /// Faces borrow from `self`, so the returned vec must not outlive `self`.
    /// Create once per frame, reuse across all rows.
    pub fn create_shaping_faces(&self) -> Vec<Option<rustybuzz::Face<'_>>> {
        let total = 4 + self.fallbacks.len();
        let mut faces = Vec::with_capacity(total);

        // Primary faces with weight variation.
        for (i, slot) in self.primary.iter().enumerate() {
            faces.push(slot.as_ref().and_then(|fd| {
                let mut face = rustybuzz::Face::from_slice(&fd.bytes, fd.face_index)?;
                if let Some(w) = weight_variation(FaceIdx(i as u16), self.weight) {
                    face.set_variations(&[rustybuzz::Variation {
                        tag: rustybuzz::ttf_parser::Tag::from_bytes(b"wght"),
                        value: w,
                    }]);
                }
                Some(face)
            }));
        }

        // Fallback faces (no weight variation).
        for fb in &self.fallbacks {
            faces.push(rustybuzz::Face::from_slice(&fb.bytes, fb.face_index));
        }

        faces
    }

    // ── Resolution ──

    /// Resolve a character to a font face and glyph ID.
    ///
    /// Tries the requested style, falls back through style substitution
    /// (with appropriate synthetic flags), then tries fallback fonts,
    /// and finally returns .notdef from Regular.
    pub fn resolve(&self, ch: char, style: GlyphStyle) -> ResolvedGlyph {
        let idx = style as usize;

        // 1. Try requested style.
        if let Some(ref fd) = self.primary[idx] {
            let gid = glyph_id(fd, ch);
            if gid != 0 {
                return ResolvedGlyph {
                    glyph_id: gid,
                    face_idx: FaceIdx(idx as u16),
                    synthetic: SyntheticFlags::NONE,
                };
            }
        }

        // 2. Style substitution with synthetic flags.
        if style != GlyphStyle::Regular {
            let synthetic = match style {
                GlyphStyle::Bold => self.try_regular_with(ch, SyntheticFlags::BOLD),
                GlyphStyle::Italic => self.try_regular_with(ch, SyntheticFlags::ITALIC),
                GlyphStyle::BoldItalic => self.resolve_bold_italic_fallback(ch),
                GlyphStyle::Regular => unreachable!(),
            };
            if let Some(resolved) = synthetic {
                return resolved;
            }
        }

        // 3. Try fallback fonts.
        for (i, fb) in self.fallbacks.iter().enumerate() {
            let gid = glyph_id(fb, ch);
            if gid != 0 {
                return ResolvedGlyph {
                    glyph_id: gid,
                    face_idx: FaceIdx((4 + i) as u16),
                    synthetic: SyntheticFlags::NONE,
                };
            }
        }

        // 4. Ultimate fallback: .notdef from Regular.
        let gid = self.primary[0].as_ref().map_or(0, |fd| glyph_id(fd, ch));
        ResolvedGlyph {
            glyph_id: gid,
            face_idx: FaceIdx::REGULAR,
            synthetic: SyntheticFlags::NONE,
        }
    }

    /// Resolve preferring fallback fonts for emoji presentation (VS16).
    ///
    /// When VS16 (U+FE0F) forces emoji presentation, tries fallback fonts
    /// first because color emoji fonts (Segoe UI Emoji, Noto Color Emoji)
    /// are typically in the fallback chain, not the primary terminal font.
    ///
    /// Falls back to normal [`resolve`] if no fallback covers the character.
    pub fn resolve_prefer_emoji(&self, ch: char, style: GlyphStyle) -> ResolvedGlyph {
        // Try fallback fonts first (color emoji fonts are typically here).
        for (i, fb) in self.fallbacks.iter().enumerate() {
            let gid = glyph_id(fb, ch);
            if gid != 0 {
                return ResolvedGlyph {
                    glyph_id: gid,
                    face_idx: FaceIdx((4 + i) as u16),
                    synthetic: SyntheticFlags::NONE,
                };
            }
        }
        // No fallback covers it — use normal resolution.
        self.resolve(ch, style)
    }

    // ── Rasterization ──

    /// Rasterize a glyph and cache the result.
    ///
    /// Returns `None` for empty glyphs (e.g. space) or unsupported formats.
    /// Subsequent calls with the same key return the cached bitmap.
    pub fn rasterize(&mut self, key: RasterKey) -> Option<&RasterizedGlyph> {
        // Cache hit — early return. Uses `contains_key` + final `get` because
        // `if let Some(g) = get()` borrows `glyph_cache` for the return
        // lifetime, conflicting with `insert` on the miss path (E0502).
        if self.glyph_cache.contains_key(&key) {
            return self.glyph_cache.get(&key);
        }

        // Inline face lookup for disjoint borrows with scale_context.
        let fd = if let Some(fb_i) = key.face_idx.fallback_index() {
            self.fallbacks.get(fb_i)?
        } else {
            self.primary[key.face_idx.as_usize()].as_ref()?
        };
        let size = effective_size_for(key.face_idx, self.size_px, &self.fallback_meta);
        let wght = weight_variation(key.face_idx, self.weight);
        let glyph = rasterize_from_face(
            fd,
            key.glyph_id,
            size,
            wght,
            self.format,
            &mut self.scale_context,
        )?;

        self.glyph_cache.insert(key, glyph);
        self.glyph_cache.get(&key)
    }

    // ── Private helpers ──

    /// Try Regular face with the given synthetic flags.
    fn try_regular_with(&self, ch: char, flags: SyntheticFlags) -> Option<ResolvedGlyph> {
        let fd = self.primary[0].as_ref()?;
        let gid = glyph_id(fd, ch);
        if gid != 0 {
            Some(ResolvedGlyph {
                glyph_id: gid,
                face_idx: FaceIdx::REGULAR,
                synthetic: flags,
            })
        } else {
            None
        }
    }

    /// Try bold → italic → regular for `BoldItalic` style substitution.
    fn resolve_bold_italic_fallback(&self, ch: char) -> Option<ResolvedGlyph> {
        // Try bold face with synthetic italic.
        if let Some(ref fd) = self.primary[GlyphStyle::Bold as usize] {
            let gid = glyph_id(fd, ch);
            if gid != 0 {
                return Some(ResolvedGlyph {
                    glyph_id: gid,
                    face_idx: FaceIdx(GlyphStyle::Bold as u16),
                    synthetic: SyntheticFlags::ITALIC,
                });
            }
        }
        // Try italic face with synthetic bold.
        if let Some(ref fd) = self.primary[GlyphStyle::Italic as usize] {
            let gid = glyph_id(fd, ch);
            if gid != 0 {
                return Some(ResolvedGlyph {
                    glyph_id: gid,
                    face_idx: FaceIdx(GlyphStyle::Italic as u16),
                    synthetic: SyntheticFlags::BOLD,
                });
            }
        }
        // Try regular with both flags.
        self.try_regular_with(ch, SyntheticFlags::BOLD | SyntheticFlags::ITALIC)
    }

    /// Pre-cache all printable ASCII glyphs (0x20–0x7E).
    fn pre_cache_ascii(&mut self) {
        let size_q6 = size_key(self.size_px);
        for ch in ' '..='~' {
            let resolved = self.resolve(ch, GlyphStyle::Regular);
            let key = RasterKey {
                glyph_id: resolved.glyph_id,
                face_idx: resolved.face_idx,
                size_q6,
            };
            let _ = self.rasterize(key);
        }
    }
}

// ── Free functions ──

/// Default OpenType features: standard ligatures + contextual alternates.
///
/// These are the features most users expect from a terminal font.
fn default_features() -> Vec<rustybuzz::Feature> {
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
pub fn parse_features(tags: &[&str]) -> Vec<rustybuzz::Feature> {
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
fn effective_size_for(face_idx: FaceIdx, base_size: f32, fallback_meta: &[FallbackMeta]) -> f32 {
    if let Some(fb_i) = face_idx.fallback_index() {
        if let Some(meta) = fallback_meta.get(fb_i) {
            return (base_size * meta.scale_factor + meta.size_offset)
                .clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        }
    }
    base_size
}

/// Compute the `wght` variation value for a face index.
///
/// Primary faces use the configured weight (Regular/Italic) or bold-derived
/// weight (Bold/BoldItalic). Fallback faces return `None`.
fn weight_variation(face_idx: FaceIdx, weight: u16) -> Option<f32> {
    if face_idx.is_fallback() {
        return None;
    }
    let i = face_idx.as_usize();
    let w = if i == 1 || i == 3 {
        (weight + 300).min(900)
    } else {
        weight
    };
    Some(w as f32)
}

#[cfg(test)]
mod tests;
