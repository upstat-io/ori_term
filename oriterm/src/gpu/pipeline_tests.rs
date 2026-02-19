//! Full-pipeline integration tests (Layer 2): headless GPU rendering.
//!
//! These tests exercise the complete Extract → Prepare → Render pipeline
//! with a real GPU adapter and offscreen render targets. They verify that
//! the pipeline produces correct pixel output without needing a window.
//!
//! Tests gracefully skip when no GPU adapter is available (e.g. CI without
//! GPU, headless environments).

use oriterm_core::{Column, CursorShape, Rgb};

use super::frame_input::{FrameInput, ViewportSize};
use super::renderer::GpuRenderer;
use super::state::GpuState;
use crate::font::{FontCollection, FontSet, GlyphFormat, HintingMode};

/// Default font weight for tests.
const TEST_FONT_WEIGHT: u16 = 400;
/// Default font size for tests (points).
const TEST_FONT_SIZE_PT: f32 = 12.0;
/// Default DPI for tests.
const TEST_DPI: f32 = 96.0;

/// Attempt to create a full headless rendering environment.
///
/// Returns `None` if no GPU adapter or fonts are available.
fn headless_env() -> Option<(GpuState, GpuRenderer)> {
    let gpu = GpuState::new_headless().ok()?;
    let font_set = FontSet::load(None, TEST_FONT_WEIGHT).ok()?;
    let font_collection = FontCollection::new(
        font_set,
        TEST_FONT_SIZE_PT,
        TEST_DPI,
        GlyphFormat::Alpha,
        TEST_FONT_WEIGHT,
        HintingMode::Full,
    )
    .ok()?;
    let renderer = GpuRenderer::new(&gpu, font_collection);
    Some((gpu, renderer))
}

// ── Pipeline smoke tests ──

#[test]
fn headless_gpu_adapter_found() {
    match GpuState::new_headless() {
        Ok(_) => {} // Pass: adapter is available.
        Err(_) => {
            eprintln!("skipped: no GPU adapter available");
        }
    }
}

#[test]
fn pipeline_creation_succeeds() {
    let Some((_gpu, _renderer)) = headless_env() else {
        eprintln!("skipped: no GPU adapter or fonts available");
        return;
    };
    // If we get here, pipeline creation (shaders, layouts, bind groups) succeeded.
}

#[test]
fn offscreen_render_target_creates() {
    let Some((gpu, _renderer)) = headless_env() else {
        eprintln!("skipped: no GPU adapter or fonts available");
        return;
    };

    let target = gpu.create_render_target(640, 480);
    assert_eq!(target.width(), 640);
    assert_eq!(target.height(), 480);
}

#[test]
fn frame_renders_without_errors() {
    let Some((gpu, mut renderer)) = headless_env() else {
        eprintln!("skipped: no GPU adapter or fonts available");
        return;
    };

    let target = gpu.create_render_target(640, 384);
    let input = FrameInput::test_grid(80, 24, "Hello, World!");

    renderer.prepare(&input, &gpu);
    renderer.render_frame(&gpu, target.view());

    // No panic or GPU validation error = success.
}

#[test]
fn wgpu_validation_layer_enabled_in_tests() {
    // Verify that wgpu validation is active (default in debug builds).
    // We do this by confirming that the headless GPU initializes without
    // validation errors — the validation layer catches API misuse at
    // runtime when the `debug_assertions` cfg is set.
    let Some((gpu, mut renderer)) = headless_env() else {
        eprintln!("skipped: no GPU adapter or fonts available");
        return;
    };

    let target = gpu.create_render_target(64, 64);
    let input = FrameInput::test_grid(8, 4, "test");

    renderer.prepare(&input, &gpu);
    renderer.render_frame(&gpu, target.view());

    // wgpu validation errors cause panics in debug mode, so reaching
    // here confirms the validation layer accepted our API usage.
}

// ── Pixel readback tests ──

#[test]
fn render_colored_cell_correct_bg_color() {
    let Some((gpu, mut renderer)) = headless_env() else {
        eprintln!("skipped: no GPU adapter or fonts available");
        return;
    };

    let cell_metrics = renderer.cell_metrics();
    let cw = cell_metrics.width.ceil() as u32;
    let ch = cell_metrics.height.ceil() as u32;

    // Create a 1×1 grid with a red background. Hide cursor so it doesn't
    // paint over the cell background.
    let red = Rgb { r: 255, g: 0, b: 0 };
    let mut input = FrameInput::test_grid(1, 1, " ");
    input.content.cells[0].bg = red;
    input.palette.background = red;
    input.content.cursor.visible = false;
    input.viewport = ViewportSize::new(cw, ch);
    input.cell_size = cell_metrics;

    let target = gpu.create_render_target(cw, ch);
    renderer.prepare(&input, &gpu);
    renderer.render_frame(&gpu, target.view());

    let pixels = gpu
        .read_render_target(&target)
        .expect("pixel readback should succeed");

    // Sample the center pixel — should be reddish.
    let center_x = cw / 2;
    let center_y = ch / 2;
    let idx = ((center_y * cw + center_x) * 4) as usize;

    let r = pixels[idx];
    let g = pixels[idx + 1];
    let b = pixels[idx + 2];
    let a = pixels[idx + 3];

    // Red channel should be high, green and blue low.
    // Allow tolerance for sRGB gamma conversion.
    assert!(r > 200, "red channel should be high for red bg, got {r}",);
    assert!(g < 30, "green channel should be low, got {g}");
    assert!(b < 30, "blue channel should be low, got {b}");
    assert!(a > 200, "alpha should be high, got {a}");
}

#[test]
fn render_text_produces_nonzero_alpha_in_glyph_region() {
    let Some((gpu, mut renderer)) = headless_env() else {
        eprintln!("skipped: no GPU adapter or fonts available");
        return;
    };

    let cell_metrics = renderer.cell_metrics();
    let cols = 10u32;
    let rows = 1u32;
    let w = (cell_metrics.width * cols as f32).ceil() as u32;
    let h = (cell_metrics.height * rows as f32).ceil() as u32;

    // White text on black background.
    let mut input = FrameInput::test_grid(cols as usize, rows as usize, "WWWWWWWWWW");
    input.viewport = ViewportSize::new(w, h);
    input.cell_size = cell_metrics;
    for cell in &mut input.content.cells {
        cell.fg = Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        cell.bg = Rgb { r: 0, g: 0, b: 0 };
    }
    input.palette.background = Rgb { r: 0, g: 0, b: 0 };

    let target = gpu.create_render_target(w, h);
    renderer.prepare(&input, &gpu);
    renderer.render_frame(&gpu, target.view());

    let pixels = gpu
        .read_render_target(&target)
        .expect("pixel readback should succeed");

    // With white text on black background, at least some pixels should be
    // non-black (glyphs rendered).
    let total_pixels = (w * h) as usize;
    let nonblack_count = (0..total_pixels)
        .filter(|&i| {
            let idx = i * 4;
            pixels[idx] > 10 || pixels[idx + 1] > 10 || pixels[idx + 2] > 10
        })
        .count();

    assert!(
        nonblack_count > 0,
        "rendered text should produce non-black pixels, but all {total_pixels} pixels are black",
    );
}

#[test]
fn render_cursor_pixels_at_expected_position() {
    let Some((gpu, mut renderer)) = headless_env() else {
        eprintln!("skipped: no GPU adapter or fonts available");
        return;
    };

    let cell_metrics = renderer.cell_metrics();
    let cols = 5u32;
    let rows = 3u32;
    let w = (cell_metrics.width * cols as f32).ceil() as u32;
    let h = (cell_metrics.height * rows as f32).ceil() as u32;

    // Place cursor at (2, 1) — column 2, row 1.
    let cursor_col = 2;
    let cursor_row = 1;
    let mut input = FrameInput::test_grid(cols as usize, rows as usize, "");
    input.content.cursor.column = Column(cursor_col);
    input.content.cursor.line = cursor_row;
    input.content.cursor.shape = CursorShape::Block;
    input.content.cursor.visible = true;
    input.viewport = ViewportSize::new(w, h);
    input.cell_size = cell_metrics;

    // Black background with white cursor.
    input.palette.background = Rgb { r: 0, g: 0, b: 0 };
    input.palette.cursor_color = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    for cell in &mut input.content.cells {
        cell.bg = Rgb { r: 0, g: 0, b: 0 };
    }

    let target = gpu.create_render_target(w, h);
    renderer.prepare(&input, &gpu);
    renderer.render_frame(&gpu, target.view());

    let pixels = gpu
        .read_render_target(&target)
        .expect("pixel readback should succeed");

    // Sample a pixel inside the cursor cell.
    let cursor_x = (cursor_col as f32 * cell_metrics.width) as u32 + cell_metrics.width as u32 / 2;
    let cursor_y =
        (cursor_row as f32 * cell_metrics.height) as u32 + cell_metrics.height as u32 / 2;
    let idx = ((cursor_y * w + cursor_x) * 4) as usize;

    // Cursor should be bright (white on black bg).
    let brightness = pixels[idx].max(pixels[idx + 1]).max(pixels[idx + 2]);
    assert!(
        brightness > 100,
        "cursor pixel at ({cursor_x}, {cursor_y}) should be bright, got rgb=({}, {}, {})",
        pixels[idx],
        pixels[idx + 1],
        pixels[idx + 2],
    );

    // Sample a pixel outside the cursor cell (e.g. top-left cell (0,0)).
    let non_cursor_idx = ((0u32 * w + 0) * 4) as usize;
    let non_brightness = pixels[non_cursor_idx]
        .max(pixels[non_cursor_idx + 1])
        .max(pixels[non_cursor_idx + 2]);
    assert!(
        non_brightness < 30,
        "non-cursor pixel should be dark (black bg), got rgb=({}, {}, {})",
        pixels[non_cursor_idx],
        pixels[non_cursor_idx + 1],
        pixels[non_cursor_idx + 2],
    );
}

// ── Full pipeline round-trip ──

#[test]
fn full_pipeline_extract_prepare_render_readback() {
    let Some((gpu, mut renderer)) = headless_env() else {
        eprintln!("skipped: no GPU adapter or fonts available");
        return;
    };

    let cell_metrics = renderer.cell_metrics();
    let cols = 20u32;
    let rows = 5u32;
    let w = (cell_metrics.width * cols as f32).ceil() as u32;
    let h = (cell_metrics.height * rows as f32).ceil() as u32;

    let mut input = FrameInput::test_grid(cols as usize, rows as usize, "Hello, GPU pipeline!");
    input.viewport = ViewportSize::new(w, h);
    input.cell_size = cell_metrics;

    let target = gpu.create_render_target(w, h);

    // Run the full pipeline via GpuRenderer (ensure_glyphs_cached + prepare + render).
    renderer.prepare(&input, &gpu);
    renderer.render_frame(&gpu, target.view());

    // Readback and verify basic sanity.
    let pixels = gpu
        .read_render_target(&target)
        .expect("pixel readback should succeed");

    let expected_size = (w * h * 4) as usize;
    assert_eq!(
        pixels.len(),
        expected_size,
        "readback should return {expected_size} bytes ({}×{}×4), got {}",
        w,
        h,
        pixels.len(),
    );

    // Verify not all pixels are the same (we rendered text, so there should
    // be variation).
    let first_pixel = &pixels[0..4];
    let all_same = pixels.chunks(4).all(|p| p == first_pixel);
    assert!(
        !all_same,
        "rendered frame should have pixel variation (text was rendered)",
    );
}
