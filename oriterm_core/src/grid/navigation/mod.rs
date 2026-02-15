//! Cursor movement and navigation operations.
//!
//! Implements CUU/CUD/CUF/CUB/CUP/CHA/VPA/CR/LF/RI/NEL/HT/CBT and
//! tab stop management. All movement is clamped to grid bounds and
//! respects the scroll region where applicable.

use crate::index::Column;

use super::Grid;

/// Tab clear mode for TBC (Tabulation Clear).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabClearMode {
    /// Clear tab stop at the current column.
    Current,
    /// Clear all tab stops.
    All,
}

impl Grid {
    /// CUU: move cursor up by `count` lines, clamped to the top of the
    /// scroll region (if inside it) or line 0.
    pub fn move_up(&mut self, count: usize) {
        let line = self.cursor.line();
        let top = if line >= self.scroll_region.start && line < self.scroll_region.end {
            self.scroll_region.start
        } else {
            0
        };
        self.cursor.set_line(line.saturating_sub(count).max(top));
    }

    /// CUD: move cursor down by `count` lines, clamped to the bottom of
    /// the scroll region (if inside it) or the last line.
    pub fn move_down(&mut self, count: usize) {
        let line = self.cursor.line();
        let bottom = if line >= self.scroll_region.start && line < self.scroll_region.end {
            self.scroll_region.end - 1
        } else {
            self.lines - 1
        };
        self.cursor.set_line((line + count).min(bottom));
    }

    /// CUF: move cursor right by `count` columns, clamped to the last column.
    pub fn move_forward(&mut self, count: usize) {
        let col = self.cursor.col().0;
        let last = self.cols - 1;
        self.cursor.set_col(Column((col + count).min(last)));
    }

    /// CUB: move cursor left by `count` columns, clamped to column 0.
    pub fn move_backward(&mut self, count: usize) {
        let col = self.cursor.col().0;
        self.cursor.set_col(Column(col.saturating_sub(count)));
    }

    /// CUP: set cursor to absolute `(line, col)`, clamped to grid bounds.
    pub fn move_to(&mut self, line: usize, col: Column) {
        self.cursor.set_line(line.min(self.lines - 1));
        self.cursor.set_col(Column(col.0.min(self.cols - 1)));
    }

    /// CHA: set cursor column to `col`, clamped to the last column.
    pub fn move_to_column(&mut self, col: Column) {
        self.cursor.set_col(Column(col.0.min(self.cols - 1)));
    }

    /// VPA: set cursor line to `line`, clamped to the last line.
    pub fn move_to_line(&mut self, line: usize) {
        self.cursor.set_line(line.min(self.lines - 1));
    }

    /// CR: move cursor to column 0.
    pub fn carriage_return(&mut self) {
        self.cursor.set_col(Column(0));
    }

    /// BS: move cursor left by one column.
    ///
    /// If the cursor is in wrap-pending state (col >= cols), snaps to the
    /// last column. Otherwise moves left by one, clamped at column 0.
    pub fn backspace(&mut self) {
        let col = self.cursor.col().0;
        let cols = self.cols;

        if col >= cols {
            // Wrap-pending: snap to last column.
            self.cursor.set_col(Column(cols - 1));
        } else if col > 0 {
            self.cursor.set_col(Column(col - 1));
        } else {
            // Already at column 0: no-op.
        }
    }

    /// LF: move cursor down one line. If at the bottom of the scroll
    /// region, scroll the region up instead of moving.
    pub fn linefeed(&mut self) {
        let line = self.cursor.line();
        if line + 1 == self.scroll_region.end {
            // At bottom of scroll region: scroll region content up.
            self.scroll_up(1);
        } else if line + 1 < self.lines {
            self.cursor.set_line(line + 1);
        } else {
            // Already at last line, outside scroll region: no-op.
        }
    }

    /// RI: move cursor up one line. If at the top of the scroll region,
    /// scroll the region down instead of moving.
    pub fn reverse_index(&mut self) {
        let line = self.cursor.line();
        if line == self.scroll_region.start {
            // At top of scroll region: scroll region content down.
            self.scroll_down(1);
        } else if line > 0 {
            self.cursor.set_line(line - 1);
        } else {
            // Already at line 0, outside scroll region: no-op.
        }
    }

    /// NEL: carriage return followed by linefeed.
    pub fn next_line(&mut self) {
        self.carriage_return();
        self.linefeed();
    }

    /// HT: advance cursor to the next tab stop, or end of line.
    pub fn tab(&mut self) {
        let col = self.cursor.col().0;
        let last = self.cols - 1;

        // Search forward for the next tab stop.
        for c in (col + 1)..self.cols {
            if self.tab_stops[c] {
                self.cursor.set_col(Column(c));
                return;
            }
        }
        // No tab stop found: move to last column.
        self.cursor.set_col(Column(last));
    }

    /// CBT: move cursor to the previous tab stop, or column 0.
    pub fn tab_backward(&mut self) {
        // Clamp to cols so wrap-pending (col == cols) or any out-of-range
        // value never indexes past the tab_stops array.
        let col = self.cursor.col().0.min(self.cols);

        // Search backward for the previous tab stop.
        for c in (0..col).rev() {
            if self.tab_stops[c] {
                self.cursor.set_col(Column(c));
                return;
            }
        }
        // No tab stop found: move to column 0.
        self.cursor.set_col(Column(0));
    }

    /// HTS: set a tab stop at the current cursor column.
    pub fn set_tab_stop(&mut self) {
        let col = self.cursor.col().0;
        if col < self.cols {
            self.tab_stops[col] = true;
        }
    }

    /// TBC: clear tab stop(s) according to mode.
    pub fn clear_tab_stop(&mut self, mode: TabClearMode) {
        match mode {
            TabClearMode::Current => {
                let col = self.cursor.col().0;
                if col < self.cols {
                    self.tab_stops[col] = false;
                }
            }
            TabClearMode::All => {
                self.tab_stops.fill(false);
            }
        }
    }

    /// DECSC: save cursor position and template.
    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some(self.cursor.clone());
    }

    /// DECRC: restore cursor from saved state, or reset to origin if
    /// nothing was saved.
    pub fn restore_cursor(&mut self) {
        if let Some(saved) = &self.saved_cursor {
            self.cursor = saved.clone();
        } else {
            self.cursor = super::cursor::Cursor::new();
        }
    }
}

#[cfg(test)]
mod tests;
