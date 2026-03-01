//! Chrome-style tab drag state machine.
//!
//! Handles within-bar reorder: click-vs-drag disambiguation via threshold,
//! pixel-perfect visual tracking, center-based insertion index, and post-drag
//! settle animation. Tear-off detection signals Section 17.2.

#[cfg(target_os = "windows")]
mod merge;
#[cfg(target_os = "windows")]
mod tear_off;

use oriterm_mux::TabId;

use oriterm_ui::widgets::tab_bar::constants::{
    DRAG_START_THRESHOLD, TAB_BAR_HEIGHT, TAB_LEFT_MARGIN, TEAR_OFF_THRESHOLD,
    TEAR_OFF_THRESHOLD_UP,
};

use super::App;

/// Drag phase: disambiguates click from drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DragPhase {
    /// Mouse down, hasn't moved enough to start dragging.
    Pending,
    /// Reordering within the tab strip.
    DraggingInBar,
}

/// Active tab drag state.
///
/// Created on mouse-down over a tab, destroyed on mouse-up or cancel.
/// All coordinates are in logical pixels.
pub(crate) struct TabDragState {
    /// Mux tab ID (for tear-off handoff and merge).
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub tab_id: TabId,
    /// Tab index when drag started (for Escape undo).
    pub original_index: usize,
    /// Current tab index (updated on each swap).
    pub current_index: usize,
    /// Cursor X at mouse-down.
    pub origin_x: f32,
    /// Cursor Y at mouse-down.
    pub origin_y: f32,
    /// Current drag phase.
    pub phase: DragPhase,
    /// Cursor offset from the tab's left edge at drag start.
    pub mouse_offset_in_tab: f32,
    /// Tab bar top edge (caption height) in logical pixels.
    pub tab_bar_y: f32,
    /// Tab bar bottom edge in logical pixels.
    pub tab_bar_bottom: f32,
}

/// Pending tear-off state for OS-level drag.
///
/// Set by [`App::tear_off_tab()`] when a tab exceeds the tear-off threshold,
/// consumed by [`App::check_torn_off_merge()`] in `about_to_wait` after the
/// OS modal drag loop completes.
#[cfg(target_os = "windows")]
pub(crate) struct TornOffPending {
    /// Winit ID of the torn-off window (the new, single-tab window).
    pub winit_id: winit::window::WindowId,
    /// Mux tab ID of the dragged tab.
    pub tab_id: TabId,
    /// Cursor offset from the tab's left edge at drag start.
    pub mouse_offset: f32,
}

// -- Pure computation helpers (testable without App) --

/// Clamp the dragged tab's visual X position.
///
/// `cursor_x` is the current cursor X, `offset` is the cursor-to-tab-left
/// distance, and `max_x` is the rightmost allowed position (right edge of
/// tab zone minus one tab width).
pub(crate) fn compute_drag_visual_x(cursor_x: f32, offset: f32, max_x: f32) -> f32 {
    (cursor_x - offset).clamp(0.0, max_x)
}

/// Compute the insertion index from the dragged tab's visual center.
///
/// Uses center-based logic: the tab center determines which slot it occupies.
/// Result is clamped to `[0, tab_count - 1]`.
pub(crate) fn compute_insertion_index(visual_x: f32, tab_width: f32, tab_count: usize) -> usize {
    if tab_count == 0 || tab_width <= 0.0 {
        return 0;
    }
    let center = visual_x + tab_width / 2.0;
    let raw = ((center - TAB_LEFT_MARGIN) / tab_width).floor();
    raw.clamp(0.0, (tab_count - 1) as f32) as usize
}

/// Check whether the cursor has exceeded the tear-off threshold.
///
/// Uses directional thresholds: upward drag is more sensitive
/// (`TEAR_OFF_THRESHOLD_UP`) than downward (`TEAR_OFF_THRESHOLD`).
pub(crate) fn exceeds_tear_off(cursor_y: f32, bar_y: f32, bar_bottom: f32) -> bool {
    if cursor_y < bar_y {
        // Above the bar: use reduced threshold.
        (bar_y - cursor_y) > TEAR_OFF_THRESHOLD_UP
    } else if cursor_y > bar_bottom {
        // Below the bar: use full threshold.
        (cursor_y - bar_bottom) > TEAR_OFF_THRESHOLD
    } else {
        false
    }
}

// -- App integration methods --

impl App {
    /// Start a pending tab drag on mouse-down over a tab.
    ///
    /// Creates a `Pending` drag state. The threshold check happens on
    /// subsequent cursor moves in [`update_tab_drag`]. Returns `true` if
    /// the drag was successfully initiated.
    pub(super) fn try_start_tab_drag(&mut self, tab_index: usize) -> bool {
        let pos = self.mouse.cursor_pos();

        // Extract layout data and compute logical coordinates.
        let (scale, tab_width, caption_h, tab_id) = {
            let Some(ctx) = self.focused_ctx() else {
                return false;
            };
            let scale = ctx.window.scale_factor().factor() as f32;
            let tw = ctx.tab_bar.layout().tab_width;
            let caption_h = ctx.chrome.caption_height();

            // Resolve the mux TabId from the index.
            let Some(mux) = self.mux.as_ref() else {
                return false;
            };
            let Some(win_id) = self.active_window else {
                return false;
            };
            let Some(win) = mux.session().get_window(win_id) else {
                return false;
            };
            let Some(&tid) = win.tabs().get(tab_index) else {
                return false;
            };
            (scale, tw, caption_h, tid)
        };

        let logical_x = pos.x as f32 / scale;
        let logical_y = pos.y as f32 / scale;

        // Compute cursor offset from the tab's left edge.
        let tab_left = TAB_LEFT_MARGIN + tab_index as f32 * tab_width;
        let offset = logical_x - tab_left;

        // Acquire width lock to prevent layout jitter during drag.
        self.acquire_tab_width_lock(tab_width);

        let state = TabDragState {
            tab_id,
            original_index: tab_index,
            current_index: tab_index,
            origin_x: logical_x,
            origin_y: logical_y,
            phase: DragPhase::Pending,
            mouse_offset_in_tab: offset,
            tab_bar_y: caption_h,
            tab_bar_bottom: caption_h + TAB_BAR_HEIGHT,
        };

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.tab_drag = Some(state);
        }
        true
    }

    /// Update an active tab drag on cursor move.
    ///
    /// Handles threshold-based phase transitions, visual tracking, reorder
    /// swaps, and tear-off detection. Returns `true` if the event was
    /// consumed (caller should skip other mouse handling).
    pub(super) fn update_tab_drag(
        &mut self,
        position: winit::dpi::PhysicalPosition<f64>,
        #[cfg(target_os = "windows")] event_loop: &winit::event_loop::ActiveEventLoop,
    ) -> bool {
        // Extract drag data (all Copy fields) to break the borrow chain.
        let drag_info = {
            let Some(ctx) = self.focused_ctx() else {
                return false;
            };
            let Some(drag) = &ctx.tab_drag else {
                return false;
            };
            DragInfo {
                phase: drag.phase,
                origin_x: drag.origin_x,
                origin_y: drag.origin_y,
                current_index: drag.current_index,
                mouse_offset: drag.mouse_offset_in_tab,
                bar_y: drag.tab_bar_y,
                bar_bottom: drag.tab_bar_bottom,
                scale: ctx.window.scale_factor().factor() as f32,
                tab_count: ctx.tab_bar.layout().tab_count,
            }
        };

        let logical_x = position.x as f32 / drag_info.scale;
        let logical_y = position.y as f32 / drag_info.scale;

        match drag_info.phase {
            DragPhase::Pending => {
                let dx = logical_x - drag_info.origin_x;
                let dy = logical_y - drag_info.origin_y;
                let distance = dx.hypot(dy);

                if distance < DRAG_START_THRESHOLD {
                    // Not yet past threshold — stay pending but consume the event.
                    return true;
                }

                // Single-tab window: skip DraggingInBar, go directly to
                // OS-level drag with merge detection.
                #[cfg(target_os = "windows")]
                if drag_info.tab_count <= 1 {
                    self.begin_single_tab_os_drag(event_loop);
                    return true;
                }

                // Transition to DraggingInBar.
                if let Some(ctx) = self.focused_ctx_mut() {
                    if let Some(drag) = &mut ctx.tab_drag {
                        drag.phase = DragPhase::DraggingInBar;
                    }
                }
                // Fall through to DraggingInBar handling below.
                self.update_drag_in_bar(
                    logical_x,
                    logical_y,
                    drag_info,
                    #[cfg(target_os = "windows")]
                    event_loop,
                );
                true
            }
            DragPhase::DraggingInBar => {
                self.update_drag_in_bar(
                    logical_x,
                    logical_y,
                    drag_info,
                    #[cfg(target_os = "windows")]
                    event_loop,
                );
                true
            }
        }
    }

    /// Core in-bar drag logic: visual tracking, reorder, tear-off check.
    fn update_drag_in_bar(
        &mut self,
        logical_x: f32,
        logical_y: f32,
        info: DragInfo,
        #[cfg(target_os = "windows")] event_loop: &winit::event_loop::ActiveEventLoop,
    ) {
        // Tear-off detection.
        if exceeds_tear_off(logical_y, info.bar_y, info.bar_bottom) {
            #[cfg(target_os = "windows")]
            self.tear_off_tab(event_loop);
            #[cfg(not(target_os = "windows"))]
            log::debug!("tab drag: tear-off not supported on this platform");
            return;
        }

        // Compute visual X and insertion index.
        let (tab_width, tab_count, max_x) = {
            let Some(ctx) = self.focused_ctx() else {
                return;
            };
            let layout = ctx.tab_bar.layout();
            let tw = layout.tab_width;
            let tc = layout.tab_count;
            // Max X: right edge of the tab zone minus one tab width.
            let max = (layout.tabs_end() - tw).max(0.0);
            (tw, tc, max)
        };

        let visual_x = compute_drag_visual_x(logical_x, info.mouse_offset, max_x);
        let new_index = compute_insertion_index(visual_x, tab_width, tab_count);

        // Reorder if the insertion index changed.
        let visual_index = if new_index == info.current_index {
            info.current_index
        } else {
            self.reorder_tab_silent(info.current_index, new_index);
            if let Some(ctx) = self.focused_ctx_mut() {
                if let Some(drag) = &mut ctx.tab_drag {
                    drag.current_index = new_index;
                }
            }
            new_index
        };

        // Update the drag visual (exactly once per cursor move).
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.tab_bar.set_drag_visual(Some((visual_index, visual_x)));
            ctx.dirty = true;
        }
    }

    /// Finish a tab drag on mouse-up.
    ///
    /// If still `Pending`, the gesture was a click (no-op — tab switch
    /// already happened). If `DraggingInBar`, clears the drag visual and
    /// starts a settle animation. Returns `true` if a drag was active.
    pub(super) fn try_finish_tab_drag(&mut self) -> bool {
        let drag_state = self.focused_ctx_mut().and_then(|ctx| ctx.tab_drag.take());

        let Some(drag) = drag_state else {
            return false;
        };

        // Clear drag visual.
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.tab_bar.set_drag_visual(None);
            ctx.dirty = true;
        }

        match drag.phase {
            DragPhase::Pending => {
                // Click, not drag. Release width lock.
                self.release_tab_width_lock();
            }
            DragPhase::DraggingInBar => {
                // Start settle animation for the dragged tab snapping to
                // its final slot. The tab moved from original_index to
                // current_index; animate displaced tabs.
                if drag.original_index != drag.current_index {
                    let tab_width = self
                        .focused_ctx()
                        .map_or(0.0, |ctx| ctx.tab_bar.layout().tab_width);
                    self.start_tab_reorder_slide(
                        drag.original_index,
                        drag.current_index,
                        tab_width,
                    );
                }
                self.release_tab_width_lock();
            }
        }

        true
    }

    /// Cancel an active tab drag (Escape or cursor-leave).
    ///
    /// Restores the tab to its original position if it was moved.
    pub(super) fn cancel_tab_drag(&mut self) {
        let drag_state = self.focused_ctx_mut().and_then(|ctx| ctx.tab_drag.take());

        let Some(drag) = drag_state else {
            return;
        };

        // Clear drag visual.
        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.tab_bar.set_drag_visual(None);
            ctx.dirty = true;
        }

        // If the tab was moved, restore it to the original position.
        if drag.phase == DragPhase::DraggingInBar && drag.current_index != drag.original_index {
            self.reorder_tab_silent(drag.current_index, drag.original_index);
        }

        self.release_tab_width_lock();
    }

    /// Whether a tab drag is currently active.
    pub(super) fn has_tab_drag(&self) -> bool {
        self.focused_ctx().is_some_and(|ctx| ctx.tab_drag.is_some())
    }

    /// Reorder a tab in the mux and sync the tab bar, without animation.
    ///
    /// During an active drag, the visual position is controlled by
    /// `drag_visual_x`, not the compositor. Animation is triggered
    /// separately in `try_finish_tab_drag`.
    fn reorder_tab_silent(&mut self, from: usize, to: usize) {
        let Some(mux) = &mut self.mux else { return };
        let Some(win_id) = self.active_window else {
            return;
        };

        if !mux.reorder_tab(win_id, from, to) {
            return;
        }
        self.sync_tab_bar_from_mux();

        if let Some(ctx) = self.focused_ctx_mut() {
            ctx.dirty = true;
        }
    }
}

/// Extracted drag info (all `Copy` fields) to break borrow chains.
#[derive(Clone, Copy)]
struct DragInfo {
    phase: DragPhase,
    origin_x: f32,
    origin_y: f32,
    current_index: usize,
    mouse_offset: f32,
    bar_y: f32,
    bar_bottom: f32,
    scale: f32,
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    tab_count: usize,
}

#[cfg(test)]
mod tests;
