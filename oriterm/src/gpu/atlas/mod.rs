// Atlas types consumed starting in Section 5.10. `allow` (not `expect`)
// because tests exercise these items, making the lint unfulfilled in test builds.
#![allow(dead_code, reason = "atlas types consumed starting in Section 5.10")]

//! Glyph atlas: shelf-packed texture pages for GPU glyph rendering.
//!
//! [`GlyphAtlas`] manages one or more 1024×1024 `R8Unorm` texture pages,
//! packing rasterized glyph bitmaps into horizontal shelves. Glyphs are
//! inserted once and looked up by [`RasterKey`] on subsequent frames.

use std::collections::HashMap;

use wgpu::{
    Device, Extent3d, Queue, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, TextureView, TextureViewDescriptor,
};

use crate::font::collection::RasterizedGlyph;
use crate::font::RasterKey;

/// Default atlas page dimension (width = height).
const PAGE_SIZE: u32 = 1024;

/// Padding between glyphs to prevent texture filtering artifacts.
const GLYPH_PADDING: u32 = 1;

/// A horizontal shelf within an atlas page.
///
/// Glyphs are placed left-to-right. The shelf height is set by the first
/// glyph placed on it (plus padding) and does not change afterward.
struct Shelf {
    /// Y offset of the shelf's top edge on the page.
    y: u32,
    /// Shelf height in pixels (includes padding).
    height: u32,
    /// Next available X position (advances right with each glyph).
    x_cursor: u32,
}

/// Location and metrics of a cached glyph in the atlas.
#[derive(Debug, Clone, Copy)]
pub struct AtlasEntry {
    /// Page index (for multi-page rendering).
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
}

/// Texture atlas for glyph bitmaps using shelf-packing.
///
/// Manages one or more 1024×1024 `R8Unorm` texture pages. Glyphs are packed
/// into horizontal shelves, uploaded via `queue.write_texture`, and cached by
/// [`RasterKey`] for O(1) lookup on subsequent frames.
pub struct GlyphAtlas {
    pages: Vec<Texture>,
    page_views: Vec<TextureView>,
    shelves: Vec<Vec<Shelf>>,
    cache: HashMap<RasterKey, AtlasEntry>,
    page_size: u32,
}

impl GlyphAtlas {
    /// Create a new atlas with one empty 1024×1024 `R8Unorm` page.
    pub fn new(device: &Device) -> Self {
        let (texture, view) = create_atlas_page(device, PAGE_SIZE, 0);

        Self {
            pages: vec![texture],
            page_views: vec![view],
            shelves: vec![vec![]],
            cache: HashMap::new(),
            page_size: PAGE_SIZE,
        }
    }

    /// Look up a previously inserted glyph.
    pub fn lookup(&self, key: RasterKey) -> Option<&AtlasEntry> {
        self.cache.get(&key)
    }

    /// Insert a rasterized glyph into the atlas.
    ///
    /// Finds space via shelf-packing, uploads the bitmap to the GPU, and
    /// caches the entry. Returns `None` for zero-size glyphs (e.g. space)
    /// or glyphs too large for an atlas page.
    pub fn insert(
        &mut self,
        key: RasterKey,
        glyph: &RasterizedGlyph,
        device: &Device,
        queue: &Queue,
    ) -> Option<AtlasEntry> {
        if let Some(&entry) = self.cache.get(&key) {
            return Some(entry);
        }

        if glyph.width == 0 || glyph.height == 0 {
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

        let (page, x, y) = self.allocate(glyph.width, glyph.height, device);

        upload_glyph(queue, &self.pages[page as usize], x, y, glyph);

        let ps = self.page_size as f32;
        let entry = AtlasEntry {
            page,
            uv_x: x as f32 / ps,
            uv_y: y as f32 / ps,
            uv_w: glyph.width as f32 / ps,
            uv_h: glyph.height as f32 / ps,
            width: glyph.width,
            height: glyph.height,
            bearing_x: glyph.bearing_x,
            bearing_y: glyph.bearing_y,
        };

        self.cache.insert(key, entry);
        Some(entry)
    }

    /// Primary (page 0) texture view for atlas bind group creation.
    pub fn primary_view(&self) -> &TextureView {
        &self.page_views[0]
    }

    /// Texture view for a specific page.
    pub fn page_view(&self, page: u32) -> Option<&TextureView> {
        self.page_views.get(page as usize)
    }

    /// Number of cached glyph entries.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Number of allocated texture pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Clear all cached glyphs and reset shelves.
    ///
    /// Keeps the first texture page but drops extras. Called on font size
    /// change when all cached glyphs become invalid.
    pub fn clear(&mut self) {
        self.cache.clear();
        for shelf_list in &mut self.shelves {
            shelf_list.clear();
        }
        self.pages.truncate(1);
        self.page_views.truncate(1);
        self.shelves.truncate(1);
    }

    // ── Private helpers ──

    /// Allocate space for a glyph, returning `(page, x, y)`.
    ///
    /// Uses best-fit shelf selection: picks the shelf with the smallest
    /// sufficient height to minimize wasted vertical space. Creates new
    /// shelves and pages as needed.
    fn allocate(&mut self, w: u32, h: u32, device: &Device) -> (u32, u32, u32) {
        let padded_w = w + GLYPH_PADDING;
        let padded_h = h + GLYPH_PADDING;

        for (page_idx, page_shelves) in self.shelves.iter_mut().enumerate() {
            if let Some((x, y)) =
                try_pack_in_page(page_shelves, padded_w, padded_h, self.page_size)
            {
                return (page_idx as u32, x, y);
            }
        }

        // All pages full — allocate a new one.
        let page_idx = self.pages.len();
        let (texture, view) = create_atlas_page(device, self.page_size, page_idx);
        self.pages.push(texture);
        self.page_views.push(view);
        self.shelves.push(vec![]);

        let page_shelves = self.shelves.last_mut().expect("just pushed");
        let (x, y) = try_pack_in_page(page_shelves, padded_w, padded_h, self.page_size)
            .expect("fresh page must fit glyph within page_size bounds");

        (page_idx as u32, x, y)
    }
}

// ── Free functions ──

/// Try to pack a glyph into an existing page using best-fit shelf selection.
///
/// Scans all shelves for the one with the smallest sufficient height that
/// has enough horizontal room. Creates a new shelf if no existing shelf fits.
fn try_pack_in_page(
    shelves: &mut Vec<Shelf>,
    padded_w: u32,
    padded_h: u32,
    page_size: u32,
) -> Option<(u32, u32)> {
    let mut best: Option<(usize, u32)> = None;

    for (i, shelf) in shelves.iter().enumerate() {
        if shelf.height >= padded_h && shelf.x_cursor + padded_w <= page_size {
            let waste = shelf.height - padded_h;
            if best.is_none_or(|(_, bw)| waste < bw) {
                best = Some((i, waste));
            }
        }
    }

    if let Some((idx, _)) = best {
        let shelf = &mut shelves[idx];
        let x = shelf.x_cursor;
        let y = shelf.y;
        shelf.x_cursor += padded_w;
        return Some((x, y));
    }

    // No existing shelf fits — start a new one.
    let new_y = shelves.last().map_or(0, |s| s.y + s.height);
    if new_y + padded_h > page_size {
        return None;
    }

    shelves.push(Shelf {
        y: new_y,
        height: padded_h,
        x_cursor: padded_w,
    });

    Some((0, new_y))
}

/// Create an `R8Unorm` atlas texture page.
fn create_atlas_page(device: &Device, size: u32, idx: usize) -> (Texture, TextureView) {
    let label = format!("atlas_page_{idx}");
    let texture = device.create_texture(&TextureDescriptor {
        label: Some(&label),
        size: Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::R8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    (texture, view)
}

/// Upload a glyph bitmap to a position on an atlas texture.
///
/// Assumes `R8Unorm` format (1 byte per pixel). The glyph must use
/// [`GlyphFormat::Alpha`](crate::font::GlyphFormat::Alpha).
fn upload_glyph(queue: &Queue, texture: &Texture, x: u32, y: u32, glyph: &RasterizedGlyph) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        &glyph.bitmap,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(glyph.width),
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
