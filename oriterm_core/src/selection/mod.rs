//! Selection — 3-point model (anchor, pivot, end) with char/word/line/block modes.
//!
//! Modeled after Windows Terminal's selection implementation: anchor is the
//! initial click position, pivot is the other boundary of the initial unit
//! (word end, line end), and end tracks the current drag position. This
//! prevents losing the originally selected unit during drag extension.

pub(crate) mod boundaries;
mod click;
#[cfg(test)]
mod tests;
pub(crate) mod text;

pub use boundaries::{logical_line_end, logical_line_start, word_boundaries};
pub use click::ClickDetector;
pub use text::extract_text;

use std::cmp::Ordering;

use crate::grid::StableRowIndex;
use crate::index::Side;

/// A point in stable grid coordinates.
///
/// Uses `StableRowIndex` so row identity survives scrollback eviction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPoint {
    pub row: StableRowIndex,
    pub col: usize,
    pub side: Side,
}

impl SelectionPoint {
    /// The effective first column included in selection at this boundary.
    ///
    /// When `side` is `Right`, the click landed on the right half of the cell,
    /// so selection starts at the next column.
    pub fn effective_start_col(&self) -> usize {
        if self.side == Side::Right {
            self.col + 1
        } else {
            self.col
        }
    }

    /// The effective last column included in selection at this boundary.
    ///
    /// When `side` is `Left` and `col > 0`, the click landed on the left half,
    /// so selection ends at the previous column.
    pub fn effective_end_col(&self) -> usize {
        if self.side == Side::Left && self.col > 0 {
            self.col - 1
        } else {
            self.col
        }
    }
}

impl Ord for SelectionPoint {
    fn cmp(&self, other: &Self) -> Ordering {
        self.row
            .cmp(&other.row)
            .then(self.col.cmp(&other.col))
            .then(match (&self.side, &other.side) {
                (Side::Left, Side::Right) => Ordering::Less,
                (Side::Right, Side::Left) => Ordering::Greater,
                _ => Ordering::Equal,
            })
    }
}

impl PartialOrd for SelectionPoint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Selection mode: character, word, line, or block (rectangular).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Character-by-character (single click + drag).
    Char,
    /// Word selection (double-click, drag expands by words).
    Word,
    /// Full logical line (triple-click, follows WRAP flag).
    Line,
    /// Rectangular block selection (Alt+click+drag).
    Block,
}

/// A selection in the terminal grid.
///
/// Uses a 3-point model (anchor, pivot, end):
/// - `anchor`: where the click started (fixed)
/// - `pivot`: other end of the initial unit (word boundary, line boundary;
///   same as anchor for Char mode)
/// - `end`: current drag position (moves with mouse)
pub struct Selection {
    pub mode: SelectionMode,
    pub anchor: SelectionPoint,
    pub pivot: SelectionPoint,
    pub end: SelectionPoint,
}

impl Selection {
    /// Create a new character-mode selection at a single point.
    pub fn new_char(row: StableRowIndex, col: usize, side: Side) -> Self {
        let point = SelectionPoint { row, col, side };
        Self {
            mode: SelectionMode::Char,
            anchor: point,
            pivot: point,
            end: point,
        }
    }

    /// Create a new word-mode selection with pre-computed boundaries.
    pub fn new_word(anchor: SelectionPoint, pivot: SelectionPoint) -> Self {
        Self {
            mode: SelectionMode::Word,
            anchor,
            pivot,
            end: anchor,
        }
    }

    /// Create a new line-mode selection with pre-computed boundaries.
    pub fn new_line(anchor: SelectionPoint, pivot: SelectionPoint) -> Self {
        Self {
            mode: SelectionMode::Line,
            anchor,
            pivot,
            end: anchor,
        }
    }

    /// Returns the normalized (start, end) range including the pivot.
    ///
    /// Computes the minimum and maximum of anchor, pivot, and end,
    /// giving a canonical range regardless of drag direction.
    pub fn ordered(&self) -> (SelectionPoint, SelectionPoint) {
        let min = self.anchor.min(self.pivot).min(self.end);
        let max = self.anchor.max(self.pivot).max(self.end);
        (min, max)
    }

    /// Precompute bounds for batch containment testing.
    ///
    /// Call once per frame, then use `SelectionBounds::contains()` for each
    /// cell. Avoids recomputing `ordered()` per cell during rendering.
    pub fn bounds(&self) -> SelectionBounds {
        let (start, end) = self.ordered();
        SelectionBounds {
            mode: self.mode,
            start,
            end,
        }
    }

    /// Test whether a cell at (`stable_row`, `col`) is within the selection.
    ///
    /// Convenience method that recomputes bounds each call. For batch testing
    /// (e.g. rendering), use `bounds()` + `SelectionBounds::contains()`.
    pub fn contains(&self, stable_row: StableRowIndex, col: usize) -> bool {
        self.bounds().contains(stable_row, col)
    }

    /// Returns true if this selection has zero area.
    ///
    /// Only possible in Char mode when anchor equals end (no drag yet).
    pub fn is_empty(&self) -> bool {
        self.mode == SelectionMode::Char && self.anchor == self.end
    }
}

/// Precomputed selection bounds for batch containment testing.
///
/// Compute once with `Selection::bounds()`, then test many cells with
/// `SelectionBounds::contains()`. This avoids redundant min/max computation
/// per cell during rendering (O(1) per cell instead of O(3) comparisons).
pub struct SelectionBounds {
    pub mode: SelectionMode,
    pub start: SelectionPoint,
    pub end: SelectionPoint,
}

impl SelectionBounds {
    /// Test whether a cell at (`stable_row`, `col`) is within these bounds.
    pub fn contains(&self, stable_row: StableRowIndex, col: usize) -> bool {
        if self.mode == SelectionMode::Block {
            let min_col = self.start.col.min(self.end.col);
            let max_col = self.start.col.max(self.end.col);
            stable_row >= self.start.row
                && stable_row <= self.end.row
                && col >= min_col
                && col <= max_col
        } else {
            if stable_row < self.start.row || stable_row > self.end.row {
                return false;
            }
            let first = if stable_row == self.start.row {
                self.start.effective_start_col()
            } else {
                0
            };
            let last = if stable_row == self.end.row {
                self.end.effective_end_col()
            } else {
                usize::MAX
            };
            col >= first && col <= last
        }
    }
}
