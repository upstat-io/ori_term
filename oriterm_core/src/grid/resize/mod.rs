//! Grid resize and text reflow.
//!
//! `Grid::resize` is the main entry point: it handles dimension changes,
//! scroll region reset, cursor clamping, and optional text reflow on
//! column changes. Row resize manages scrollback interaction (shrink
//! pushes rows to scrollback, grow pulls them back). Column reflow uses
//! Ghostty-style cell-by-cell rewriting to wrap/unwrap soft-wrapped lines.

use crate::cell::{Cell, CellFlags};
use crate::index::Column;

use super::Grid;
use super::row::Row;

impl Grid {
    /// Resize the grid to new dimensions.
    ///
    /// When `reflow` is true, soft-wrapped lines are re-wrapped to fit the
    /// new column width (cell-by-cell rewriting). When false, rows are simply
    /// truncated or extended (for alternate screen).
    ///
    /// Resets scroll region, clamps cursor, and marks everything dirty.
    pub fn resize(&mut self, new_lines: usize, new_cols: usize, reflow: bool) {
        if new_cols == 0 || new_lines == 0 {
            return;
        }
        if new_cols == self.cols && new_lines == self.lines {
            return;
        }

        if reflow && new_cols != self.cols {
            if new_cols > self.cols {
                // Growing cols: reflow first (unwrap), then adjust rows.
                self.reflow_cols(new_cols);
                self.cols = new_cols;
                Self::reset_tab_stops(&mut self.tab_stops, new_cols);
                self.resize_rows(new_lines);
            } else {
                // Shrinking cols: adjust rows first, then reflow (wrap).
                self.resize_rows(new_lines);
                self.reflow_cols(new_cols);
                self.cols = new_cols;
                Self::reset_tab_stops(&mut self.tab_stops, new_cols);
            }
        } else {
            self.resize_no_reflow(new_cols, new_lines);
        }

        // Reset scroll region, clamp cursor, mark dirty.
        self.finalize_resize();
    }

    /// Resize without text reflow (for alt screen or same-width changes).
    fn resize_no_reflow(&mut self, new_cols: usize, new_lines: usize) {
        self.resize_rows(new_lines);
        if new_cols != self.cols {
            for row in &mut self.rows {
                row.resize(new_cols);
            }
            self.cols = new_cols;
            Self::reset_tab_stops(&mut self.tab_stops, new_cols);
        }
    }

    /// Common post-resize cleanup: scroll region, cursor clamping, dirty.
    ///
    /// `dirty.resize()` unconditionally calls `mark_all()`, so all lines
    /// are guaranteed dirty after this returns. Callers need not mark dirty
    /// separately.
    fn finalize_resize(&mut self) {
        self.scroll_region = 0..self.lines;

        let max_line = self.lines.saturating_sub(1);
        let max_col = self.cols.saturating_sub(1);
        if self.cursor.line() > max_line {
            self.cursor.set_line(max_line);
        }
        if self.cursor.col().0 > max_col {
            self.cursor.set_col(Column(max_col));
        }
        if let Some(saved) = &mut self.saved_cursor {
            if saved.line() > max_line {
                saved.set_line(max_line);
            }
            if saved.col().0 > max_col {
                saved.set_col(Column(max_col));
            }
        }

        // Reset to live view. Reflow rewrites scrollback entirely, so the
        // old display_offset no longer points at the same content. Keeping a
        // stale offset causes the renderer to show corrupted/duplicated
        // scrollback instead of the live cursor position.
        self.display_offset = 0;
        self.dirty.resize(self.lines);
    }

    /// Resize the number of visible lines.
    fn resize_rows(&mut self, new_lines: usize) {
        if new_lines == self.lines {
            return;
        }
        if new_lines < self.lines {
            self.shrink_rows(new_lines);
        } else {
            self.grow_rows(new_lines);
        }
        self.lines = new_lines;
        self.dirty.resize(new_lines);
    }

    /// Shrink visible rows: trim trailing blanks, push excess to scrollback.
    fn shrink_rows(&mut self, new_lines: usize) {
        let to_remove = self.lines - new_lines;
        let trimmed = self.count_trailing_blank_rows(to_remove);
        for _ in 0..trimmed {
            self.rows.pop();
        }
        let push_count = (to_remove - trimmed).min(self.rows.len());
        for row in self.rows.drain(..push_count) {
            if self.scrollback.push(row).is_some() {
                self.total_evicted += 1;
            }
        }
        self.cursor
            .set_line(self.cursor.line().saturating_sub(push_count));
        self.rows.truncate(new_lines);
        while self.rows.len() < new_lines {
            self.rows.push(Row::new(self.cols));
        }
    }

    /// Grow visible rows: consume scrollback slots and add blank rows.
    ///
    /// When the cursor is at the bottom, scrollback rows are consumed
    /// (maintaining stable row indices) but their content is cleared before
    /// insertion. This prevents stale scrollback content from appearing in
    /// the visible area where shell incremental-rendering (e.g. Ink) might
    /// skip overwriting it, causing ghosting.
    fn grow_rows(&mut self, new_lines: usize) {
        let delta = new_lines - self.lines;
        if self.cursor.line() >= self.lines.saturating_sub(1) {
            let from_sb = delta.min(self.scrollback.len());
            // Consume scrollback slots to maintain StableRowIndex stability,
            // but don't show stale content — insert blank rows instead.
            for _ in 0..from_sb {
                self.scrollback.pop_newest();
            }
            self.resize_pushed = self.resize_pushed.saturating_sub(from_sb);
            // Insert blank rows at top, shifting cursor down.
            let blanks: Vec<Row> = (0..from_sb).map(|_| Row::new(self.cols)).collect();
            let blank_count = blanks.len();
            self.rows.splice(0..0, blanks);
            self.cursor.set_line(self.cursor.line() + blank_count);
            for _ in 0..(delta - from_sb) {
                self.rows.push(Row::new(self.cols));
            }
        } else {
            for _ in 0..delta {
                self.rows.push(Row::new(self.cols));
            }
        }
    }

    /// Count trailing blank rows from the bottom, below the cursor.
    fn count_trailing_blank_rows(&self, max: usize) -> usize {
        let len = self.rows.len();
        let mut count = 0;
        while count < max && len > count + 1 {
            let idx = len - 1 - count;
            if idx <= self.cursor.line() {
                break;
            }
            if !self.rows[idx].is_blank() {
                break;
            }
            count += 1;
        }
        count
    }

    /// Reflow content to fit new column width using cell-by-cell rewriting.
    ///
    /// Handles both growing (unwrapping) and shrinking (re-wrapping).
    /// Cursor position is tracked through the reflow.
    fn reflow_cols(&mut self, new_cols: usize) {
        let old_cols = self.cols;
        if old_cols == new_cols || new_cols == 0 {
            return;
        }

        // Collect all rows: scrollback (oldest first) then visible.
        let (all_rows, visible_start) = self.collect_all_rows();
        let cursor_abs = visible_start + self.cursor.line();
        let cursor_col = self.cursor.col().0;

        // Real history ends where previous reflow overflow begins.
        // In `all_rows`: [0..history_end) = real history,
        // [history_end..visible_start) = reflow overflow from last resize,
        // [visible_start..) = visible rows.
        let history_boundary = visible_start.saturating_sub(self.resize_pushed);

        // Reflow cells into new-width rows.
        let (result, new_cursor_abs, new_cursor_col, new_history_boundary) = reflow_cells(
            &all_rows,
            old_cols,
            new_cols,
            cursor_abs,
            cursor_col,
            history_boundary,
        );

        // Distribute into scrollback + visible, update cursor.
        self.apply_reflow_result(
            result,
            new_cols,
            new_cursor_abs,
            new_cursor_col,
            new_history_boundary,
        );
    }

    /// Collect all rows (scrollback oldest-first + visible) for reflow.
    fn collect_all_rows(&mut self) -> (Vec<Row>, usize) {
        let mut all_rows = self.scrollback.drain_oldest_first();
        let visible_start = all_rows.len();
        all_rows.append(&mut self.rows);
        (all_rows, visible_start)
    }

    /// Apply reflow result: split into scrollback + visible, update cursor.
    ///
    /// `new_history_boundary` is the output row index where real scrollback
    /// history ends. Rows beyond that in the scrollback portion are reflow
    /// overflow (stale copies of visible content that wrapped).
    #[expect(
        clippy::too_many_arguments,
        reason = "reflow result distribution: rows, dimensions, cursor, history boundary"
    )]
    fn apply_reflow_result(
        &mut self,
        mut result: Vec<Row>,
        new_cols: usize,
        new_cursor_abs: usize,
        new_cursor_col: usize,
        new_history_boundary: usize,
    ) {
        // All rows in `result` are already `new_cols` wide (created by
        // `Row::new(new_cols)` in `reflow_cells`), so no resize needed.
        if result.is_empty() {
            result.push(Row::new(new_cols));
        }

        // Trim trailing blank rows so they don't push real content into
        // scrollback. Keep at least `self.lines` rows (visible area) and
        // enough to include the cursor position.
        let min_rows = self.lines.max(new_cursor_abs + 1);
        while result.len() > min_rows && result.last().is_some_and(Row::is_blank) {
            result.pop();
        }

        let total = result.len();
        self.scrollback.clear();
        if total > self.lines {
            let sb_count = total - self.lines;
            for row in result.drain(..sb_count) {
                self.scrollback.push(row);
            }
            // Overflow = scrollback rows beyond the real history boundary.
            self.resize_pushed = sb_count.saturating_sub(new_history_boundary);
        } else {
            self.resize_pushed = 0;
            while result.len() < self.lines {
                result.push(Row::new(new_cols));
            }
        }
        self.rows = result;

        let sb_len = self.scrollback.len();
        self.cursor.set_line(if new_cursor_abs >= sb_len {
            (new_cursor_abs - sb_len).min(self.lines.saturating_sub(1))
        } else {
            0
        });
        self.cursor
            .set_col(Column(new_cursor_col.min(new_cols.saturating_sub(1))));
    }
}

/// Reflow all rows from old column width to new column width.
///
/// Returns (reflowed rows, new cursor abs, new cursor col, new history boundary).
/// `history_boundary` is the source row index where real scrollback history ends.
#[expect(
    clippy::too_many_arguments,
    reason = "reflow state: source rows, dimensions, cursor position, history boundary"
)]
fn reflow_cells(
    all_rows: &[Row],
    old_cols: usize,
    new_cols: usize,
    cursor_abs: usize,
    cursor_col: usize,
    history_boundary: usize,
) -> (Vec<Row>, usize, usize, usize) {
    let mut new_cursor_abs = 0usize;
    let mut new_cursor_col = 0usize;
    let mut new_history_boundary = 0usize;
    let mut history_tracked = false;
    let mut result: Vec<Row> = Vec::with_capacity(all_rows.len());
    let mut out_row = Row::new(new_cols);
    let mut out_col = 0usize;

    for (src_idx, src_row) in all_rows.iter().enumerate() {
        // Track where the history boundary maps in the output.
        if !history_tracked && src_idx >= history_boundary {
            new_history_boundary = result.len();
            history_tracked = true;
        }

        let wrapped = old_cols > 0
            && src_row.cols() >= old_cols
            && src_row[Column(old_cols - 1)]
                .flags
                .contains(CellFlags::WRAP);

        let content_len = if wrapped {
            old_cols
        } else {
            src_row.content_len()
        };

        reflow_row_cells(
            src_row,
            src_idx,
            content_len,
            new_cols,
            cursor_abs,
            cursor_col,
            &mut result,
            &mut out_row,
            &mut out_col,
            &mut new_cursor_abs,
            &mut new_cursor_col,
        );

        // Track cursor when it's past content on this source row.
        if src_idx == cursor_abs && cursor_col >= content_len {
            new_cursor_abs = result.len();
            new_cursor_col = if wrapped {
                out_col.min(new_cols.saturating_sub(1))
            } else {
                cursor_col.min(new_cols.saturating_sub(1))
            };
        }

        // End of source row: finalize if not wrapped.
        if !wrapped {
            result.push(out_row);
            out_row = Row::new(new_cols);
            out_col = 0;
        }
    }

    // If all rows are real history (boundary at or past end).
    if !history_tracked {
        new_history_boundary = result.len() + usize::from(out_col > 0);
    }

    if out_col > 0 {
        result.push(out_row);
    }

    (result, new_cursor_abs, new_cursor_col, new_history_boundary)
}

/// Reflow cells from a single source row into the output.
#[expect(
    clippy::too_many_arguments,
    reason = "cell-by-cell reflow: source context, output state, cursor tracking"
)]
fn reflow_row_cells(
    src_row: &Row,
    src_idx: usize,
    content_len: usize,
    new_cols: usize,
    cursor_abs: usize,
    cursor_col: usize,
    result: &mut Vec<Row>,
    out_row: &mut Row,
    out_col: &mut usize,
    new_cursor_abs: &mut usize,
    new_cursor_col: &mut usize,
) {
    for src_col in 0..content_len {
        let cell = &src_row[Column(src_col)];

        // Skip spacer cells (regenerated at new positions).
        if cell
            .flags
            .intersects(CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER)
        {
            if src_idx == cursor_abs && src_col == cursor_col {
                *new_cursor_abs = result.len();
                *new_cursor_col = out_col.saturating_sub(1);
            }
            continue;
        }

        let is_wide = cell.flags.contains(CellFlags::WIDE_CHAR) && new_cols >= 2;
        let cell_width = if is_wide { 2 } else { 1 };

        // Wrap to next output row if cell doesn't fit.
        if *out_col + cell_width > new_cols {
            if *out_col > 0 {
                let boundary = &mut out_row[Column(new_cols - 1)];
                boundary.flags.insert(CellFlags::WRAP);
                // Wide char at boundary with a gap cell: the cell at
                // new_cols - 1 is padding, not content. Mark it so
                // reflow/selection/search skips it.
                if is_wide && *out_col < new_cols {
                    boundary.ch = ' ';
                    boundary.flags.insert(CellFlags::LEADING_WIDE_CHAR_SPACER);
                }
            }
            out_row.set_occ(new_cols);
            result.push(std::mem::replace(out_row, Row::new(new_cols)));
            *out_col = 0;
        }

        // Track cursor position.
        if src_idx == cursor_abs && src_col == cursor_col {
            *new_cursor_abs = result.len();
            *new_cursor_col = *out_col;
        }

        // Write cell (strip old WRAP and LEADING_WIDE_CHAR_SPACER flags).
        let mut new_cell = cell.clone();
        new_cell
            .flags
            .remove(CellFlags::WRAP | CellFlags::LEADING_WIDE_CHAR_SPACER);
        if !is_wide && cell.flags.contains(CellFlags::WIDE_CHAR) {
            new_cell.flags.remove(CellFlags::WIDE_CHAR);
        }
        out_row[Column(*out_col)] = new_cell;
        *out_col += 1;

        // Write wide char spacer in next column.
        if is_wide {
            let mut spacer = Cell::default();
            spacer.flags.insert(CellFlags::WIDE_CHAR_SPACER);
            spacer.fg = cell.fg;
            spacer.bg = cell.bg;
            out_row[Column(*out_col)] = spacer;
            *out_col += 1;
        }
        out_row.set_occ(*out_col);
    }
}

#[cfg(test)]
mod tests;
