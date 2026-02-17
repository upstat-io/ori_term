//! Tests for offscreen render targets and pixel readback.

use super::strip_row_padding;
use crate::gpu::state::GpuState;

// --- strip_row_padding ---

#[test]
fn strip_padding_no_padding_needed() {
    // 4px wide = 16 bytes per row, aligned to 256 = 256 bytes padded row.
    // But if padded_row == unpadded_row (e.g. 64px = 256 bytes), no stripping.
    let width = 64u32; // 64 * 4 = 256 bytes = exactly aligned
    let height = 2u32;
    let padded_row = 256u32;

    let mut data = vec![0xAA; padded_row as usize * height as usize];
    // Mark each row distinctly.
    data[0] = 0x11;
    data[padded_row as usize] = 0x22;

    let result = strip_row_padding(&data, width, height, padded_row);
    assert_eq!(result.len(), 256 * 2);
    assert_eq!(result[0], 0x11);
    assert_eq!(result[256], 0x22);
}

#[test]
fn strip_padding_removes_trailing_bytes() {
    // 2px wide = 8 bytes per row. Padded to 256 bytes.
    let width = 2u32;
    let height = 2u32;
    let padded_row = 256u32;
    let unpadded_row = 8u32;

    let mut data = vec![0x00; padded_row as usize * height as usize];
    // Fill meaningful data at start of each row.
    for i in 0..unpadded_row as usize {
        data[i] = 0xAA;
        data[padded_row as usize + i] = 0xBB;
    }

    let result = strip_row_padding(&data, width, height, padded_row);
    assert_eq!(result.len(), unpadded_row as usize * height as usize);
    assert!(result[..8].iter().all(|&b| b == 0xAA));
    assert!(result[8..16].iter().all(|&b| b == 0xBB));
}

#[test]
fn strip_padding_single_pixel() {
    let width = 1u32;
    let height = 1u32;
    let padded_row = 256u32;

    let mut data = vec![0x00; padded_row as usize];
    data[0] = 0xFF;
    data[1] = 0x80;
    data[2] = 0x40;
    data[3] = 0x20;

    let result = strip_row_padding(&data, width, height, padded_row);
    assert_eq!(result, vec![0xFF, 0x80, 0x40, 0x20]);
}

// --- GPU integration tests (require adapter) ---

#[test]
fn create_render_target_succeeds() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let target = gpu.create_render_target(640, 480);
    assert_eq!(target.width(), 640);
    assert_eq!(target.height(), 480);
}

#[test]
fn create_render_target_clamps_zero_to_one() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let target = gpu.create_render_target(0, 0);
    assert_eq!(target.width(), 1);
    assert_eq!(target.height(), 1);
}

#[test]
fn read_render_target_returns_correct_size() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let target = gpu.create_render_target(16, 16);
    let pixels = gpu
        .read_render_target(&target)
        .expect("readback should succeed");

    // 16 * 16 * 4 (RGBA) = 1024 bytes.
    assert_eq!(pixels.len(), 16 * 16 * 4);
}

#[test]
fn read_render_target_non_aligned_width() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    // 10px wide = 40 bytes per row, not aligned to 256.
    let target = gpu.create_render_target(10, 8);
    let pixels = gpu
        .read_render_target(&target)
        .expect("readback should succeed");

    assert_eq!(pixels.len(), 10 * 8 * 4);
}

#[test]
fn read_render_target_single_pixel() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let target = gpu.create_render_target(1, 1);
    let pixels = gpu
        .read_render_target(&target)
        .expect("readback should succeed");

    // 1 pixel = 4 bytes (RGBA).
    assert_eq!(pixels.len(), 4);
}

#[test]
fn render_target_debug_format() {
    let Ok(gpu) = GpuState::new_headless() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let target = gpu.create_render_target(320, 240);
    let debug = format!("{target:?}");
    assert!(debug.contains("320"));
    assert!(debug.contains("240"));
}
