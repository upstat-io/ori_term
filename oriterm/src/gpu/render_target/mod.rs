//! Offscreen render targets for headless rendering and pixel readback.
//!
//! [`RenderTarget`] wraps a wgpu texture with `RENDER_ATTACHMENT | COPY_SRC`
//! usage, matching the GPU's render format so all pipelines work identically
//! on both surfaces and offscreen targets. Used for tab previews, headless
//! test rendering, thumbnails, and visual regression tests.

use std::fmt;

use super::state::GpuState;

/// An offscreen render target backed by a GPU texture.
///
/// Same format as the GPU's `render_format` so pipelines are reusable for
/// both on-screen and off-screen rendering.
#[allow(
    dead_code,
    reason = "used by tests now, production consumers in later sections"
)]
pub struct RenderTarget {
    /// The backing texture (`RENDER_ATTACHMENT | COPY_SRC`).
    texture: wgpu::Texture,
    /// View into the texture for use as a render pass color attachment.
    view: wgpu::TextureView,
    /// Width in pixels.
    width: u32,
    /// Height in pixels.
    height: u32,
}

#[allow(
    dead_code,
    reason = "used by tests now, production consumers in later sections"
)]
impl RenderTarget {
    /// Returns the texture view for use as a render pass attachment.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Returns the width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }
}

impl fmt::Debug for RenderTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderTarget")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

/// Error returned when pixel readback fails.
#[derive(Debug)]
#[allow(
    dead_code,
    reason = "used by tests now, production consumers in later sections"
)]
pub enum ReadbackError {
    /// GPU device polling failed.
    Poll(wgpu::PollError),
    /// Readback channel disconnected before receiving result.
    Channel(std::sync::mpsc::RecvError),
    /// Async buffer mapping failed.
    MapAsync(wgpu::BufferAsyncError),
}

impl fmt::Display for ReadbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Poll(e) => write!(f, "GPU poll failed: {e}"),
            Self::Channel(e) => write!(f, "readback channel error: {e}"),
            Self::MapAsync(e) => write!(f, "buffer map failed: {e}"),
        }
    }
}

impl std::error::Error for ReadbackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Poll(e) => Some(e),
            Self::Channel(e) => Some(e),
            Self::MapAsync(e) => Some(e),
        }
    }
}

impl GpuState {
    /// Create an offscreen render target with the given dimensions.
    ///
    /// The texture uses the same `render_format` as surfaces so pipelines
    /// work identically. Includes `COPY_SRC` usage for pixel readback.
    #[allow(
        dead_code,
        reason = "used by tests now, production consumers in later sections"
    )]
    pub fn create_render_target(&self, width: u32, height: u32) -> RenderTarget {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_render_target"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.render_format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        RenderTarget {
            texture,
            view,
            width: width.max(1),
            height: height.max(1),
        }
    }

    /// Read RGBA pixels from a render target back to the CPU.
    ///
    /// Returns a `Vec<u8>` of tightly-packed RGBA bytes (`4 * width * height`).
    /// Handles wgpu's row alignment requirements (256-byte `COPY_BYTES_PER_ROW_ALIGNMENT`)
    /// by stripping padding rows when needed.
    ///
    /// Blocks until the GPU copy completes — intended for testing and offline
    /// use, not real-time rendering.
    #[allow(
        dead_code,
        reason = "used by tests now, production consumers in later sections"
    )]
    pub fn read_render_target(&self, target: &RenderTarget) -> Result<Vec<u8>, ReadbackError> {
        let bytes_per_pixel = 4u32; // RGBA8
        let unpadded_row = target.width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_row = unpadded_row.div_ceil(align) * align;
        let buffer_size = u64::from(padded_row) * u64::from(target.height);

        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render_target_readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback_encoder"),
            });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(target.height),
                },
            },
            wgpu::Extent3d {
                width: target.width,
                height: target.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(ReadbackError::Poll)?;

        rx.recv()
            .map_err(ReadbackError::Channel)?
            .map_err(ReadbackError::MapAsync)?;

        let data = slice.get_mapped_range();
        let pixels = strip_row_padding(&data, target.width, target.height, padded_row);
        drop(data);
        staging.unmap();

        Ok(pixels)
    }
}

/// Strip row padding from GPU readback data, returning tightly-packed RGBA.
fn strip_row_padding(data: &[u8], width: u32, height: u32, padded_row: u32) -> Vec<u8> {
    let bytes_per_pixel = 4u32;
    let unpadded_row = width * bytes_per_pixel;

    if padded_row == unpadded_row {
        return data[..(unpadded_row as usize * height as usize)].to_vec();
    }

    let mut out = Vec::with_capacity(unpadded_row as usize * height as usize);
    for row in 0..height {
        let start = (row * padded_row) as usize;
        let end = start + unpadded_row as usize;
        out.extend_from_slice(&data[start..end]);
    }
    out
}

#[cfg(test)]
mod tests;
