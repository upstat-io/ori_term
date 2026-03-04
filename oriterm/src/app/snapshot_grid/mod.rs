//! Grid query adapter over [`PaneSnapshot`].
//!
//! [`SnapshotGrid`] wraps a `&PaneSnapshot` and provides the grid query
//! interface needed by selection, mark mode, and word boundary detection.
//! This is the client-side replacement for direct `Grid` access — all GUI
//! code that needs grid geometry or cell data uses this adapter instead of
//! locking the terminal.

use oriterm_core::grid::StableRowIndex;
use oriterm_mux::{PaneSnapshot, WireCell, WireCellFlags};

/// `CellFlags::WIDE_CHAR_SPACER` bit in the wire format.
///
/// Maps 1:1 to `oriterm_core::CellFlags::WIDE_CHAR_SPACER` (bit 9).
const WIDE_CHAR_SPACER_BIT: WireCellFlags = 1 << 9;

/// `CellFlags::WRAP` bit in the wire format.
///
/// Maps 1:1 to `oriterm_core::CellFlags::WRAP` (bit 10).
const WRAP_BIT: WireCellFlags = 1 << 10;

/// Read-only grid accessor backed by a [`PaneSnapshot`].
///
/// Provides the same query interface that `oriterm_core::grid::Grid` offers
/// for selection operations, but operates entirely on snapshot data — no
/// terminal lock required. Lifetime-tied to the snapshot borrow.
pub(crate) struct SnapshotGrid<'a> {
    snapshot: &'a PaneSnapshot,
}

impl<'a> SnapshotGrid<'a> {
    /// Wrap a snapshot reference.
    pub(crate) fn new(snapshot: &'a PaneSnapshot) -> Self {
        Self { snapshot }
    }

    /// Number of columns in the grid.
    pub(crate) fn cols(&self) -> usize {
        self.snapshot.cols as usize
    }

    /// Number of visible rows in the viewport.
    pub(crate) fn lines(&self) -> usize {
        self.snapshot.cells.len()
    }

    /// Number of scrollback rows above the viewport.
    pub(crate) fn scrollback_len(&self) -> usize {
        self.snapshot.scrollback_len as usize
    }

    /// Current scroll position (0 = bottom).
    pub(crate) fn display_offset(&self) -> usize {
        self.snapshot.display_offset as usize
    }

    /// Stable row index of the first viewport row.
    #[allow(dead_code, reason = "used in tests + Section 09 (search)")]
    pub(crate) fn stable_row_base(&self) -> u64 {
        self.snapshot.stable_row_base
    }

    /// Total rows in the grid (scrollback + visible).
    pub(crate) fn total_rows(&self) -> usize {
        self.scrollback_len() + self.lines()
    }

    /// Number of rows evicted from scrollback (derived from snapshot metadata).
    ///
    /// Mirrors `Grid::total_evicted()`. Computed from `stable_row_base`:
    /// `total_evicted = stable_row_base - first_visible_abs` where
    /// `first_visible_abs = scrollback_len - display_offset`.
    pub(crate) fn total_evicted(&self) -> u64 {
        let first_visible = (self.snapshot.scrollback_len as u64)
            .saturating_sub(self.snapshot.display_offset as u64);
        self.snapshot.stable_row_base.saturating_sub(first_visible)
    }

    /// Convert a stable row index to an absolute row index.
    ///
    /// Absolute row 0 = oldest scrollback row. Returns `None` if the row
    /// has been evicted or is beyond the grid extent.
    /// Mirrors `StableRowIndex::to_absolute(grid)`.
    pub(crate) fn stable_to_absolute(&self, stable: StableRowIndex) -> Option<usize> {
        let evicted = self.total_evicted();
        if stable.0 < evicted {
            return None;
        }
        let abs = (stable.0 - evicted) as usize;
        if abs < self.total_rows() {
            Some(abs)
        } else {
            None
        }
    }

    /// Convert an absolute row index to a stable row index.
    ///
    /// Mirrors `StableRowIndex::from_absolute(grid, abs_row)`.
    pub(crate) fn absolute_to_stable(&self, abs_row: usize) -> StableRowIndex {
        StableRowIndex(self.total_evicted() + abs_row as u64)
    }

    /// Absolute row index of the first visible viewport row.
    pub(crate) fn first_visible_absolute(&self) -> usize {
        self.scrollback_len().saturating_sub(self.display_offset())
    }

    /// Character at a viewport cell, defaulting to space for out-of-bounds.
    pub(crate) fn cell_char(&self, row: usize, col: usize) -> char {
        self.cell(row, col).map_or(' ', |c| c.ch)
    }

    /// Raw wire flags at a viewport cell, defaulting to 0 for out-of-bounds.
    pub(crate) fn cell_flags(&self, row: usize, col: usize) -> WireCellFlags {
        self.cell(row, col).map_or(0, |c| c.flags)
    }

    /// Reference to a cell, if in bounds.
    fn cell(&self, row: usize, col: usize) -> Option<&WireCell> {
        self.snapshot.cells.get(row)?.get(col)
    }

    /// Convert a viewport line to a stable row index.
    pub(crate) fn viewport_to_stable_row(&self, line: usize) -> StableRowIndex {
        StableRowIndex(self.snapshot.stable_row_base.saturating_add(line as u64))
    }

    /// Convert a stable row index to a viewport line, if visible.
    #[allow(dead_code, reason = "used in tests + Section 09 (search)")]
    pub(crate) fn stable_row_to_viewport(&self, stable: StableRowIndex) -> Option<usize> {
        let delta = stable.0.checked_sub(self.snapshot.stable_row_base)? as usize;
        if delta < self.lines() {
            Some(delta)
        } else {
            None
        }
    }

    /// Redirect a column to the base cell if it lands on a wide-char spacer.
    ///
    /// Wide characters occupy two cells: the base cell and a trailing spacer.
    /// Clicking on the spacer should act as if the user clicked on the base cell.
    pub(crate) fn redirect_spacer(&self, row: usize, col: usize) -> usize {
        if col == 0 {
            return col;
        }
        if self.cell_flags(row, col) & WIDE_CHAR_SPACER_BIT != 0 {
            col - 1
        } else {
            col
        }
    }

    /// Find word boundaries around (`viewport_row`, `col`) in the snapshot.
    ///
    /// Returns (`start_col`, `end_col`) inclusive. Wide-char spacers are
    /// redirected and skipped, matching the behavior of
    /// `oriterm_core::selection::word_boundaries`.
    pub(crate) fn word_boundaries(
        &self,
        viewport_row: usize,
        col: usize,
        word_delimiters: &str,
    ) -> (usize, usize) {
        let cols = self.cols();
        if cols == 0 || col >= cols {
            return (col, col);
        }

        // Redirect spacer to base cell.
        let click_col = if self.cell_flags(viewport_row, col) & WIDE_CHAR_SPACER_BIT != 0 && col > 0
        {
            col - 1
        } else {
            col
        };

        let ch = self.cell_char(viewport_row, click_col);
        let class = delimiter_class(ch, word_delimiters);

        // Scan left, skipping wide-char spacers.
        let mut start = click_col;
        while start > 0 {
            let prev = start - 1;
            if self.cell_flags(viewport_row, prev) & WIDE_CHAR_SPACER_BIT != 0 && prev > 0 {
                // Spacer: check the base cell before it.
                if delimiter_class(self.cell_char(viewport_row, prev - 1), word_delimiters) == class
                {
                    start = prev - 1;
                } else {
                    break;
                }
            } else if delimiter_class(self.cell_char(viewport_row, prev), word_delimiters) == class
            {
                start = prev;
            } else {
                break;
            }
        }

        // Scan right, skipping wide-char spacers.
        let mut end = click_col;
        while end + 1 < cols {
            let next = end + 1;
            if self.cell_flags(viewport_row, next) & WIDE_CHAR_SPACER_BIT != 0 {
                // Spacer belongs to the wide char at `end` — include it.
                end = next;
                continue;
            }
            if delimiter_class(self.cell_char(viewport_row, next), word_delimiters) == class {
                end = next;
            } else {
                break;
            }
        }

        (start, end)
    }

    /// Walk backwards to find the start of a logical (soft-wrapped) line.
    ///
    /// Returns the viewport row of the first row in the logical line.
    /// Stops at the viewport boundary (row 0).
    pub(crate) fn logical_line_start(&self, viewport_row: usize) -> usize {
        let mut current = viewport_row;
        while current > 0 {
            let prev = current - 1;
            let last_col = self.cols().saturating_sub(1);
            if self.cell_flags(prev, last_col) & WRAP_BIT != 0 {
                current = prev;
            } else {
                break;
            }
        }
        current
    }

    /// Walk forwards to find the end of a logical (soft-wrapped) line.
    ///
    /// Returns the viewport row of the last row in the logical line.
    /// Stops at the viewport boundary.
    pub(crate) fn logical_line_end(&self, viewport_row: usize) -> usize {
        let last_col = self.cols().saturating_sub(1);
        let mut current = viewport_row;
        while current + 1 < self.lines() {
            if self.cell_flags(current, last_col) & WRAP_BIT != 0 {
                current += 1;
            } else {
                break;
            }
        }
        current
    }
}

/// Character classification for word boundary detection.
///
/// Mirrors `oriterm_core::selection::boundaries::delimiter_class` (which is
/// `pub(crate)` and not accessible from this crate).
fn delimiter_class(c: char, word_delimiters: &str) -> u8 {
    if c == '\0' || c == ' ' || c == '\t' {
        1
    } else if word_delimiters.contains(c) {
        2
    } else {
        0
    }
}

#[cfg(test)]
mod tests;
