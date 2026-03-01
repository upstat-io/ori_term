//! Tab tear-off: detach a tab into a new window and start an OS-level drag.
//!
//! When the cursor exceeds the tear-off threshold during an in-bar drag, this
//! module creates a new window for the tab and initiates a `drag_window()` OS
//! drag session. The platform layer (`oriterm_ui::platform_windows`) handles
//! `WM_MOVING` position correction and merge rect detection.

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use oriterm_mux::TabId;
use oriterm_ui::platform_windows::{self, OsDragConfig};
use oriterm_ui::widgets::tab_bar::constants::{
    CONTROLS_ZONE_WIDTH, TAB_BAR_HEIGHT, TAB_LEFT_MARGIN,
};

use super::TornOffPending;
use crate::app::App;

impl App {
    /// Tear off the currently dragged tab into a new window.
    ///
    /// Extracts the drag state, creates a bare window, moves the tab via mux,
    /// positions the window under the cursor, and starts an OS drag session.
    pub(super) fn tear_off_tab(&mut self, event_loop: &ActiveEventLoop) {
        // Extract drag state from the source window.
        let (tab_id, mouse_offset, origin_y, source_winit_id) = {
            let Some(ctx) = self.focused_ctx_mut() else {
                return;
            };
            let Some(drag) = ctx.tab_drag.take() else {
                return;
            };
            // Clear drag visual on source.
            ctx.tab_bar.set_drag_visual(None);
            ctx.dirty = true;
            let wid = ctx.window.window_id();
            (drag.tab_id, drag.mouse_offset_in_tab, drag.origin_y, wid)
        };

        // Release width lock on source window.
        self.release_tab_width_lock();

        // Refuse to tear off the last tab in the session.
        let is_last = self
            .mux
            .as_ref()
            .is_some_and(|m| m.session().tab_count() <= 1);
        if is_last {
            log::warn!("tear_off_tab: refused — last tab in session");
            return;
        }

        // Create bare window (hidden, no tabs).
        let Some((new_winit_id, new_mux_wid)) = self.create_window_bare(event_loop) else {
            return;
        };

        // Move tab from source window to new window via mux.
        let moved = {
            let Some(mux) = &mut self.mux else { return };
            mux.move_tab_to_window_at(tab_id, new_mux_wid, 0)
        };
        if !moved {
            log::error!("tear_off_tab: move_tab_to_window_at failed");
            self.remove_empty_window(new_winit_id);
            return;
        }

        // Drain mux notifications from the move.
        self.pump_mux_events();

        // Sync tab bars on both windows.
        self.sync_tab_bar_for_window(source_winit_id);
        self.sync_tab_bar_for_window(new_winit_id);

        // Compute grab offset: where the cursor anchors to the new window.
        let (grab_offset, screen_pos) = {
            let Some(ctx) = self.windows.get(&new_winit_id) else {
                return;
            };
            let scale = ctx.window.scale_factor().factor() as f32;
            let grab_x = ((TAB_LEFT_MARGIN + mouse_offset) * scale).round() as i32;
            let grab_y = (origin_y * scale).round() as i32;
            let cursor = platform_windows::cursor_screen_pos();
            let pos_x = cursor.0 - grab_x;
            let pos_y = cursor.1 - grab_y;
            ((grab_x, grab_y), (pos_x, pos_y))
        };

        // Position the new window so the cursor is at the grab offset.
        if let Some(ctx) = self.windows.get(&new_winit_id) {
            ctx.window
                .window()
                .set_outer_position(winit::dpi::PhysicalPosition::new(
                    screen_pos.0,
                    screen_pos.1,
                ));
        }

        // Clear surface with background color before showing.
        let theme = self
            .config
            .colors
            .resolve_theme(crate::platform::theme::system_theme);
        let palette =
            crate::app::config_reload::build_palette_from_config(&self.config.colors, theme);
        let opacity = self.config.window.effective_opacity();
        if let Some(gpu) = self.gpu.as_ref() {
            if let Some(ctx) = self.windows.get(&new_winit_id) {
                gpu.clear_surface(ctx.window.surface(), palette.background(), opacity);
            }
        }

        // Show new window, then re-render source (z-order).
        if let Some(ctx) = self.windows.get(&new_winit_id) {
            ctx.window.set_visible(true);
        }
        if let Some(ctx) = self.windows.get_mut(&source_winit_id) {
            ctx.dirty = true;
        }

        // If source window is now empty, remove it.
        let source_empty = self
            .windows
            .get(&source_winit_id)
            .and_then(|ctx| {
                let mux = self.mux.as_ref()?;
                let win = mux.session().get_window(ctx.window.mux_window_id())?;
                Some(win.tabs().is_empty())
            })
            .unwrap_or(false);
        if source_empty {
            self.remove_empty_window(source_winit_id);
        }

        // Start OS drag on the new window.
        self.begin_os_tab_drag(new_winit_id, tab_id, mouse_offset, grab_offset);
    }

    /// Configure and start an OS-level drag session.
    ///
    /// Collects merge rects from other windows, configures `WM_MOVING`, sets
    /// `torn_off_pending`, and calls `drag_window()` which blocks in the OS
    /// modal move loop until mouse-up.
    fn begin_os_tab_drag(
        &mut self,
        winit_id: WindowId,
        tab_id: TabId,
        mouse_offset: f32,
        grab_offset: (i32, i32),
    ) {
        let merge_rects = self.collect_merge_rects(winit_id);

        if let Some(ctx) = self.windows.get(&winit_id) {
            platform_windows::begin_os_drag(
                ctx.window.window(),
                OsDragConfig {
                    grab_offset,
                    merge_rects,
                    skip_count: 3,
                },
            );
        }

        self.torn_off_pending = Some(TornOffPending {
            winit_id,
            tab_id,
            mouse_offset,
        });

        // `drag_window()` enters the OS modal move loop — blocks until
        // mouse-up or merge detection releases capture.
        if let Some(ctx) = self.windows.get(&winit_id) {
            let _ = ctx.window.window().drag_window();
        }
    }

    /// Start an OS-level drag on a single-tab window.
    ///
    /// When a single-tab window's tab is dragged past the threshold, there's
    /// no in-bar reorder — the entire window follows the cursor via OS drag,
    /// with merge detection to snap into another window's tab bar.
    pub(super) fn begin_single_tab_os_drag(&mut self, _event_loop: &ActiveEventLoop) {
        // Extract drag state.
        let (tab_id, mouse_offset, origin_y, winit_id) = {
            let Some(ctx) = self.focused_ctx_mut() else {
                return;
            };
            let Some(drag) = ctx.tab_drag.take() else {
                return;
            };
            ctx.tab_bar.set_drag_visual(None);
            let wid = ctx.window.window_id();
            (drag.tab_id, drag.mouse_offset_in_tab, drag.origin_y, wid)
        };

        self.release_tab_width_lock();

        // Compute grab offset from cursor to window origin.
        let grab_offset = {
            let Some(ctx) = self.windows.get(&winit_id) else {
                return;
            };
            let scale = ctx.window.scale_factor().factor() as f32;
            let grab_x = ((TAB_LEFT_MARGIN + mouse_offset) * scale).round() as i32;
            let grab_y = (origin_y * scale).round() as i32;
            (grab_x, grab_y)
        };

        self.begin_os_tab_drag(winit_id, tab_id, mouse_offset, grab_offset);
    }

    /// Collect tab bar merge rects from all windows except `exclude`.
    ///
    /// Each rect is `[left, top, right, tab_bar_bottom]` in screen coordinates,
    /// excluding the window controls zone on the right.
    fn collect_merge_rects(&self, exclude: WindowId) -> Vec<[i32; 4]> {
        let mut rects = Vec::new();
        for (&wid, ctx) in &self.windows {
            if wid == exclude {
                continue;
            }
            let scale = ctx.window.scale_factor().factor() as f32;
            let tab_bar_h = (TAB_BAR_HEIGHT * scale).round() as i32;
            let controls_w = (CONTROLS_ZONE_WIDTH * scale).round() as i32;
            if let Some((l, t, r, _)) = platform_windows::visible_frame_bounds(ctx.window.window())
            {
                rects.push([l, t, r - controls_w, t + tab_bar_h]);
            }
        }
        rects
    }
}
