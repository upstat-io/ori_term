//! GPU device, adapter, queue, and surface management.
//!
//! [`GpuState`] is shared across all windows and owns the wgpu device lifetime.
//! It handles backend selection (Vulkan preferred, DX12+`DirectComposition` for
//! Windows transparency), adapter enumeration (discrete GPU preferred), sRGB
//! surface format derivation, and Vulkan pipeline cache persistence.

// GpuState and helpers are fully implemented but not yet called from the event
// loop (added in Section 05). Suppress dead-code warnings until then.
#![expect(dead_code, reason = "GPU infrastructure used in Section 05")]

use std::path::PathBuf;
use std::sync::Arc;

use winit::window::Window;

/// GPU state shared across all windows.
pub struct GpuState {
    instance: wgpu::Instance,
    /// Shared device for all render commands.
    pub(crate) device: wgpu::Device,
    /// Shared queue for all render submissions.
    pub(crate) queue: wgpu::Queue,
    /// Native surface format (used for surface configuration).
    surface_format: wgpu::TextureFormat,
    /// sRGB format for render passes and pipelines. May differ from
    /// `surface_format` when the surface doesn't natively support sRGB
    /// (e.g. DX12 `DirectComposition`).
    pub(crate) render_format: wgpu::TextureFormat,
    /// Composite alpha mode negotiated with the compositor.
    surface_alpha_mode: wgpu::CompositeAlphaMode,
    /// Whether the backend supports `view_formats` for sRGB reinterpretation.
    supports_view_formats: bool,
    /// Vulkan pipeline cache (compiled shaders cached to disk across sessions).
    pub(crate) pipeline_cache: Option<wgpu::PipelineCache>,
    pipeline_cache_path: Option<PathBuf>,
}

impl GpuState {
    /// Initialize GPU: create instance, surface, adapter, device, and queue.
    ///
    /// When `transparent` is true on Windows, uses DX12 with `DirectComposition`
    /// (the only path that gives `PreMultiplied` alpha on Windows HWND
    /// swapchains). Otherwise prefers Vulkan (supports pipeline caching for
    /// faster subsequent launches).
    pub fn new(window: &Arc<Window>, transparent: bool) -> Self {
        #[cfg(not(target_os = "windows"))]
        let _ = transparent;

        // On Windows with transparency, DX12+DComp is the only path for
        // PreMultiplied alpha.
        #[cfg(target_os = "windows")]
        if transparent {
            if let Some(state) = Self::try_init(window, wgpu::Backends::DX12, true) {
                return state;
            }
            log::warn!("DX12 DirectComposition init failed, falling back to Vulkan");
        }

        // Prefer Vulkan — it supports pipeline caching (compiled shaders
        // persisted to disk).
        if let Some(state) = Self::try_init(window, wgpu::Backends::VULKAN, false) {
            return state;
        }

        // Fall back to other primary backends (DX12, Metal).
        if let Some(state) = Self::try_init(window, wgpu::Backends::PRIMARY, false) {
            return state;
        }

        // Last resort: secondary backends (GL, etc.).
        Self::try_init(window, wgpu::Backends::SECONDARY, false)
            .expect("failed to initialize GPU with any backend")
    }

    /// Returns the native surface format used for surface configuration.
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }

    /// Returns the sRGB render format used for pipelines.
    pub fn render_format(&self) -> wgpu::TextureFormat {
        self.render_format
    }

    /// Returns true if the surface alpha mode supports transparency.
    pub fn supports_transparency(&self) -> bool {
        !matches!(self.surface_alpha_mode, wgpu::CompositeAlphaMode::Opaque)
    }

    /// Create and configure a new surface for a window.
    pub fn create_surface(
        &self,
        window: &Arc<Window>,
    ) -> Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)> {
        let surface = self.instance.create_surface(window.clone()).ok()?;
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: self.surface_alpha_mode,
            view_formats: self.view_formats(),
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&self.device, &config);
        Some((surface, config))
    }

    /// Save the pipeline cache to disk. Call before exit.
    pub fn save_pipeline_cache(&self) {
        let (Some(cache), Some(path)) = (&self.pipeline_cache, &self.pipeline_cache_path) else {
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
        if std::fs::write(&temp, &data).is_ok() {
            let _ = std::fs::rename(&temp, path);
            log::info!(
                "pipeline cache: saved {} bytes to {}",
                data.len(),
                path.display(),
            );
        }
    }

    /// Try to initialize GPU with the given backend set.
    ///
    /// Returns `None` if no compatible adapter is found or device creation
    /// fails, allowing the caller to fall back to the next backend.
    fn try_init(window: &Arc<Window>, backends: wgpu::Backends, dcomp: bool) -> Option<Self> {
        let instance = Self::create_instance(backends, dcomp);
        let surface = instance.create_surface(window.clone()).ok()?;
        let adapter = Self::pick_adapter(&instance, &surface, backends)?;

        // Request PIPELINE_CACHE if the adapter supports it (Vulkan only).
        let mut features = wgpu::Features::empty();
        if adapter.features().contains(wgpu::Features::PIPELINE_CACHE) {
            features |= wgpu::Features::PIPELINE_CACHE;
        }

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("oriterm"),
                required_features: features,
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
        ))
        .map_err(|e| log::error!("GPU device request failed: {e}"))
        .ok()?;

        let caps = surface.get_capabilities(&adapter);
        let downlevel = adapter.get_downlevel_capabilities();
        let (surface_format, render_format, view_formats) =
            Self::select_formats(&caps, downlevel.flags)?;
        let surface_alpha_mode = Self::select_alpha_mode(&caps);
        let supports_view_formats = downlevel
            .flags
            .contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS);

        // Configure the initial surface.
        let size = window.inner_size();
        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: surface_alpha_mode,
                view_formats,
                desired_maximum_frame_latency: 2,
            },
        );

        let info = adapter.get_info();
        let transparency_supported =
            !matches!(surface_alpha_mode, wgpu::CompositeAlphaMode::Opaque);
        log::info!(
            "GPU: adapter={}, backend={:?}, surface={surface_format:?}, \
             render={render_format:?}, alpha={surface_alpha_mode:?} (available: {:?}), \
             transparency={}, view_formats={}",
            info.name,
            info.backend,
            caps.alpha_modes,
            if transparency_supported { "yes" } else { "no" },
            if supports_view_formats {
                "supported"
            } else {
                "not supported"
            },
        );

        let (pipeline_cache, pipeline_cache_path) = Self::load_pipeline_cache(&device, &info);

        // Adapter is no longer needed — device and queue are independent.
        drop(adapter);

        Some(Self {
            instance,
            device,
            queue,
            surface_format,
            render_format,
            surface_alpha_mode,
            supports_view_formats,
            pipeline_cache,
            pipeline_cache_path,
        })
    }

    /// Create a wgpu instance with the specified backends.
    fn create_instance(backends: wgpu::Backends, dcomp: bool) -> wgpu::Instance {
        wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            backend_options: wgpu::BackendOptions {
                dx12: wgpu::Dx12BackendOptions {
                    presentation_system: if dcomp {
                        wgpu::Dx12SwapchainKind::DxgiFromVisual
                    } else {
                        wgpu::Dx12SwapchainKind::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        })
    }

    /// Enumerate adapters and pick the best one for our surface.
    ///
    /// Prefers discrete GPUs over integrated, falling back to any compatible
    /// adapter.
    fn pick_adapter(
        instance: &wgpu::Instance,
        surface: &wgpu::Surface<'_>,
        backends: wgpu::Backends,
    ) -> Option<wgpu::Adapter> {
        let mut discrete: Option<wgpu::Adapter> = None;
        let mut fallback: Option<wgpu::Adapter> = None;

        for a in pollster::block_on(instance.enumerate_adapters(backends)) {
            if !a.is_surface_supported(surface) {
                continue;
            }
            if a.get_info().device_type == wgpu::DeviceType::DiscreteGpu {
                discrete = Some(a);
                break;
            }
            if fallback.is_none() {
                fallback = Some(a);
            }
        }

        discrete.or(fallback)
    }

    /// Select surface format and derive sRGB render format.
    ///
    /// Some backends (DX12 `DirectComposition`) only expose non-sRGB surface
    /// formats. We use `view_formats` + `add_srgb_suffix()` so the GPU still
    /// performs gamma-aware blending — but only when the backend supports
    /// `SURFACE_VIEW_FORMATS` (checked via downlevel flags).
    ///
    /// Returns `None` if `caps.formats` is empty (incompatible surface).
    fn select_formats(
        caps: &wgpu::SurfaceCapabilities,
        downlevel_flags: wgpu::DownlevelFlags,
    ) -> Option<(wgpu::TextureFormat, wgpu::TextureFormat, Vec<wgpu::TextureFormat>)> {
        let surface_format = *caps.formats.first()?;
        let render_format = surface_format.add_srgb_suffix();

        // Only set view_formats when the backend supports reinterpreting
        // the surface format as sRGB. Without this flag, view_formats
        // may be silently ignored or cause errors on limited backends.
        let needs_view_format = render_format != surface_format;
        let supports_view_formats = downlevel_flags
            .contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS);

        let view_formats = if needs_view_format && supports_view_formats {
            vec![render_format]
        } else {
            vec![]
        };

        Some((surface_format, render_format, view_formats))
    }

    /// Select the best composite alpha mode for transparency.
    ///
    /// Prefers non-opaque modes so the compositor can see transparent pixels
    /// and show blur/acrylic through them.
    fn select_alpha_mode(caps: &wgpu::SurfaceCapabilities) -> wgpu::CompositeAlphaMode {
        if caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else {
            caps.alpha_modes[0]
        }
    }

    /// Compute the `view_formats` list needed for surface configuration.
    fn view_formats(&self) -> Vec<wgpu::TextureFormat> {
        let needs_view_format = self.render_format != self.surface_format;
        if needs_view_format && self.supports_view_formats {
            vec![self.render_format]
        } else {
            vec![]
        }
    }

    /// Load a pipeline cache from disk (Vulkan only).
    ///
    /// # Safety
    ///
    /// `create_pipeline_cache` is unsafe because it accepts arbitrary bytes.
    /// If the data is corrupt or from a different driver version, Vulkan
    /// silently ignores it and starts with an empty cache (`fallback: true`).
    #[allow(unsafe_code, reason = "wgpu pipeline cache API requires unsafe for raw byte loading")]
    fn load_pipeline_cache(
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
}

/// Returns the platform-specific cache directory for oriterm.
///
/// Uses `APPDATA/ori_term` on Windows, `XDG_CONFIG_HOME/ori_term` or
/// `~/.config/ori_term` on Unix.
fn cache_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("ori_term");
        }
        PathBuf::from(".").join("ori_term")
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("ori_term");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config").join("ori_term");
        }
        PathBuf::from(".").join("ori_term")
    }
}

/// Validate GPU availability by creating an instance and enumerating adapters.
///
/// Logs adapter info for each compatible GPU found. Returns the number of
/// adapters discovered. This is a lightweight check that does not require a
/// window or surface.
pub fn validate_gpu() -> usize {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });

    let adapters: Vec<_> = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
        .into_iter()
        .collect();

    for a in &adapters {
        let info = a.get_info();
        log::info!(
            "GPU adapter: {} ({:?}, {:?})",
            info.name,
            info.backend,
            info.device_type,
        );
    }

    if adapters.is_empty() {
        log::warn!("no GPU adapters found");
    }

    adapters.len()
}

#[cfg(test)]
mod tests;
