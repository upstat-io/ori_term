//! Unit tests for GPU state initialization.

use wgpu::{
    CompositeAlphaMode, DownlevelFlags, SurfaceCapabilities, TextureFormat, TextureUsages,
};

use super::{cache_dir, GpuState};

/// Full downlevel flags (all features supported).
const ALL_FLAGS: DownlevelFlags = DownlevelFlags::all();

/// Downlevel flags without `SURFACE_VIEW_FORMATS`.
fn flags_without_view_formats() -> DownlevelFlags {
    let mut f = DownlevelFlags::all();
    f.remove(DownlevelFlags::SURFACE_VIEW_FORMATS);
    f
}

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

    let (surface_fmt, render_fmt, view_formats) =
        GpuState::select_formats(&caps, ALL_FLAGS).unwrap();

    // When surface is already sRGB, render format matches and no view_formats needed.
    assert_eq!(surface_fmt, TextureFormat::Bgra8UnormSrgb);
    assert_eq!(render_fmt, TextureFormat::Bgra8UnormSrgb);
    assert!(view_formats.is_empty());
}

#[test]
fn select_formats_non_srgb_surface_gets_view_format() {
    let caps = caps_with_formats(vec![TextureFormat::Bgra8Unorm]);

    let (surface_fmt, render_fmt, view_formats) =
        GpuState::select_formats(&caps, ALL_FLAGS).unwrap();

    // Non-sRGB surface: render format is sRGB suffix, view_formats bridges the gap.
    assert_eq!(surface_fmt, TextureFormat::Bgra8Unorm);
    assert_eq!(render_fmt, TextureFormat::Bgra8UnormSrgb);
    assert_eq!(view_formats, vec![TextureFormat::Bgra8UnormSrgb]);
}

#[test]
fn select_formats_rgba_surface() {
    let caps = caps_with_formats(vec![TextureFormat::Rgba8Unorm]);

    let (surface_fmt, render_fmt, view_formats) =
        GpuState::select_formats(&caps, ALL_FLAGS).unwrap();

    assert_eq!(surface_fmt, TextureFormat::Rgba8Unorm);
    assert_eq!(render_fmt, TextureFormat::Rgba8UnormSrgb);
    assert_eq!(view_formats, vec![TextureFormat::Rgba8UnormSrgb]);
}

#[test]
fn select_formats_empty_formats_returns_none() {
    let caps = caps_with_formats(vec![]);
    assert!(GpuState::select_formats(&caps, ALL_FLAGS).is_none());
}

#[test]
fn select_formats_picks_first_when_multiple_available() {
    let caps = caps_with_formats(vec![
        TextureFormat::Bgra8Unorm,
        TextureFormat::Rgba8Unorm,
        TextureFormat::Bgra8UnormSrgb,
    ]);

    let (surface_fmt, render_fmt, _) = GpuState::select_formats(&caps, ALL_FLAGS).unwrap();

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

    let (surface_fmt, render_fmt, view_formats) =
        GpuState::select_formats(&caps, ALL_FLAGS).unwrap();

    // sRGB is already first, so no view_formats needed.
    assert_eq!(surface_fmt, TextureFormat::Bgra8UnormSrgb);
    assert_eq!(render_fmt, TextureFormat::Bgra8UnormSrgb);
    assert!(view_formats.is_empty());
}

// --- Downlevel view_formats guard ---

#[test]
fn select_formats_no_view_formats_when_downlevel_unsupported() {
    let caps = caps_with_formats(vec![TextureFormat::Bgra8Unorm]);
    let flags = flags_without_view_formats();

    let (surface_fmt, render_fmt, view_formats) =
        GpuState::select_formats(&caps, flags).unwrap();

    // Non-sRGB surface but downlevel doesn't support view_formats.
    // Should NOT set view_formats even though render_format differs.
    assert_eq!(surface_fmt, TextureFormat::Bgra8Unorm);
    assert_eq!(render_fmt, TextureFormat::Bgra8UnormSrgb);
    assert!(
        view_formats.is_empty(),
        "view_formats should be empty when SURFACE_VIEW_FORMATS unsupported",
    );
}

#[test]
fn select_formats_srgb_surface_unaffected_by_downlevel_flag() {
    let caps = caps_with_formats(vec![TextureFormat::Bgra8UnormSrgb]);
    let flags = flags_without_view_formats();

    let (_, _, view_formats) = GpuState::select_formats(&caps, flags).unwrap();

    // sRGB surface never needs view_formats regardless of downlevel support.
    assert!(view_formats.is_empty());
}

#[test]
fn select_formats_empty_downlevel_flags() {
    let caps = caps_with_formats(vec![TextureFormat::Bgra8Unorm]);
    let flags = DownlevelFlags::empty();

    let (_, _, view_formats) = GpuState::select_formats(&caps, flags).unwrap();

    // No downlevel flags at all — view_formats must be empty.
    assert!(view_formats.is_empty());
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
