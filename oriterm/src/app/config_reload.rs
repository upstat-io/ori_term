//! Configuration hot-reload — applies config changes without restart.
//!
//! When the config file watcher detects changes, [`App::apply_config_reload`]
//! loads the new config, computes deltas, and applies only what changed:
//! fonts, colors, cursor style, window, behavior, bell, keybindings.

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
        if new_config.bell != self.config.bell {
            log::info!("config reload: bell settings updated");
        }

        // Store the new config as current.
        self.config = new_config;

        // Update UI chrome theme if the config override changed.
        let new_theme = super::resolve_ui_theme(&self.config);
        if new_theme != self.ui_theme {
            self.ui_theme = new_theme;
            for ctx in self.windows.values_mut() {
                ctx.chrome.apply_theme(&self.ui_theme);
                ctx.tab_bar.apply_theme(&self.ui_theme);
            }
        }

        // Invalidate pane render cache and mark dirty for redraw.
        for ctx in self.windows.values_mut() {
            ctx.pane_cache.invalidate_all();
            ctx.dirty = true;
        }

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

        // Scoped to release renderer/gpu/window borrows before sync_grid_layout.
        let (w, h) = {
            let (Some(renderer), Some(gpu), Some(ctx)) = (
                &mut self.renderer,
                &self.gpu,
                self.focused_window_id.and_then(|id| self.windows.get(&id)),
            ) else {
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
            let scale = ctx.window.scale_factor().factor();
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

            let size = ctx.window.size_px();
            renderer.replace_font_collection(collection, gpu);
            size
        };

        // Grid dimensions, terminal widget, PTY, and resize increments all
        // depend on cell metrics — sync_grid_layout handles them together.
        let Some(winit_id) = self.focused_window_id else {
            return;
        };
        self.sync_grid_layout(winit_id, w, h);
    }

    /// Detect and apply color config changes.
    ///
    /// Resolves the effective theme (honoring config override), resolves the
    /// color scheme, builds the palette, applies overrides, and marks all lines
    /// dirty so colors are re-resolved.
    fn apply_color_changes(&self, new: &Config) {
        if new.colors == self.config.colors {
            return;
        }

        let Some(pane) = self.active_pane() else {
            return;
        };
        let mut term = pane.terminal().lock();

        // Resolve theme: config override takes priority over system detection.
        let theme = new
            .colors
            .resolve_theme(crate::platform::theme::system_theme);

        // Update the terminal's stored theme and rebuild from scheme.
        term.set_theme(theme);
        let palette = build_palette_from_config(&new.colors, theme);

        // Replace the palette and mark all lines dirty.
        *term.palette_mut() = palette;
        term.grid_mut().dirty_mut().mark_all();

        log::info!("config reload: colors updated (theme={theme:?})");
    }

    /// Detect and apply cursor style and blink interval changes.
    fn apply_cursor_changes(&mut self, new: &Config) {
        if new.terminal.cursor_style != self.config.terminal.cursor_style {
            let shape = new.terminal.cursor_style.to_shape();
            if let Some(pane) = self.active_pane() {
                pane.terminal().lock().set_cursor_shape(shape);
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

        let Some(ctx) = self.focused_ctx() else {
            return;
        };
        let opacity = new.window.effective_opacity();
        let blur = new.window.blur && opacity < 1.0;

        ctx.window.set_transparency(opacity, blur);

        log::info!("config reload: window opacity={opacity:.2}, blur={blur}",);
    }

    /// Detect and apply behavior config changes.
    ///
    /// Behavior flags are read from `self.config` at usage sites, so
    /// storing the new config is sufficient. If `bold_is_bright` changed,
    /// marks all lines dirty since existing cells may render differently.
    fn apply_behavior_changes(&self, new: &Config) {
        if new.behavior.bold_is_bright != self.config.behavior.bold_is_bright {
            if let Some(pane) = self.active_pane() {
                pane.terminal().lock().grid_mut().dirty_mut().mark_all();
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

    // Selection color overrides.
    if let Some(rgb) = colors
        .selection_foreground
        .as_deref()
        .and_then(config::parse_hex_color)
    {
        palette.set_selection_fg(Some(rgb));
    }
    if let Some(rgb) = colors
        .selection_background
        .as_deref()
        .and_then(config::parse_hex_color)
    {
        palette.set_selection_bg(Some(rgb));
    }
}

/// Build a palette from the configured color scheme and theme.
///
/// Resolves the scheme name (supporting conditional `"dark:X, light:Y"` syntax),
/// looks up the scheme (built-in then TOML file), builds the palette from scheme
/// colors, and applies user color overrides on top. Falls back to the default
/// theme-based palette if the scheme cannot be found.
pub(crate) fn build_palette_from_config(
    colors: &config::ColorConfig,
    theme: oriterm_core::Theme,
) -> oriterm_core::Palette {
    use crate::scheme;

    let scheme_name = scheme::resolve_scheme_name(&colors.scheme, theme);
    let mut palette = if let Some(s) = scheme::resolve_scheme(scheme_name) {
        log::info!("scheme: resolved '{scheme_name}' -> '{}'", s.name);
        scheme::palette_from_scheme(&s)
    } else {
        if !colors.scheme.is_empty() {
            log::warn!("scheme: '{scheme_name}' not found, using defaults");
        }
        oriterm_core::Palette::for_theme(theme)
    };
    apply_color_overrides(&mut palette, colors);
    palette
}
