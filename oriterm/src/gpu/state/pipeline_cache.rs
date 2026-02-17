//! Vulkan pipeline cache persistence.
//!
//! Compiled shaders are cached to disk across sessions for faster subsequent
//! launches. Only supported on Vulkan backends with `PIPELINE_CACHE` feature.

use std::path::PathBuf;

/// Load a pipeline cache from disk (Vulkan only).
///
/// # Safety
///
/// `create_pipeline_cache` is unsafe because it accepts arbitrary bytes.
/// If the data is corrupt or from a different driver version, Vulkan
/// silently ignores it and starts with an empty cache (`fallback: true`).
#[allow(unsafe_code, reason = "wgpu pipeline cache API requires unsafe for raw byte loading")]
pub(super) fn load_pipeline_cache(
    device: &wgpu::Device,
    adapter_info: &wgpu::AdapterInfo,
) -> (Option<wgpu::PipelineCache>, Option<PathBuf>) {
    let cache_key = match wgpu::util::pipeline_cache_key(adapter_info) {
        Some(key) if device.features().contains(wgpu::Features::PIPELINE_CACHE) => key,
        _ => return (None, None),
    };

    let cache_dir = cache_dir();
    let cache_path = cache_dir.join(cache_key);
    let cache_data = std::fs::read(&cache_path).ok();

    // SAFETY: cache data came from a previous `get_data()` call on the
    // same adapter. If corrupt or from a different driver, wgpu/Vulkan
    // silently ignores it (`fallback: true`).
    let cache = unsafe {
        device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
            label: Some("oriterm_pipeline_cache"),
            data: cache_data.as_deref(),
            fallback: true,
        })
    };

    log::info!(
        "pipeline cache: {} ({})",
        cache_path.display(),
        if cache_data.is_some() {
            "loaded existing"
        } else {
            "created new"
        },
    );

    (Some(cache), Some(cache_path))
}

/// Save the pipeline cache to disk. Call before exit.
pub(super) fn save_pipeline_cache(
    pipeline_cache: Option<&wgpu::PipelineCache>,
    pipeline_cache_path: Option<&PathBuf>,
) {
    let (Some(cache), Some(path)) = (pipeline_cache, pipeline_cache_path) else {
        return;
    };
    let Some(data) = cache.get_data() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Atomic write: write to temp, then rename.
    let temp = path.with_extension("tmp");
    match std::fs::write(&temp, &data) {
        Ok(()) => {
            let _ = std::fs::rename(&temp, path);
            log::info!(
                "pipeline cache: saved {} bytes to {}",
                data.len(),
                path.display(),
            );
        }
        Err(e) => log::warn!("pipeline cache: failed to write {}: {e}", temp.display()),
    }
}

/// Returns the platform-specific cache directory for oriterm.
///
/// Pipeline caches are non-essential cached data, so we use the cache
/// directory (not config). Windows: `LOCALAPPDATA` (preferred) or `APPDATA`.
#[cfg(target_os = "windows")]
pub(super) fn cache_dir() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local).join("ori_term");
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("ori_term");
    }
    PathBuf::from(".").join("ori_term")
}

/// Returns the platform-specific cache directory for oriterm.
///
/// Pipeline caches are non-essential cached data, so we use the cache
/// directory (not config). Unix: `XDG_CACHE_HOME` or `~/.cache`.
#[cfg(not(target_os = "windows"))]
pub(super) fn cache_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(xdg).join("ori_term");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".cache").join("ori_term");
    }
    PathBuf::from(".").join("ori_term")
}
