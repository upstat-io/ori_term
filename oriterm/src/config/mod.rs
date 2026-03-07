//! Configuration structures and loading logic.
//!
//! All structs use `#[serde(default)]` so partial TOML files
//! fill in defaults for missing fields.

mod behavior;
mod bell;
mod color_config;
mod font_config;
mod io;
pub(crate) mod monitor;
mod paste_warning;

pub(crate) use io::config_path;

#[allow(unused_imports, reason = "used in state persistence (Section 15)")]
pub(crate) use io::WindowState;
#[allow(unused_imports, reason = "used in state persistence (Section 15)")]
pub(crate) use io::state_path;

pub(crate) use color_config::{ColorConfig, ThemeOverride};
pub(crate) use font_config::FontConfig;

#[allow(unused_imports, reason = "used in color config application")]
pub(crate) use color_config::AlphaBlending;
#[allow(
    unused_imports,
    reason = "used in font discovery and codepoint mapping"
)]
pub(crate) use font_config::{CodepointMapConfig, FallbackFontConfig};

use oriterm_core::Rgb;
use serde::{Deserialize, Serialize};

/// Process model for how the terminal manages multiplexer state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessModel {
    /// Multi-process mode: auto-start a mux daemon, connect via IPC.
    /// Each window is a separate OS process. Tabs migrate between windows.
    #[default]
    Daemon,
    /// Single-process mode: embedded mux, no IPC, no daemon.
    /// Used for testing, sandboxed environments, or when daemon unavailable.
    Embedded,
}

/// Top-level configuration structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub process_model: ProcessModel,
    pub font: FontConfig,
    pub terminal: TerminalConfig,
    pub colors: ColorConfig,
    pub window: WindowConfig,
    pub behavior: BehaviorConfig,
    pub bell: BellConfig,
    pub pane: PaneConfig,
    #[serde(default)]
    pub keybind: Vec<KeybindConfig>,
}

/// Cursor style for config deserialization.
///
/// Maps user-facing TOML strings to `oriterm_core::CursorShape` values.
/// `"beam"` is accepted as an alias for `Bar` (common in other terminals).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorStyle {
    #[default]
    Block,
    #[serde(alias = "beam")]
    Bar,
    Underline,
}

impl CursorStyle {
    /// Convert to the rendering-layer `CursorShape`.
    pub fn to_shape(self) -> oriterm_core::CursorShape {
        match self {
            Self::Block => oriterm_core::CursorShape::Block,
            Self::Bar => oriterm_core::CursorShape::Bar,
            Self::Underline => oriterm_core::CursorShape::Underline,
        }
    }
}

/// Terminal behavior configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    /// Override shell (default: system shell).
    pub shell: Option<String>,
    /// Scrollback lines (default: 10,000).
    pub scrollback: usize,
    /// Cursor style (default: block).
    pub cursor_style: CursorStyle,
    /// Enable cursor blinking (default: true).
    pub cursor_blink: bool,
    /// Blink interval in milliseconds (default: 530).
    pub cursor_blink_interval_ms: u64,
    /// Enable/disable all image protocols (Kitty, Sixel, iTerm2).
    pub image_protocol: bool,
    /// CPU-side image cache memory limit in bytes (default: 320 MB).
    pub image_memory_limit: usize,
    /// GPU texture cache memory limit in bytes (default: 512 MB).
    pub image_gpu_memory_limit: usize,
    /// Enable animated image display (default: true).
    pub image_animation: bool,
    /// Maximum single image size in bytes (default: 64 MB).
    pub image_max_single_size: usize,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: None,
            scrollback: 10_000,
            cursor_style: CursorStyle::default(),
            cursor_blink: true,
            cursor_blink_interval_ms: 530,
            image_protocol: true,
            image_memory_limit: 320 * 1_000_000,
            image_gpu_memory_limit: 512 * 1_000_000,
            image_animation: true,
            image_max_single_size: 64 * 1_000_000,
        }
    }
}

impl TerminalConfig {
    /// Build an [`ImageConfig`](oriterm_mux::ImageConfig) from these settings.
    pub fn image_config(&self) -> oriterm_mux::ImageConfig {
        oriterm_mux::ImageConfig {
            enabled: self.image_protocol,
            memory_limit: self.image_memory_limit,
            max_single: self.image_max_single_size,
            animation_enabled: self.image_animation,
        }
    }
}

/// Window decoration mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decorations {
    /// OS-native title bar and borders.
    Full,
    /// Frameless window with custom CSD (default).
    #[default]
    None,
    /// macOS: transparent titlebar.
    Transparent,
    /// macOS: hide traffic lights.
    Buttonless,
}

/// Window size and opacity configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    /// Initial terminal columns (default: 120).
    pub columns: usize,
    /// Initial terminal rows (default: 30).
    pub rows: usize,
    /// Window opacity 0.0-1.0 (default: 1.0).
    pub opacity: f32,
    /// Independent tab bar opacity (falls back to `opacity`).
    pub tab_bar_opacity: Option<f32>,
    /// Enable backdrop blur (default: true).
    pub blur: bool,
    /// Window decoration mode (default: `None` for frameless CSD).
    pub decorations: Decorations,
    /// Snap resize to cell boundaries (default: false).
    pub resize_increments: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            columns: 120,
            rows: 30,
            opacity: 1.0,
            tab_bar_opacity: None,
            blur: true,
            decorations: Decorations::default(),
            resize_increments: false,
        }
    }
}

impl WindowConfig {
    /// Returns opacity clamped to [0.0, 1.0], defaulting to 1.0 for NaN.
    pub fn effective_opacity(&self) -> f32 {
        clamp_or_default(self.opacity, 0.0, 1.0, 1.0)
    }

    /// Returns tab bar opacity clamped to [0.0, 1.0].
    /// Falls back to `opacity` when not explicitly set.
    #[allow(dead_code, reason = "used in tab bar rendering (Section 16)")]
    pub fn effective_tab_bar_opacity(&self) -> f32 {
        clamp_or_default(self.tab_bar_opacity.unwrap_or(self.opacity), 0.0, 1.0, 1.0)
    }
}

pub(crate) use behavior::{BehaviorConfig, NotifyOnCommandFinish};
pub(crate) use paste_warning::PasteWarning;

pub(crate) use bell::BellConfig;

/// Pane splitting and layout configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PaneConfig {
    /// Divider thickness in logical pixels between split panes.
    pub divider_px: f32,
    /// Minimum pane size in cells `(columns, rows)`.
    pub min_cells: (u16, u16),
    /// Dim inactive (unfocused) panes to reduce visual clutter.
    pub dim_inactive: bool,
    /// Alpha multiplier for inactive pane glyphs (0.0–1.0).
    pub inactive_opacity: f32,
    /// Divider line color between split panes (hex, e.g. `"#505050"`).
    pub divider_color: Option<String>,
    /// Focus border accent color for the active pane (hex, e.g. `"#6495ED"`).
    pub focus_border_color: Option<String>,
}

/// Default divider color: neutral gray (`#505050`).
const DEFAULT_DIVIDER_COLOR: Rgb = Rgb {
    r: 80,
    g: 80,
    b: 80,
};
/// Default focus border accent: cornflower blue (`#6495ED`).
const DEFAULT_FOCUS_BORDER_COLOR: Rgb = Rgb {
    r: 100,
    g: 149,
    b: 237,
};

impl Default for PaneConfig {
    fn default() -> Self {
        Self {
            divider_px: 1.0,
            min_cells: (10, 3),
            dim_inactive: false,
            inactive_opacity: 0.7,
            divider_color: None,
            focus_border_color: None,
        }
    }
}

impl PaneConfig {
    /// Returns `inactive_opacity` clamped to [0.0, 1.0], defaulting to 0.7 for NaN.
    pub fn effective_inactive_opacity(&self) -> f32 {
        clamp_or_default(self.inactive_opacity, 0.0, 1.0, 0.7)
    }

    /// Resolved divider color, falling back to the default neutral gray.
    pub fn effective_divider_color(&self) -> Rgb {
        self.divider_color
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(DEFAULT_DIVIDER_COLOR)
    }

    /// Resolved focus border color, falling back to the default cornflower blue.
    pub fn effective_focus_border_color(&self) -> Rgb {
        self.focus_border_color
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(DEFAULT_FOCUS_BORDER_COLOR)
    }
}

/// TOML-serializable keybinding entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindConfig {
    /// Key name (e.g. "c", "Tab", "F1").
    pub key: String,
    /// Pipe-separated modifiers (e.g. "Ctrl|Shift"). Empty for no modifiers.
    #[serde(default)]
    pub mods: String,
    /// Action name (e.g. "Copy", "Paste", "SendText:\x1b[A").
    pub action: String,
}

/// Clamp a float to `[min, max]`, returning `default` for NaN.
///
/// `f32::clamp` passes NaN through unchanged, which can propagate into
/// rendering calculations. This helper treats NaN as "no valid value".
fn clamp_or_default(value: f32, min: f32, max: f32, default: f32) -> f32 {
    if value.is_nan() {
        default
    } else {
        value.clamp(min, max)
    }
}

/// Parse a `#RRGGBB` hex color string to [`Rgb`].
///
/// Accepts with or without the leading `#`. Returns `None` on invalid input.
pub(crate) fn parse_hex_color(hex: &str) -> Option<Rgb> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    let bytes = hex.as_bytes();
    if bytes.len() != 6 || !bytes.iter().all(u8::is_ascii_hexdigit) {
        log::warn!("config: invalid hex color (expected #RRGGBB): {hex}");
        return None;
    }
    // Safe: validated 6 ASCII hex digits above — all single-byte UTF-8.
    let r = (hex_nibble(bytes[0]) << 4) | hex_nibble(bytes[1]);
    let g = (hex_nibble(bytes[2]) << 4) | hex_nibble(bytes[3]);
    let b = (hex_nibble(bytes[4]) << 4) | hex_nibble(bytes[5]);
    Some(Rgb { r, g, b })
}

/// Convert a single ASCII hex digit to its numeric value.
fn hex_nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0, // unreachable after validation
    }
}

#[cfg(test)]
mod tests;
