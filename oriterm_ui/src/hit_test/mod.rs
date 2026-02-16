//! Platform-independent hit testing for frameless window chrome.
//!
//! Translates a point in window coordinates into a semantic region
//! (`Client`, `Caption`, or `ResizeBorder`). The OS-specific window
//! procedure calls this pure function and maps the result to native
//! constants (e.g. `WM_NCHITTEST` on Windows).

use crate::geometry::{Point, Rect, Size};

/// The semantic region a point falls in within a frameless window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTestResult {
    /// Standard client area (terminal grid, buttons, tabs).
    Client,
    /// Draggable caption area (title bar / tab bar background).
    Caption,
    /// Resizable border or corner.
    ResizeBorder(ResizeDirection),
}

/// Direction for a resize border hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    /// Top edge.
    Top,
    /// Bottom edge.
    Bottom,
    /// Left edge.
    Left,
    /// Right edge.
    Right,
    /// Top-left corner.
    TopLeft,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-right corner.
    BottomRight,
}

/// Determines the semantic region for a point within a frameless window.
///
/// Priority hierarchy (from Chromium's decision tree):
///
/// 1. Interactive rects within caption (buttons, tabs) -> `Client`.
/// 2. Resize edges/corners (unless maximized) -> `ResizeBorder`.
/// 3. Caption area -> `Caption` (draggable).
/// 4. Everything else -> `Client`.
///
/// Corners take priority over edges: a point in the top-left corner
/// returns `TopLeft`, not `Top` or `Left`.
pub fn hit_test(
    point: Point,
    window_size: Size,
    border_width: f32,
    caption_height: f32,
    interactive_rects: &[Rect],
    is_maximized: bool,
) -> HitTestResult {
    // 1. Check interactive rects first — buttons/tabs within caption are
    //    clickable, not draggable.
    for rect in interactive_rects {
        if rect.contains(point) {
            return HitTestResult::Client;
        }
    }

    // 2. Check resize borders (suppressed when maximized).
    if !is_maximized {
        if let Some(direction) = resize_direction(point, window_size, border_width) {
            return HitTestResult::ResizeBorder(direction);
        }
    }

    // 3. Check caption area.
    if point.y < caption_height {
        return HitTestResult::Caption;
    }

    // 4. Everything else is client area.
    HitTestResult::Client
}

/// Returns the resize direction if the point is within `border_width` of
/// any window edge. Corners take priority over edges.
fn resize_direction(
    point: Point,
    window_size: Size,
    border_width: f32,
) -> Option<ResizeDirection> {
    let w = window_size.width();
    let h = window_size.height();

    let on_left = point.x < border_width;
    let on_right = point.x >= w - border_width;
    let on_top = point.y < border_width;
    let on_bottom = point.y >= h - border_width;

    // Corners first (higher priority than edges).
    match (on_left, on_right, on_top, on_bottom) {
        (true, _, true, _) => Some(ResizeDirection::TopLeft),
        (_, true, true, _) => Some(ResizeDirection::TopRight),
        (true, _, _, true) => Some(ResizeDirection::BottomLeft),
        (_, true, _, true) => Some(ResizeDirection::BottomRight),
        (true, _, _, _) => Some(ResizeDirection::Left),
        (_, true, _, _) => Some(ResizeDirection::Right),
        (_, _, true, _) => Some(ResizeDirection::Top),
        (_, _, _, true) => Some(ResizeDirection::Bottom),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
