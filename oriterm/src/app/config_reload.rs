//! Configuration hot-reload — applies config changes without restart.
//!
//! When the config file watcher detects changes, [`App::apply_config_reload`]
//! loads the new config, computes deltas, and applies only what changed:
//! fonts, colors, cursor style, window, behavior, bell, keybindings.

use oriterm_ui::widgets::window_chrome::WindowChromeWidget;

use super::{App, DEFAULT_DPI};
use crate::config::{self, Config, FontConfig};
use crate::font::{
    FaceIdx, FontCollection, FontSet, HintingMode, SubpixelMode, parse_features, parse_hex_range,
};
use crate::keybindings;

impl App {
    /// Apply a reloaded configuration to the running application.
    ///
    /// Reloads the config file, compares against the current config, and
    /// applies only the fields that changed. On parse error, logs a warning
    /// and keeps the previous config.
    pub(super) fn apply_config_reload(&mut self) {
        let new_config = match Config::try_load() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("config reload: {e}");
                return;
            }
        };

        self.apply_font_changes(&new_config);
        self.apply_color_changes(&new_config);
        self.apply_cursor_changes(&new_config);
        self.apply_window_changes(&new_config);
        self.apply_behavior_changes(&new_config);
        self.apply_keybinding_changes(&new_config);

        // Bell config is read from self.config at usage sites, so
        // storing the new config is sufficient. Log if it changed.
        if new_config.bell.duration_ms != self.config.bell.duration_ms
            || new_config.bell.animation != self.config.bell.animation
            || new_config.bell.color != self.config.bell.color
        {
            log::info!("config reload: bell settings updated");
        }

        // Store the new config as current.
        self.config = new_config;

        // Mark everything dirty for redraw.
        self.dirty = true;

        log::info!("config reload: applied successfully");
    }

    /// Detect and apply font changes (family, size, weight, features, fallback,
    /// hinting, subpixel mode, variations, codepoint map).
    fn apply_font_changes(&mut self, new: &Config) {
        let old = &self.config.font;
        let font_changed = (new.font.size - old.size).abs() > f32::EPSILON
            || new.font.family != old.family
            || new.font.weight != old.weight
            || new.font.features != old.features
            || new.font.fallback != old.fallback
            || new.font.hinting != old.hinting
            || new.font.subpixel_mode != old.subpixel_mode
            || new.font.variations != old.variations
            || new.font.codepoint_map != old.codepoint_map;

        if !font_changed {
            return;
        }

        let (Some(renderer), Some(gpu), Some(window)) =
            (&mut self.renderer, &self.gpu, &self.window)
        else {
            return;
        };

        let weight = new.font.effective_weight();
        let mut font_set = match FontSet::load(new.font.family.as_deref(), weight) {
            Ok(fs) => fs,
            Err(e) => {
                log::warn!("config reload: font load failed: {e}");
                return;
            }
        };

        // Prepend user-configured fallback fonts before system fallbacks.
        let user_fb_families: Vec<&str> = new
            .font
            .fallback
            .iter()
            .map(|f| f.family.as_str())
            .collect();
        let user_fb_count = font_set.prepend_user_fallbacks(&user_fb_families);

        // Resolve hinting and subpixel mode: config overrides auto-detection.
        let scale = window.scale_factor().factor();
        let hinting = resolve_hinting(&new.font, scale);
        let format = resolve_subpixel_mode(&new.font, scale).glyph_format();

        let scale = scale as f32;
        let physical_dpi = DEFAULT_DPI * scale;
        let mut collection = match FontCollection::new(
            font_set,
            new.font.size,
            physical_dpi,
            format,
            weight,
            hinting,
        ) {
            Ok(fc) => fc,
            Err(e) => {
                log::warn!("config reload: font collection failed: {e}");
                return;
            }
        };

        // Apply all font config settings.
        apply_font_config(&mut collection, &new.font, user_fb_count);

        let cell = collection.cell_metrics();
        log::info!(
            "config reload: font size={:.1}, cell={}x{}",
            new.font.size,
            cell.width,
            cell.height,
        );

        renderer.replace_font_collection(collection, gpu);

        // Resize grid to match new cell metrics (physical pixels).
        let cell = renderer.cell_metrics();
        let (w, h) = window.size_px();
        let caption_height = self
            .chrome
            .as_ref()
            .map_or(0.0, WindowChromeWidget::caption_height);
        let caption_px = (caption_height * scale).round() as u32;
        let grid_h = h.saturating_sub(caption_px);
        let cols = cell.columns(w).max(1);
        let rows = cell.rows(grid_h).max(1);

        if let Some(grid) = &mut self.terminal_grid {
            grid.set_cell_metrics(cell.width, cell.height);
            grid.set_grid_size(cols, rows);
            grid.set_bounds(oriterm_ui::geometry::Rect::new(
                0.0,
                caption_height * scale,
                cols as f32 * cell.width,
                rows as f32 * cell.height,
            ));
        }

        if let Some(tab) = &self.tab {
            tab.resize_grid(rows as u16, cols as u16);
            tab.resize_pty(rows as u16, cols as u16);
        }
    }

    /// Detect and apply color config changes.
    ///
    /// Resolves the effective theme (honoring config override), rebuilds
    /// the palette, applies config color overrides, and marks all lines
    /// dirty so colors are re-resolved.
    fn apply_color_changes(&self, new: &Config) {
        if new.colors == self.config.colors {
            return;
        }

        let Some(tab) = &self.tab else { return };
        let mut term = tab.terminal().lock();

        // Resolve theme: config override takes priority over system detection.
        let theme = new
            .colors
            .resolve_theme(crate::platform::theme::system_theme);

        // Update the terminal's stored theme and rebuild the palette.
        term.set_theme(theme);
        let mut palette = oriterm_core::Palette::for_theme(theme);
        apply_color_overrides(&mut palette, &new.colors);

        // Replace the palette and mark all lines dirty.
        *term.palette_mut() = palette;
        term.grid_mut().dirty_mut().mark_all();

        log::info!("config reload: colors updated (theme={theme:?})");
    }

    /// Detect and apply cursor style and blink interval changes.
    fn apply_cursor_changes(&mut self, new: &Config) {
        if new.terminal.cursor_style != self.config.terminal.cursor_style {
            let shape = new.terminal.cursor_style.to_shape();
            if let Some(tab) = &self.tab {
                tab.terminal().lock().set_cursor_shape(shape);
            }
        }

        if new.terminal.cursor_blink_interval_ms != self.config.terminal.cursor_blink_interval_ms {
            let interval = std::time::Duration::from_millis(new.terminal.cursor_blink_interval_ms);
            self.cursor_blink.set_interval(interval);
            log::info!(
                "config reload: cursor blink interval={}ms",
                new.terminal.cursor_blink_interval_ms
            );
        }
    }

    /// Detect and apply window transparency/blur changes.
    fn apply_window_changes(&self, new: &Config) {
        let opacity_changed =
            (new.window.effective_opacity() - self.config.window.effective_opacity()).abs()
                > f32::EPSILON;
        let blur_changed = new.window.blur != self.config.window.blur;

        if !opacity_changed && !blur_changed {
            return;
        }

        let Some(window) = &self.window else { return };
        let opacity = new.window.effective_opacity();
        let blur = new.window.blur && opacity < 1.0;

        window.set_transparency(opacity, blur);

        log::info!("config reload: window opacity={opacity:.2}, blur={blur}",);
    }

    /// Detect and apply behavior config changes.
    ///
    /// Behavior flags are read from `self.config` at usage sites, so
    /// storing the new config is sufficient. If `bold_is_bright` changed,
    /// marks all lines dirty since existing cells may render differently.
    fn apply_behavior_changes(&self, new: &Config) {
        if new.behavior.bold_is_bright != self.config.behavior.bold_is_bright {
            if let Some(tab) = &self.tab {
                tab.terminal().lock().grid_mut().dirty_mut().mark_all();
            }
            log::info!("config reload: bold_is_bright changed");
        }
    }

    /// Rebuild keybinding table from new config.
    fn apply_keybinding_changes(&mut self, new: &Config) {
        self.bindings = keybindings::merge_bindings(&new.keybind);
    }
}

// ── Font config helpers ──

/// Apply all font configuration settings to a collection after creation.
///
/// Handles: user features, per-fallback metadata (`size_offset`, features),
/// user variable font variations, and codepoint-to-font mappings.
///
/// `user_fb_count` is the number of user-configured fallbacks that were
/// successfully loaded and prepended (for matching config indices to
/// fallback array indices).
pub(crate) fn apply_font_config(
    collection: &mut FontCollection,
    config: &FontConfig,
    user_fb_count: usize,
) {
    // 1. Apply user-configured OpenType features (replace defaults).
    let feature_refs: Vec<&str> = config.features.iter().map(String::as_str).collect();
    let features = parse_features(&feature_refs);
    collection.set_features(features);

    // 2. Apply per-fallback metadata (size_offset, features) to user fallbacks.
    // User fallbacks occupy indices 0..user_fb_count in the fallback array.
    // We apply config metadata to each loaded user fallback in order.
    for (i, fb_config) in config.fallback.iter().enumerate() {
        if i >= user_fb_count {
            break;
        }
        let fb_features = fb_config.features.as_ref().map(|f| {
            let refs: Vec<&str> = f.iter().map(String::as_str).collect();
            parse_features(&refs)
        });
        collection.set_fallback_meta(i, fb_config.size_offset.unwrap_or(0.0), fb_features);
    }

    // 3. Apply codepoint-to-font mappings.
    // Codepoint map entries reference families by name. The mapped face index
    // must point to an actual loaded fallback. We search the user fallback
    // config entries for a matching family name and use that index.
    for entry in &config.codepoint_map {
        let Some((start, end)) = parse_hex_range(&entry.range) else {
            log::warn!(
                "config: invalid codepoint_map range {:?}, skipping",
                entry.range
            );
            continue;
        };
        // Find the fallback index for this family name.
        let face_idx = config
            .fallback
            .iter()
            .position(|fb| fb.family == entry.family)
            .map(FaceIdx::from_fallback_index);
        match face_idx {
            Some(idx) => {
                collection.add_codepoint_mapping(start, end, idx);
                log::info!(
                    "config: codepoint map {:?} → {:?} (face {})",
                    entry.range,
                    entry.family,
                    idx.0,
                );
            }
            None => {
                log::warn!(
                    "config: codepoint_map family {:?} not found in [[font.fallback]], skipping",
                    entry.family,
                );
            }
        }
    }
}

/// Resolve hinting mode from config, falling back to auto-detection.
///
/// Config override takes priority; auto-detection uses display scale factor.
pub(crate) fn resolve_hinting(config: &FontConfig, scale_factor: f64) -> HintingMode {
    match config.hinting.as_deref() {
        Some("full") => HintingMode::Full,
        Some("none") => HintingMode::None,
        Some(other) => {
            log::warn!("config: unknown hinting mode {other:?}, using auto-detection");
            HintingMode::from_scale_factor(scale_factor)
        }
        None => HintingMode::from_scale_factor(scale_factor),
    }
}

/// Resolve subpixel mode from config, falling back to auto-detection.
///
/// Config override takes priority; auto-detection uses display scale factor.
pub(crate) fn resolve_subpixel_mode(config: &FontConfig, scale_factor: f64) -> SubpixelMode {
    match config.subpixel_mode.as_deref() {
        Some("rgb") => SubpixelMode::Rgb,
        Some("bgr") => SubpixelMode::Bgr,
        Some("none") => SubpixelMode::None,
        Some(other) => {
            log::warn!("config: unknown subpixel_mode {other:?}, using auto-detection");
            SubpixelMode::from_scale_factor(scale_factor)
        }
        None => SubpixelMode::from_scale_factor(scale_factor),
    }
}

// ── Color config helpers ──

/// Apply color overrides from [`ColorConfig`](crate::config::ColorConfig) to a palette.
///
/// Sets both live and default values so OSC 104 resets to config values.
pub(crate) fn apply_color_overrides(
    palette: &mut oriterm_core::Palette,
    colors: &config::ColorConfig,
) {
    if let Some(rgb) = colors
        .foreground
        .as_deref()
        .and_then(config::parse_hex_color)
    {
        palette.set_foreground(rgb);
    }
    if let Some(rgb) = colors
        .background
        .as_deref()
        .and_then(config::parse_hex_color)
    {
        palette.set_background(rgb);
    }
    if let Some(rgb) = colors.cursor.as_deref().and_then(config::parse_hex_color) {
        palette.set_cursor_color(rgb);
    }

    // ANSI colors 0–7.
    for (key, hex) in &colors.ansi {
        if let (Ok(idx), Some(rgb)) = (key.parse::<usize>(), config::parse_hex_color(hex)) {
            if idx < 8 {
                palette.set_default(idx, rgb);
            } else {
                log::warn!("config: ansi color index {idx} out of range 0-7");
            }
        }
    }

    // Bright ANSI colors: keys 0–7 map to palette indices 8–15.
    for (key, hex) in &colors.bright {
        if let (Ok(idx), Some(rgb)) = (key.parse::<usize>(), config::parse_hex_color(hex)) {
            if idx < 8 {
                palette.set_default(idx + 8, rgb);
            } else {
                log::warn!("config: bright color index {idx} out of range 0-7");
            }
        }
    }
}
