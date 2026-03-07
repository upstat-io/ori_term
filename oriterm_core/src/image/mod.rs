//! Image storage, placement, and cache for inline image protocols.
//!
//! Supports Kitty Graphics Protocol, Sixel, and iTerm2 image protocol.
//! Images are stored as decoded RGBA pixel data with reference-counted
//! sharing across placements. Memory-managed with configurable limits
//! and LRU eviction.

mod cache;
mod decode;
pub mod iterm2;
pub mod kitty;
pub mod sixel;

use std::sync::Arc;
use std::time::Duration;

pub use cache::ImageCache;
pub use decode::{
    GifFrames, ImageFormat, decode_gif_frames, decode_to_rgba, detect_format, rgb_to_rgba,
};

use crate::grid::StableRowIndex;

/// Kitty virtual placeholder character (U+10EEEE).
///
/// Programs using Kitty's unicode placeholder mode (`U=1`) write this
/// character into grid cells to reserve space for images. Selection
/// text extraction skips these characters.
pub const KITTY_PLACEHOLDER: char = '\u{10EEEE}';

/// Unique image identifier within a terminal instance.
///
/// IDs start at `2_147_483_647` (mid-range u32) for auto-assigned images
/// to avoid collisions with client-assigned IDs that typically start at 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageId(pub(crate) u32);

impl ImageId {
    /// Construct an `ImageId` from a raw u32 value.
    pub fn from_raw(val: u32) -> Self {
        Self(val)
    }

    /// Get the underlying u32 value.
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// Source of image data (how it was transmitted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageSource {
    /// Data sent directly in the protocol payload.
    Direct,
    /// Data loaded from a file path.
    File(std::path::PathBuf),
    /// Data loaded from shared memory (platform-specific).
    SharedMemory,
}

/// Decoded image pixel data.
#[derive(Debug, Clone)]
pub struct ImageData {
    /// Unique image identifier.
    pub id: ImageId,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Decoded RGBA pixel data (shared across placements).
    ///
    /// GPU layer receives `&[u8]` via `data.as_slice()` — never clone
    /// the `Arc` across the core-to-GPU boundary.
    pub data: Arc<Vec<u8>>,
    /// Original format before decode.
    pub format: ImageFormat,
    /// How the image was transmitted.
    pub source: ImageSource,
    /// Monotonic counter for LRU eviction ordering.
    pub last_accessed: u64,
}

/// How a placement's display dimensions are determined on resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementSizing {
    /// Display size = `cols * cell_width` × `rows * cell_height`.
    ///
    /// Scales proportionally when cell dimensions change (e.g. font
    /// size change). Used when the protocol explicitly specifies cell
    /// counts (Kitty `c=`/`r=`, iTerm2 cell-count mode).
    CellCount,

    /// Display size is fixed at these pixel dimensions.
    ///
    /// `cols`/`rows` are recomputed via `ImageCache::update_cell_coverage`
    /// when cell dimensions change so viewport intersection and region
    /// queries remain correct. Used for Sixel, Kitty auto-sized, and
    /// iTerm2 pixel/auto/percent modes.
    FixedPixels {
        /// Display width in pixels.
        width: u32,
        /// Display height in pixels.
        height: u32,
    },
}

/// A placed instance of an image on the terminal grid.
#[derive(Debug, Clone)]
pub struct ImagePlacement {
    /// Reference to image data.
    pub image_id: ImageId,
    /// Kitty placement ID (for updates/deletes).
    pub placement_id: Option<u32>,
    /// Pixel offset within image (source rect origin).
    pub source_x: u32,
    /// Pixel offset within image (source rect origin).
    pub source_y: u32,
    /// Source rect size in pixels.
    pub source_w: u32,
    /// Source rect size in pixels.
    pub source_h: u32,
    /// Grid column (top-left cell).
    pub cell_col: usize,
    /// Grid row as stable row index (survives scrollback eviction).
    pub cell_row: StableRowIndex,
    /// Number of columns the image spans.
    pub cols: usize,
    /// Number of rows the image spans.
    pub rows: usize,
    /// Layer ordering: negative = below text, positive = above text.
    pub z_index: i32,
    /// Sub-cell pixel offset (Kitty `X=` param).
    pub cell_x_offset: u16,
    /// Sub-cell pixel offset (Kitty `Y=` param).
    pub cell_y_offset: u16,
    /// How display dimensions are determined on resize.
    pub sizing: PlacementSizing,
}

/// Errors from image operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageError {
    /// Single image exceeds `max_single_image_bytes`.
    OversizedImage,
    /// Image format not recognized or not supported.
    InvalidFormat,
    /// Image decoding failed (corrupt data, truncated, etc.).
    DecodeFailed(String),
    /// Total image memory would exceed cache limit even after eviction.
    MemoryLimitExceeded,
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OversizedImage => write!(f, "image exceeds maximum size limit"),
            Self::InvalidFormat => write!(f, "unrecognized image format"),
            Self::DecodeFailed(msg) => write!(f, "image decode failed: {msg}"),
            Self::MemoryLimitExceeded => write!(f, "image memory limit exceeded"),
        }
    }
}

impl std::error::Error for ImageError {}

/// Kitty frame composition mode (`X=` key in `a=f` commands).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionMode {
    /// Alpha-blend new frame data over the previous frame.
    AlphaBlend,
    /// New frame data overwrites the target region entirely.
    Overwrite,
}

/// Minimum frame duration for animated images (60fps cap).
///
/// Prevents abusive GIFs with 0ms or 10ms frame durations from burning
/// CPU/GPU. Matches `WezTerm`'s approach.
const MIN_FRAME_DURATION: Duration = Duration::from_millis(16);

/// Per-image animation state for multi-frame images (GIF, Kitty animated).
///
/// Stored in `ImageCache::animations` keyed by `ImageId`. Multiple
/// placements of the same image share one animation state (same frame
/// displayed everywhere).
#[derive(Debug, Clone)]
pub struct AnimationState {
    /// Index of the currently displayed frame.
    pub current_frame: usize,
    /// Duration for each frame (0-indexed, parallel to frame data).
    pub frame_durations: Vec<Duration>,
    /// Total number of frames.
    pub total_frames: usize,
    /// Number of loops (None = infinite).
    pub loop_count: Option<u32>,
    /// Number of complete loops performed so far.
    pub loops_completed: u32,
    /// Whether animation is paused (Kitty `a=s`).
    pub paused: bool,
}

impl AnimationState {
    /// Create a new animation state.
    pub fn new(frame_durations: Vec<Duration>, loop_count: Option<u32>) -> Self {
        let total_frames = frame_durations.len();
        Self {
            current_frame: 0,
            frame_durations,
            total_frames,
            loop_count,
            loops_completed: 0,
            paused: false,
        }
    }

    /// Duration of the current frame (clamped to minimum).
    pub fn current_duration(&self) -> Duration {
        self.frame_durations
            .get(self.current_frame)
            .copied()
            .unwrap_or(MIN_FRAME_DURATION)
            .max(MIN_FRAME_DURATION)
    }

    /// Advance to the next frame. Returns `true` if the frame changed.
    ///
    /// Handles loop counting and stops at the final frame when loops
    /// are exhausted.
    pub fn advance(&mut self) -> bool {
        if self.paused || self.total_frames <= 1 {
            return false;
        }

        let next = self.current_frame + 1;
        if next >= self.total_frames {
            // Looped back to start.
            if let Some(max_loops) = self.loop_count {
                self.loops_completed += 1;
                if self.loops_completed >= max_loops {
                    return false; // All loops complete.
                }
            }
            self.current_frame = 0;
        } else {
            self.current_frame = next;
        }
        true
    }

    /// Whether the animation has finished all its loops.
    pub fn is_finished(&self) -> bool {
        if let Some(max_loops) = self.loop_count {
            self.loops_completed >= max_loops
        } else {
            false // Infinite loop.
        }
    }
}

#[cfg(test)]
mod tests;
