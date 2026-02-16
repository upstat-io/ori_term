//! Linux platform glue for frameless window management.
//!
//! Thin layer — winit handles X11 `_NET_WM_MOVERESIZE` and Wayland
//! `xdg_toplevel.move`/`xdg_toplevel.resize` internally. No additional
//! platform dependencies are needed.

use winit::window::Window;

use crate::hit_test::ResizeDirection;

/// Initiates a window drag (title bar drag).
///
/// Called when `hit_test()` returns `Caption`. winit translates this to
/// `_NET_WM_MOVERESIZE` on X11 or `xdg_toplevel.move` on Wayland.
pub fn start_drag(window: &Window) {
    if let Err(e) = window.drag_window() {
        log::warn!("drag_window failed: {e}");
    }
}

/// Initiates a window resize from the given edge or corner.
///
/// Called when `hit_test()` returns `ResizeBorder(direction)`. winit maps
/// the direction to `_NET_WM_MOVERESIZE` on X11 or `xdg_toplevel.resize`
/// on Wayland.
pub fn start_resize(window: &Window, direction: ResizeDirection) {
    if let Err(e) = window.drag_resize_window(to_winit_direction(direction)) {
        log::warn!("drag_resize_window failed: {e}");
    }
}

/// Maps our [`ResizeDirection`] to winit's compass-based direction.
fn to_winit_direction(dir: ResizeDirection) -> winit::window::ResizeDirection {
    match dir {
        ResizeDirection::Top => winit::window::ResizeDirection::North,
        ResizeDirection::Bottom => winit::window::ResizeDirection::South,
        ResizeDirection::Left => winit::window::ResizeDirection::West,
        ResizeDirection::Right => winit::window::ResizeDirection::East,
        ResizeDirection::TopLeft => winit::window::ResizeDirection::NorthWest,
        ResizeDirection::TopRight => winit::window::ResizeDirection::NorthEast,
        ResizeDirection::BottomLeft => winit::window::ResizeDirection::SouthWest,
        ResizeDirection::BottomRight => winit::window::ResizeDirection::SouthEast,
    }
}
