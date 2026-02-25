//! Row text extraction for search and URL detection.
//!
//! Extracts text from grid rows while maintaining a column map that
//! enables mapping byte positions in the extracted text back to grid
//! column indices.

use crate::cell::CellFlags;
use crate::grid::Row;
use crate::index::Column;

/// Extract text content and column map from a grid row.
///
/// Iterates cells, skipping wide-char spacers and replacing null chars
/// with spaces. Appends zero-width combining marks from `CellExtra`.
///
/// Returns `(text, col_map)` where `col_map[char_index]` gives the grid
/// column that produced that character. Zero-width chars share their
/// base character's column.
pub fn extract_row_text(row: &Row) -> (String, Vec<usize>) {
    let mut text = String::new();
    let mut col_map = Vec::new();

    for col in 0..row.cols() {
        let cell = &row[Column(col)];
        if cell
            .flags
            .intersects(CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }
        let c = if cell.ch == '\0' { ' ' } else { cell.ch };
        text.push(c);
        col_map.push(col);

        if let Some(extra) = &cell.extra {
            for &zw in &extra.zerowidth {
                text.push(zw);
                col_map.push(col);
            }
        }
    }

    (text, col_map)
}

/// Convert a byte span in extracted text to inclusive grid column range.
///
/// `byte_start` and `byte_end` are byte offsets into the text returned by
/// `extract_row_text`. Returns `Some((start_col, end_col))` with inclusive
/// column bounds, or `None` if the span is empty or out of range.
pub(crate) fn byte_span_to_cols(
    text: &str,
    col_map: &[usize],
    byte_start: usize,
    byte_end: usize,
) -> Option<(usize, usize)> {
    if byte_start >= byte_end || col_map.is_empty() {
        return None;
    }

    let start_char = char_index_at_byte(text, byte_start)?;
    let end_char = char_index_containing_byte(text, byte_end.saturating_sub(1))?;

    let start_col = *col_map.get(start_char)?;
    let end_col = *col_map.get(end_char)?;
    Some((start_col, end_col))
}

/// Find the char index of the first character starting at or after `byte_offset`.
fn char_index_at_byte(text: &str, byte_offset: usize) -> Option<usize> {
    if byte_offset >= text.len() {
        return None;
    }
    Some(
        text.char_indices()
            .position(|(i, _)| i >= byte_offset)
            .unwrap_or(0),
    )
}

/// Find the char index of the character whose encoding contains `byte_offset`.
fn char_index_containing_byte(text: &str, byte_offset: usize) -> Option<usize> {
    if text.is_empty() {
        return None;
    }
    let clamped = byte_offset.min(text.len().saturating_sub(1));
    // Walk forward, tracking the last char that starts at or before clamped.
    let mut result = None;
    for (idx, (i, _)) in text.char_indices().enumerate() {
        if i <= clamped {
            result = Some(idx);
        } else {
            break;
        }
    }
    result
}
