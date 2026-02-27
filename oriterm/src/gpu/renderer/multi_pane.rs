//! Multi-pane rendering: prepare multiple panes into a single GPU frame.
//!
//! The key insight is that [`fill_frame_shaped`] appends instances to a
//! [`PreparedFrame`] without clearing — so calling it once per pane with
//! different origins accumulates all panes into one frame for a single
//! GPU submission.

use oriterm_core::Rgb;

use super::super::frame_input::FrameInput;
use super::super::instance_writer::ScreenRect;
use super::super::prepare;
use super::super::state::GpuState;
use super::{CombinedAtlasLookup, GpuRenderer};
use crate::gpu::frame_input::ViewportSize;
use oriterm_mux::layout::{DividerLayout, Rect};

use helpers::{ensure_shaped_glyphs_cached, shape_frame};

use super::helpers;

impl GpuRenderer {
    /// Begin a multi-pane frame: reset atlases, clear instance buffers, set viewport.
    ///
    /// Call once before [`prepare_pane`] calls. Sets the viewport and clear
    /// color for the entire window, then clears all instance buffers so pane
    /// instances can accumulate cleanly.
    pub fn begin_multi_pane_frame(
        &mut self,
        viewport: ViewportSize,
        background: Rgb,
        opacity: f64,
    ) {
        self.atlas.begin_frame();
        self.subpixel_atlas.begin_frame();
        self.color_atlas.begin_frame();

        self.prepared.clear();
        self.prepared.viewport = viewport;
        self.prepared.set_clear_color(background, opacity);
    }

    /// Shape, cache, and fill one pane into the shared prepared frame.
    ///
    /// Appends instances to `self.prepared` without clearing — call
    /// [`begin_multi_pane_frame`] once before the first pane. The `origin`
    /// offset positions this pane's cells at its layout-computed pixel rect.
    pub fn prepare_pane(
        &mut self,
        input: &FrameInput,
        gpu: &GpuState,
        origin: (f32, f32),
        cursor_blink_visible: bool,
    ) {
        // Phase A: Shape all rows for this pane.
        shape_frame(input, &self.font_collection, &mut self.shaping);

        // Phase B: Ensure shaped glyphs cached.
        ensure_shaped_glyphs_cached(
            &self.shaping.frame,
            &mut self.atlas,
            &mut self.subpixel_atlas,
            &mut self.color_atlas,
            &mut self.empty_keys,
            &mut self.font_collection,
            &gpu.queue,
        );

        // Phase B2: Built-in geometric glyphs + decoration patterns.
        super::super::builtin_glyphs::ensure_builtins_cached(
            input,
            self.shaping.frame.size_q6(),
            &mut self.atlas,
            &mut self.empty_keys,
            &gpu.queue,
        );

        // Phase C: Fill (append) into the shared prepared frame.
        let bridge = CombinedAtlasLookup {
            mono: &self.atlas,
            subpixel: &self.subpixel_atlas,
            color: &self.color_atlas,
        };
        prepare::fill_frame_shaped(
            input,
            &bridge,
            &self.shaping.frame,
            &mut self.prepared,
            origin,
            cursor_blink_visible,
        );
    }

    /// Append divider rectangles to the backgrounds buffer.
    ///
    /// Dividers are solid-color rectangles between split panes. Drawn into
    /// the background layer so they appear behind glyphs and cursors.
    pub fn append_dividers(&mut self, dividers: &[DividerLayout], color: Rgb) {
        for div in dividers {
            self.prepared.backgrounds.push_rect(
                ScreenRect {
                    x: div.rect.x,
                    y: div.rect.y,
                    w: div.rect.width,
                    h: div.rect.height,
                },
                color,
                1.0,
            );
        }
    }

    /// Append a 2px focus border around the active pane.
    ///
    /// Draws four thin rectangles (top, bottom, left, right) into the cursor
    /// layer so the border renders on top of cell backgrounds and glyphs.
    pub fn append_focus_border(&mut self, rect: &Rect, color: Rgb) {
        let border = 2.0_f32;
        let bx = rect.x;
        let by = rect.y;
        let bw = rect.width;
        let bh = rect.height;

        // Top edge.
        self.prepared.cursors.push_cursor(
            ScreenRect {
                x: bx,
                y: by,
                w: bw,
                h: border,
            },
            color,
            1.0,
        );
        // Bottom edge.
        self.prepared.cursors.push_cursor(
            ScreenRect {
                x: bx,
                y: by + bh - border,
                w: bw,
                h: border,
            },
            color,
            1.0,
        );
        // Left edge.
        self.prepared.cursors.push_cursor(
            ScreenRect {
                x: bx,
                y: by,
                w: border,
                h: bh,
            },
            color,
            1.0,
        );
        // Right edge.
        self.prepared.cursors.push_cursor(
            ScreenRect {
                x: bx + bw - border,
                y: by,
                w: border,
                h: bh,
            },
            color,
            1.0,
        );
    }
}
