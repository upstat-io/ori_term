//! Human-readable grid snapshot rendering for snapshot tests.
//!
//! Produces text diagrams like:
//! ```text
//! [Grid 3x10 cursor=(1,3)]
//! |Hello     |
//! |wo rld    |
//! |          |
//! ```
//!
//! Used by test files across `grid/` submodules via `Grid::snapshot()`.

#[cfg(test)]
use std::fmt::Write;

#[cfg(test)]
use crate::cell::CellFlags;
#[cfg(test)]
use crate::index::Column;

#[cfg(test)]
use super::Grid;

/// Renders a grid snapshot as a human-readable text diagram.
#[cfg(test)]
pub(crate) struct GridSnapshot<'a> {
    grid: &'a Grid,
    show_scrollback: bool,
}

#[cfg(test)]
impl<'a> GridSnapshot<'a> {
    /// Create a snapshot renderer for the given grid.
    pub(crate) fn new(grid: &'a Grid) -> Self {
        Self {
            grid,
            show_scrollback: false,
        }
    }

    /// Include scrollback rows above the visible area.
    pub(crate) fn with_scrollback(mut self) -> Self {
        self.show_scrollback = true;
        self
    }

    /// Render the grid as a text diagram.
    pub(crate) fn render(&self) -> String {
        let g = self.grid;
        let mut out = String::new();

        // Header line.
        let sb_len = g.scrollback().len();
        if self.show_scrollback && sb_len > 0 {
            writeln!(
                out,
                "[Grid {}x{} cursor=({},{}) scrollback={}]",
                g.lines(),
                g.cols(),
                g.cursor().line(),
                g.cursor().col().0,
                sb_len,
            )
            .unwrap();
        } else {
            writeln!(
                out,
                "[Grid {}x{} cursor=({},{})]",
                g.lines(),
                g.cols(),
                g.cursor().line(),
                g.cursor().col().0,
            )
            .unwrap();
        }

        // Scrollback rows (oldest first).
        if self.show_scrollback && sb_len > 0 {
            writeln!(out, "--- scrollback ---").unwrap();
            for i in (0..sb_len).rev() {
                let row = g.scrollback().get(i).unwrap();
                Self::render_row(row, &mut out);
            }
            writeln!(out, "--- visible ---").unwrap();
        }

        // Visible rows.
        for line in 0..g.lines() {
            let row = &g[crate::index::Line(line as i32)];
            Self::render_row(row, &mut out);
        }

        // Trim trailing newline for cleaner inline snapshots.
        if out.ends_with('\n') {
            out.pop();
        }

        out
    }

    /// Render a single row as `|cells|` or `|cells+` (soft wrap).
    fn render_row(row: &super::row::Row, out: &mut String) {
        let cols = row.cols();
        out.push('|');
        for c in 0..cols {
            let cell = &row[Column(c)];
            if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                out.push('_');
            } else if cell.flags.contains(CellFlags::LEADING_WIDE_CHAR_SPACER) {
                out.push('^');
            } else {
                out.push(cell.ch);
            }
        }
        // Check WRAP flag on last cell.
        let has_wrap = cols > 0 && row[Column(cols - 1)].flags.contains(CellFlags::WRAP);
        if has_wrap {
            out.push('+');
        } else {
            out.push('|');
        }
        out.push('\n');
    }
}

#[cfg(test)]
impl Grid {
    /// Render a human-readable snapshot of the visible grid.
    pub(crate) fn snapshot(&self) -> String {
        GridSnapshot::new(self).render()
    }

    /// Render a snapshot including scrollback history.
    pub(crate) fn snapshot_with_scrollback(&self) -> String {
        GridSnapshot::new(self).with_scrollback().render()
    }
}
