//! Draw list conversion: append UI and overlay draw commands to the prepared frame.

use super::super::state::GpuState;
use super::{CombinedAtlasLookup, WindowRenderer};
use crate::font::size_key;

use super::helpers::{ensure_glyphs_cached, ui_text_raster_keys};

/// Which draw list tier to write into.
#[derive(Clone, Copy)]
enum DrawListTier {
    /// Chrome / UI tier (tab bar, window controls).
    Chrome,
    /// Overlay tier (dialogs, menus — renders on top of chrome text).
    Overlay,
}

impl WindowRenderer {
    /// Append UI draw commands **with text** from a [`DrawList`].
    ///
    /// Rasterizes uncached UI text glyphs, converts text commands into glyph
    /// instances, and processes clip commands. Writes to the chrome tier
    /// (draws 6–9).
    pub fn append_ui_draw_list_with_text(
        &mut self,
        draw_list: &oriterm_ui::draw::DrawList,
        scale: f32,
        opacity: f32,
        gpu: &GpuState,
    ) {
        self.append_draw_list_tier(draw_list, scale, opacity, gpu, DrawListTier::Chrome);
    }

    /// Append overlay draw commands **with text** into the overlay tier.
    ///
    /// Same as [`append_ui_draw_list_with_text`](Self::append_ui_draw_list_with_text)
    /// but writes to the overlay buffers (draws 10–13) so overlay content
    /// renders ON TOP of all chrome text.
    pub fn append_overlay_draw_list_with_text(
        &mut self,
        draw_list: &oriterm_ui::draw::DrawList,
        scale: f32,
        opacity: f32,
        gpu: &GpuState,
    ) {
        self.append_draw_list_tier(draw_list, scale, opacity, gpu, DrawListTier::Overlay);
    }

    /// Shared implementation for appending a draw list to a specific tier.
    #[expect(
        clippy::too_many_arguments,
        reason = "tier parameter avoids duplicating the method"
    )]
    fn append_draw_list_tier(
        &mut self,
        draw_list: &oriterm_ui::draw::DrawList,
        scale: f32,
        opacity: f32,
        gpu: &GpuState,
        tier: DrawListTier,
    ) {
        let ui_fc = self
            .ui_font_collection
            .as_mut()
            .unwrap_or(&mut self.font_collection);
        let size_q6 = size_key(ui_fc.size_px());
        let hinted = ui_fc.hinting_mode().hint_flag();

        self.ui_raster_keys.clear();
        ui_text_raster_keys(draw_list, size_q6, hinted, scale, &mut self.ui_raster_keys);
        ensure_glyphs_cached(
            self.ui_raster_keys.iter().copied(),
            &mut self.atlas,
            &mut self.subpixel_atlas,
            &mut self.color_atlas,
            &mut self.empty_keys,
            ui_fc,
            &gpu.queue,
        );

        let bridge = CombinedAtlasLookup {
            mono: &self.atlas,
            subpixel: &self.subpixel_atlas,
            color: &self.color_atlas,
        };

        let vp = &self.prepared.viewport;
        let vw = vp.width;
        let vh = vp.height;

        let (rects, glyphs, subpixel, color, clips) = match tier {
            DrawListTier::Chrome => (
                &mut self.prepared.ui_rects,
                &mut self.prepared.ui_glyphs,
                &mut self.prepared.ui_subpixel_glyphs,
                &mut self.prepared.ui_color_glyphs,
                &mut self.prepared.ui_clips,
            ),
            DrawListTier::Overlay => (
                &mut self.prepared.overlay_rects,
                &mut self.prepared.overlay_glyphs,
                &mut self.prepared.overlay_subpixel_glyphs,
                &mut self.prepared.overlay_color_glyphs,
                &mut self.prepared.overlay_clips,
            ),
        };

        let mut text_ctx = super::super::draw_list_convert::TextContext {
            atlas: &bridge,
            mono_writer: glyphs,
            subpixel_writer: subpixel,
            color_writer: color,
            size_q6,
            hinted,
        };
        let mut clip_ctx = super::super::draw_list_convert::ClipContext {
            clips,
            stack: &mut self.clip_stack,
            viewport_w: vw,
            viewport_h: vh,
        };
        super::super::draw_list_convert::convert_draw_list(
            draw_list,
            rects,
            Some(&mut text_ctx),
            Some(&mut clip_ctx),
            scale,
            opacity,
        );
    }
}
