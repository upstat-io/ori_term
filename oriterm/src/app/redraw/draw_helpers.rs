//! Draw helper methods for window chrome, tab bar, and overlays.
//!
//! Extracted from `mod.rs` to keep the module under the 500-line limit.

use std::cell::Cell;
use std::time::Instant;

use oriterm_ui::draw::DrawList;
use oriterm_ui::overlay::OverlayManager;
use oriterm_ui::theme::UiTheme;
use oriterm_ui::widgets::window_chrome::WindowChromeWidget;
use oriterm_ui::widgets::{DrawCtx, Widget};

use super::super::App;
use crate::font::UiFontMeasurer;
use crate::gpu::state::GpuState;

impl App {
    /// Draw window chrome into the renderer's UI rect layer.
    ///
    /// Chrome widget coordinates are in logical pixels. The `scale` factor
    /// converts logical draw list positions to physical pixels for the GPU
    /// pipeline (`screen_size` uniform is physical).
    ///
    /// Returns `true` if chrome has running animations that need continued
    /// redraws. The `draw_list` is cleared and reused across frames to
    /// avoid per-frame allocation.
    #[expect(
        clippy::too_many_arguments,
        reason = "chrome drawing: widget, renderer, draw list, viewport, scale, theme"
    )]
    pub(in crate::app::redraw) fn draw_chrome(
        chrome: Option<&WindowChromeWidget>,
        renderer: &mut crate::gpu::WindowRenderer,
        draw_list: &mut DrawList,
        logical_width: u32,
        scale: f32,
        theme: &UiTheme,
    ) -> bool {
        let Some(chrome) = chrome else {
            return false;
        };
        if !chrome.is_visible() {
            return false;
        }

        // Build draw list with real measurer (immutable borrow on renderer
        // ends after chrome.draw — NLL lets the mutable append follow).
        draw_list.clear();
        let animations_running = Cell::new(false);
        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
        let caption_h = chrome.caption_height();
        let bounds = oriterm_ui::geometry::Rect::new(0.0, 0.0, logical_width as f32, caption_h);

        let mut ctx = DrawCtx {
            measurer: &measurer,
            draw_list,
            bounds,
            focused_widget: None,
            now: Instant::now(),
            animations_running: &animations_running,
            theme,
        };
        chrome.draw(&mut ctx);
        let animating = animations_running.get();

        // Chrome uses geometric symbols only — no text context needed.
        renderer.append_ui_draw_list(draw_list, scale, 1.0);
        animating
    }

    /// Draw the tab bar below the window chrome caption.
    ///
    /// Tab bar coordinates are in logical pixels, positioned at `y = caption_h`.
    /// Uses [`append_ui_draw_list_with_text`](crate::gpu::WindowRenderer::append_ui_draw_list_with_text)
    /// because tab titles are rendered as shaped text.
    ///
    /// Returns `true` if the tab bar has running animations (e.g. bell pulse).
    #[expect(
        clippy::too_many_arguments,
        reason = "tab bar drawing: widget, renderer, draw list, viewport, caption offset, scale, GPU, theme"
    )]
    pub(in crate::app::redraw) fn draw_tab_bar(
        tab_bar: Option<&oriterm_ui::widgets::tab_bar::TabBarWidget>,
        renderer: &mut crate::gpu::WindowRenderer,
        draw_list: &mut DrawList,
        logical_width: f32,
        caption_h: f32,
        scale: f32,
        gpu: &GpuState,
        theme: &UiTheme,
    ) -> bool {
        let Some(tab_bar) = tab_bar else {
            return false;
        };
        if tab_bar.tab_count() == 0 {
            return false;
        }

        let tab_bar_h = oriterm_ui::widgets::tab_bar::constants::TAB_BAR_HEIGHT;
        let bounds = oriterm_ui::geometry::Rect::new(0.0, caption_h, logical_width, tab_bar_h);

        draw_list.clear();
        let animations_running = Cell::new(false);
        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);

        let mut ctx = DrawCtx {
            measurer: &measurer,
            draw_list,
            bounds,
            focused_widget: None,
            now: Instant::now(),
            animations_running: &animations_running,
            theme,
        };
        tab_bar.draw(&mut ctx);
        let animating = animations_running.get();

        // Tab bar contains text — use text-aware conversion to rasterize
        // tab title glyphs into the UI overlay layer.
        renderer.append_ui_draw_list_with_text(draw_list, scale, 1.0, gpu);
        animating
    }

    /// Draw overlays (active + dismissing) with per-overlay compositor opacity.
    ///
    /// Each overlay is drawn individually so its compositor layer opacity
    /// can be applied independently (e.g. during simultaneous fade-in/fade-out).
    /// Modal dim rects are emitted before their content overlay.
    ///
    /// Returns `true` if overlays have running animations (fade-in/fade-out).
    #[expect(
        clippy::too_many_arguments,
        reason = "overlay drawing: manager, renderer, draw list, viewport, scale, GPU, tree, theme"
    )]
    pub(in crate::app::redraw) fn draw_overlays(
        overlays: &mut OverlayManager,
        renderer: &mut crate::gpu::WindowRenderer,
        draw_list: &mut DrawList,
        logical_size: (f32, f32),
        scale: f32,
        gpu: &GpuState,
        tree: &oriterm_ui::compositor::layer_tree::LayerTree,
        theme: &UiTheme,
    ) -> bool {
        let count = overlays.draw_count();
        if count == 0 {
            return false;
        }

        let bounds = oriterm_ui::geometry::Rect::new(0.0, 0.0, logical_size.0, logical_size.1);
        let animations_running = Cell::new(false);
        let mut animating = false;

        // Layout + draw phase: measurer borrows renderer immutably, then
        // drops before the mutable append_ui_draw_list_with_text call.
        // We collect (opacity) per overlay, then append after the borrow ends.
        {
            let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
            overlays.layout_overlays(&measurer, theme);
        }

        for i in 0..count {
            draw_list.clear();
            // Re-create measurer per iteration — cheap (no allocation), and
            // the immutable borrow drops before the mutable append below.
            let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
            let mut ctx = DrawCtx {
                measurer: &measurer,
                draw_list,
                bounds,
                focused_widget: None,
                now: Instant::now(),
                animations_running: &animations_running,
                theme,
            };
            let opacity = overlays.draw_overlay_at(i, &mut ctx, tree);

            // If opacity is < 1.0 an animation is running.
            if opacity < 1.0 - f32::EPSILON {
                animating = true;
            }

            // measurer (immutable borrow on renderer) is dropped here by NLL.
            // Overlays write to the overlay tier (draws 10–13) so their
            // backgrounds render ON TOP of chrome text (draws 7–9).
            renderer.append_overlay_draw_list_with_text(draw_list, scale, opacity, gpu);
        }

        animating || animations_running.get()
    }
}
