//! Terminal grid: 2D cell storage with cursor, scrollback, and dirty tracking.
//!
//! The `Grid` is the central data structure for terminal emulation. It stores
//! visible rows, manages cursor state, and tracks tab stops. Scrollback,
//! dirty tracking, and editing operations are added in submodules.

pub mod cursor;
pub mod dirty;
pub mod editing;
pub mod navigation;
pub mod ring;
pub mod row;
pub mod scroll;

use std::ops::{Index, IndexMut, Range};

use crate::index::Line;

pub use cursor::{Cursor, CursorShape};
pub use dirty::{DirtyIter, DirtyTracker};
pub use editing::{DisplayEraseMode, LineEraseMode};
pub use navigation::TabClearMode;
pub use ring::ScrollbackBuffer;
pub use row::Row;

/// The 2D terminal cell grid.
///
/// Stores visible rows indexed `0..lines` (top to bottom), a cursor,
/// tab stops, scrollback history, and dirty tracking for damage-based
/// rendering.
#[derive(Debug, Clone)]
pub struct Grid {
    /// Visible rows (index 0 = top of screen).
    rows: Vec<Row>,
    /// Number of columns.
    cols: usize,
    /// Number of visible lines.
    lines: usize,
    /// Current cursor position and template.
    cursor: Cursor,
    /// DECSC/DECRC saved cursor.
    saved_cursor: Option<Cursor>,
    /// Tab stop at each column (true = stop).
    tab_stops: Vec<bool>,
    /// DECSTBM scroll region: top (inclusive) .. bottom (exclusive).
    scroll_region: Range<usize>,
    /// Scrollback history (rows that scrolled off the top).
    scrollback: ScrollbackBuffer,
    /// How many lines scrolled back into history (0 = live view).
    display_offset: usize,
    /// Tracks which rows have changed since last drain.
    dirty: DirtyTracker,
}

impl Grid {
    /// Create a new grid with the given dimensions and default scrollback.
    ///
    /// Initializes all rows as empty, cursor at (0, 0), and tab stops
    /// every 8 columns.
    pub fn new(lines: usize, cols: usize) -> Self {
        Self::with_scrollback(lines, cols, ring::DEFAULT_MAX_SCROLLBACK)
    }

    /// Create a new grid with explicit scrollback capacity.
    pub fn with_scrollback(lines: usize, cols: usize, max_scrollback: usize) -> Self {
        debug_assert!(lines >= 1 && cols >= 1, "Grid dimensions must be >= 1 (got {lines}x{cols})");
        let rows = (0..lines).map(|_| Row::new(cols)).collect();
        let tab_stops = Self::init_tab_stops(cols);

        Self {
            rows,
            cols,
            lines,
            cursor: Cursor::new(),
            saved_cursor: None,
            tab_stops,
            scroll_region: 0..lines,
            scrollback: ScrollbackBuffer::new(max_scrollback),
            display_offset: 0,
            dirty: DirtyTracker::new(lines),
        }
    }

    /// Number of visible lines.
    pub fn lines(&self) -> usize {
        self.lines
    }

    /// Number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Immutable reference to the cursor.
    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// Mutable reference to the cursor.
    pub fn cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursor
    }

    /// Immutable reference to tab stops.
    #[cfg(test)]
    pub(crate) fn tab_stops(&self) -> &[bool] {
        &self.tab_stops
    }

    /// Total lines: visible + scrollback history.
    pub fn total_lines(&self) -> usize {
        self.lines + self.scrollback.len()
    }

    /// How many lines scrolled back into history (0 = live view).
    pub fn display_offset(&self) -> usize {
        self.display_offset
    }

    /// Immutable reference to the scrollback buffer.
    pub fn scrollback(&self) -> &ScrollbackBuffer {
        &self.scrollback
    }

    /// The scroll region as a half-open range (top inclusive, bottom exclusive).
    pub fn scroll_region(&self) -> &Range<usize> {
        &self.scroll_region
    }

    /// Immutable reference to the dirty tracker.
    pub fn dirty(&self) -> &DirtyTracker {
        &self.dirty
    }

    /// Mutable reference to the dirty tracker.
    pub fn dirty_mut(&mut self) -> &mut DirtyTracker {
        &mut self.dirty
    }

    /// Adjust display offset (positive = scroll back, negative = scroll forward).
    ///
    /// Clamped to `0..=scrollback.len()`.
    pub fn scroll_display(&mut self, delta: isize) {
        let max = self.scrollback.len();
        let current = self.display_offset as isize;
        let target = (current + delta).clamp(0, max as isize) as usize;

        if target != self.display_offset {
            self.display_offset = target;
            self.dirty.mark_all();
        }
    }

    /// Reset the grid to initial state.
    ///
    /// Clears all rows, resets cursor to (0,0) with default template,
    /// clears saved cursor, resets tab stops and scroll region, clears
    /// scrollback history, and marks everything dirty. Does not affect
    /// scrollback capacity.
    pub fn reset(&mut self) {
        for row in &mut self.rows {
            row.reset(self.cols, &crate::cell::Cell::default());
        }
        self.cursor = Cursor::new();
        self.saved_cursor = None;
        Self::reset_tab_stops(&mut self.tab_stops, self.cols);
        self.scroll_region = 0..self.lines;
        self.scrollback.clear();
        self.display_offset = 0;
        self.dirty.mark_all();
    }

    /// Initialize tab stops every 8 columns.
    fn init_tab_stops(cols: usize) -> Vec<bool> {
        (0..cols).map(|c| c % 8 == 0).collect()
    }

    /// Reset tab stops in-place every 8 columns, reusing the existing allocation.
    fn reset_tab_stops(tab_stops: &mut Vec<bool>, cols: usize) {
        tab_stops.resize(cols, false);
        for (i, stop) in tab_stops.iter_mut().enumerate() {
            *stop = i % 8 == 0;
        }
    }

    /// Mark the current cursor line dirty and move to `new_line`.
    ///
    /// Marks both old and new lines dirty so a damage-aware renderer
    /// redraws the cursor in both its old and new positions.
    pub(crate) fn move_cursor_line(&mut self, new_line: usize) {
        self.dirty.mark(self.cursor.line());
        self.cursor.set_line(new_line);
        self.dirty.mark(self.cursor.line());
    }

    /// Mark the current cursor line dirty and move to `new_col`.
    ///
    /// The cursor stays on the same line, so only the current line
    /// needs to be marked dirty (the cursor's old and new positions
    /// are both on this line).
    pub(crate) fn move_cursor_col(&mut self, new_col: crate::index::Column) {
        self.dirty.mark(self.cursor.line());
        self.cursor.set_col(new_col);
    }
}

impl Index<Line> for Grid {
    type Output = Row;

    fn index(&self, line: Line) -> &Row {
        debug_assert!(line.0 >= 0, "negative Line index on Grid (got {})", line.0);
        &self.rows[line.0 as usize]
    }
}

impl IndexMut<Line> for Grid {
    fn index_mut(&mut self, line: Line) -> &mut Row {
        debug_assert!(line.0 >= 0, "negative Line index on Grid (got {})", line.0);
        &mut self.rows[line.0 as usize]
    }
}

#[cfg(test)]
mod tests;
