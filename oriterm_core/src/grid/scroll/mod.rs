//! Scroll region management and scroll operations.
//!
//! Provides `set_scroll_region` (DECSTBM), `scroll_up`, `scroll_down`,
//! `insert_lines`, and `delete_lines`. All operations use O(1) rotation
//! of existing row allocations and fill new rows with BCE background.

use std::mem;
use std::ops::Range;

use crate::cell::Cell;

use super::Grid;
use super::row::Row;

impl Grid {
    /// DECSTBM: set the scroll region.
    ///
    /// Parameters are 1-based (matching VTE/ECMA-48). `top` is the first
    /// line of the region, `bottom` is the last line (or `None` for the
    /// screen height). Stored internally as a 0-based half-open range.
    ///
    /// Does **not** move the cursor — that's the handler's job (via
    /// `goto(0, 0)` which respects ORIGIN mode).
    pub fn set_scroll_region(&mut self, top: usize, bottom: Option<usize>) {
        // 1-based params: top=0 is invalid, treat as 1.
        let top = top.max(1) - 1;
        let bottom = bottom.map_or(self.lines, |b| b.min(self.lines));

        // Region must span at least 2 lines.
        if top + 1 >= bottom {
            return;
        }

        self.scroll_region = top..bottom;
    }

    /// Scroll the scroll region up by `count` lines.
    ///
    /// When the scroll region covers the full screen, evicted top rows
    /// are pushed to scrollback history. With a sub-region, top rows
    /// are lost. Blank rows appear at the bottom of the region.
    pub fn scroll_up(&mut self, count: usize) {
        let start = self.scroll_region.start;
        let end = self.scroll_region.end;
        let len = end - start;
        if len == 0 {
            return;
        }
        let count = count.min(len);

        // Push evicted rows to scrollback when scrolling the full screen.
        let is_full_screen = start == 0 && end == self.lines;
        if is_full_screen {
            // Keep user's scrollback view stable when new content arrives.
            if self.display_offset > 0 {
                let max_after_push =
                    (self.scrollback.len() + count).min(self.scrollback.max_scrollback());
                self.display_offset = (self.display_offset + count).min(max_after_push);
            }

            for i in 0..count {
                // Move the row out, leave a zero-alloc placeholder in its place.
                // The placeholder rotates to the bottom via scroll_range_up,
                // where reset() will resize it to the correct column count.
                let evicted = mem::replace(&mut self.rows[i], Row::new(0));
                if let Some(mut recycled) = self.scrollback.push(evicted) {
                    // Scrollback was full: oldest row evicted. Track for
                    // StableRowIndex stability.
                    self.total_evicted += 1;
                    recycled.reset(self.cols, &Cell::default());
                    self.rows[i] = recycled;
                }
            }
        }

        self.scroll_range_up(start..end, count);
    }

    /// Scroll the scroll region down by `count` lines.
    ///
    /// Bottom rows are lost. Blank rows appear at the top of the region.
    pub fn scroll_down(&mut self, count: usize) {
        let start = self.scroll_region.start;
        let end = self.scroll_region.end;
        self.scroll_range_down(start..end, count);
    }

    /// IL: insert `count` blank lines at the cursor, pushing existing
    /// lines down within the scroll region.
    ///
    /// Only operates if the cursor is inside the scroll region. Lines
    /// pushed past the bottom of the region are lost.
    pub fn insert_lines(&mut self, count: usize) {
        let line = self.cursor.line();
        if line < self.scroll_region.start || line >= self.scroll_region.end {
            return;
        }
        let range = line..self.scroll_region.end;
        self.scroll_range_down(range, count);
    }

    /// DL: delete `count` lines at the cursor, pulling remaining lines
    /// up within the scroll region.
    ///
    /// Only operates if the cursor is inside the scroll region. Blank
    /// lines appear at the bottom of the region.
    pub fn delete_lines(&mut self, count: usize) {
        let line = self.cursor.line();
        if line < self.scroll_region.start || line >= self.scroll_region.end {
            return;
        }
        let range = line..self.scroll_region.end;
        self.scroll_range_up(range, count);
    }

    /// Scroll a range of rows up by `count` using O(1) rotation.
    ///
    /// Top rows rotate to the bottom and are reset with BCE background.
    fn scroll_range_up(&mut self, range: Range<usize>, count: usize) {
        let len = range.end - range.start;
        if len == 0 {
            return;
        }
        let count = count.min(len);
        if count == 0 {
            return;
        }
        let template = Cell::from(self.cursor.template.bg);

        self.rows[range.start..range.end].rotate_left(count);

        for i in (range.end - count)..range.end {
            self.rows[i].reset(self.cols, &template);
        }

        self.dirty.mark_range(range);
    }

    /// Scroll a range of rows down by `count` using O(1) rotation.
    ///
    /// Bottom rows rotate to the top and are reset with BCE background.
    fn scroll_range_down(&mut self, range: Range<usize>, count: usize) {
        let len = range.end - range.start;
        if len == 0 {
            return;
        }
        let count = count.min(len);
        if count == 0 {
            return;
        }
        let template = Cell::from(self.cursor.template.bg);

        self.rows[range.start..range.end].rotate_right(count);

        for i in range.start..range.start + count {
            self.rows[i].reset(self.cols, &template);
        }

        self.dirty.mark_range(range);
    }
}

#[cfg(test)]
mod tests;
