//! GPU instance buffer writer for batched quad rendering.
//!
//! Each visible element (background rect, glyph, cursor, underline) becomes
//! one 80-byte instance record in a GPU buffer. [`InstanceWriter`] accumulates
//! these records on the CPU side, then the Render phase uploads the backing
//! `Vec<u8>` to a `wgpu::Buffer` in one copy.
//!
//! The 80-byte layout is designed for a single `VertexBufferLayout` with
//! known offsets — no padding, no alignment surprises. All multi-byte fields
//! are little-endian (matching GPU expectations on all target platforms).

// InstanceWriter is consumed starting in Section 5.8; suppress until then.
#![expect(dead_code, reason = "GPU infrastructure used starting in Section 5.8")]

use oriterm_core::Rgb;

/// Bytes per instance record in the GPU buffer.
pub const INSTANCE_SIZE: usize = 80;

// Field offsets within the 80-byte record.
const OFF_POS_X: usize = 0; //  f32  — pixel X
const OFF_POS_Y: usize = 4; //  f32  — pixel Y
const OFF_SIZE_W: usize = 8; //  f32  — width in pixels
const OFF_SIZE_H: usize = 12; //  f32  — height in pixels
const OFF_UV_X: usize = 16; //  f32  — atlas U left
const OFF_UV_Y: usize = 20; //  f32  — atlas V top
const OFF_UV_W: usize = 24; //  f32  — atlas U width
const OFF_UV_H: usize = 28; //  f32  — atlas V height
const OFF_FG_R: usize = 32; //  f32  — foreground R [0..1]
const OFF_FG_G: usize = 36; //  f32  — foreground G [0..1]
const OFF_FG_B: usize = 40; //  f32  — foreground B [0..1]
const OFF_FG_A: usize = 44; //  f32  — foreground A [0..1]
const OFF_BG_R: usize = 48; //  f32  — background R [0..1]
const OFF_BG_G: usize = 52; //  f32  — background G [0..1]
const OFF_BG_B: usize = 56; //  f32  — background B [0..1]
const OFF_BG_A: usize = 60; //  f32  — background A [0..1]
const OFF_KIND: usize = 64; //  u32  — instance kind (rect/glyph/cursor)
const OFF_PAD1: usize = 68; //  u32  — reserved
const OFF_PAD2: usize = 72; //  u32  — reserved
const OFF_PAD3: usize = 76; //  u32  — reserved

/// Instance kind tag written into the record at offset 64.
///
/// The shader uses this to select between solid-fill (rect/cursor) and
/// texture-sampled (glyph) rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum InstanceKind {
    /// Solid-color background rectangle.
    Rect = 0,
    /// Texture-sampled glyph from the atlas.
    Glyph = 1,
    /// Cursor rectangle (may blend differently).
    Cursor = 2,
}

/// CPU-side accumulator for GPU instance records.
///
/// Maintains a `Vec<u8>` that grows as instances are pushed. The buffer
/// never shrinks — [`clear`](InstanceWriter::clear) resets the length but
/// retains allocated capacity for the next frame.
pub struct InstanceWriter {
    /// Backing byte buffer. Length is always a multiple of [`INSTANCE_SIZE`].
    buf: Vec<u8>,
}

impl InstanceWriter {
    /// Create an empty writer with no pre-allocated capacity.
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Create a writer pre-allocated for `capacity` instances.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity * INSTANCE_SIZE),
        }
    }

    /// Number of instance records currently stored.
    pub fn len(&self) -> usize {
        self.buf.len() / INSTANCE_SIZE
    }

    /// Whether the writer contains zero instances.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Raw byte slice for GPU upload.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Total bytes currently stored.
    pub fn byte_len(&self) -> usize {
        self.buf.len()
    }

    /// Reset to zero instances, retaining allocated memory.
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    /// Push a solid-color rectangle instance.
    ///
    /// UV coordinates are zeroed (no texture sampling for rects).
    pub fn push_rect(&mut self, x: f32, y: f32, w: f32, h: f32, bg: Rgb, alpha: f32) {
        self.push_instance(
            x,
            y,
            w,
            h,
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            rgb_to_floats(bg, alpha),
            InstanceKind::Rect,
        );
    }

    /// Push a texture-sampled glyph instance.
    ///
    /// `uv` is `[u_left, v_top, u_width, v_height]` in atlas texture
    /// coordinates (0..1).
    pub fn push_glyph(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv: [f32; 4],
        fg: Rgb,
        alpha: f32,
    ) {
        self.push_instance(
            x,
            y,
            w,
            h,
            uv,
            rgb_to_floats(fg, alpha),
            [0.0, 0.0, 0.0, 0.0],
            InstanceKind::Glyph,
        );
    }

    /// Push a cursor rectangle instance.
    pub fn push_cursor(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgb, alpha: f32) {
        self.push_instance(
            x,
            y,
            w,
            h,
            [0.0, 0.0, 0.0, 0.0],
            rgb_to_floats(color, alpha),
            [0.0, 0.0, 0.0, 0.0],
            InstanceKind::Cursor,
        );
    }

    /// Push a raw pre-encoded instance record.
    ///
    /// # Panics
    ///
    /// Panics if `bytes.len() != INSTANCE_SIZE`.
    pub fn push_raw(&mut self, bytes: &[u8]) {
        assert_eq!(
            bytes.len(),
            INSTANCE_SIZE,
            "raw instance must be exactly {INSTANCE_SIZE} bytes",
        );
        self.buf.extend_from_slice(bytes);
    }

    /// Encode and append one 80-byte instance record.
    fn push_instance(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv: [f32; 4],
        fg: [f32; 4],
        bg: [f32; 4],
        kind: InstanceKind,
    ) {
        let start = self.buf.len();
        self.buf.resize(start + INSTANCE_SIZE, 0);
        let rec = &mut self.buf[start..];

        write_f32(rec, OFF_POS_X, x);
        write_f32(rec, OFF_POS_Y, y);
        write_f32(rec, OFF_SIZE_W, w);
        write_f32(rec, OFF_SIZE_H, h);

        write_f32(rec, OFF_UV_X, uv[0]);
        write_f32(rec, OFF_UV_Y, uv[1]);
        write_f32(rec, OFF_UV_W, uv[2]);
        write_f32(rec, OFF_UV_H, uv[3]);

        write_f32(rec, OFF_FG_R, fg[0]);
        write_f32(rec, OFF_FG_G, fg[1]);
        write_f32(rec, OFF_FG_B, fg[2]);
        write_f32(rec, OFF_FG_A, fg[3]);

        write_f32(rec, OFF_BG_R, bg[0]);
        write_f32(rec, OFF_BG_G, bg[1]);
        write_f32(rec, OFF_BG_B, bg[2]);
        write_f32(rec, OFF_BG_A, bg[3]);

        write_u32(rec, OFF_KIND, kind as u32);
        // Padding already zeroed by resize.
    }
}

impl Default for InstanceWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert `Rgb` + alpha to `[f32; 4]` in 0..1 range.
fn rgb_to_floats(c: Rgb, a: f32) -> [f32; 4] {
    [
        f32::from(c.r) / 255.0,
        f32::from(c.g) / 255.0,
        f32::from(c.b) / 255.0,
        a,
    ]
}

/// Write a little-endian `f32` at the given byte offset.
fn write_f32(buf: &mut [u8], offset: usize, val: f32) {
    buf[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
}

/// Write a little-endian `u32` at the given byte offset.
fn write_u32(buf: &mut [u8], offset: usize, val: u32) {
    buf[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
}

#[cfg(test)]
mod tests;
