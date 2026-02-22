//! Configuration hot-reload — applies config changes without restart.
//!
//! When the config file watcher detects changes, [`App::apply_config_reload`]
//! loads the new config, computes deltas, and applies only what changed:
//! fonts, colors, cursor style, window, behavior, bell, keybindings.

use super::{App, DEFAULT_DPI};
use crate::config::{self, Config};
use crate::font::{FontCollection, FontSet, HintingMode};
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
        if let Some(window) = &self.window {
            window.request_redraw();
        }

        log::info!("config reload: applied successfully");
    }

    /// Detect and apply font changes (family, size, weight, features, fallback).
    fn apply_font_changes(&mut self, new: &Config) {
        let old = &self.config.font;
        let font_changed = (new.font.size - old.size).abs() > f32::EPSILON
            || new.font.family != old.family
            || new.font.weight != old.weight
            || new.font.features != old.features
            || new.font.fallback != old.fallback;

        if !font_changed {
            return;
        }

        let (Some(renderer), Some(gpu), Some(window)) =
            (&mut self.renderer, &self.gpu, &self.window)
        else {
            return;
        };

        let weight = new.font.effective_weight();
        let font_set = match FontSet::load(new.font.family.as_deref(), weight) {
            Ok(fs) => fs,
            Err(e) => {
                log::warn!("config reload: font load failed: {e}");
                return;
            }
        };

        // Get current scale and hinting from the renderer's existing collection.
        let scale = window.scale_factor().factor();
        let cur_hinting = HintingMode::from_scale_factor(scale);
        let cur_format = crate::font::SubpixelMode::from_scale_factor(scale).glyph_format();

        let collection = match FontCollection::new(
            font_set,
            new.font.size,
            DEFAULT_DPI,
            cur_format,
            weight,
            cur_hinting,
        ) {
            Ok(fc) => fc,
            Err(e) => {
                log::warn!("config reload: font collection failed: {e}");
                return;
            }
        };

        let cell = collection.cell_metrics();
        log::info!(
            "config reload: font size={:.1}, cell={}x{}",
            new.font.size,
            cell.width,
            cell.height,
        );

        renderer.replace_font_collection(collection, gpu);

        // Resize grid to match new cell metrics.
        let cell = renderer.cell_metrics();
        let (w, h) = window.size_px();
        let cols = cell.columns(w).max(1);
        let rows = cell.rows(h).max(1);

        if let Some(grid) = &mut self.terminal_grid {
            grid.set_cell_metrics(cell.width, cell.height);
            grid.set_grid_size(cols, rows);
            grid.set_bounds(oriterm_ui::geometry::Rect::new(
                0.0,
                0.0,
                cols as f32 * cell.width,
                rows as f32 * cell.height,
            ));
        }

        if let Some(tab) = &self.tab {
            tab.resize(rows as u16, cols as u16);
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
