//! Sixel graphics decoder.
//!
//! Streaming sixel parser that processes DCS data byte-by-byte and produces
//! an RGBA pixel buffer. Each sixel character encodes 6 vertical pixels
//! (one column × 6 rows). The parser handles color palette definitions
//! (RGB and HLS), repeat operators, raster attributes, and newlines.
//!
//! Reference: VT340 Sixel Graphics spec, `WezTerm` `sixel.rs`.

mod color;

use crate::image::ImageError;

use color::{VT340_PALETTE, hls_to_rgb};

/// Maximum dimensions to prevent OOM from malicious input.
const MAX_DIMENSION: usize = 10_000;

/// Maximum pixel buffer size (100 MB) — reject sixel images exceeding this.
const MAX_PIXEL_BYTES: usize = 100_000_000;

/// Sixel background mode (from DCS P2 parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SixelBgMode {
    /// P2=0: use device default background.
    DeviceDefault,
    /// P2=1: undrawn pixels are transparent (alpha=0).
    NoChange,
    /// P2=2: fill undrawn pixels with terminal background color.
    SetToBg,
}

/// Streaming sixel parser state machine.
///
/// Feed bytes via `feed()` one at a time. When the DCS sequence ends,
/// call `finish()` to get the final RGBA pixel buffer and dimensions.
#[derive(Debug)]
pub struct SixelParser {
    /// Current image width in pixels (grows as data arrives).
    width: usize,
    /// Current image height in pixels (grows as data arrives).
    height: usize,
    /// RGBA pixel buffer (grows dynamically by doubling).
    pixels: Vec<u8>,
    /// Color palette: up to 256 entries (RGB).
    palette: Vec<[u8; 3]>,
    /// Currently selected palette color index.
    current_color: u16,
    /// Current drawing X position (pixel column).
    x: usize,
    /// Current drawing Y position (pixel row, top of sixel band).
    y: usize,
    /// Background mode (from P2).
    bg_mode: SixelBgMode,
    /// Explicit width from raster attributes (if provided).
    raster_width: Option<usize>,
    /// Explicit height from raster attributes (if provided).
    raster_height: Option<usize>,
    /// Max X seen so far (tracks actual width).
    max_x: usize,
    /// Max Y seen so far (tracks actual height).
    max_y: usize,
    /// Whether the parser has been aborted (oversized image).
    aborted: bool,

    // Command accumulation state.
    /// Active command character (0 = none, `!` `#` `"` etc.).
    cmd: u8,
    /// Parameter accumulator (up to 5 params).
    params: [u32; 5],
    /// Current parameter index.
    param_idx: usize,
}

impl SixelParser {
    /// Create a new sixel parser from DCS parameters.
    ///
    /// `params` correspond to P1 (aspect ratio), P2 (background select),
    /// P3 (horizontal grid size — ignored).
    pub fn new(params: &[u16]) -> Self {
        let bg_mode = match params.get(1).copied().unwrap_or(0) {
            1 => SixelBgMode::NoChange,
            2 => SixelBgMode::SetToBg,
            _ => SixelBgMode::DeviceDefault,
        };

        // Initialize palette with VT340 defaults.
        let mut palette = vec![[0u8; 3]; 256];
        for (i, &color) in VT340_PALETTE.iter().enumerate() {
            palette[i] = color;
        }

        Self {
            width: 0,
            height: 0,
            pixels: Vec::new(),
            palette,
            current_color: 0,
            x: 0,
            y: 0,
            bg_mode,
            raster_width: None,
            raster_height: None,
            max_x: 0,
            max_y: 0,
            aborted: false,
            cmd: 0,
            params: [0; 5],
            param_idx: 0,
        }
    }

    /// Process one byte of sixel data. No allocation per byte —
    /// pixel buffer grows by doubling, palette mutations are in-place.
    pub fn feed(&mut self, byte: u8) {
        if self.aborted {
            return;
        }

        match byte {
            // Sixel data characters: 0x3F–0x7E.
            0x3F..=0x7E => {
                let value = byte - 0x3F;
                if self.cmd == b'!' {
                    // Repeat operator: `!count<char>`.
                    let count = (self.params[0] as usize).max(1);
                    self.cmd = 0;
                    self.emit_sixel(value, count);
                } else {
                    if self.cmd != 0 {
                        self.finish_command();
                    }
                    self.emit_sixel(value, 1);
                }
            }
            // Digit — accumulate parameter.
            b'0'..=b'9' => {
                if self.cmd != 0 && self.param_idx < 5 {
                    self.params[self.param_idx] = self.params[self.param_idx]
                        .saturating_mul(10)
                        .saturating_add(u32::from(byte - b'0'));
                }
            }
            // Semicolon — advance to next parameter.
            b';' => {
                if self.cmd != 0 && self.param_idx < 4 {
                    self.param_idx += 1;
                }
            }
            // Carriage return — reset x to left margin.
            b'$' => {
                if self.cmd != 0 {
                    self.finish_command();
                }
                self.x = 0;
            }
            // New line — move down 6 pixels, reset x.
            b'-' => {
                if self.cmd != 0 {
                    self.finish_command();
                }
                self.y += 6;
                self.x = 0;
            }
            // Start repeat command.
            b'!' => {
                if self.cmd != 0 {
                    self.finish_command();
                }
                self.cmd = b'!';
                self.params = [0; 5];
                self.param_idx = 0;
            }
            // Start color command.
            b'#' => {
                if self.cmd != 0 {
                    self.finish_command();
                }
                self.cmd = b'#';
                self.params = [0; 5];
                self.param_idx = 0;
            }
            // Start raster attributes command.
            b'"' => {
                if self.cmd != 0 {
                    self.finish_command();
                }
                self.cmd = b'"';
                self.params = [0; 5];
                self.param_idx = 0;
            }
            // Other bytes: ignore (per VT340 spec, unknown bytes are discarded).
            _ => {}
        }
    }

    /// Finalize the parser and return the RGBA pixel buffer with dimensions.
    pub fn finish(mut self) -> Result<(Vec<u8>, u32, u32), ImageError> {
        if self.cmd != 0 {
            self.finish_command();
        }

        if self.aborted {
            return Err(ImageError::OversizedImage);
        }

        // Use raster attributes dimensions if provided, else computed.
        let w = self.raster_width.unwrap_or(self.max_x).max(self.max_x);
        let h = self.raster_height.unwrap_or(self.max_y).max(self.max_y);

        if w == 0 || h == 0 {
            return Err(ImageError::DecodeFailed(
                "sixel image has zero dimensions".into(),
            ));
        }

        // Build the final RGBA buffer at the correct dimensions.
        let total = w.checked_mul(h).and_then(|n| n.checked_mul(4));
        let Some(total) = total else {
            return Err(ImageError::OversizedImage);
        };
        if total > MAX_PIXEL_BYTES {
            return Err(ImageError::OversizedImage);
        }

        // Background fill for undrawn pixels depends on bg_mode:
        // NoChange: transparent (alpha=0), DeviceDefault/SetToBg: opaque black.
        let bg = match self.bg_mode {
            SixelBgMode::NoChange => [0u8, 0, 0, 0],
            _ => [0u8, 0, 0, 255],
        };

        let mut result = vec![0u8; total];
        for row in 0..h {
            for col in 0..w {
                let dst = (row * w + col) * 4;
                let src = self.pixel_offset(col, row);
                let mut drawn = false;
                if let Some(src) = src {
                    if src + 3 < self.pixels.len() && self.pixels[src + 3] != 0 {
                        result[dst] = self.pixels[src];
                        result[dst + 1] = self.pixels[src + 1];
                        result[dst + 2] = self.pixels[src + 2];
                        result[dst + 3] = self.pixels[src + 3];
                        drawn = true;
                    }
                }
                if !drawn {
                    result[dst] = bg[0];
                    result[dst + 1] = bg[1];
                    result[dst + 2] = bg[2];
                    result[dst + 3] = bg[3];
                }
            }
        }

        Ok((result, w as u32, h as u32))
    }

    /// Complete a pending command (`#` or `"`).
    ///
    /// Repeat (`!`) is handled directly in `feed()` when the sixel data
    /// byte arrives — it never reaches `finish_command`.
    fn finish_command(&mut self) {
        let cmd = self.cmd;
        self.cmd = 0;

        match cmd {
            b'#' => self.apply_color(),
            b'"' => self.apply_raster_attrs(),
            _ => {}
        }
    }

    /// Apply color command.
    ///
    /// `#<n>` selects color `n`.
    /// `#<n>;2;<r>;<g>;<b>` defines RGB color (0-100 range, scale to 0-255).
    /// `#<n>;1;<h>;<l>;<s>` defines HLS color.
    fn apply_color(&mut self) {
        let idx = self.params[0] as u16;

        if self.param_idx >= 2 {
            // Color definition.
            let system = self.params[1];
            match system {
                2 => {
                    // RGB: values in 0-100 range.
                    let r = (self.params[2].min(100) * 255 / 100) as u8;
                    let g = (self.params[3].min(100) * 255 / 100) as u8;
                    let b = (self.params[4].min(100) * 255 / 100) as u8;
                    if (idx as usize) < self.palette.len() {
                        self.palette[idx as usize] = [r, g, b];
                    }
                }
                1 => {
                    // HLS: H=0-360, L=0-100, S=0-100.
                    let hue = self.params[2].min(360);
                    let lightness = self.params[3].min(100);
                    let saturation = self.params[4].min(100);
                    let rgb = hls_to_rgb(hue, lightness, saturation);
                    if (idx as usize) < self.palette.len() {
                        self.palette[idx as usize] = rgb;
                    }
                }
                _ => {} // Unknown color system — ignore.
            }
        }

        // Select color.
        self.current_color = idx;
    }

    /// Apply raster attributes: `"<pan>;<pad>;<width>;<height>`.
    fn apply_raster_attrs(&mut self) {
        // pan and pad (aspect ratio) — informational, not used for rendering.
        if self.param_idx >= 3 {
            let w = self.params[2] as usize;
            let h = self.params[3] as usize;

            if w > MAX_DIMENSION || h > MAX_DIMENSION {
                self.aborted = true;
                return;
            }
            let total = w.checked_mul(h).and_then(|n| n.checked_mul(4));
            if total.is_none_or(|t| t > MAX_PIXEL_BYTES) {
                self.aborted = true;
                return;
            }

            self.raster_width = Some(w);
            self.raster_height = Some(h);
        }
    }

    /// Draw a sixel value at the current position, optionally repeated.
    ///
    /// Each sixel character encodes 6 vertical pixels (LSB = top).
    fn emit_sixel(&mut self, value: u8, count: usize) {
        let count = count.min(MAX_DIMENSION);
        let color_idx = self.current_color as usize;
        let rgb = if color_idx < self.palette.len() {
            self.palette[color_idx]
        } else {
            [255, 255, 255] // Fallback: white.
        };

        for _ in 0..count {
            if self.x >= MAX_DIMENSION {
                break;
            }

            for bit in 0..6u8 {
                if value & (1 << bit) != 0 {
                    let px = self.x;
                    let py = self.y + bit as usize;
                    self.set_pixel(px, py, rgb);
                }
            }

            self.x += 1;
            self.max_x = self.max_x.max(self.x);
        }
        self.max_y = self.max_y.max(self.y + 6);
    }

    /// Set a single pixel in the buffer, growing it as needed.
    fn set_pixel(&mut self, x: usize, y: usize, rgb: [u8; 3]) {
        let need_w = x + 1;
        let need_h = y + 1;

        if need_w > self.width || need_h > self.height {
            self.grow_buffer(need_w, need_h);
        }

        if self.aborted {
            return;
        }

        let offset = (y * self.width + x) * 4;
        if offset + 3 < self.pixels.len() {
            self.pixels[offset] = rgb[0];
            self.pixels[offset + 1] = rgb[1];
            self.pixels[offset + 2] = rgb[2];
            self.pixels[offset + 3] = 255;
        }
    }

    /// Grow the internal pixel buffer to fit at least `w` × `h`.
    fn grow_buffer(&mut self, need_w: usize, need_h: usize) {
        let new_w = if need_w > self.width {
            need_w.next_power_of_two().min(MAX_DIMENSION)
        } else {
            self.width
        };
        let new_h = if need_h > self.height {
            need_h.next_power_of_two().min(MAX_DIMENSION)
        } else {
            self.height
        };

        let new_total = new_w.checked_mul(new_h).and_then(|n| n.checked_mul(4));
        let Some(new_total) = new_total else {
            self.aborted = true;
            return;
        };
        if new_total > MAX_PIXEL_BYTES {
            self.aborted = true;
            return;
        }

        // Allocate new buffer and copy existing rows.
        let mut new_pixels = vec![0u8; new_total];
        for row in 0..self.height {
            let old_start = row * self.width * 4;
            let old_end = old_start + self.width * 4;
            let new_start = row * new_w * 4;
            if old_end <= self.pixels.len() {
                let new_end = new_start + self.width * 4;
                new_pixels[new_start..new_end].copy_from_slice(&self.pixels[old_start..old_end]);
            }
        }

        self.pixels = new_pixels;
        self.width = new_w;
        self.height = new_h;
    }

    /// Get the byte offset for pixel (x, y) in the internal buffer.
    fn pixel_offset(&self, x: usize, y: usize) -> Option<usize> {
        if x < self.width && y < self.height {
            Some((y * self.width + x) * 4)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests;
