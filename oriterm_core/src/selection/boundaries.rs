//! Word and line boundary detection for selection.
//!
//! `word_boundaries` finds contiguous same-class character regions around
//! a click position. `logical_line_start`/`logical_line_end` walk the WRAP
//! flag chain to find the extent of soft-wrapped lines.

use crate::cell::CellFlags;
use crate::grid::Grid;
use crate::index::Column;

/// Character classification for word boundary detection.
///
/// Returns 0 for word characters (alphanumeric + `_`), 1 for whitespace
/// (space, null, tab), 2 for punctuation/other.
pub fn delimiter_class(c: char) -> u8 {
    if c.is_alphanumeric() || c == '_' {
        0
    } else if c == ' ' || c == '\0' || c == '\t' {
        1
    } else {
        2
    }
}

/// Returns true if the character is a word delimiter (not a word character).
pub fn is_word_delimiter(c: char) -> bool {
    delimiter_class(c) != 0
}

/// Internal alias used during boundary scanning.
fn char_class(c: char) -> u8 {
    delimiter_class(c)
}

/// Find word boundaries around (`abs_row`, `col`) in the grid.
///
/// Returns (`start_col`, `end_col`) inclusive. Wide-char spacers are
/// redirected to their base cell and skipped during scanning so that
/// double-clicking a CJK character selects the full character.
pub fn word_boundaries(grid: &Grid, abs_row: usize, col: usize) -> (usize, usize) {
    let row = match grid.absolute_row(abs_row) {
        Some(r) => r,
        None => return (col, col),
    };

    let cols = row.cols();
    if cols == 0 || col >= cols {
        return (col, col);
    }

    // If clicked on a wide-char spacer, redirect to the base cell.
    let click_col = if row[Column(col)].flags.contains(CellFlags::WIDE_CHAR_SPACER) && col > 0 {
        col - 1
    } else {
        col
    };

    let ch = row[Column(click_col)].ch;
    let class = char_class(ch);

    // Scan left, skipping wide-char spacers.
    let mut start = click_col;
    while start > 0 {
        let prev = start - 1;
        if row[Column(prev)]
            .flags
            .contains(CellFlags::WIDE_CHAR_SPACER)
            && prev > 0
        {
            // Spacer: check the base cell before it.
            if char_class(row[Column(prev - 1)].ch) == class {
                start = prev - 1;
            } else {
                break;
            }
        } else if char_class(row[Column(prev)].ch) == class {
            start = prev;
        } else {
            break;
        }
    }

    // Scan right, skipping wide-char spacers.
    let mut end = click_col;
    while end + 1 < cols {
        let next = end + 1;
        if row[Column(next)]
            .flags
            .contains(CellFlags::WIDE_CHAR_SPACER)
        {
            // Spacer belongs to the wide char at `end` — include it.
            end = next;
            continue;
        }
        if char_class(row[Column(next)].ch) == class {
            end = next;
        } else {
            break;
        }
    }

    (start, end)
}

/// Walk backwards to find the start of a logical (soft-wrapped) line.
///
/// Returns the absolute row index of the first row in the logical line.
pub fn logical_line_start(grid: &Grid, abs_row: usize) -> usize {
    let mut current = abs_row;
    while current > 0 {
        let prev = current - 1;
        let Some(row) = grid.absolute_row(prev) else {
            break;
        };
        // The WRAP flag on a row means it continues onto the next row.
        let last_col = row.cols().saturating_sub(1);
        if row[Column(last_col)].flags.contains(CellFlags::WRAP) {
            current = prev;
        } else {
            break;
        }
    }
    current
}

/// Walk forwards to find the end of a logical (soft-wrapped) line.
///
/// Returns the absolute row index of the last row in the logical line.
pub fn logical_line_end(grid: &Grid, abs_row: usize) -> usize {
    let mut current = abs_row;
    loop {
        let Some(row) = grid.absolute_row(current) else {
            break;
        };
        let last_col = row.cols().saturating_sub(1);
        if row[Column(last_col)].flags.contains(CellFlags::WRAP) {
            current += 1;
        } else {
            break;
        }
    }
    current
}
