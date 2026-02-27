//! Divider drag interaction: hover detection, cursor icon, and drag state.
//!
//! Detects when the cursor is over a split divider, changes the cursor icon
//! to a resize handle, and tracks drag state for live ratio updates.

use winit::dpi::PhysicalPosition;
use winit::window::CursorIcon;

use oriterm_mux::PaneId;
use oriterm_mux::layout::split_tree::SplitDirection;
use oriterm_mux::layout::{DividerLayout, Rect};

use super::App;

/// Half the total hit zone width (in physical pixels).
///
/// A 2px divider with a 5px hit zone needs 2.5px padding on each side,
/// but since the divider itself contributes some width, we use half the
/// desired total zone: `(hit_zone - divider_width) / 2`.
const HIT_ZONE_HALF_PAD: f32 = 1.5;

/// Active divider drag state.
pub(super) struct DividerDragState {
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
        if self.divider_drag.is_some() {
            self.update_divider_drag(position);
            return true;
        }

        let px = position.x as f32;
        let py = position.y as f32;

        // Lazily populate divider cache from the current layout.
        if self.cached_dividers.is_none() {
            self.cached_dividers = self.compute_pane_layouts().map(|(_, d)| d);
        }
        let Some(dividers) = self.cached_dividers.as_ref() else {
            self.clear_divider_hover();
            return false;
        };

        let hit = dividers.iter().find(|d| {
            let zone = Self::divider_hit_rect(d);
            zone.contains_point(px, py)
        });

        if let Some(d) = hit {
            let icon = match d.direction {
                SplitDirection::Vertical => CursorIcon::ColResize,
                SplitDirection::Horizontal => CursorIcon::RowResize,
            };
            if let Some(window) = &self.window {
                window.window().set_cursor(icon);
            }
            self.hovering_divider = Some(*d);
        } else {
            self.clear_divider_hover();
        }

        false
    }

    /// Clear divider hover state and restore the default cursor.
    pub(super) fn clear_divider_hover(&mut self) {
        if self.hovering_divider.is_some() {
            self.hovering_divider = None;
            if let Some(window) = &self.window {
                window.window().set_cursor(CursorIcon::Default);
            }
        }
    }

    /// Try to start a divider drag on left-click.
    ///
    /// Returns `true` if a divider drag was started (caller should consume
    /// the click and not forward to selection/reporting).
    pub(super) fn try_start_divider_drag(&mut self) -> bool {
        let divider = match self.hovering_divider {
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

        self.divider_drag = Some(DividerDragState {
            pane_before: divider.pane_before,
            pane_after: divider.pane_after,
            direction: divider.direction,
            initial_ratio,
            origin_px,
            total_px,
        });

        true
    }

    /// Update divider ratio during an active drag.
    fn update_divider_drag(&mut self, position: PhysicalPosition<f64>) {
        let drag = match &self.divider_drag {
            Some(d) => d,
            None => return,
        };

        let current_px = match drag.direction {
            SplitDirection::Vertical => position.x as f32,
            SplitDirection::Horizontal => position.y as f32,
        };

        let usable = drag.total_px;
        if usable <= 0.0 {
            return;
        }

        let delta_px = current_px - drag.origin_px;
        let delta_ratio = delta_px / usable;
        let new_ratio = (drag.initial_ratio + delta_ratio).clamp(0.1, 0.9);

        let pane_before = drag.pane_before;
        let pane_after = drag.pane_after;

        // Update the tree through the mux.
        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };
        let Some(mux) = &mut self.mux else { return };
        mux.set_divider_ratio(tab_id, pane_before, pane_after, new_ratio);
        self.dirty = true;
    }

    /// Finish a divider drag on mouse release.
    ///
    /// Returns `true` if a drag was active (caller should consume the release).
    pub(super) fn try_finish_divider_drag(&mut self) -> bool {
        if self.divider_drag.take().is_some() {
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
        if self.divider_drag.take().is_some() {
            // Restore to initial state — the tree was updated live, so we'd
            // need the initial tree to truly revert. For now, just accept the
            // last committed ratio and resize panes.
            self.resize_all_panes();
        }
    }
}
