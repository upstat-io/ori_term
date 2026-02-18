//! Glyph atlas: guillotine-packed texture array for GPU glyph rendering.
//!
//! [`GlyphAtlas`] manages a pre-allocated `Texture2DArray` (2048×2048 × 4
//! pages) using guillotine bin packing for mixed glyph sizes. Pages are
//! evicted via LRU when all are full. Glyphs are inserted once and looked
//! up by [`RasterKey`] on subsequent frames.
//!
//! Two atlas instances are used at runtime:
//! - **Monochrome** (`R8Unorm`): standard glyph alpha masks.
//! - **Color** (`Rgba8Unorm`): color emoji and bitmap glyphs.

mod rect_packer;

use std::collections::{HashMap, HashSet};

use wgpu::{
    Device, Extent3d, Queue, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
};

use self::rect_packer::RectPacker;
use crate::font::collection::RasterizedGlyph;
use crate::font::{GlyphFormat, RasterKey};

/// Atlas page dimension (width = height).
const PAGE_SIZE: u32 = 2048;

/// Maximum number of texture array layers.
const MAX_PAGES: u32 = 4;

/// Padding between glyphs to prevent texture filtering artifacts.
const GLYPH_PADDING: u32 = 1;

/// Per-page packing state and LRU metadata.
struct AtlasPage {
    packer: RectPacker,
    last_used_frame: u64,
    glyph_count: u32,
}

/// Location and metrics of a cached glyph in the atlas.
#[derive(Debug, Clone, Copy)]
pub struct AtlasEntry {
    /// Page index (texture array layer).
    pub page: u32,
    /// Normalized U coordinate of left edge (0.0–1.0).
    pub uv_x: f32,
    /// Normalized V coordinate of top edge (0.0–1.0).
    pub uv_y: f32,
    /// Normalized width (0.0–1.0).
    pub uv_w: f32,
    /// Normalized height (0.0–1.0).
    pub uv_h: f32,
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
    /// Horizontal bearing (pixels from glyph origin to left edge).
    pub bearing_x: i32,
    /// Vertical bearing (pixels from baseline to top edge; positive = above).
    pub bearing_y: i32,
    /// Whether this entry lives in the color (RGBA) atlas.
    pub is_color: bool,
}

/// Texture atlas for glyph bitmaps using guillotine packing on a `Texture2DArray`.
///
/// Manages a single pre-allocated texture array with up to [`MAX_PAGES`]
/// layers. The texture format is determined at construction: `R8Unorm` for
/// monochrome glyphs, `Rgba8Unorm` for color emoji. Glyphs are packed using
/// guillotine best-short-side-fit, uploaded via `queue.write_texture`, and
/// cached by [`RasterKey`] for O(1) lookup. When all pages are full, the
/// least-recently-used page is evicted.
pub struct GlyphAtlas {
    /// Single pre-allocated `Texture2DArray`.
    texture: Texture,
    /// `D2Array` view over all layers.
    view: TextureView,
    /// Per-page packing state + LRU metadata.
    pages: Vec<AtlasPage>,
    /// Glyph cache: `RasterKey` → atlas entry.
    cache: HashMap<RasterKey, AtlasEntry>,
    /// Keys known to produce zero-size glyphs (spaces, non-printing chars).
    empty_keys: HashSet<RasterKey>,
    page_size: u32,
    max_pages: u32,
    /// Monotonically increasing frame counter for LRU tracking.
    frame_counter: u64,
    /// Pixel format of this atlas texture.
    format: GlyphFormat,
}

impl GlyphAtlas {
    /// Create a new atlas with a pre-allocated texture array and one active page.
    ///
    /// `format` determines the texture format:
    /// - [`GlyphFormat::Alpha`] → `R8Unorm` (1 byte/pixel).
    /// - [`GlyphFormat::Color`] → `Rgba8Unorm` (4 bytes/pixel).
    pub fn new(device: &Device, format: GlyphFormat) -> Self {
        let tex_format = match format {
            GlyphFormat::Color => TextureFormat::Rgba8Unorm,
            _ => TextureFormat::R8Unorm,
        };
        let (texture, view) = create_texture_array(device, PAGE_SIZE, MAX_PAGES, tex_format);

        Self {
            texture,
            view,
            pages: vec![AtlasPage {
                packer: RectPacker::new(PAGE_SIZE, PAGE_SIZE),
                last_used_frame: 0,
                glyph_count: 0,
            }],
            cache: HashMap::new(),
            empty_keys: HashSet::new(),
            page_size: PAGE_SIZE,
            max_pages: MAX_PAGES,
            frame_counter: 0,
            format,
        }
    }

    /// Increment the frame counter for LRU tracking.
    ///
    /// Call at the start of each frame before any glyph lookups or inserts.
    pub fn begin_frame(&mut self) {
        self.frame_counter += 1;
    }

    /// Look up a previously inserted glyph.
    ///
    /// For LRU correctness, callers with `&mut` access should also call
    /// [`touch_page`](Self::touch_page) with the entry's page index.
    pub fn lookup(&self, key: RasterKey) -> Option<&AtlasEntry> {
        self.cache.get(&key)
    }

    /// Look up a glyph and touch its page for LRU tracking in one call.
    ///
    /// Combines [`lookup`](Self::lookup) and [`touch_page`](Self::touch_page)
    /// atomically so callers can't forget to update LRU on cache hits.
    pub fn lookup_touch(&mut self, key: RasterKey) -> Option<AtlasEntry> {
        let entry = self.cache.get(&key).copied()?;
        if let Some(p) = self.pages.get_mut(entry.page as usize) {
            p.last_used_frame = self.frame_counter;
        }
        Some(entry)
    }

    /// Mark a page as used this frame for LRU tracking.
    ///
    /// Call after [`lookup`](Self::lookup) when you have mutable access to
    /// ensure recently-used pages are not evicted.
    pub fn touch_page(&mut self, page: u32) {
        if let Some(p) = self.pages.get_mut(page as usize) {
            p.last_used_frame = self.frame_counter;
        }
    }

    /// Whether the key is known to produce a zero-size glyph.
    ///
    /// Callers should skip rasterization when this returns `true`, since
    /// the glyph will never produce an atlas entry.
    pub fn is_known_empty(&self, key: RasterKey) -> bool {
        self.empty_keys.contains(&key)
    }

    /// Record that a key produces no visible glyph.
    ///
    /// Prevents repeated fruitless rasterization attempts on subsequent
    /// frames for codepoints that are in a built-in range but have no
    /// rendering path.
    pub fn mark_empty(&mut self, key: RasterKey) {
        self.empty_keys.insert(key);
    }

    /// Insert a rasterized glyph into the atlas.
    ///
    /// Finds space via guillotine packing, uploads the bitmap to the GPU, and
    /// caches the entry. Returns `None` for zero-size glyphs (e.g. space)
    /// or glyphs too large for an atlas page.
    pub fn insert(
        &mut self,
        key: RasterKey,
        glyph: &RasterizedGlyph,
        queue: &Queue,
    ) -> Option<AtlasEntry> {
        if let Some(&entry) = self.cache.get(&key) {
            return Some(entry);
        }

        if glyph.width == 0 || glyph.height == 0 {
            self.empty_keys.insert(key);
            return None;
        }

        let max_dim = self.page_size.saturating_sub(GLYPH_PADDING);
        if glyph.width > max_dim || glyph.height > max_dim {
            log::warn!(
                "glyph too large for atlas: {}×{} exceeds page size {}",
                glyph.width,
                glyph.height,
                self.page_size,
            );
            return None;
        }

        let (page_idx, x, y) = self.find_space(glyph.width, glyph.height);

        upload_glyph(queue, &self.texture, page_idx, x, y, glyph);

        self.pages[page_idx as usize].last_used_frame = self.frame_counter;
        self.pages[page_idx as usize].glyph_count += 1;

        let is_color = self.format == GlyphFormat::Color;
        let ps = self.page_size as f32;
        let entry = AtlasEntry {
            page: page_idx,
            uv_x: x as f32 / ps,
            uv_y: y as f32 / ps,
            uv_w: glyph.width as f32 / ps,
            uv_h: glyph.height as f32 / ps,
            width: glyph.width,
            height: glyph.height,
            bearing_x: glyph.bearing_x,
            bearing_y: glyph.bearing_y,
            is_color,
        };

        self.cache.insert(key, entry);
        Some(entry)
    }

    /// Look up or insert a glyph in one call.
    ///
    /// If the key is already cached, returns the entry (touching LRU).
    /// If the key is known empty, returns `None`. Otherwise, calls
    /// `rasterize` to produce the glyph and inserts it.
    ///
    /// This unifies the lookup-rasterize-insert pattern used by
    /// [`ensure_glyphs_cached`](crate::gpu::renderer::GpuRenderer::ensure_glyphs_cached).
    #[allow(dead_code, reason = "convenience API for later integration")]
    pub fn get_or_insert(
        &mut self,
        key: RasterKey,
        rasterize: impl FnOnce() -> Option<RasterizedGlyph>,
        queue: &Queue,
    ) -> Option<AtlasEntry> {
        // Cache hit — touch page and return.
        if let Some(entry) = self.cache.get(&key).copied() {
            self.touch_page(entry.page);
            return Some(entry);
        }

        // Known empty — skip rasterization.
        if self.empty_keys.contains(&key) {
            return None;
        }

        // Cache miss — rasterize and insert.
        let glyph = rasterize()?;
        self.insert(key, &glyph, queue)
    }

    /// `Texture2DArray` view for atlas bind group creation.
    pub fn view(&self) -> &TextureView {
        &self.view
    }

    /// Number of cached glyph entries.
    #[allow(dead_code, reason = "used in tests and diagnostics")]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the cache is empty.
    #[allow(dead_code, reason = "used in tests and diagnostics")]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Number of active atlas pages.
    #[allow(dead_code, reason = "used in tests and diagnostics")]
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Current frame counter value.
    #[allow(dead_code, reason = "used in tests and diagnostics")]
    pub fn frame_counter(&self) -> u64 {
        self.frame_counter
    }

    /// Clear all cached glyphs and reset packing state.
    ///
    /// Keeps the texture array but resets to one active page. Called on font
    /// size change when all cached glyphs become invalid.
    #[allow(dead_code, reason = "used in tests and font size change")]
    pub fn clear(&mut self) {
        self.cache.clear();
        self.empty_keys.clear();
        for page in &mut self.pages {
            page.packer.reset();
            page.glyph_count = 0;
        }
        self.pages.truncate(1);
    }

    // ── Private helpers ──

    /// Find space for a glyph, returning `(page_idx, x, y)`.
    ///
    /// Tries each existing page's guillotine packer. If all are full and
    /// fewer than `max_pages` exist, adds a new page. If at the page limit,
    /// evicts the least-recently-used page.
    fn find_space(&mut self, w: u32, h: u32) -> (u32, u32, u32) {
        let padded_w = w + GLYPH_PADDING;
        let padded_h = h + GLYPH_PADDING;

        // Try existing pages.
        for (i, page) in self.pages.iter_mut().enumerate() {
            if let Some((x, y)) = page.packer.pack(padded_w, padded_h) {
                return (i as u32, x, y);
            }
        }

        // All pages full — add a new one if under the limit.
        if (self.pages.len() as u32) < self.max_pages {
            let page_idx = self.pages.len();
            self.pages.push(AtlasPage {
                packer: RectPacker::new(self.page_size, self.page_size),
                last_used_frame: self.frame_counter,
                glyph_count: 0,
            });

            let (x, y) = self.pages[page_idx]
                .packer
                .pack(padded_w, padded_h)
                .expect("fresh page must fit glyph within page_size bounds");

            return (page_idx as u32, x, y);
        }

        // At max pages — LRU eviction.
        let evicted = self.find_lru_page();
        self.evict_page(evicted);

        let (x, y) = self.pages[evicted]
            .packer
            .pack(padded_w, padded_h)
            .expect("freshly evicted page must fit glyph");

        (evicted as u32, x, y)
    }

    /// Find the page index with the smallest `last_used_frame`.
    fn find_lru_page(&self) -> usize {
        self.pages
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| p.last_used_frame)
            .map(|(i, _)| i)
            .expect("at least one page exists")
    }

    /// Evict a page: reset its packer and remove all cache entries on it.
    fn evict_page(&mut self, page_idx: usize) {
        self.pages[page_idx].packer.reset();
        self.pages[page_idx].glyph_count = 0;
        self.pages[page_idx].last_used_frame = self.frame_counter;
        self.cache.retain(|_, e| e.page as usize != page_idx);
    }
}

// ── Free functions ──

/// Create a pre-allocated texture array with the given format.
fn create_texture_array(
    device: &Device,
    size: u32,
    max_pages: u32,
    format: TextureFormat,
) -> (Texture, TextureView) {
    let label = match format {
        TextureFormat::Rgba8Unorm => "color_glyph_atlas_array",
        _ => "glyph_atlas_array",
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: max_pages,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let view = texture.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..Default::default()
    });

    (texture, view)
}

/// Upload a glyph bitmap to a position on a texture array layer.
///
/// Handles both `R8Unorm` (1 byte/pixel) and `Rgba8Unorm` (4 bytes/pixel)
/// textures based on the glyph's format.
fn upload_glyph(
    queue: &Queue,
    texture: &Texture,
    page_idx: u32,
    x: u32,
    y: u32,
    glyph: &RasterizedGlyph,
) {
    let bpp = glyph.format.bytes_per_pixel();
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x,
                y,
                z: page_idx,
            },
            aspect: wgpu::TextureAspect::All,
        },
        &glyph.bitmap,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(glyph.width * bpp),
            rows_per_image: None,
        },
        Extent3d {
            width: glyph.width,
            height: glyph.height,
            depth_or_array_layers: 1,
        },
    );
}

#[cfg(test)]
mod tests;
