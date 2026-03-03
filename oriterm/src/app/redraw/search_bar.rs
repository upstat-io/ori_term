//! Search bar overlay rendering.

use std::fmt::Write as _;

use oriterm_ui::draw::DrawList;
use oriterm_ui::geometry::Point;
use oriterm_ui::widgets::status_badge::StatusBadge;

use super::App;
use crate::font::UiFontMeasurer;
use crate::gpu::FrameSearch;
use crate::gpu::state::GpuState;

impl App {
    /// Draw the search bar overlay above the grid area.
    ///
    /// Shows the current query and match count ("N of M") as a floating
    /// [`StatusBadge`]. Coordinates are in logical pixels; `scale` converts
    /// to physical pixels for the GPU pipeline.
    #[expect(
        clippy::too_many_arguments,
        reason = "search bar drawing: search state, renderer, draw list, buffer, viewport, caption, scale, GPU"
    )]
    pub(in crate::app::redraw) fn draw_search_bar(
        search: &FrameSearch,
        renderer: &mut crate::gpu::GpuRenderer,
        draw_list: &mut DrawList,
        buf: &mut String,
        logical_width: f32,
        caption_h: f32,
        scale: f32,
        gpu: &GpuState,
    ) {
        buf.clear();
        let query = search.query();
        if query.is_empty() {
            buf.push_str("Search: ");
        } else if search.match_count() == 0 {
            let _ = write!(buf, "Search: {query}  No matches");
        } else {
            let _ = write!(
                buf,
                "Search: {query}  {} of {}",
                search.focused_display(),
                search.match_count()
            );
        }

        let badge = StatusBadge::new(buf);

        // Shape text and measure badge (immutable borrow on renderer ends
        // after shape — NLL lets the mutable append follow).
        let max_text_w = logical_width * 0.4;
        let measurer = UiFontMeasurer::new(renderer.active_ui_collection(), scale);
        let (w, _h) = badge.measure(&measurer, max_text_w);

        // Position: top-right of grid area, inset from edges.
        let margin = 8.0;
        let pos = Point::new(logical_width - w - margin, caption_h + margin);

        draw_list.clear();
        let _ = badge.draw(draw_list, &measurer, pos, max_text_w);

        renderer.append_ui_draw_list_with_text(draw_list, scale, 1.0, gpu);
    }
}
