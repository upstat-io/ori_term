//! Text extraction from grid selection.
//!
//! Converts a `Selection` range into a `String`, handling wide-char spacers,
//! combining marks (zero-width characters), soft-wrapped lines (WRAP flag),
//! and block/rectangular mode.

use crate::cell::CellFlags;
use crate::grid::Grid;
use crate::grid::Row;
use crate::index::Column;

use super::{Selection, SelectionMode};

/// Extract selected text from the grid.
///
/// Respects selection mode: linear selections follow effective column
/// boundaries and join soft-wrapped lines without newlines; block
/// selections extract rectangular regions with newlines between rows.
pub fn extract_text(grid: &Grid, selection: &Selection) -> String {
    let (start, end) = selection.ordered();
    let mut result = String::new();

    let Some(start_abs) = start.row.to_absolute(grid) else {
        return result;
    };
    let Some(end_abs) = end.row.to_absolute(grid) else {
        return result;
    };

    if selection.mode == SelectionMode::Block {
        let min_col = start.col.min(end.col);
        let max_col = start.col.max(end.col);

        for abs_row in start_abs..=end_abs {
            if let Some(row) = grid.absolute_row(abs_row) {
                let line = cells_to_text(row, min_col, max_col);
                result.push_str(line.trim_end());
            }
            if abs_row < end_abs {
                result.push('\n');
            }
        }
    } else {
        for abs_row in start_abs..=end_abs {
            if let Some(row) = grid.absolute_row(abs_row) {
                let row_start = if abs_row == start_abs {
                    start.effective_start_col()
                } else {
                    0
                };
                let row_end = if abs_row == end_abs {
                    end.effective_end_col()
                } else {
                    row.cols().saturating_sub(1)
                };

                let line = cells_to_text(row, row_start, row_end);

                // Soft-wrapped rows continue without newline or trim.
                let last_col = row.cols().saturating_sub(1);
                let is_wrapped =
                    row.cols() > 0 && row[Column(last_col)].flags.contains(CellFlags::WRAP);

                if is_wrapped && abs_row < end_abs {
                    result.push_str(&line);
                } else {
                    result.push_str(line.trim_end());
                    if abs_row < end_abs {
                        result.push('\n');
                    }
                }
            }
        }
    }

    result
}

/// Collect visible cell characters from `col_start..=col_end` into a string.
///
/// Skips wide-char spacers and replaces null chars with spaces.
/// Appends zero-width combining marks from `CellExtra`.
fn cells_to_text(row: &Row, col_start: usize, col_end: usize) -> String {
    let mut text = String::new();
    let last = col_end.min(row.cols().saturating_sub(1));
    for col in col_start..=last {
        let cell = &row[Column(col)];
        if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
            continue;
        }
        let c = if cell.ch == '\0' { ' ' } else { cell.ch };
        text.push(c);
        if let Some(extra) = &cell.extra {
            for &zw in &extra.zerowidth {
                text.push(zw);
            }
        }
    }
    text
}
