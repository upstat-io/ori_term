//! Application struct and winit event loop handler.
//!
//! [`App`] implements winit's [`ApplicationHandler`] to drive the terminal.
//! It wires together the three-phase rendering pipeline (Extract → Prepare →
//! Render), handles window events, and dispatches terminal events from the
//! PTY reader thread.

mod chrome;
mod clipboard_ops;
pub(crate) mod config_reload;
mod cursor_blink;
mod cursor_hover;
mod divider_drag;
mod event_loop;
mod floating_drag;
mod init;
mod keyboard_input;
mod mark_mode;
mod mouse_input;
mod mouse_report;
mod mouse_selection;
mod mux_pump;
mod pane_ops;
mod redraw;
mod search_ui;
mod tab_bar_input;
mod tab_management;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use winit::event_loop::EventLoopProxy;
use winit::keyboard::ModifiersState;

use oriterm_core::TermMode;
use oriterm_mux::layout::DividerLayout;
use oriterm_mux::{PaneId, WindowId as MuxWindowId};

use self::cursor_blink::CursorBlink;
use self::keyboard_input::ImeState;
use self::mouse_selection::MouseState;
use crate::clipboard::Clipboard;
use crate::config::Config;
use crate::config::monitor::ConfigMonitor;
use crate::event::TermEvent;
use crate::gpu::{FrameInput, GpuRenderer, GpuState, PaneRenderCache};
use crate::keybindings::{self, KeyBinding};
use crate::mux::InProcessMux;
use crate::mux_event::MuxNotification;
use crate::pane::Pane;
use crate::url_detect::{DetectedUrl, UrlDetectCache};
use crate::widgets::terminal_grid::TerminalGridWidget;
use crate::window::TermWindow;

use oriterm_ui::overlay::OverlayManager;
use oriterm_ui::theme::UiTheme;
use oriterm_ui::widgets::tab_bar::TabBarWidget;
use oriterm_ui::widgets::window_chrome::WindowChromeWidget;

/// Default DPI for font rasterization.
const DEFAULT_DPI: f32 = 96.0;

/// Terminal application state and event loop handler.
///
/// Owns all top-level resources: GPU state, renderer, window, and tab.
/// Implements winit's `ApplicationHandler<TermEvent>` to receive both
/// window events and terminal events from the PTY reader thread.
pub(crate) struct App {
    // GPU + rendering (lazy init on Resumed).
    gpu: Option<GpuState>,
    renderer: Option<GpuRenderer>,
    window: Option<TermWindow>,

    // Mux layer (Section 31): owns registries, ID allocators, and domain.
    mux: Option<InProcessMux>,
    // Pane store: Pane structs live here, keyed by PaneId.
    // The mux tracks metadata (PaneEntry); App owns the actual Pane.
    panes: HashMap<PaneId, Pane>,
    // Active mux window ID (maps to the single TermWindow for now).
    active_window: Option<MuxWindowId>,
    // Double-buffer for mux notifications (avoids per-frame allocation).
    notification_buf: Vec<MuxNotification>,

    // Terminal grid widget (layout + event routing participant).
    terminal_grid: Option<TerminalGridWidget>,

    // Window chrome widget (title bar + controls).
    chrome: Option<WindowChromeWidget>,

    // Tab bar widget (tab strip rendering).
    tab_bar: Option<TabBarWidget>,

    // Event loop proxy for waking the event loop from background threads.
    event_proxy: EventLoopProxy<TermEvent>,

    // Per-pane render cache (multi-pane only; skips re-prepare for clean panes).
    pane_cache: PaneRenderCache,

    // Per-frame reusable extraction buffer (lazily initialized on first redraw).
    frame: Option<FrameInput>,

    // Reusable draw list for chrome rendering (avoids per-frame allocation).
    chrome_draw_list: oriterm_ui::draw::DrawList,

    // Redraw coalescing.
    dirty: bool,

    // Keyboard modifier state (updated on ModifiersChanged).
    modifiers: ModifiersState,

    // Cursor blink state (application-level, not terminal-level).
    cursor_blink: CursorBlink,

    // Whether the terminal's CURSOR_BLINKING mode is active.
    // Cached from the last extracted frame to gate blink timer in about_to_wait.
    blinking_active: bool,

    // Mouse selection state (click detection, drag tracking).
    mouse: MouseState,

    // System clipboard for copy/paste.
    clipboard: Clipboard,

    // User configuration (loaded from TOML, hot-reloaded on file change).
    config: Config,

    // Merged keybinding table (defaults + user overrides).
    bindings: Vec<KeyBinding>,

    // Config file watcher (kept alive for the lifetime of the app).
    _config_monitor: Option<ConfigMonitor>,

    // IME composition state machine.
    ime: ImeState,

    // Overlay manager for modal dialogs and popups.
    overlays: OverlayManager,

    // Text pending paste confirmation (stored while dialog is shown).
    pending_paste: Option<String>,

    // URL detection cache (lazily populated per logical line).
    url_cache: UrlDetectCache,

    // Currently hovered URL (set on Ctrl+mouse move, cleared on Ctrl release).
    hovered_url: Option<DetectedUrl>,

    // Cached divider layouts for hit testing (invalidated on layout change).
    cached_dividers: Option<Vec<DividerLayout>>,
    // Divider currently under the cursor (for hover cursor icon).
    hovering_divider: Option<DividerLayout>,
    // Active divider drag state (ratio tracking during drag).
    divider_drag: Option<divider_drag::DividerDragState>,
    // Active floating pane drag/resize state.
    floating_drag: Option<floating_drag::FloatingDragState>,

    // Active UI theme. Centralized here so all widget creation and event
    // contexts use a single source of truth. When dynamic theming arrives,
    // only this field and the theme-change handler need updating.
    ui_theme: UiTheme,

    // Reusable buffer for search bar text (avoids per-frame allocation).
    search_bar_buf: String,

    // Timestamp of last left-click in the tab bar drag area (for double-click maximize).
    last_drag_area_press: Option<Instant>,
}

impl App {
    /// Create a new application instance.
    ///
    /// All GPU/window/tab state is `None` until [`resumed`] is called by
    /// the event loop (lazy initialization pattern from winit docs).
    pub(crate) fn new(event_proxy: EventLoopProxy<TermEvent>, config: Config) -> Self {
        let bindings = keybindings::merge_bindings(&config.keybind);
        let monitor = ConfigMonitor::new(event_proxy.clone());
        let (builtin_count, user_count) = crate::scheme::discover_count();
        log::info!(
            "themes: {} available ({} built-in, {} user)",
            builtin_count + user_count,
            builtin_count,
            user_count,
        );
        let blink_interval = Duration::from_millis(config.terminal.cursor_blink_interval_ms);
        let ui_theme = resolve_ui_theme(&config);
        Self {
            gpu: None,
            renderer: None,
            window: None,
            mux: None,
            panes: HashMap::new(),
            active_window: None,
            notification_buf: Vec::new(),
            terminal_grid: None,
            chrome: None,
            tab_bar: None,
            event_proxy,
            pane_cache: PaneRenderCache::new(),
            frame: None,
            chrome_draw_list: oriterm_ui::draw::DrawList::new(),
            dirty: false,
            modifiers: ModifiersState::empty(),
            cursor_blink: CursorBlink::new(blink_interval),
            blinking_active: false,
            mouse: MouseState::new(),
            clipboard: Clipboard::new(),
            config,
            bindings,
            _config_monitor: monitor,
            ime: ImeState::new(),
            overlays: OverlayManager::new(oriterm_ui::geometry::Rect::default()),
            pending_paste: None,
            url_cache: UrlDetectCache::default(),
            hovered_url: None,
            cached_dividers: None,
            hovering_divider: None,
            divider_drag: None,
            floating_drag: None,
            ui_theme,
            search_bar_buf: String::new(),
            last_drag_area_press: None,
        }
    }

    /// Re-rasterize fonts and update rendering settings for a new DPI scale.
    ///
    /// Called when the window moves between monitors with different scale
    /// factors. Recalculates font size at physical DPI, updates hinting
    /// and subpixel mode, and clears/recaches glyph atlases.
    fn handle_dpi_change(&mut self, scale_factor: f64) {
        let (Some(renderer), Some(gpu)) = (&mut self.renderer, &self.gpu) else {
            return;
        };
        let scale = scale_factor as f32;
        let physical_dpi = DEFAULT_DPI * scale;

        // Re-rasterize at new physical DPI. This recomputes cell metrics
        // and clears the glyph cache + GPU atlases.
        renderer.set_font_size(self.config.font.size, physical_dpi, gpu);

        // Update hinting and subpixel mode for the new scale factor.
        let hinting = config_reload::resolve_hinting(&self.config.font, scale_factor);
        let format =
            config_reload::resolve_subpixel_mode(&self.config.font, scale_factor).glyph_format();
        renderer.set_hinting_and_format(hinting, format, gpu);

        // Mark all grid lines dirty so the frame extraction re-reads every
        // cell with the new cell metrics. Without this, the terminal content
        // appears stale until PTY output marks individual lines dirty.
        if let Some(pane) = self.active_pane() {
            pane.terminal().lock().grid_mut().dirty_mut().mark_all();
        }

        // Invalidate pane render cache (atlas + cell metrics changed).
        self.pane_cache.invalidate_all();
        self.dirty = true;
    }

    /// Handle system dark/light theme change.
    ///
    /// Updates the terminal palette and UI chrome colors. Respects
    /// [`ThemeOverride`]: if the user forced dark/light, the system
    /// notification is ignored — only `Auto` delegates to the system.
    fn handle_theme_changed(&mut self, winit_theme: winit::window::Theme) {
        let system_theme = match winit_theme {
            winit::window::Theme::Dark => oriterm_core::Theme::Dark,
            winit::window::Theme::Light => oriterm_core::Theme::Light,
        };
        let theme = self.config.colors.resolve_theme(|| system_theme);
        if let Some(pane) = self.active_pane() {
            let mut term = pane.terminal().lock();
            term.set_theme(theme);
            let palette = config_reload::build_palette_from_config(&self.config.colors, theme);
            *term.palette_mut() = palette;
            term.grid_mut().dirty_mut().mark_all();
        }
        // Update UI chrome theme (tab bar, window controls).
        self.ui_theme = resolve_ui_theme_with(&self.config, system_theme);
        if let Some(chrome) = &mut self.chrome {
            chrome.apply_theme(&self.ui_theme);
        }
        if let Some(tab_bar) = &mut self.tab_bar {
            tab_bar.apply_theme(&self.ui_theme);
        }
        // Invalidate pane render cache (palette colors changed).
        self.pane_cache.invalidate_all();
        self.dirty = true;
    }

    /// Save pipeline cache and exit the process.
    ///
    /// Centralizes shutdown to avoid duplicating cleanup logic across
    /// `ChildExit`, `CloseRequested`, and `WindowClose`. wgpu
    /// `Device::drop()` calls `vkDeviceWaitIdle()` which blocks for
    /// seconds — the OS reclaims all GPU resources on process exit anyway.
    fn shutdown(&self, code: i32) -> ! {
        if let Some(gpu) = &self.gpu {
            gpu.save_pipeline_cache_async();
        }
        std::process::exit(code)
    }

    /// Apply the color palette from the current config to a pane's terminal.
    ///
    /// Builds the palette from the config's color scheme and user overrides,
    /// then writes it into the pane's terminal. Used after spawning a new
    /// pane (tab create, split, floating).
    fn apply_palette_to_pane(&self, pane: &Pane, theme: oriterm_core::Theme) {
        let mut term = pane.terminal().lock();
        let palette = config_reload::build_palette_from_config(&self.config.colors, theme);
        *term.palette_mut() = palette;
    }

    /// Read the terminal mode, locking briefly.
    ///
    /// Returns `None` if no active pane is present.
    fn terminal_mode(&self) -> Option<TermMode> {
        self.active_pane().map(|p| p.terminal().lock().mode())
    }

    // -- Mux pane accessors --

    /// The active pane's ID, derived from the mux session model.
    fn active_pane_id(&self) -> Option<PaneId> {
        let mux = self.mux.as_ref()?;
        let win_id = self.active_window?;
        let win = mux.session().get_window(win_id)?;
        let tab_id = win.active_tab()?;
        let tab = mux.session().get_tab(tab_id)?;
        Some(tab.active_pane())
    }

    /// Immutable reference to the active pane.
    fn active_pane(&self) -> Option<&Pane> {
        let id = self.active_pane_id()?;
        self.panes.get(&id)
    }

    /// Mutable reference to the active pane.
    fn active_pane_mut(&mut self) -> Option<&mut Pane> {
        let id = self.active_pane_id()?;
        self.panes.get_mut(&id)
    }

    /// Current tab width lock value, if active.
    ///
    /// Delegates to the tab bar widget — the widget is the single source
    /// of truth for this value.
    pub(super) fn tab_width_lock(&self) -> Option<f32> {
        self.tab_bar.as_ref().and_then(TabBarWidget::tab_width_lock)
    }

    /// Freeze tab widths at `width` to prevent layout jitter.
    pub(super) fn acquire_tab_width_lock(&mut self, width: f32) {
        if let Some(tab_bar) = &mut self.tab_bar {
            tab_bar.set_tab_width_lock(Some(width));
        }
    }

    /// Release the tab width lock, allowing tabs to recompute widths.
    pub(super) fn release_tab_width_lock(&mut self) {
        if self.tab_width_lock().is_some() {
            if let Some(tab_bar) = &mut self.tab_bar {
                tab_bar.set_tab_width_lock(None);
            }
            self.dirty = true;
        }
    }
}

use event_loop::{resolve_ui_theme, resolve_ui_theme_with, winit_mods_to_ui};

#[cfg(test)]
mod tests;
