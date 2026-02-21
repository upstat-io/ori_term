//! Stable row identity that survives scrollback eviction.

use super::Grid;

/// Monotonically increasing row identity that survives scrollback eviction.
///
/// Row 0 is the first row ever written to this grid. Unlike absolute
/// indices (which shift when scrollback evicts rows), `StableRowIndex`
/// values remain valid across eviction, scroll, and resize.
///
/// Formula: `StableRowIndex = total_evicted + absolute_row_index`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableRowIndex(pub u64);

impl StableRowIndex {
    /// Convert a visible viewport line to a stable index.
    ///
    /// `line` is a 0-based viewport offset (0 = top of visible area),
    /// adjusted for the current `display_offset`.
    pub fn from_visible(grid: &Grid, line: usize) -> Self {
        let abs = grid
            .scrollback()
            .len()
            .saturating_sub(grid.display_offset())
            + line;
        Self(grid.total_evicted() as u64 + abs as u64)
    }

    /// Convert an absolute row index to a stable index.
    ///
    /// Absolute row 0 = oldest scrollback row, then visible rows follow.
    pub fn from_absolute(grid: &Grid, abs_row: usize) -> Self {
        Self(grid.total_evicted() as u64 + abs_row as u64)
    }

    /// Convert back to an absolute row index.
    ///
    /// Returns `None` if the row has been evicted from scrollback.
    pub fn to_absolute(self, grid: &Grid) -> Option<usize> {
        let evicted = grid.total_evicted() as u64;
        if self.0 < evicted {
            return None;
        }
        let abs = (self.0 - evicted) as usize;
        let total = grid.scrollback().len() + grid.lines();
        if abs < total { Some(abs) } else { None }
    }
}
