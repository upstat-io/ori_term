//! Rendering snapshot extraction from terminal state.
//!
//! Extracted from `term/mod.rs` to keep the main file under the 500-line
//! limit. These methods build `RenderableContent` and manage damage state.

use crate::event::EventListener;
use crate::grid::CursorShape;
use crate::index::Column;

use super::Term;
use super::mode::TermMode;
use super::renderable::{
    self, RenderableCell, RenderableContent, RenderableCursor, RenderableImageData,
    RenderablePlacement, TermDamage,
};

impl<T: EventListener> Term<T> {
    /// Extract a complete rendering snapshot.
    ///
    /// Convenience wrapper that allocates a fresh [`RenderableContent`] and
    /// fills it. For hot-path rendering, prefer [`renderable_content_into`]
    /// with a reused buffer to avoid per-frame allocation.
    ///
    /// This is a pure read — dirty state is **not** cleared. Callers must
    /// drain dirty state separately via `grid_mut().dirty_mut().drain()`
    /// after consuming the snapshot.
    ///
    /// [`renderable_content_into`]: Self::renderable_content_into
    pub fn renderable_content(&self) -> RenderableContent {
        let grid = self.grid();
        let mut out = RenderableContent {
            cells: Vec::with_capacity(grid.lines() * grid.cols()),
            cursor: RenderableCursor {
                line: 0,
                column: Column(0),
                shape: CursorShape::default(),
                visible: false,
            },
            display_offset: 0,
            stable_row_base: 0,
            mode: TermMode::empty(),
            all_dirty: false,
            damage: Vec::new(),
            images: Vec::new(),
            image_data: Vec::new(),
            images_dirty: false,
        };
        self.renderable_content_into(&mut out);
        out
    }

    /// Fill an existing [`RenderableContent`] with the current terminal state.
    ///
    /// Clears `out` and refills it, reusing the underlying `Vec` allocations.
    /// The renderer should keep a single `RenderableContent` and pass it each
    /// frame to avoid the ~`lines * cols * 56` byte allocation that
    /// [`renderable_content`] performs.
    ///
    /// This is a pure read — dirty state is **not** cleared. Callers must
    /// drain dirty state separately via `grid_mut().dirty_mut().drain()`
    /// after consuming the snapshot.
    ///
    /// [`renderable_content`]: Self::renderable_content
    pub fn renderable_content_into(&self, out: &mut RenderableContent) {
        out.cells.clear();
        out.damage.clear();

        let grid = self.grid();
        let raw_offset = grid.display_offset();
        debug_assert!(
            raw_offset <= grid.scrollback().len(),
            "display_offset ({raw_offset}) must be <= scrollback.len() ({})",
            grid.scrollback().len(),
        );
        let offset = raw_offset.min(grid.scrollback().len());
        let lines = grid.lines();
        let cols = grid.cols();
        let palette = &self.palette;

        for vis_line in 0..lines {
            // Top `offset` lines come from scrollback; the rest from the grid.
            let row = if vis_line < offset {
                let sb_idx = offset - 1 - vis_line;
                match grid.scrollback().get(sb_idx) {
                    Some(row) => row,
                    None => continue,
                }
            } else {
                let grid_line = vis_line - offset;
                &grid[crate::index::Line(grid_line as i32)]
            };

            for col_idx in 0..cols {
                let col = Column(col_idx);
                let cell = &row[col];

                let fg = renderable::resolve_fg(cell.fg, cell.flags, palette);
                let bg = renderable::resolve_bg(cell.bg, palette);
                let (fg, bg) = renderable::apply_inverse(fg, bg, cell.flags);

                let (underline_color, has_hyperlink, zerowidth) = match cell.extra.as_ref() {
                    Some(e) => (
                        e.underline_color.map(|c| palette.resolve(c)),
                        e.hyperlink.is_some(),
                        e.zerowidth.clone(),
                    ),
                    None => (None, false, Vec::new()),
                };

                out.cells.push(RenderableCell {
                    line: vis_line,
                    column: col,
                    ch: cell.ch,
                    fg,
                    bg,
                    flags: cell.flags,
                    underline_color,
                    has_hyperlink,
                    zerowidth,
                });
            }
        }

        // Cursor is visible when SHOW_CURSOR is set and we're at the live view.
        let cursor_visible = self.mode.contains(TermMode::SHOW_CURSOR)
            && offset == 0
            && self.cursor_shape != CursorShape::Hidden;

        out.cursor = RenderableCursor {
            line: grid.cursor().line(),
            column: grid.cursor().col(),
            shape: self.cursor_shape,
            visible: cursor_visible,
        };

        out.all_dirty = renderable::collect_damage(grid, lines, cols, &mut out.damage);
        out.display_offset = offset;
        let base_abs = grid.scrollback().len().saturating_sub(offset);
        out.stable_row_base = grid.total_evicted() as u64 + base_abs as u64;
        out.mode = self.mode;

        // Image placements visible in the viewport.
        Self::extract_images(
            self.image_cache(),
            out.stable_row_base,
            lines,
            self.cell_pixel_width,
            self.cell_pixel_height,
            &mut out.images,
            &mut out.image_data,
        );

        // Propagate image dirty flag. When images changed, force a full
        // viewport repaint since image mutations don't set per-line grid
        // dirty flags. The dirty flag is cleared by `reset_damage()`.
        out.images_dirty = self.image_cache().is_dirty();
        if out.images_dirty {
            out.all_dirty = true;
        }
    }

    /// Drain damage from the active grid.
    ///
    /// Returns a [`TermDamage`] iterator that yields dirty lines and clears
    /// marks as it goes. Check [`TermDamage::is_all_dirty`] first — when true,
    /// repaint everything and drop the iterator (which clears remaining marks).
    /// Also clears the image cache dirty flag.
    pub fn damage(&mut self) -> TermDamage<'_> {
        self.image_cache_mut().take_dirty();
        let grid = self.grid_mut();
        let cols = grid.cols();
        let all_dirty = grid.dirty().is_all_dirty();
        TermDamage::new(grid.dirty_mut().drain(), cols, all_dirty)
    }

    /// Clear all damage marks without reading them.
    ///
    /// Called when the renderer wants to discard pending damage (e.g. after
    /// a full repaint that doesn't need per-line tracking). Also clears the
    /// image cache dirty flag.
    pub fn reset_damage(&mut self) {
        self.grid_mut().dirty_mut().drain().for_each(drop);
        self.image_cache_mut().take_dirty();
    }

    /// Extract visible image placements and their pixel data.
    ///
    /// Converts `ImagePlacement` cell coordinates to viewport pixel positions
    /// and collects the decoded RGBA data for GPU texture upload.
    #[expect(clippy::too_many_arguments, reason = "image extraction parameters")]
    fn extract_images(
        cache: &crate::image::ImageCache,
        stable_row_base: u64,
        viewport_lines: usize,
        cell_w: u16,
        cell_h: u16,
        images: &mut Vec<RenderablePlacement>,
        image_data: &mut Vec<RenderableImageData>,
    ) {
        images.clear();
        image_data.clear();

        if cache.placement_count() == 0 {
            return;
        }

        let top = crate::grid::StableRowIndex(stable_row_base);
        let bottom =
            crate::grid::StableRowIndex(stable_row_base + viewport_lines.saturating_sub(1) as u64);

        let visible = cache.placements_in_viewport(top, bottom);
        if visible.is_empty() {
            return;
        }

        let cw = f32::from(cell_w);
        let ch = f32::from(cell_h);

        // Collect unique image IDs for pixel data.
        let mut seen_ids: std::collections::HashSet<crate::image::ImageId> =
            std::collections::HashSet::new();

        for p in &visible {
            // Signed offset: images starting above the viewport have negative Y,
            // so their visible bottom portion renders correctly. The GPU clips
            // fragments outside the framebuffer (implicit viewport scissor).
            let row_offset = p.cell_row.0 as i64 - stable_row_base as i64;
            let vp_x = p.cell_col as f32 * cw + f32::from(p.cell_x_offset);
            let vp_y = row_offset as f32 * ch + f32::from(p.cell_y_offset);

            let (disp_w, disp_h) = match p.sizing {
                crate::image::PlacementSizing::CellCount => {
                    (p.cols as f32 * cw, p.rows as f32 * ch)
                }
                crate::image::PlacementSizing::FixedPixels { width, height } => {
                    (width as f32, height as f32)
                }
            };

            // Compute UV source rect (normalized 0..1 within the image).
            let (src_x, src_y, src_w, src_h) = if let Some(img) = cache.get_no_touch(p.image_id) {
                let iw = img.width as f32;
                let ih = img.height as f32;
                if iw > 0.0 && ih > 0.0 {
                    let sx = p.source_x as f32 / iw;
                    let sy = p.source_y as f32 / ih;
                    let sw = if p.source_w > 0 {
                        p.source_w as f32 / iw
                    } else {
                        1.0 - sx
                    };
                    let sh = if p.source_h > 0 {
                        p.source_h as f32 / ih
                    } else {
                        1.0 - sy
                    };
                    (sx, sy, sw, sh)
                } else {
                    (0.0, 0.0, 1.0, 1.0)
                }
            } else {
                // Image data missing — skip this placement.
                continue;
            };

            images.push(RenderablePlacement {
                image_id: p.image_id,
                viewport_x: vp_x,
                viewport_y: vp_y,
                display_width: disp_w,
                display_height: disp_h,
                source_x: src_x,
                source_y: src_y,
                source_w: src_w,
                source_h: src_h,
                z_index: p.z_index,
                opacity: 1.0,
            });

            seen_ids.insert(p.image_id);
        }

        // Collect pixel data for referenced images.
        for id in seen_ids {
            if let Some(img) = cache.get_no_touch(id) {
                image_data.push(RenderableImageData {
                    id,
                    data: img.data.clone(),
                    width: img.width,
                    height: img.height,
                });
            }
        }
    }
}
