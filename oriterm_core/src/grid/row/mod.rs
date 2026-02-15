//! Terminal grid row.
//!
//! A `Row` is a contiguous array of `Cell`s representing one terminal line,
//! with occupancy tracking for efficient sparse-row operations.

use std::ops::{Index, IndexMut, Range};

use crate::cell::Cell;
use crate::index::Column;

/// One row of cells in the terminal grid.
#[derive(Debug, Clone)]
pub struct Row {
    /// The cells in this row.
    inner: Vec<Cell>,
    /// Upper bound on cells modified since last `reset()`.
    ///
    /// After `reset()`, occ is 0. `IndexMut` bumps occ to track writes.
    /// The value may exceed the true occupancy (lazy dirty-tracking, matching
    /// Alacritty's pattern). Use `clamp_occ` / `set_occ` for O(1) adjustments.
    occ: usize,
}

/// Equality compares cell content only — `occ` is internal bookkeeping
/// (dirty-tracking upper bound) and must not affect semantic equality.
impl PartialEq for Row {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Row {}

impl Row {
    /// Create a new row of `cols` default cells.
    pub fn new(cols: usize) -> Self {
        Self {
            inner: vec![Cell::default(); cols],
            occ: 0,
        }
    }

    /// Reset all cells to the template, resizing if needed.
    ///
    /// Only iterates `[0..occ]` when the template background matches existing
    /// empty cells (the common case). When template bg differs (BCE), marks
    /// the entire row dirty first so all cells get the new background.
    pub fn reset(&mut self, cols: usize, template: &Cell) {
        // If template bg differs from what empty cells currently contain,
        // the entire row needs updating (BCE background change).
        if self.inner.last().is_some_and(|last| last.bg != template.bg) {
            self.occ = self.inner.len();
        }

        self.inner.resize_with(cols, || template.clone());
        for cell in &mut self.inner[..self.occ.min(cols)] {
            cell.reset(template);
        }
        self.occ = 0;
    }

    /// Number of columns in this row.
    pub fn cols(&self) -> usize {
        self.inner.len()
    }

    /// Occupancy upper bound (see `occ` field docs).
    pub(crate) fn occ(&self) -> usize {
        self.occ
    }

    /// Clear cells in the given column range, resetting them to the template.
    pub fn clear_range(&mut self, range: Range<Column>, template: &Cell) {
        let start = range.start.0;
        let end = range.end.0.min(self.inner.len());
        if start >= end {
            return;
        }
        for cell in &mut self.inner[start..end] {
            cell.reset(template);
        }
        if template.is_empty() {
            // Default-bg clear: existing occ is still a valid upper bound
            // (cells replaced in-place, no rightward shift).
        } else {
            // BCE clear: the cleared cells are dirty (non-default bg).
            // Bump occ to cover them so reset() doesn't miss them when
            // the cleared range doesn't include the last cell.
            self.occ = self.occ.max(end);
        }
    }

    /// Clear from the given column to the end of the row.
    pub fn truncate(&mut self, col: Column, template: &Cell) {
        let start = col.0;
        if start >= self.inner.len() {
            return;
        }
        for cell in &mut self.inner[start..] {
            cell.reset(template);
        }
        if template.is_empty() {
            self.occ = self.occ.min(start);
        } else {
            // BCE: cells [start..end] are dirty. Bump occ to cover them
            // explicitly rather than relying on reset's last-cell sentinel.
            self.occ = self.inner.len();
        }
    }

    /// Mutable access to the inner cell slice.
    ///
    /// # Occ contract
    ///
    /// Callers **must** maintain the occ invariant after mutation:
    /// either call `clamp_occ` / `set_occ`, or verify that the
    /// existing occ is still a valid upper bound.
    pub(crate) fn as_mut_slice(&mut self) -> &mut [Cell] {
        &mut self.inner
    }

    /// Write a cell at the given column, updating occupancy.
    #[cfg(test)]
    pub(crate) fn append(&mut self, col: Column, cell: &Cell) {
        let idx = col.0;
        if idx < self.inner.len() {
            self.inner[idx] = cell.clone();
            if !cell.is_empty() && idx + 1 > self.occ {
                self.occ = idx + 1;
            }
        }
    }

    /// Clamp occ to at most `max`, maintaining it as a valid upper bound.
    pub(crate) fn clamp_occ(&mut self, max: usize) {
        self.occ = self.occ.min(max);
    }

    /// Set occ to an explicit upper bound (must be valid).
    pub(crate) fn set_occ(&mut self, occ: usize) {
        debug_assert!(
            occ <= self.inner.len(),
            "occ {occ} exceeds row length {}",
            self.inner.len(),
        );
        self.occ = occ;
    }

}

impl Index<Column> for Row {
    type Output = Cell;

    fn index(&self, col: Column) -> &Cell {
        &self.inner[col.0]
    }
}

impl IndexMut<Column> for Row {
    fn index_mut(&mut self, col: Column) -> &mut Cell {
        let idx = col.0;
        if idx + 1 > self.occ {
            self.occ = idx + 1;
        }
        &mut self.inner[idx]
    }
}

#[cfg(test)]
mod tests;
