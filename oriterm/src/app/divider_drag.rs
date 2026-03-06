//! Divider drag interaction: hover detection, cursor icon, and drag state.
//!
//! Detects when the cursor is over a split divider, changes the cursor icon
//! to a resize handle, and tracks drag state for live ratio updates.

use winit::dpi::PhysicalPosition;
use winit::window::CursorIcon;

use oriterm_mux::PaneId;

use crate::session::{DividerLayout, Rect, SplitDirection};

use super::App;

/// Half the total hit zone width (in physical pixels).
///
/// A 2px divider with a 5px hit zone needs 2.5px padding on each side,
/// but since the divider itself contributes some width, we use half the
/// desired total zone: `(hit_zone - divider_width) / 2`.
const HIT_ZONE_HALF_PAD: f32 = 1.5;

/// Active divider drag state.
pub(crate) struct DividerDragState {
    /// The divider being dragged.
    pub pane_before: PaneId,
    pub pane_after: PaneId,
    pub direction: SplitDirection,
    /// Ratio when drag started.
    pub initial_ratio: f32,
    /// Mouse position (in the drag axis) when drag started, in physical pixels.
    pub origin_px: f32,
    /// Total usable size (width or height of both panes, excluding divider),
    /// in physical pixels.
    pub total_px: f32,
}

impl App {
    /// Expand a divider rect into a hit zone for easier targeting.
    fn divider_hit_rect(divider: &DividerLayout) -> Rect {
        match divider.direction {
            SplitDirection::Vertical => Rect {
                x: divider.rect.x - HIT_ZONE_HALF_PAD,
                y: divider.rect.y,
                width: divider.rect.width + 2.0 * HIT_ZONE_HALF_PAD,
                height: divider.rect.height,
            },
            SplitDirection::Horizontal => Rect {
                x: divider.rect.x,
                y: divider.rect.y - HIT_ZONE_HALF_PAD,
                width: divider.rect.width,
                height: divider.rect.height + 2.0 * HIT_ZONE_HALF_PAD,
            },
        }
    }

    /// Update divider hover state on cursor move.
    ///
    /// Hit-tests the cursor against all dividers in the active tab layout.
    /// Updates the cursor icon to a resize handle when hovering a divider.
    /// Returns `true` if a divider drag is active (caller should skip other
    /// mouse handling).
    pub(super) fn update_divider_hover(&mut self, position: PhysicalPosition<f64>) -> bool {
        // If actively dragging, update the ratio.
        let has_drag = self
            .focused_ctx()
            .is_some_and(|ctx| ctx.divider_drag.is_some());
        if has_drag {
            self.update_divider_drag(position);
            return true;
        }

        let px = position.x as f32;
        let py = position.y as f32;

        // Lazily populate divider cache from the current layout.
        let needs_cache = self
            .focused_ctx()
            .is_some_and(|ctx| ctx.cached_dividers.is_none());
        if needs_cache {
            let dividers = self.compute_pane_layouts().map(|(_, d)| d);
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.cached_dividers = dividers;
            }
        }

        // Find hit divider.
        let hit = self.focused_ctx().and_then(|ctx| {
            ctx.cached_dividers.as_ref().and_then(|dividers| {
                dividers
                    .iter()
                    .find(|d| {
                        let zone = Self::divider_hit_rect(d);
                        zone.contains_point(px, py)
                    })
                    .copied()
            })
        });

        if let Some(d) = hit {
            let icon = match d.direction {
                SplitDirection::Vertical => CursorIcon::ColResize,
                SplitDirection::Horizontal => CursorIcon::RowResize,
            };
            if let Some(ctx) = self.focused_ctx() {
                ctx.window.window().set_cursor(icon);
            }
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.hovering_divider = Some(d);
            }
        } else {
            self.clear_divider_hover();
        }

        false
    }

    /// Clear divider hover state and restore the default cursor.
    pub(super) fn clear_divider_hover(&mut self) {
        let is_hovering = self
            .focused_ctx()
            .is_some_and(|ctx| ctx.hovering_divider.is_some());
        if is_hovering {
            if let Some(ctx) = self.focused_ctx_mut() {
                ctx.hovering_divider = None;
            }
            if let Some(ctx) = self.focused_ctx() {
                ctx.window.window().set_cursor(CursorIcon::Default);
            }
        }
    }

    /// Try to start a divider drag on left-click.
    ///
    /// Returns `true` if a divider drag was started (caller should consume
    /// the click and not forward to selection/reporting).
    pub(super) fn try_start_divider_drag(&mut self) -> bool {
        let divider = match self.focused_ctx().and_then(|ctx| ctx.hovering_divider) {
            Some(d) => d,
            None => return false,
        };

        // Compute initial ratio and total container size from the current
        // pixel layout. This is correct even for deeply nested splits
        // because the pane rects reflect the actual rendered positions.
        let Some((layouts, _)) = self.compute_pane_layouts() else {
            return false;
        };
        let before = layouts.iter().find(|l| l.pane_id == divider.pane_before);
        let after = layouts.iter().find(|l| l.pane_id == divider.pane_after);
        let (Some(b), Some(a)) = (before, after) else {
            return false;
        };

        let (before_size, after_size) = match divider.direction {
            SplitDirection::Vertical => (b.pixel_rect.width, a.pixel_rect.width),
            SplitDirection::Horizontal => (b.pixel_rect.height, a.pixel_rect.height),
        };

        let usable = before_size + after_size;
        if usable <= 0.0 {
            return false;
        }
        let total_px = usable;
        let initial_ratio = before_size / usable;

        let pos = self.mouse.cursor_pos();
        let origin_px = match divider.direction {
            SplitDirection::Vertical => pos.x as f32,
            SplitDirection::Horizontal => pos.y as f32,
        };

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.divider_drag = Some(DividerDragState {
                pane_before: divider.pane_before,
                pane_after: divider.pane_after,
                direction: divider.direction,
                initial_ratio,
                origin_px,
                total_px,
            });
        }

        true
    }

    /// Update divider ratio during an active drag.
    fn update_divider_drag(&mut self, position: PhysicalPosition<f64>) {
        // Extract drag data (Copy fields).
        let (direction, total_px, initial_ratio, origin_px, pane_before, pane_after) = {
            let Some(ctx) = self.focused_ctx() else {
                return;
            };
            let Some(drag) = &ctx.divider_drag else {
                return;
            };
            (
                drag.direction,
                drag.total_px,
                drag.initial_ratio,
                drag.origin_px,
                drag.pane_before,
                drag.pane_after,
            )
        };

        let current_px = match direction {
            SplitDirection::Vertical => position.x as f32,
            SplitDirection::Horizontal => position.y as f32,
        };

        if total_px <= 0.0 {
            return;
        }

        let delta_px = current_px - origin_px;
        let delta_ratio = delta_px / total_px;
        let new_ratio = (initial_ratio + delta_ratio).clamp(0.1, 0.9);

        // Update the tree locally.
        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };
        if let Some(tab) = self.session.get_tab_mut(tab_id) {
            let new_tree = tab
                .tree()
                .set_divider_ratio(pane_before, pane_after, new_ratio);
            if new_tree != *tab.tree() {
                tab.set_tree(new_tree);
            }
        }
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }

    /// Finish a divider drag on mouse release.
    ///
    /// Returns `true` if a drag was active (caller should consume the release).
    pub(super) fn try_finish_divider_drag(&mut self) -> bool {
        let had_drag = self
            .focused_ctx_mut()
            .and_then(|ctx| ctx.divider_drag.take())
            .is_some();
        if had_drag {
            // Final ratio was already committed during drag. Just trigger
            // a resize of all panes to update PTY dimensions.
            self.resize_all_panes();
            true
        } else {
            false
        }
    }

    /// Cancel any active divider drag (e.g., cursor left window).
    pub(super) fn cancel_divider_drag(&mut self) {
        let had_drag = self
            .focused_ctx_mut()
            .and_then(|ctx| ctx.divider_drag.take())
            .is_some();
        if had_drag {
            // Restore to initial state — the tree was updated live, so we'd
            // need the initial tree to truly revert. For now, just accept the
            // last committed ratio and resize panes.
            self.resize_all_panes();
        }
    }
}
