//! Font collection: face data, rasterization, metrics, glyph cache.
//!
//! [`FontSet`] loads raw font bytes from discovery results. [`FontCollection`]
//! validates faces, computes cell metrics, resolves characters to glyph IDs,
//! and rasterizes glyphs into bitmaps ready for GPU atlas upload.

mod face;

use std::collections::HashMap;
use std::sync::Arc;

use swash::scale::ScaleContext;

use super::discovery::{self, FontOrigin};
use super::{FontError, GlyphFormat, GlyphStyle, RasterKey, ResolvedGlyph, SyntheticFlags};
use crate::gpu::frame_input::CellMetrics;
use face::{build_face, cap_height_px, compute_metrics, glyph_id, rasterize_from_face, FaceData};

pub use face::size_key;

// ── Public types ──

/// Raw font bytes and collection index (pre-validation).
pub struct FontData {
    /// Font file bytes shared via `Arc` for rustybuzz face creation.
    data: Arc<Vec<u8>>,
    /// Face index within a `.ttc` collection (0 for standalone `.ttf`).
    index: u32,
}

/// Four style variants plus an ordered fallback chain.
///
/// Constructed by [`FontSet::load`] from discovery results. Passed to
/// [`FontCollection::new`] for validation and metrics computation.
pub struct FontSet {
    /// Human-readable family name.
    family_name: String,
    /// Regular face data (always present).
    regular: FontData,
    /// Bold face data (if a real bold variant was found).
    bold: Option<FontData>,
    /// Italic face data (if a real italic variant was found).
    italic: Option<FontData>,
    /// Bold-italic face data (if a real bold-italic variant was found).
    bold_italic: Option<FontData>,
    /// Which style slots have real font files.
    has_variant: [bool; 4],
    /// Ordered fallback fonts for missing-glyph coverage.
    fallbacks: Vec<FontData>,
}

impl FontSet {
    /// Load font data from discovery results.
    ///
    /// If `family` is `None`, uses platform defaults (with embedded fallback).
    /// The `weight` parameter is CSS-style (100–900) for the Regular slot.
    pub fn load(family: Option<&str>, weight: u16) -> Result<Self, FontError> {
        let result = discovery::discover_fonts(family, weight);
        Self::from_discovery(&result)
    }

    /// Build a `FontSet` from a discovery result.
    fn from_discovery(result: &discovery::DiscoveryResult) -> Result<Self, FontError> {
        let primary = &result.primary;

        let regular = load_font_data(primary, 0)?;

        let bold = if primary.has_variant[1] {
            load_font_data(primary, 1).ok()
        } else {
            None
        };
        let italic = if primary.has_variant[2] {
            load_font_data(primary, 2).ok()
        } else {
            None
        };
        let bold_italic = if primary.has_variant[3] {
            load_font_data(primary, 3).ok()
        } else {
            None
        };

        let fallbacks = result
            .fallbacks
            .iter()
            .filter_map(|fb| {
                let bytes = match std::fs::read(&fb.path) {
                    Ok(b) => b,
                    Err(e) => {
                        log::warn!(
                            "font: failed to load fallback {}: {e}",
                            fb.path.display()
                        );
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
    has_variant: [bool; 4],
    fallbacks: Vec<FaceData>,
    // Metrics
    size_px: f32,
    cell_width: f32,
    cell_height: f32,
    baseline: f32,
    #[expect(dead_code, reason = "used for fallback normalization in Section 6")]
    cap_height_px: f32,
    // Rasterization
    format: GlyphFormat,
    glyph_cache: HashMap<RasterKey, RasterizedGlyph>,
    scale_context: ScaleContext,
    // Config
    weight: u16,
    family_name: String,
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

        // Validate fallbacks.
        let fallbacks: Vec<FaceData> = font_set
            .fallbacks
            .iter()
            .filter_map(|fd| build_face(Arc::clone(&fd.data), fd.index))
            .collect();

        let mut collection = Self {
            primary: [Some(regular_face), bold, italic, bold_italic],
            has_variant: font_set.has_variant,
            fallbacks,
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
    pub fn format(&self) -> GlyphFormat {
        self.format
    }

    /// Number of cached glyphs.
    pub fn cache_len(&self) -> usize {
        self.glyph_cache.len()
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
                    face_idx: idx as u16,
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
                    face_idx: (4 + i) as u16,
                    synthetic: SyntheticFlags::NONE,
                };
            }
        }

        // 4. Ultimate fallback: .notdef from Regular.
        let gid = self.primary[0].as_ref().map_or(0, |fd| glyph_id(fd, ch));
        ResolvedGlyph {
            glyph_id: gid,
            face_idx: 0,
            synthetic: SyntheticFlags::NONE,
        }
    }

    /// Find which face covers a character for the given style.
    ///
    /// Convenience wrapper around [`resolve`](Self::resolve).
    pub fn find_face_for_char(&self, ch: char, style: GlyphStyle) -> ResolvedGlyph {
        self.resolve(ch, style)
    }

    // ── Rasterization ──

    /// Rasterize a glyph and cache the result.
    ///
    /// Returns `None` for empty glyphs (e.g. space) or unsupported formats.
    /// Subsequent calls with the same key return the cached bitmap.
    pub fn rasterize(&mut self, key: RasterKey) -> Option<&RasterizedGlyph> {
        if self.glyph_cache.contains_key(&key) {
            return self.glyph_cache.get(&key);
        }

        // Inline face lookup for disjoint borrows with scale_context.
        let i = key.face_idx as usize;
        let fd = if i < 4 {
            self.primary[i].as_ref()?
        } else {
            self.fallbacks.get(i - 4)?
        };
        let wght = weight_variation(key.face_idx, self.weight);
        let glyph = rasterize_from_face(
            fd,
            key.glyph_id,
            self.size_px,
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
                face_idx: 0,
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
                    face_idx: GlyphStyle::Bold as u16,
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
                    face_idx: GlyphStyle::Italic as u16,
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

/// Compute the `wght` variation value for a face index.
///
/// Primary faces use the configured weight (Regular/Italic) or bold-derived
/// weight (Bold/BoldItalic). Fallback faces return `None`.
fn weight_variation(face_idx: u16, weight: u16) -> Option<f32> {
    let i = face_idx as usize;
    if i < 4 {
        let w = if i == 1 || i == 3 {
            (weight + 300).min(900)
        } else {
            weight
        };
        Some(w as f32)
    } else {
        None
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

#[cfg(test)]
mod tests;
