//! Layout computation — converts abstract trees into concrete pixel rects.
//!
//! Takes a `SplitTree` + `FloatingLayer` + dimensions and produces a flat
//! list of `PaneLayout` records for the renderer, plus `DividerLayout`
//! records for split divider rendering.

use crate::id::PaneId;
use crate::layout::floating::{FloatingLayer, Rect};
use crate::layout::split_tree::{SplitDirection, SplitTree};

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
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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
    let mut layouts = Vec::new();

    // Compute tiled pane layouts from split tree.
    compute_tiled(tree, desc.available, focused, desc, &mut layouts);

    // Append floating pane layouts.
    for fp in floating.panes() {
        let cols = (fp.width / desc.cell_width).floor() as u16;
        let rows = (fp.height / desc.cell_height).floor() as u16;
        layouts.push(PaneLayout {
            pane_id: fp.pane_id,
            pixel_rect: Rect {
                x: fp.x,
                y: fp.y,
                width: fp.width,
                height: fp.height,
            },
            cols: cols.max(1),
            rows: rows.max(1),
            is_focused: fp.pane_id == focused,
            is_floating: true,
        });
    }

    layouts
}

/// Compute divider layouts from a split tree.
///
/// Returns one `DividerLayout` per internal `Split` node.
pub fn compute_dividers(tree: &SplitTree, desc: &LayoutDescriptor) -> Vec<DividerLayout> {
    let mut dividers = Vec::new();
    compute_dividers_inner(tree, desc.available, desc, &mut dividers);
    dividers
}

// ── Private helpers ───────────────────────────────────────────────

/// Recursively subdivide the available rect according to the split tree.
fn compute_tiled(
    tree: &SplitTree,
    available: Rect,
    focused: PaneId,
    desc: &LayoutDescriptor,
    out: &mut Vec<PaneLayout>,
) {
    match tree {
        SplitTree::Leaf(pane_id) => {
            let cols = (available.width / desc.cell_width).floor() as u16;
            let rows = (available.height / desc.cell_height).floor() as u16;
            out.push(PaneLayout {
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

            // Enforce minimum pane size by clamping.
            let (first_rect, second_rect) =
                clamp_split(available, *direction, first_rect, second_rect, desc);

            compute_tiled(first, first_rect, focused, desc, out);
            compute_tiled(second, second_rect, focused, desc, out);
        }
    }
}

/// Recursively compute divider rects.
fn compute_dividers_inner(
    tree: &SplitTree,
    available: Rect,
    desc: &LayoutDescriptor,
    out: &mut Vec<DividerLayout>,
) {
    if let SplitTree::Split {
        direction,
        ratio,
        first,
        second,
    } = tree
    {
        let (first_rect, second_rect) = split_rect(available, *direction, *ratio, desc.divider_px);

        let (first_rect, second_rect) =
            clamp_split(available, *direction, first_rect, second_rect, desc);

        // Divider rect sits between the two child rects.
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

        // Find the nearest leaf on each side for targeting.
        let pane_before = rightmost_leaf(first);
        let pane_after = leftmost_leaf(second);

        out.push(DividerLayout {
            rect: divider_rect,
            direction: *direction,
            pane_before,
            pane_after,
        });

        compute_dividers_inner(first, first_rect, desc, out);
        compute_dividers_inner(second, second_rect, desc, out);
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
