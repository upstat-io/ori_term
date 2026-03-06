//! Floating pane drag/resize interaction.
//!
//! State machine for moving and resizing floating panes via mouse. Models
//! after `divider_drag.rs`: hover detection, drag initiation, live updates,
//! and commit on release.

use winit::dpi::PhysicalPosition;
use winit::window::CursorIcon;

use oriterm_mux::PaneId;

use crate::session::{Rect, snap_to_edge};

use super::App;

/// Floating pane title bar height in physical pixels.
const TITLE_BAR_HEIGHT: f32 = 24.0;

/// Edge detection threshold in physical pixels.
const EDGE_THRESHOLD: f32 = 5.0;

/// Corner detection size in physical pixels.
const CORNER_SIZE: f32 = 10.0;

/// Minimum floating pane dimension in physical pixels.
const MIN_SIZE_PX: f32 = 100.0;

/// Which edge or corner of a floating pane is being resized.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ResizeEdge {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Active floating pane drag or resize state.
#[derive(Debug)]
pub(crate) enum FloatingDragState {
    /// Dragging by the title bar to move the pane.
    Moving {
        pane_id: PaneId,
        /// Mouse offset from pane origin when drag started.
        offset_x: f32,
        offset_y: f32,
    },
    /// Dragging an edge or corner to resize the pane.
    Resizing {
        pane_id: PaneId,
        edge: ResizeEdge,
        initial_rect: Rect,
        /// Mouse position when drag started.
        origin_x: f32,
        origin_y: f32,
    },
}

/// Result of hit-testing a point against a floating pane's zones.
#[derive(Debug, Clone, Copy)]
enum HitZone {
    /// Over the title bar (drag to move).
    TitleBar,
    /// Over an edge or corner (drag to resize).
    Edge(ResizeEdge),
    /// Over the interior (no drag, just click-through).
    Interior,
}

/// Hit-test a point against a floating pane's zones.
///
/// Returns `None` if the point is outside the pane rect.
fn hit_test_zone(px: f32, py: f32, rect: &Rect) -> Option<HitZone> {
    if !rect.contains_point(px, py) {
        return None;
    }

    let dx_left = px - rect.x;
    let dx_right = (rect.x + rect.width) - px;
    let dy_top = py - rect.y;
    let dy_bottom = (rect.y + rect.height) - py;

    // Corners (highest priority).
    if dx_left < CORNER_SIZE && dy_top < CORNER_SIZE {
        return Some(HitZone::Edge(ResizeEdge::TopLeft));
    }
    if dx_right < CORNER_SIZE && dy_top < CORNER_SIZE {
        return Some(HitZone::Edge(ResizeEdge::TopRight));
    }
    if dx_left < CORNER_SIZE && dy_bottom < CORNER_SIZE {
        return Some(HitZone::Edge(ResizeEdge::BottomLeft));
    }
    if dx_right < CORNER_SIZE && dy_bottom < CORNER_SIZE {
        return Some(HitZone::Edge(ResizeEdge::BottomRight));
    }

    // Edges.
    if dx_left < EDGE_THRESHOLD {
        return Some(HitZone::Edge(ResizeEdge::Left));
    }
    if dx_right < EDGE_THRESHOLD {
        return Some(HitZone::Edge(ResizeEdge::Right));
    }
    if dy_top < EDGE_THRESHOLD {
        return Some(HitZone::Edge(ResizeEdge::Top));
    }
    if dy_bottom < EDGE_THRESHOLD {
        return Some(HitZone::Edge(ResizeEdge::Bottom));
    }

    // Title bar (top region excluding edges).
    if dy_top < TITLE_BAR_HEIGHT {
        return Some(HitZone::TitleBar);
    }

    Some(HitZone::Interior)
}

/// Map a resize edge to a cursor icon.
fn edge_cursor(edge: ResizeEdge) -> CursorIcon {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorIcon::NsResize,
        ResizeEdge::Left | ResizeEdge::Right => CursorIcon::EwResize,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorIcon::NwseResize,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorIcon::NeswResize,
    }
}

impl App {
    /// Update floating pane hover state on cursor move.
    ///
    /// Changes cursor icon when over a floating pane edge or title bar.
    /// If a floating drag is active, updates the drag. Returns `true` if
    /// a floating drag is active (caller should skip other mouse handling).
    pub(super) fn update_floating_hover(&mut self, position: PhysicalPosition<f64>) -> bool {
        let has_drag = self
            .focused_ctx()
            .is_some_and(|ctx| ctx.floating_drag.is_some());
        if has_drag {
            self.update_floating_drag(position);
            return true;
        }

        let px = position.x as f32;
        let py = position.y as f32;

        // Resolve tab context first (both IDs are Copy, borrow ends here).
        let Some((tab_id, _)) = self.active_pane_context() else {
            return false;
        };

        // Find topmost floating pane under cursor.
        let floating_hit = {
            let Some(tab) = self.session.get_tab(tab_id) else {
                return false;
            };
            // Check in reverse z-order (topmost first).
            let mut result = None;
            for fp in tab.floating().panes().iter().rev() {
                if let Some(zone) = hit_test_zone(px, py, &fp.rect) {
                    result = Some((fp.pane_id, fp.rect, zone));
                    break;
                }
            }
            result
        };

        if let Some((_pane_id, _rect, zone)) = floating_hit {
            let icon = match zone {
                HitZone::Edge(edge) => edge_cursor(edge),
                HitZone::TitleBar => CursorIcon::Grab,
                HitZone::Interior => CursorIcon::Default,
            };
            if let Some(ctx) = self.focused_ctx() {
                ctx.window.window().set_cursor(icon);
            }
            true
        } else {
            false
        }
    }

    /// Try to start a floating pane drag on left-click.
    ///
    /// Returns `true` if a floating drag was started (caller should consume
    /// the click and not forward to selection/reporting).
    pub(super) fn try_start_floating_drag(&mut self) -> bool {
        let pos = self.mouse.cursor_pos();
        let px = pos.x as f32;
        let py = pos.y as f32;

        // Resolve tab context first (both IDs are Copy, borrow ends here).
        let Some((tab_id, _)) = self.active_pane_context() else {
            return false;
        };

        let floating_hit = {
            let Some(tab) = self.session.get_tab(tab_id) else {
                return false;
            };
            let mut result = None;
            for fp in tab.floating().panes().iter().rev() {
                if let Some(zone) = hit_test_zone(px, py, &fp.rect) {
                    result = Some((fp.pane_id, fp.rect, zone));
                    break;
                }
            }
            result
        };

        let Some((pane_id, rect, zone)) = floating_hit else {
            return false;
        };

        match zone {
            HitZone::TitleBar => {
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.floating_drag = Some(FloatingDragState::Moving {
                        pane_id,
                        offset_x: px - rect.x,
                        offset_y: py - rect.y,
                    });
                }
                if let Some(ctx) = self.focused_ctx() {
                    ctx.window.window().set_cursor(CursorIcon::Grabbing);
                }
                true
            }
            HitZone::Edge(edge) => {
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.floating_drag = Some(FloatingDragState::Resizing {
                        pane_id,
                        edge,
                        initial_rect: rect,
                        origin_x: px,
                        origin_y: py,
                    });
                }
                true
            }
            HitZone::Interior => false,
        }
    }

    /// Update floating pane position/size during an active drag.
    fn update_floating_drag(&mut self, position: PhysicalPosition<f64>) {
        let px = position.x as f32;
        let py = position.y as f32;

        // Extract drag data before mutable borrows.
        let drag_info = self.focused_ctx().and_then(|ctx| {
            ctx.floating_drag.as_ref().map(|drag| match drag {
                FloatingDragState::Moving {
                    pane_id,
                    offset_x,
                    offset_y,
                } => DragInfo::Move {
                    pane_id: *pane_id,
                    offset_x: *offset_x,
                    offset_y: *offset_y,
                },
                FloatingDragState::Resizing {
                    pane_id,
                    edge,
                    initial_rect,
                    origin_x,
                    origin_y,
                } => DragInfo::Resize {
                    pane_id: *pane_id,
                    edge: *edge,
                    initial_rect: *initial_rect,
                    origin_x: *origin_x,
                    origin_y: *origin_y,
                },
            })
        });

        let Some(drag) = drag_info else { return };

        let Some((tab_id, _)) = self.active_pane_context() else {
            return;
        };

        match drag {
            DragInfo::Move {
                pane_id,
                offset_x,
                offset_y,
            } => {
                let new_x = px - offset_x;
                let new_y = py - offset_y;

                // Snap to grid boundary edges.
                let pane_rect = {
                    let Some(tab) = self.session.get_tab(tab_id) else {
                        return;
                    };
                    tab.floating().pane_rect(pane_id)
                };
                let Some(rect) = pane_rect else { return };

                let bounds = match self.grid_available_rect() {
                    Some(b) => b,
                    None => return,
                };

                let (sx, sy) = snap_to_edge(new_x, new_y, rect.width, rect.height, &bounds);

                if let Some(tab) = self.session.get_tab_mut(tab_id) {
                    tab.floating_mut().move_pane_mut(pane_id, sx, sy);
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
            DragInfo::Resize {
                pane_id,
                edge,
                initial_rect: ir,
                origin_x,
                origin_y,
            } => {
                let dx = px - origin_x;
                let dy = py - origin_y;

                let (new_rect, needs_move) = compute_resize(ir, edge, dx, dy);

                if let Some(tab) = self.session.get_tab_mut(tab_id) {
                    if needs_move {
                        tab.floating_mut().set_pane_rect_mut(pane_id, new_rect);
                    } else {
                        tab.floating_mut().resize_pane_mut(
                            pane_id,
                            new_rect.width,
                            new_rect.height,
                        );
                    }
                }
                if let Some(ctx) = self.focused_ctx_mut() {
                    ctx.dirty = true;
                }
            }
        }
    }

    /// Finish a floating drag on mouse release.
    ///
    /// Returns `true` if a drag was active (caller should consume the release).
    pub(super) fn try_finish_floating_drag(&mut self) -> bool {
        let had_drag = self
            .focused_ctx_mut()
            .and_then(|ctx| ctx.floating_drag.take())
            .is_some();
        if had_drag {
            // Sync PTY dimensions to the new layout.
            self.resize_all_panes();
            if let Some(ctx) = self.focused_ctx() {
                ctx.window.window().set_cursor(CursorIcon::Default);
            }
            true
        } else {
            false
        }
    }

    /// Cancel any active floating drag.
    pub(super) fn cancel_floating_drag(&mut self) {
        let had_drag = self
            .focused_ctx_mut()
            .and_then(|ctx| ctx.floating_drag.take())
            .is_some();
        if had_drag {
            self.resize_all_panes();
        }
    }
}

/// Extracted drag info (all `Copy` fields) to break borrow chain.
enum DragInfo {
    Move {
        pane_id: PaneId,
        offset_x: f32,
        offset_y: f32,
    },
    Resize {
        pane_id: PaneId,
        edge: ResizeEdge,
        initial_rect: Rect,
        origin_x: f32,
        origin_y: f32,
    },
}

/// Compute the new rect after a resize drag.
///
/// Returns `(new_rect, needs_move)`. `needs_move` is true when the drag
/// affects the top or left edge, shifting the origin.
fn compute_resize(initial: Rect, edge: ResizeEdge, dx: f32, dy: f32) -> (Rect, bool) {
    let mut r = initial;
    let mut moved = false;

    match edge {
        ResizeEdge::Right => {
            r.width = (initial.width + dx).max(MIN_SIZE_PX);
        }
        ResizeEdge::Bottom => {
            r.height = (initial.height + dy).max(MIN_SIZE_PX);
        }
        ResizeEdge::Left => {
            let new_w = (initial.width - dx).max(MIN_SIZE_PX);
            r.x = initial.x + initial.width - new_w;
            r.width = new_w;
            moved = true;
        }
        ResizeEdge::Top => {
            let new_h = (initial.height - dy).max(MIN_SIZE_PX);
            r.y = initial.y + initial.height - new_h;
            r.height = new_h;
            moved = true;
        }
        ResizeEdge::TopLeft => {
            let new_w = (initial.width - dx).max(MIN_SIZE_PX);
            let new_h = (initial.height - dy).max(MIN_SIZE_PX);
            r.x = initial.x + initial.width - new_w;
            r.y = initial.y + initial.height - new_h;
            r.width = new_w;
            r.height = new_h;
            moved = true;
        }
        ResizeEdge::TopRight => {
            r.width = (initial.width + dx).max(MIN_SIZE_PX);
            let new_h = (initial.height - dy).max(MIN_SIZE_PX);
            r.y = initial.y + initial.height - new_h;
            r.height = new_h;
            moved = true;
        }
        ResizeEdge::BottomLeft => {
            let new_w = (initial.width - dx).max(MIN_SIZE_PX);
            r.x = initial.x + initial.width - new_w;
            r.width = new_w;
            r.height = (initial.height + dy).max(MIN_SIZE_PX);
            moved = true;
        }
        ResizeEdge::BottomRight => {
            r.width = (initial.width + dx).max(MIN_SIZE_PX);
            r.height = (initial.height + dy).max(MIN_SIZE_PX);
        }
    }

    (r, moved)
}
