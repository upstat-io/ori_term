//! Image cache with LRU eviction and configurable memory limits.

mod animation;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::grid::StableRowIndex;

use super::{AnimationState, ImageData, ImageError, ImageId, ImagePlacement, PlacementSizing};

/// Default memory limit for decoded image data (320 MiB, matching Ghostty).
const DEFAULT_MEMORY_LIMIT: usize = 320 * 1024 * 1024;

/// Default maximum size for a single image (64 MiB).
const DEFAULT_MAX_SINGLE_IMAGE: usize = 64 * 1024 * 1024;

/// Starting ID for auto-assigned images (mid-range u32 to avoid
/// collisions with client-assigned IDs that start at 1).
const AUTO_ID_START: u32 = 2_147_483_647;

/// In-memory image cache with reference counting, eviction, and
/// configurable memory limits.
///
/// Each terminal screen (primary and alternate) owns its own cache.
/// Placements use `StableRowIndex` so they scroll with text
/// automatically.
pub struct ImageCache {
    /// Image data store keyed by image ID.
    pub(super) images: HashMap<ImageId, ImageData>,
    /// Active placements sorted by row for efficient viewport queries.
    pub(super) placements: Vec<ImagePlacement>,
    /// Total bytes of decoded image data currently stored.
    memory_used: usize,
    /// Configurable maximum memory for decoded image data.
    memory_limit: usize,
    /// Reject single images exceeding this size.
    pub(super) max_single_image_bytes: usize,
    /// Monotonic ID allocator for auto-assigned images.
    next_id: u32,
    /// Monotonic counter bumped on each image access (LRU ordering).
    access_counter: u64,
    /// Set when placements/images change; caller clears via `take_dirty()`.
    pub(super) dirty: bool,
    /// Per-image animation state for multi-frame images.
    pub(super) animations: HashMap<ImageId, AnimationState>,
    /// All decoded frames for animated images (frame 0 is also in `ImageData.data`).
    pub(super) animation_frames: HashMap<ImageId, Vec<Arc<Vec<u8>>>>,
    /// When each animated image's current frame started displaying.
    pub(super) frame_starts: HashMap<ImageId, Instant>,
    /// Whether animation is enabled (from config).
    pub(super) animation_enabled: bool,
}

impl ImageCache {
    /// Create a new empty image cache with default limits.
    pub fn new() -> Self {
        Self {
            images: HashMap::new(),
            placements: Vec::new(),
            memory_used: 0,
            memory_limit: DEFAULT_MEMORY_LIMIT,
            max_single_image_bytes: DEFAULT_MAX_SINGLE_IMAGE,
            next_id: AUTO_ID_START,
            access_counter: 0,
            dirty: false,
            animations: HashMap::new(),
            animation_frames: HashMap::new(),
            frame_starts: HashMap::new(),
            animation_enabled: true,
        }
    }

    /// Returns `true` if any images or placements have changed since
    /// the last `take_dirty()` call.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Returns the current dirty flag and clears it.
    ///
    /// Called by `Term::renderable_content_into()` when building
    /// the snapshot for the renderer.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::replace(&mut self.dirty, false)
    }

    /// Total bytes of decoded image data in the cache.
    pub fn memory_used(&self) -> usize {
        self.memory_used
    }

    /// Number of stored images.
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// Number of active placements.
    pub fn placement_count(&self) -> usize {
        self.placements.len()
    }

    /// Maximum allowed size for a single image in bytes.
    pub fn max_single_image_bytes(&self) -> usize {
        self.max_single_image_bytes
    }

    /// Update the memory limit. Triggers eviction if currently over.
    pub fn set_memory_limit(&mut self, limit: usize) {
        self.memory_limit = limit;
        self.evict_lru();
    }

    /// Update the max single image size.
    pub fn set_max_single_image(&mut self, limit: usize) {
        self.max_single_image_bytes = limit;
    }

    /// Allocate the next auto-assigned image ID.
    pub fn next_image_id(&mut self) -> ImageId {
        let id = ImageId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    /// Store image data in the cache.
    ///
    /// Rejects images exceeding `max_single_image_bytes`. Evicts LRU
    /// images if the cache would exceed `memory_limit`.
    pub fn store(&mut self, mut data: ImageData) -> Result<ImageId, ImageError> {
        let size = data.data.len();
        if size > self.max_single_image_bytes {
            return Err(ImageError::OversizedImage);
        }

        // Evict until we have room (or can't evict more).
        let placed = self.placed_id_set();
        while self.memory_used + size > self.memory_limit && !self.images.is_empty() {
            if !self.evict_one(&placed) {
                return Err(ImageError::MemoryLimitExceeded);
            }
        }

        // Stamp access counter so store order determines initial LRU rank.
        self.access_counter += 1;
        data.last_accessed = self.access_counter;

        let id = data.id;
        self.memory_used += size;
        self.images.insert(id, data);
        self.dirty = true;
        Ok(id)
    }

    /// Add a placement for an existing image.
    pub fn place(&mut self, placement: ImagePlacement) {
        self.placements.push(placement);
        self.dirty = true;
    }

    /// Remove an image and all its placements.
    pub fn remove_image(&mut self, id: ImageId) {
        if let Some(img) = self.images.remove(&id) {
            self.memory_used = self.memory_used.saturating_sub(img.data.len());

            // Subtract animation frames 1..N (frame 0 is already subtracted
            // above as `img.data`).
            if let Some(frames) = self.animation_frames.remove(&id) {
                let extra: usize = frames.iter().skip(1).map(|f| f.len()).sum();
                self.memory_used = self.memory_used.saturating_sub(extra);
            }

            self.placements.retain(|p| p.image_id != id);
            self.animations.remove(&id);
            self.frame_starts.remove(&id);
            self.dirty = true;
        }
    }

    /// Remove a specific placement by image ID and placement ID.
    pub fn remove_placement(&mut self, image_id: ImageId, placement_id: u32) {
        let before = self.placements.len();
        self.placements
            .retain(|p| !(p.image_id == image_id && p.placement_id == Some(placement_id)));
        if self.placements.len() != before {
            self.dirty = true;
        }
    }

    /// Remove all placements for an image ID (without removing the image data).
    pub fn remove_placements_for_image(&mut self, image_id: ImageId) {
        let before = self.placements.len();
        self.placements.retain(|p| p.image_id != image_id);
        if self.placements.len() != before {
            self.dirty = true;
        }
    }

    /// Remove placements at a specific column.
    pub fn remove_placements_at_column(&mut self, col: usize) {
        let before = self.placements.len();
        self.placements.retain(|p| {
            let right = p.cell_col + p.cols.saturating_sub(1);
            !(p.cell_col <= col && right >= col)
        });
        if self.placements.len() != before {
            self.dirty = true;
        }
    }

    /// Remove placements at a specific row.
    pub fn remove_placements_at_row(&mut self, row: StableRowIndex) {
        let before = self.placements.len();
        self.placements.retain(|p| {
            let bottom = StableRowIndex(p.cell_row.0 + p.rows.saturating_sub(1) as u64);
            !(p.cell_row <= row && bottom >= row)
        });
        if self.placements.len() != before {
            self.dirty = true;
        }
    }

    /// Remove placements with a specific z-index.
    pub fn remove_placements_by_z_index(&mut self, z: i32) {
        let before = self.placements.len();
        self.placements.retain(|p| p.z_index != z);
        if self.placements.len() != before {
            self.dirty = true;
        }
    }

    /// Remove all placements at a specific cell position.
    pub fn remove_by_position(&mut self, col: usize, row: StableRowIndex) {
        let before = self.placements.len();
        self.placements
            .retain(|p| !(p.cell_col == col && p.cell_row == row));
        if self.placements.len() != before {
            self.dirty = true;
        }
    }

    /// Return placements visible in the given stable row range (inclusive).
    pub fn placements_in_viewport(
        &self,
        top_row: StableRowIndex,
        bottom_row: StableRowIndex,
    ) -> Vec<&ImagePlacement> {
        self.placements
            .iter()
            .filter(|p| {
                let placement_bottom =
                    StableRowIndex(p.cell_row.0 + p.rows.saturating_sub(1) as u64);
                // Placement overlaps viewport if it starts before bottom
                // and ends after top.
                p.cell_row <= bottom_row && placement_bottom >= top_row
            })
            .collect()
    }

    /// Remove placements whose `cell_row` is before the eviction boundary.
    ///
    /// Called when scrollback evicts rows so stale placements don't
    /// accumulate. Also removes images with zero remaining placements
    /// (Ghostty pattern: unused images evicted first).
    pub fn prune_scrollback(&mut self, evicted_before: StableRowIndex) {
        let before = self.placements.len();
        self.placements.retain(|p| p.cell_row >= evicted_before);
        if self.placements.len() != before {
            self.dirty = true;
            self.remove_orphans();
        }
    }

    /// Remove placements overlapping a rectangular region.
    ///
    /// Used by ED/EL erase operations. If `left`/`right` are `None`,
    /// the full row width is cleared.
    pub fn remove_placements_in_region(
        &mut self,
        top: StableRowIndex,
        bottom: StableRowIndex,
        left: Option<usize>,
        right: Option<usize>,
    ) {
        let before = self.placements.len();
        self.placements.retain(|p| {
            let placement_bottom = StableRowIndex(p.cell_row.0 + p.rows.saturating_sub(1) as u64);
            let placement_right = p.cell_col + p.cols.saturating_sub(1);

            // Check row overlap.
            let row_overlap = p.cell_row <= bottom && placement_bottom >= top;
            if !row_overlap {
                return true; // Keep — no row overlap.
            }

            // Check column overlap (if bounds specified).
            let col_overlap = match (left, right) {
                (Some(l), Some(r)) => p.cell_col <= r && placement_right >= l,
                (Some(l), None) => placement_right >= l,
                (None, Some(r)) => p.cell_col <= r,
                (None, None) => true, // Full row erase.
            };

            !col_overlap // Keep if no column overlap.
        });
        if self.placements.len() != before {
            self.dirty = true;
        }
    }

    /// Remove all images and placements.
    pub fn clear(&mut self) {
        if !self.images.is_empty() || !self.placements.is_empty() {
            self.dirty = true;
        }
        self.images.clear();
        self.placements.clear();
        self.animations.clear();
        self.animation_frames.clear();
        self.frame_starts.clear();
        self.memory_used = 0;
    }

    /// Get image data by ID, updating access counter for LRU.
    pub fn get(&mut self, id: ImageId) -> Option<&ImageData> {
        self.access_counter += 1;
        let counter = self.access_counter;
        if let Some(img) = self.images.get_mut(&id) {
            img.last_accessed = counter;
            Some(img)
        } else {
            None
        }
    }

    /// Get image data by ID without updating access counter.
    pub fn get_no_touch(&self, id: ImageId) -> Option<&ImageData> {
        self.images.get(&id)
    }

    /// Recalculate `cols`/`rows` for `FixedPixels` placements.
    ///
    /// Called when cell pixel dimensions change (font size, zoom) so
    /// viewport intersection and region queries use correct cell counts.
    pub fn update_cell_coverage(&mut self, cell_w: u16, cell_h: u16) {
        let cw = cell_w.max(1) as usize;
        let ch = cell_h.max(1) as usize;

        for p in &mut self.placements {
            if let PlacementSizing::FixedPixels { width, height } = p.sizing {
                let new_cols = (width as usize).div_ceil(cw);
                let new_rows = (height as usize).div_ceil(ch);
                if p.cols != new_cols || p.rows != new_rows {
                    p.cols = new_cols;
                    p.rows = new_rows;
                    self.dirty = true;
                }
            }
        }
    }

    /// Evict least-recently-used images until under memory limit.
    ///
    /// Prefers images with zero placements first, then evicts placed
    /// images by LRU order (Ghostty pattern). Builds a placed-ID set
    /// once to avoid O(n*m) per-eviction placement scans.
    fn evict_lru(&mut self) {
        let placed = self.placed_id_set();

        while self.memory_used > self.memory_limit && !self.images.is_empty() {
            if !self.evict_one(&placed) {
                break;
            }
        }
    }

    /// Evict the single least-recently-used image. Returns `true` if
    /// an image was evicted.
    ///
    /// Uses a precomputed set of placed image IDs for O(n) candidate
    /// selection (unplaced first, then oldest access counter).
    fn evict_one(&mut self, placed: &std::collections::HashSet<ImageId>) -> bool {
        let victim = self
            .images
            .iter()
            .map(|(id, img)| (*id, img.last_accessed, placed.contains(id)))
            .min_by(|a, b| {
                a.2.cmp(&b.2) // false (no placements) < true
                    .then(a.1.cmp(&b.1)) // oldest access first
            });

        if let Some((id, _, _)) = victim {
            self.remove_image(id);
            true
        } else {
            false
        }
    }

    /// Build a `HashSet` of image IDs that have at least one placement.
    fn placed_id_set(&self) -> std::collections::HashSet<ImageId> {
        self.placements.iter().map(|p| p.image_id).collect()
    }

    /// Remove images that have no remaining placements.
    pub fn remove_orphans(&mut self) {
        let placed = self.placed_id_set();
        let orphans: Vec<ImageId> = self
            .images
            .keys()
            .filter(|id| !placed.contains(id))
            .copied()
            .collect();

        for id in orphans {
            if let Some(img) = self.images.remove(&id) {
                self.memory_used = self.memory_used.saturating_sub(img.data.len());
            }
        }
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ImageCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageCache")
            .field("images", &self.images.len())
            .field("placements", &self.placements.len())
            .field("memory_used", &self.memory_used)
            .field("memory_limit", &self.memory_limit)
            .field("max_single_image_bytes", &self.max_single_image_bytes)
            .field("next_id", &self.next_id)
            .field("access_counter", &self.access_counter)
            .field("dirty", &self.dirty)
            .field("animations", &self.animations.len())
            .field("animation_frames", &self.animation_frames.len())
            .field("frame_starts", &self.frame_starts.len())
            .field("animation_enabled", &self.animation_enabled)
            .finish()
    }
}
