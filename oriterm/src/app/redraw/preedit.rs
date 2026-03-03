//! IME preedit overlay for the terminal grid.

use unicode_width::UnicodeWidthChar;

use oriterm_core::{CellFlags, RenderableContent};

/// Overlay IME preedit characters into the renderable content at the cursor.
///
/// Replaces cells at the cursor position with preedit characters, adding
/// [`CellFlags::UNDERLINE`] to visually distinguish composition text from
/// committed text. Wide (CJK) characters occupy two cells; the spacer cell
/// gets [`CellFlags::WIDE_CHAR_SPACER`]. Characters beyond the grid width
/// are clipped.
///
/// The content's cursor visibility is set to `false` so the prepare phase
/// does not emit a cursor on top of the preedit text.
pub(in crate::app) fn overlay_preedit_cells(
    preedit: &str,
    content: &mut RenderableContent,
    cols: usize,
) {
    if content.cells.is_empty() || cols == 0 {
        return;
    }

    let line = content.cursor.line;
    let start_col = content.cursor.column.0;

    // Hide the terminal cursor while preedit is active.
    content.cursor.visible = false;

    let mut col = start_col;
    for ch in preedit.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w == 0 {
            continue;
        }
        if col >= cols {
            break;
        }

        let idx = line * cols + col;
        if idx >= content.cells.len() {
            break;
        }

        // Preserve the cell's colors but replace character and add underline.
        let cell = &mut content.cells[idx];
        cell.ch = ch;
        cell.flags = (cell.flags
            - CellFlags::WIDE_CHAR
            - CellFlags::WIDE_CHAR_SPACER
            - CellFlags::LEADING_WIDE_CHAR_SPACER)
            | CellFlags::UNDERLINE;
        cell.zerowidth.clear();

        if w == 2 {
            cell.flags |= CellFlags::WIDE_CHAR;
            // Mark the next cell as a spacer for the wide character.
            if col + 1 < cols {
                let spacer_idx = idx + 1;
                if spacer_idx < content.cells.len() {
                    let spacer = &mut content.cells[spacer_idx];
                    spacer.ch = ' ';
                    spacer.flags = CellFlags::WIDE_CHAR_SPACER;
                    spacer.zerowidth.clear();
                }
            }
        }

        col += w;
    }
}
