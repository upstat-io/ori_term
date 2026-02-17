//! Grid editing operations.
//!
//! Character insertion, deletion, and erase operations. These are the
//! primitives the VTE handler calls for writing text and manipulating
//! grid content.

use unicode_width::UnicodeWidthChar;

use crate::cell::{Cell, CellFlags};
use crate::index::Column;

use super::Grid;

/// Erase mode for display erase operations (ED / CSI Ps J).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayEraseMode {
    /// Erase from cursor to end of display.
    Below,
    /// Erase from start of display to cursor.
    Above,
    /// Erase entire display.
    All,
    /// Erase scrollback buffer only (CSI 3 J).
    Scrollback,
}

/// Erase mode for line erase operations (EL / CSI Ps K).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEraseMode {
    /// Erase from cursor to end of line.
    Right,
    /// Erase from start of line to cursor.
    Left,
    /// Erase entire line.
    All,
}

impl Grid {
    /// Write a character at the cursor position.
    ///
    /// Handles wide characters (writes cell + spacer), wrap at end of line,
    /// and clearing overwritten wide char pairs.
    pub fn put_char(&mut self, ch: char) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        let width = UnicodeWidthChar::width(ch).unwrap_or(1);
        let cols = self.cols;

        // Wide char can never fit in this terminal width — skip it.
        // Without this guard, a width-2 char on a 1-column grid would
        // loop forever: wrap → col 0 → can't fit → wrap → col 0 → …
        if width > cols {
            return;
        }

        loop {
            let line = self.cursor.line();
            let col = self.cursor.col().0;

            // If a pending wrap is active and we're at the last column, wrap now.
            if col >= cols {
                self.rows[line][Column(cols - 1)].flags |= CellFlags::WRAP;
                self.linefeed();
                self.cursor.set_col(Column(0));
                continue;
            }

            // For wide chars at the last column, wrap instead of splitting.
            if width == 2 && col + 1 >= cols {
                self.rows[line][Column(col)].flags |= CellFlags::WRAP;
                self.linefeed();
                self.cursor.set_col(Column(0));
                continue;
            }

            // Clear any wide char pair that we're overwriting.
            self.clear_wide_char_at(line, col);

            // Extract template fields before mutable row borrow. `rows` and
            // `cursor` are disjoint Grid fields, so this avoids a full Cell clone.
            let tmpl_fg = self.cursor.template.fg;
            let tmpl_bg = self.cursor.template.bg;
            let tmpl_flags = self.cursor.template.flags;
            let tmpl_extra = self.cursor.template.extra.clone();
            let cell = &mut self.rows[line][Column(col)];
            cell.ch = ch;
            cell.fg = tmpl_fg;
            cell.bg = tmpl_bg;
            cell.flags = tmpl_flags;
            cell.extra = tmpl_extra;

            if width == 2 {
                cell.flags |= CellFlags::WIDE_CHAR;

                // Write the spacer in the next column.
                if col + 1 < cols {
                    self.clear_wide_char_at(line, col + 1);
                    let spacer = &mut self.rows[line][Column(col + 1)];
                    spacer.ch = ' ';
                    spacer.fg = tmpl_fg;
                    spacer.bg = tmpl_bg;
                    spacer.flags = CellFlags::WIDE_CHAR_SPACER;
                    spacer.extra = None;
                }
            }

            // Advance cursor by character width.
            self.cursor.set_col(Column(col + width));

            self.dirty.mark(line);
            break;
        }
    }

    /// Append a zero-width character (combining mark) to the previous cell.
    ///
    /// Backtracks from the cursor to find the cell that was just written.
    /// If the cursor is at column 0 with no previous cell, the character
    /// is silently discarded. Handles wrap-pending state and wide-char
    /// spacers.
    pub fn push_zerowidth(&mut self, ch: char) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        let col = self.cursor.col().0;
        let cols = self.cols;

        // Determine the column of the previous cell.
        let prev_col = if col < cols {
            // Normal: cursor hasn't wrapped yet.
            col.checked_sub(1)
        } else {
            // Wrap pending: cursor is past last column; previous cell is
            // the last column.
            Some(cols.saturating_sub(1))
        };

        let Some(mut prev_col) = prev_col else {
            // Column 0 with no previous cell — discard.
            return;
        };

        let line = self.cursor.line();

        // If on a wide-char spacer, step back to the base cell.
        if self.rows[line][Column(prev_col)]
            .flags
            .contains(CellFlags::WIDE_CHAR_SPACER)
        {
            prev_col = prev_col.saturating_sub(1);
        }

        self.rows[line][Column(prev_col)].push_zerowidth(ch);
        self.dirty.mark(line);
    }

    /// Insert `count` blank cells at the cursor, shifting existing cells right.
    ///
    /// Cells that shift past the right edge are lost.
    pub fn insert_blank(&mut self, count: usize) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        let line = self.cursor.line();
        let col = self.cursor.col().0;
        let cols = self.cols;
        // BCE: erased cells get only the current background color.
        let template = Cell::from(self.cursor.template.bg);

        if col >= cols {
            return;
        }

        let count = count.min(cols - col);

        // Clean the partner of any wide char pair at the insertion point,
        // then strip the cell's own wide flag so the shifted copy doesn't
        // carry a stale WIDE_CHAR or WIDE_CHAR_SPACER to its new position.
        self.clear_wide_char_at(line, col);
        self.rows[line][Column(col)]
            .flags
            .remove(CellFlags::WIDE_CHAR | CellFlags::WIDE_CHAR_SPACER);

        let row = &mut self.rows[line];
        let cells = row.as_mut_slice();

        // Shift cells right by swapping (no allocation).
        for i in (col + count..cols).rev() {
            cells.swap(i, i - count);
        }

        // Reset the gap cells in-place.
        for cell in &mut cells[col..col + count] {
            cell.reset(&template);
        }

        // Fix wide char base pushed to the right edge (spacer fell off-screen).
        if cells[cols - 1].flags.contains(CellFlags::WIDE_CHAR) {
            cells[cols - 1].ch = ' ';
            cells[cols - 1].flags.remove(CellFlags::WIDE_CHAR);
        }

        // Cells shifted right: occ grows by at most `count`, capped at cols.
        row.set_occ((row.occ() + count).min(cols));

        self.dirty.mark(line);
    }

    /// Delete `count` cells at the cursor, shifting remaining cells left.
    ///
    /// New cells at the right edge are blank.
    pub fn delete_chars(&mut self, count: usize) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        let line = self.cursor.line();
        let col = self.cursor.col().0;
        let cols = self.cols;
        // BCE: erased cells get only the current background color.
        let template = Cell::from(self.cursor.template.bg);

        if col >= cols {
            return;
        }

        let count = count.min(cols - col);

        // Clean wide char pair at the cursor so stale flags don't persist.
        self.clear_wide_char_at(line, col);
        // Spacer at first shifted position: its base is in the delete zone.
        if col + count < cols
            && self.rows[line][Column(col + count)]
                .flags
                .contains(CellFlags::WIDE_CHAR_SPACER)
        {
            self.rows[line][Column(col + count)].ch = ' ';
            self.rows[line][Column(col + count)]
                .flags
                .remove(CellFlags::WIDE_CHAR_SPACER);
        }

        let row = &mut self.rows[line];
        let cells = row.as_mut_slice();

        // Shift cells left by swapping (no allocation).
        for i in col..cols - count {
            cells.swap(i, i + count);
        }

        // Reset the vacated right cells in-place.
        for cell in &mut cells[cols - count..cols] {
            cell.reset(&template);
        }

        if !template.is_empty() {
            // BCE: fill cells at [cols-count..cols] are dirty.
            row.set_occ(cols);
        }
        // else: Content shifted left; existing occ remains a valid upper
        // bound. Fill cells are empty and don't extend the dirty range.

        self.dirty.mark(line);
    }

    /// Erase part or all of the display.
    pub fn erase_display(&mut self, mode: DisplayEraseMode) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        // BCE: erased cells get only the current background color.
        let template = Cell::from(self.cursor.template.bg);
        match mode {
            DisplayEraseMode::Below => {
                self.erase_line_with_template(LineEraseMode::Right, &template);
                let cursor_line = self.cursor.line();
                for line in cursor_line + 1..self.lines {
                    self.rows[line].reset(self.cols, &template);
                    self.dirty.mark(line);
                }
            }
            DisplayEraseMode::Above => {
                self.erase_line_with_template(LineEraseMode::Left, &template);
                let cursor_line = self.cursor.line();
                for line in 0..cursor_line {
                    self.rows[line].reset(self.cols, &template);
                    self.dirty.mark(line);
                }
            }
            DisplayEraseMode::All => {
                for line in 0..self.lines {
                    self.rows[line].reset(self.cols, &template);
                }
                self.dirty.mark_all();
            }
            DisplayEraseMode::Scrollback => {
                // Scrollback clearing will be implemented in 1.10.
            }
        }
    }

    /// Erase part or all of the current line.
    pub fn erase_line(&mut self, mode: LineEraseMode) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        let template = Cell::from(self.cursor.template.bg);
        self.erase_line_with_template(mode, &template);
    }

    /// Erase part or all of the current line using a pre-built BCE template.
    fn erase_line_with_template(&mut self, mode: LineEraseMode, template: &Cell) {
        let line = self.cursor.line();
        let col = self.cursor.col().0;
        let cols = self.cols;

        match mode {
            LineEraseMode::Right => {
                // Fix spacer at cursor whose base is before the erase range.
                self.fix_wide_boundaries(line, col, cols);
                let row = &mut self.rows[line];
                let cells = row.as_mut_slice();
                for cell in &mut cells[col..cols] {
                    cell.reset(template);
                }
                if template.is_empty() {
                    row.clamp_occ(col);
                } else {
                    row.set_occ(cols);
                }
            }
            LineEraseMode::Left => {
                let end = col.min(cols - 1) + 1;
                // Fix base at end-1 whose spacer is after the erase range.
                self.fix_wide_boundaries(line, 0, end);
                let row = &mut self.rows[line];
                let cells = row.as_mut_slice();
                for cell in &mut cells[..end] {
                    cell.reset(template);
                }
                if template.is_empty() {
                    // Cells [0..end] are now empty. Only cells beyond end
                    // may be dirty, so if occ was within the erased range
                    // all dirty cells are gone.
                    if row.occ() <= end {
                        row.set_occ(0);
                    }
                } else {
                    row.set_occ(row.occ().max(end));
                }
            }
            LineEraseMode::All => {
                self.rows[line].reset(cols, template);
            }
        }

        self.dirty.mark(line);
    }

    /// Erase `count` cells starting at cursor (replace with template, don't shift).
    pub fn erase_chars(&mut self, count: usize) {
        debug_assert!(
            self.cursor.line() < self.lines,
            "cursor line {} out of bounds (lines={})",
            self.cursor.line(),
            self.lines,
        );
        let line = self.cursor.line();
        let col = self.cursor.col().0;
        let cols = self.cols;
        // BCE: erased cells get only the current background color.
        let template = Cell::from(self.cursor.template.bg);

        let end = (col + count).min(cols);

        // Fix wide char pairs split by the erase boundary.
        self.fix_wide_boundaries(line, col, end);

        let row = &mut self.rows[line];
        let cells = row.as_mut_slice();
        for cell in &mut cells[col..end] {
            cell.reset(&template);
        }
        // BCE template has a colored bg — the erased cells are dirty.
        // Default template produces truly empty cells, so existing occ
        // remains a valid upper bound (we only cleared, didn't extend).
        if !template.is_empty() {
            row.set_occ(row.occ().max(end));
        }

        self.dirty.mark(line);
    }

    /// Fix wide char pairs split by an erase of `[start..end)`.
    ///
    /// Clears orphaned halves OUTSIDE the range. Call BEFORE resetting.
    fn fix_wide_boundaries(&mut self, line: usize, start: usize, end: usize) {
        let cols = self.cols;
        if start > 0
            && start < cols
            && self.rows[line][Column(start)]
                .flags
                .contains(CellFlags::WIDE_CHAR_SPACER)
        {
            self.rows[line][Column(start - 1)].ch = ' ';
            self.rows[line][Column(start - 1)]
                .flags
                .remove(CellFlags::WIDE_CHAR);
        }
        if end > 0
            && end < cols
            && self.rows[line][Column(end - 1)]
                .flags
                .contains(CellFlags::WIDE_CHAR)
        {
            self.rows[line][Column(end)].ch = ' ';
            self.rows[line][Column(end)]
                .flags
                .remove(CellFlags::WIDE_CHAR_SPACER);
        }
    }

    /// Clear any wide char pair at the given position.
    ///
    /// If the cell is a wide char spacer, clears the preceding wide char.
    /// If the cell is a wide char, clears its trailing spacer.
    fn clear_wide_char_at(&mut self, line: usize, col: usize) {
        let cols = self.cols;

        if col >= cols {
            return;
        }

        let flags = self.rows[line][Column(col)].flags;

        // Overwriting a spacer: clear the wide char that owns it.
        if flags.contains(CellFlags::WIDE_CHAR_SPACER) && col > 0 {
            let prev = &mut self.rows[line][Column(col - 1)];
            prev.ch = ' ';
            prev.flags.remove(CellFlags::WIDE_CHAR);
        }

        // Overwriting a wide char: clear its spacer.
        if flags.contains(CellFlags::WIDE_CHAR) && col + 1 < cols {
            let next = &mut self.rows[line][Column(col + 1)];
            next.ch = ' ';
            next.flags.remove(CellFlags::WIDE_CHAR_SPACER);
        }
    }
}

#[cfg(test)]
mod tests;
