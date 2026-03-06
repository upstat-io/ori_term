//! Layout computation — converts abstract trees into concrete pixel rects.
//!
//! Takes a `SplitTree` + `FloatingLayer` + dimensions and produces a flat
//! list of `PaneLayout` records for the renderer, plus `DividerLayout`
//! records for split divider rendering.

use super::floating::FloatingLayer;
use super::rect::Rect;
use super::split_tree::{SplitDirection, SplitTree};
use oriterm_mux::PaneId;

/// Input parameters for layout computation.
#[derive(Debug, Clone)]
pub struct LayoutDescriptor {
    /// Total available pixel area for the tab content (excludes tab bar).
    pub available: Rect,
    /// Cell width in logical pixels.
    pub cell_width: f32,
    /// Cell height in logical pixels.
    pub cell_height: f32,
    /// Divider thickness in logical pixels.
    pub divider_px: f32,
    /// Minimum pane size in cells (columns, rows).
    pub min_pane_cells: (u16, u16),
}

/// Output per pane from layout computation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneLayout {
    /// Which pane this layout describes.
    pub pane_id: PaneId,
    /// Pixel rect within the window.
    pub pixel_rect: Rect,
    /// Number of terminal columns that fit in this rect.
    pub cols: u16,
    /// Number of terminal rows that fit in this rect.
    pub rows: u16,
    /// Whether this pane currently has focus.
    pub is_focused: bool,
    /// Whether this pane is floating (overlay) vs. tiled.
    pub is_floating: bool,
}

/// Output for divider rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DividerLayout {
    /// Pixel rect for the divider.
    pub rect: Rect,
    /// Split direction (determines divider orientation).
    pub direction: SplitDirection,
    /// Pane ID on the first (left/top) side.
    pub pane_before: PaneId,
    /// Pane ID on the second (right/bottom) side.
    pub pane_after: PaneId,
}

/// Accumulator for the recursive tree traversal.
struct TreeOutput {
    panes: Vec<PaneLayout>,
    dividers: Vec<DividerLayout>,
}

/// Compute both pane layouts and divider layouts in a single tree traversal.
///
/// Returns `(pane_layouts, divider_layouts)`. Tiled panes appear first in the
/// pane list, followed by floating panes.
pub fn compute_all(
    tree: &SplitTree,
    floating: &FloatingLayer,
    focused: PaneId,
    desc: &LayoutDescriptor,
) -> (Vec<PaneLayout>, Vec<DividerLayout>) {
    let mut out = TreeOutput {
        panes: Vec::new(),
        dividers: Vec::new(),
    };

    compute_tree(tree, desc.available, focused, desc, &mut out);
    append_floating(floating, focused, desc, &mut out.panes);

    (out.panes, out.dividers)
}

/// Compute pane layouts from a split tree and floating layer.
///
/// Returns a flat list of `PaneLayout` records — tiled panes first (from the
/// split tree), then floating panes overlaid on top.
pub fn compute_layout(
    tree: &SplitTree,
    floating: &FloatingLayer,
    focused: PaneId,
    desc: &LayoutDescriptor,
) -> Vec<PaneLayout> {
    compute_all(tree, floating, focused, desc).0
}

/// Compute divider layouts from a split tree.
///
/// Returns one `DividerLayout` per internal `Split` node.
pub fn compute_dividers(tree: &SplitTree, desc: &LayoutDescriptor) -> Vec<DividerLayout> {
    let mut out = TreeOutput {
        panes: Vec::new(),
        dividers: Vec::new(),
    };
    compute_tree(tree, desc.available, PaneId::from_raw(0), desc, &mut out);
    out.dividers
}

/// Recursively subdivide the available rect, producing both pane and divider
/// layouts in a single pass.
fn compute_tree(
    tree: &SplitTree,
    available: Rect,
    focused: PaneId,
    desc: &LayoutDescriptor,
    out: &mut TreeOutput,
) {
    match tree {
        SplitTree::Leaf(pane_id) => {
            let cols = (available.width / desc.cell_width).floor() as u16;
            let rows = (available.height / desc.cell_height).floor() as u16;
            out.panes.push(PaneLayout {
                pane_id: *pane_id,
                pixel_rect: snap_to_grid(available, desc.cell_width, desc.cell_height),
                cols: cols.max(1),
                rows: rows.max(1),
                is_focused: *pane_id == focused,
                is_floating: false,
            });
        }
        SplitTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let (first_rect, second_rect) =
                split_rect(available, *direction, *ratio, desc.divider_px);
            let (first_rect, second_rect) =
                clamp_split(available, *direction, first_rect, second_rect, desc);

            // Emit divider between the two child rects.
            let divider_rect = match direction {
                SplitDirection::Horizontal => Rect {
                    x: available.x,
                    y: first_rect.y + first_rect.height,
                    width: available.width,
                    height: desc.divider_px,
                },
                SplitDirection::Vertical => Rect {
                    x: first_rect.x + first_rect.width,
                    y: available.y,
                    width: desc.divider_px,
                    height: available.height,
                },
            };
            out.dividers.push(DividerLayout {
                rect: divider_rect,
                direction: *direction,
                pane_before: rightmost_leaf(first),
                pane_after: leftmost_leaf(second),
            });

            compute_tree(first, first_rect, focused, desc, out);
            compute_tree(second, second_rect, focused, desc, out);
        }
    }
}

/// Append floating pane layouts to the output list.
///
/// Floating pane rects are snapped to the cell grid, matching the tiled pane
/// contract where `pixel_rect` dimensions are exact multiples of cell size.
/// Dimensions are clamped to the minimum floating pane size (20 columns × 5
/// rows) defined in `floating::MIN_FLOATING_PANE_CELLS`.
fn append_floating(
    floating: &FloatingLayer,
    focused: PaneId,
    desc: &LayoutDescriptor,
    out: &mut Vec<PaneLayout>,
) {
    use super::floating::MIN_FLOATING_PANE_CELLS;

    let min_w = f32::from(MIN_FLOATING_PANE_CELLS.0) * desc.cell_width;
    let min_h = f32::from(MIN_FLOATING_PANE_CELLS.1) * desc.cell_height;

    for fp in floating.panes() {
        // Enforce minimum dimensions before grid snapping.
        let clamped = Rect {
            width: fp.rect.width.max(min_w),
            height: fp.rect.height.max(min_h),
            ..fp.rect
        };
        let snapped = snap_to_grid(clamped, desc.cell_width, desc.cell_height);
        let cols = (snapped.width / desc.cell_width).floor() as u16;
        let rows = (snapped.height / desc.cell_height).floor() as u16;
        out.push(PaneLayout {
            pane_id: fp.pane_id,
            pixel_rect: snapped,
            cols: cols.max(1),
            rows: rows.max(1),
            is_focused: fp.pane_id == focused,
            is_floating: true,
        });
    }
}

/// Split a rect into two parts according to direction and ratio.
fn split_rect(
    available: Rect,
    direction: SplitDirection,
    ratio: f32,
    divider_px: f32,
) -> (Rect, Rect) {
    match direction {
        SplitDirection::Horizontal => {
            let usable = (available.height - divider_px).max(0.0);
            let first_h = (usable * ratio).floor();
            let second_h = usable - first_h;
            (
                Rect {
                    x: available.x,
                    y: available.y,
                    width: available.width,
                    height: first_h,
                },
                Rect {
                    x: available.x,
                    y: available.y + first_h + divider_px,
                    width: available.width,
                    height: second_h,
                },
            )
        }
        SplitDirection::Vertical => {
            let usable = (available.width - divider_px).max(0.0);
            let first_w = (usable * ratio).floor();
            let second_w = usable - first_w;
            (
                Rect {
                    x: available.x,
                    y: available.y,
                    width: first_w,
                    height: available.height,
                },
                Rect {
                    x: available.x + first_w + divider_px,
                    y: available.y,
                    width: second_w,
                    height: available.height,
                },
            )
        }
    }
}

/// Clamp split rects to enforce minimum pane sizes.
fn clamp_split(
    available: Rect,
    direction: SplitDirection,
    first: Rect,
    second: Rect,
    desc: &LayoutDescriptor,
) -> (Rect, Rect) {
    let min_w = f32::from(desc.min_pane_cells.0) * desc.cell_width;
    let min_h = f32::from(desc.min_pane_cells.1) * desc.cell_height;
    match direction {
        SplitDirection::Horizontal => {
            let usable = (available.height - desc.divider_px).max(0.0);
            let mut fh = first.height;
            let mut sh = second.height;
            if fh < min_h && usable >= 2.0 * min_h {
                fh = min_h;
                sh = usable - fh;
            } else if sh < min_h && usable >= 2.0 * min_h {
                sh = min_h;
                fh = usable - sh;
            } else {
                // Both sides are already above minimum, or there isn't
                // enough space for two minimum panes — keep as-is.
            }
            (
                Rect {
                    height: fh,
                    ..first
                },
                Rect {
                    y: available.y + fh + desc.divider_px,
                    height: sh,
                    ..second
                },
            )
        }
        SplitDirection::Vertical => {
            let usable = (available.width - desc.divider_px).max(0.0);
            let mut fw = first.width;
            let mut sw = second.width;
            if fw < min_w && usable >= 2.0 * min_w {
                fw = min_w;
                sw = usable - fw;
            } else if sw < min_w && usable >= 2.0 * min_w {
                sw = min_w;
                fw = usable - sw;
            } else {
                // Both sides are already above minimum, or there isn't
                // enough space for two minimum panes — keep as-is.
            }
            (
                Rect { width: fw, ..first },
                Rect {
                    x: available.x + fw + desc.divider_px,
                    width: sw,
                    ..second
                },
            )
        }
    }
}

/// Snap a rect to the cell grid by trimming fractional cells.
fn snap_to_grid(rect: Rect, cell_w: f32, cell_h: f32) -> Rect {
    let cols = (rect.width / cell_w).floor();
    let rows = (rect.height / cell_h).floor();
    Rect {
        x: rect.x,
        y: rect.y,
        width: cols * cell_w,
        height: rows * cell_h,
    }
}

/// Find the rightmost (deepest second-child) leaf in a subtree.
fn rightmost_leaf(tree: &SplitTree) -> PaneId {
    match tree {
        SplitTree::Leaf(id) => *id,
        SplitTree::Split { second, .. } => rightmost_leaf(second),
    }
}

/// Find the leftmost (deepest first-child) leaf in a subtree.
fn leftmost_leaf(tree: &SplitTree) -> PaneId {
    match tree {
        SplitTree::Leaf(id) => *id,
        SplitTree::Split { first, .. } => leftmost_leaf(first),
    }
}

#[cfg(test)]
mod tests;
