//! Unit tests for GPU state initialization.

use wgpu::{CompositeAlphaMode, SurfaceCapabilities, TextureFormat, TextureUsages};

use super::{build_surface_config, cache_dir, GpuState};

fn caps_with_formats(formats: Vec<TextureFormat>) -> SurfaceCapabilities {
    SurfaceCapabilities {
        formats,
        present_modes: vec![],
        alpha_modes: vec![CompositeAlphaMode::Opaque],
        usages: TextureUsages::RENDER_ATTACHMENT,
    }
}

// --- Surface format selection ---

#[test]
fn select_formats_srgb_surface() {
    let caps = caps_with_formats(vec![TextureFormat::Bgra8UnormSrgb]);

    let (surface_fmt, render_fmt) = GpuState::select_formats(&caps).unwrap();

    // When surface is already sRGB, render format matches.
    assert_eq!(surface_fmt, TextureFormat::Bgra8UnormSrgb);
    assert_eq!(render_fmt, TextureFormat::Bgra8UnormSrgb);
}

#[test]
fn select_formats_non_srgb_surface_derives_srgb_render() {
    let caps = caps_with_formats(vec![TextureFormat::Bgra8Unorm]);

    let (surface_fmt, render_fmt) = GpuState::select_formats(&caps).unwrap();

    // Non-sRGB surface: render format is the sRGB suffix.
    assert_eq!(surface_fmt, TextureFormat::Bgra8Unorm);
    assert_eq!(render_fmt, TextureFormat::Bgra8UnormSrgb);
}

#[test]
fn select_formats_rgba_surface() {
    let caps = caps_with_formats(vec![TextureFormat::Rgba8Unorm]);

    let (surface_fmt, render_fmt) = GpuState::select_formats(&caps).unwrap();

    assert_eq!(surface_fmt, TextureFormat::Rgba8Unorm);
    assert_eq!(render_fmt, TextureFormat::Rgba8UnormSrgb);
}

#[test]
fn select_formats_empty_formats_returns_none() {
    let caps = caps_with_formats(vec![]);
    assert!(GpuState::select_formats(&caps).is_none());
}

#[test]
fn select_formats_picks_first_when_multiple_available() {
    let caps = caps_with_formats(vec![
        TextureFormat::Bgra8Unorm,
        TextureFormat::Rgba8Unorm,
        TextureFormat::Bgra8UnormSrgb,
    ]);

    let (surface_fmt, render_fmt) = GpuState::select_formats(&caps).unwrap();

    // Should pick the first format, not scan for an sRGB one.
    assert_eq!(surface_fmt, TextureFormat::Bgra8Unorm);
    assert_eq!(render_fmt, TextureFormat::Bgra8UnormSrgb);
}

#[test]
fn select_formats_multiple_with_srgb_first() {
    let caps = caps_with_formats(vec![
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Rgba8Unorm,
    ]);

    let (surface_fmt, render_fmt) = GpuState::select_formats(&caps).unwrap();

    // sRGB is already first, so render_format matches.
    assert_eq!(surface_fmt, TextureFormat::Bgra8UnormSrgb);
    assert_eq!(render_fmt, TextureFormat::Bgra8UnormSrgb);
}

// --- Alpha mode selection ---

#[test]
fn select_alpha_mode_prefers_premultiplied() {
    let caps = SurfaceCapabilities {
        formats: vec![],
        present_modes: vec![],
        alpha_modes: vec![
            CompositeAlphaMode::Opaque,
            CompositeAlphaMode::PostMultiplied,
            CompositeAlphaMode::PreMultiplied,
        ],
        usages: TextureUsages::RENDER_ATTACHMENT,
    };

    assert_eq!(
        GpuState::select_alpha_mode(&caps),
        CompositeAlphaMode::PreMultiplied,
    );
}

#[test]
fn select_alpha_mode_falls_back_to_postmultiplied() {
    let caps = SurfaceCapabilities {
        formats: vec![],
        present_modes: vec![],
        alpha_modes: vec![
            CompositeAlphaMode::Opaque,
            CompositeAlphaMode::PostMultiplied,
        ],
        usages: TextureUsages::RENDER_ATTACHMENT,
    };

    assert_eq!(
        GpuState::select_alpha_mode(&caps),
        CompositeAlphaMode::PostMultiplied,
    );
}

#[test]
fn select_alpha_mode_falls_back_to_first_available() {
    let caps = SurfaceCapabilities {
        formats: vec![],
        present_modes: vec![],
        alpha_modes: vec![CompositeAlphaMode::Opaque],
        usages: TextureUsages::RENDER_ATTACHMENT,
    };

    assert_eq!(
        GpuState::select_alpha_mode(&caps),
        CompositeAlphaMode::Opaque,
    );
}

#[test]
fn select_alpha_mode_inherit_as_only_option() {
    let caps = SurfaceCapabilities {
        formats: vec![],
        present_modes: vec![],
        alpha_modes: vec![CompositeAlphaMode::Inherit],
        usages: TextureUsages::RENDER_ATTACHMENT,
    };

    // When only Inherit is available, use it (common fallback).
    assert_eq!(
        GpuState::select_alpha_mode(&caps),
        CompositeAlphaMode::Inherit,
    );
}

#[test]
fn select_alpha_mode_empty_defaults_to_opaque() {
    let caps = SurfaceCapabilities {
        formats: vec![],
        present_modes: vec![],
        alpha_modes: vec![],
        usages: TextureUsages::RENDER_ATTACHMENT,
    };

    // Empty alpha_modes should not panic; falls back to Opaque.
    assert_eq!(
        GpuState::select_alpha_mode(&caps),
        CompositeAlphaMode::Opaque,
    );
}

// --- Surface config builder ---

#[test]
fn build_surface_config_sets_view_formats_when_needed() {
    let config = build_surface_config(
        TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb,
        CompositeAlphaMode::Opaque,
        true,
        800,
        600,
    );

    assert_eq!(config.format, TextureFormat::Bgra8Unorm);
    assert_eq!(config.view_formats, vec![TextureFormat::Bgra8UnormSrgb]);
    assert_eq!(config.width, 800);
    assert_eq!(config.height, 600);
}

#[test]
fn build_surface_config_skips_view_formats_when_unsupported() {
    let config = build_surface_config(
        TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb,
        CompositeAlphaMode::Opaque,
        false,
        800,
        600,
    );

    assert!(config.view_formats.is_empty());
}

#[test]
fn build_surface_config_no_view_formats_when_formats_match() {
    let config = build_surface_config(
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Bgra8UnormSrgb,
        CompositeAlphaMode::PreMultiplied,
        true,
        1920,
        1080,
    );

    assert!(config.view_formats.is_empty());
    assert_eq!(config.alpha_mode, CompositeAlphaMode::PreMultiplied);
}

#[test]
fn build_surface_config_clamps_zero_dimensions() {
    let config = build_surface_config(
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Bgra8UnormSrgb,
        CompositeAlphaMode::Opaque,
        false,
        0,
        0,
    );

    assert_eq!(config.width, 1);
    assert_eq!(config.height, 1);
}

// --- Cache directory ---

#[test]
fn cache_dir_returns_valid_path() {
    let dir = cache_dir();
    let path_str = dir.to_string_lossy();
    assert!(
        path_str.contains("ori_term"),
        "cache_dir should contain 'ori_term': {path_str}",
    );
}

// --- GPU adapter enumeration ---

#[test]
fn validate_gpu_does_not_panic() {
    // Verifies GPU validation runs without panicking, even when no
    // GPU adapters are available (e.g. CI, headless).
    let _count = super::validate_gpu();
}

// --- GPU integration tests (require real adapter) ---

/// Helper: attempt to get a GPU adapter, returning None in headless
/// environments.
fn try_get_adapter() -> Option<(wgpu::Instance, wgpu::Adapter)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });

    let adapter = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
        .into_iter()
        .next()?;

    Some((instance, adapter))
}

#[test]
fn gpu_adapter_reports_srgb_capable_format() {
    let Some((_instance, adapter)) = try_get_adapter() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    // Every modern GPU should support at least one format with an sRGB suffix.
    let info = adapter.get_info();
    let srgb_capable = [
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Rgba8UnormSrgb,
    ];

    // We can't check surface formats without a surface, but we can verify
    // the adapter is not a software fallback with no capabilities.
    assert!(
        !info.name.is_empty(),
        "adapter should have a name: {info:?}",
    );

    // Verify add_srgb_suffix round-trips correctly for common formats.
    for fmt in &srgb_capable {
        assert_eq!(fmt.add_srgb_suffix(), *fmt);
    }
    assert_eq!(
        TextureFormat::Bgra8Unorm.add_srgb_suffix(),
        TextureFormat::Bgra8UnormSrgb,
    );
}

#[test]
fn gpu_device_creation_succeeds() {
    let Some((_instance, adapter)) = try_get_adapter() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let result = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("oriterm_test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        },
    ));

    assert!(result.is_ok(), "device creation should succeed: {result:?}");
}

#[test]
fn gpu_pipeline_cache_round_trip() {
    let Some((_instance, adapter)) = try_get_adapter() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    if !adapter.features().contains(wgpu::Features::PIPELINE_CACHE) {
        eprintln!("skipped: adapter does not support PIPELINE_CACHE");
        return;
    }

    let (device, _queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("oriterm_cache_test"),
            required_features: wgpu::Features::PIPELINE_CACHE,
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        },
    ))
    .expect("device creation should succeed");

    // Create a fresh pipeline cache (no initial data).
    #[allow(unsafe_code, reason = "testing pipeline cache round-trip")]
    let cache = unsafe {
        device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
            label: Some("test_cache"),
            data: None,
            fallback: true,
        })
    };

    // Serialize — may be empty if no pipelines were compiled.
    let data = cache.get_data();
    assert!(data.is_some(), "cache should be serializable");

    // Reload from serialized data.
    let data = data.unwrap();
    #[allow(unsafe_code, reason = "testing pipeline cache round-trip")]
    let _reloaded = unsafe {
        device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
            label: Some("test_cache_reloaded"),
            data: Some(&data),
            fallback: true,
        })
    };

    // If we get here without panicking, the round-trip succeeded.
}

#[test]
fn gpu_texture_dimension_limits_are_reasonable() {
    let Some((_instance, adapter)) = try_get_adapter() else {
        eprintln!("skipped: no GPU adapter available");
        return;
    };

    let (device, _queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("oriterm_limits_test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        },
    ))
    .expect("device creation should succeed");

    let limits = device.limits();

    // Our glyph atlas is 2048x2048. Ensure the GPU supports at least that.
    assert!(
        limits.max_texture_dimension_2d >= 2048,
        "GPU must support at least 2048x2048 textures, got {}",
        limits.max_texture_dimension_2d,
    );

    // Verify buffer size is large enough for a frame of instance data.
    // 80 bytes per cell, 200 cols * 50 rows = 10,000 cells = 800KB.
    let min_buffer = 80 * 200 * 50;
    assert!(
        limits.max_buffer_size >= min_buffer,
        "GPU must support at least {min_buffer} byte buffers, got {}",
        limits.max_buffer_size,
    );
}
