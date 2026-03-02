//! Mark mode — keyboard-driven cursor navigation and selection.
//!
//! Modeled after Windows Terminal's mark mode: Ctrl+Shift+M enters, arrow
//! keys move a visible cursor, Shift+arrows extend selection, Escape cancels.
//! Uses the existing 3-point [`Selection`] model for selection state.

pub(crate) mod motion;

use winit::event::KeyEvent;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};

use oriterm_core::grid::Grid;
use oriterm_core::selection::word_boundaries;
use oriterm_core::{Selection, SelectionMode, SelectionPoint, Side, StableRowIndex};

use self::motion::{AbsCursor, GridBounds, WordContext};
use oriterm_mux::pane::{MarkCursor, Pane};

/// Result of processing a key event in mark mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarkAction {
    /// Key was handled (consumed by mark mode).
    Handled,
    /// Key was not recognized by mark mode (fall through).
    Ignored,
    /// Exit mark mode. `copy` indicates whether to copy the selection.
    Exit {
        /// Whether to copy the selection to the clipboard on exit.
        copy: bool,
    },
}

/// Cursor motion direction.
#[derive(Clone, Copy)]
enum Motion {
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    LineStart,
    LineEnd,
    BufferStart,
    BufferEnd,
    WordLeft,
    WordRight,
}

/// Dispatch a key event while mark mode is active.
///
/// Returns [`MarkAction::Handled`] for consumed keys, [`MarkAction::Exit`]
/// when leaving mark mode, or [`MarkAction::Ignored`] for unrecognized keys.
pub(crate) fn handle_mark_mode_key(
    pane: &mut Pane,
    event: &KeyEvent,
    mods: ModifiersState,
    word_delimiters: &str,
) -> MarkAction {
    // Ctrl+Shift+M toggles mark mode off.
    if mods.control_key()
        && mods.shift_key()
        && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyM))
    {
        pane.clear_selection();
        pane.exit_mark_mode();
        return MarkAction::Exit { copy: false };
    }

    // Ctrl+A selects the entire buffer.
    if mods.control_key()
        && !mods.shift_key()
        && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyA))
    {
        select_all(pane);
        return MarkAction::Handled;
    }

    // Named keys: Escape, Enter, arrow/page/home/end navigation.
    if let Key::Named(named) = &event.logical_key {
        match named {
            NamedKey::Escape => {
                pane.clear_selection();
                pane.exit_mark_mode();
                return MarkAction::Exit { copy: false };
            }
            NamedKey::Enter => {
                pane.exit_mark_mode();
                return MarkAction::Exit { copy: true };
            }
            _ => {
                if let Some(m) = resolve_motion(*named, mods) {
                    apply_motion(pane, m, mods.shift_key(), word_delimiters);
                    return MarkAction::Handled;
                }
            }
        }
    }

    MarkAction::Ignored
}

/// Map a named key + modifiers to a cursor motion.
fn resolve_motion(named: NamedKey, mods: ModifiersState) -> Option<Motion> {
    let ctrl = mods.control_key();
    match named {
        NamedKey::ArrowLeft => Some(if ctrl { Motion::WordLeft } else { Motion::Left }),
        NamedKey::ArrowRight => Some(if ctrl {
            Motion::WordRight
        } else {
            Motion::Right
        }),
        NamedKey::ArrowUp => Some(Motion::Up),
        NamedKey::ArrowDown => Some(Motion::Down),
        NamedKey::PageUp => Some(Motion::PageUp),
        NamedKey::PageDown => Some(Motion::PageDown),
        NamedKey::Home => Some(if ctrl {
            Motion::BufferStart
        } else {
            Motion::LineStart
        }),
        NamedKey::End => Some(if ctrl {
            Motion::BufferEnd
        } else {
            Motion::LineEnd
        }),
        _ => None,
    }
}

/// Apply a cursor motion, optionally extending the selection.
fn apply_motion(pane: &mut Pane, m: Motion, shift: bool, word_delimiters: &str) {
    let Some(old_cursor) = pane.mark_cursor() else {
        return;
    };

    // Compute the new cursor position under terminal lock.
    let new_cursor = {
        let term = pane.terminal().lock();
        let g = term.grid();
        let Some(abs_row) = old_cursor.row.to_absolute(g) else {
            return;
        };
        let bounds = GridBounds {
            total_rows: g.scrollback().len() + g.lines(),
            cols: g.cols(),
            visible_lines: g.lines(),
        };
        let cur = AbsCursor {
            abs_row,
            col: old_cursor.col,
        };

        let word_ctx = matches!(m, Motion::WordLeft | Motion::WordRight)
            .then(|| extract_word_context(g, abs_row, cur.col, word_delimiters));

        let new_abs = match m {
            Motion::Left => motion::move_left(cur, bounds),
            Motion::Right => motion::move_right(cur, bounds),
            Motion::Up => motion::move_up(cur),
            Motion::Down => motion::move_down(cur, bounds),
            Motion::PageUp => motion::page_up(cur, bounds),
            Motion::PageDown => motion::page_down(cur, bounds),
            Motion::LineStart => motion::line_start(cur),
            Motion::LineEnd => motion::line_end(cur, bounds),
            Motion::BufferStart => motion::buffer_start(),
            Motion::BufferEnd => motion::buffer_end(bounds),
            Motion::WordLeft => motion::word_left(cur, word_ctx.as_ref().expect("computed above")),
            Motion::WordRight => {
                motion::word_right(cur, word_ctx.as_ref().expect("computed above"), bounds)
            }
        };

        let stable = StableRowIndex::from_absolute(g, new_abs.abs_row);
        MarkCursor {
            row: stable,
            col: new_abs.col,
        }
    };

    // Update selection or clear it.
    if shift {
        extend_or_create_selection(pane, &old_cursor, &new_cursor);
    } else {
        pane.clear_selection();
    }

    pane.set_mark_cursor(new_cursor);
    ensure_visible(pane, &new_cursor);
}

/// Extract word boundary data under terminal lock for pure word motions.
fn extract_word_context(
    g: &Grid,
    abs_row: usize,
    col: usize,
    word_delimiters: &str,
) -> WordContext {
    let (ws, we) = word_boundaries(g, abs_row, col, word_delimiters);
    let cols = g.cols();
    let total_rows = g.scrollback().len() + g.lines();

    let prev_same_row_ws = if ws > 0 {
        Some(word_boundaries(g, abs_row, ws - 1, word_delimiters).0)
    } else {
        None
    };
    let prev_row_ws = if abs_row > 0 {
        let end = cols.saturating_sub(1);
        Some(word_boundaries(g, abs_row - 1, end, word_delimiters).0)
    } else {
        None
    };
    let next_same_row_we = if we + 1 < cols {
        Some(word_boundaries(g, abs_row, we + 1, word_delimiters).1)
    } else {
        None
    };
    let next_row_we = if abs_row + 1 < total_rows {
        Some(word_boundaries(g, abs_row + 1, 0, word_delimiters).1)
    } else {
        None
    };

    WordContext {
        ws,
        we,
        prev_same_row_ws,
        prev_row_ws,
        next_same_row_we,
        next_row_we,
    }
}

/// Extend an existing selection or create a new one toward `end_cursor`.
///
/// If a selection exists, updates its endpoint. Otherwise creates a new
/// char-mode selection anchored at `anchor_cursor`.
fn extend_or_create_selection(
    pane: &mut Pane,
    anchor_cursor: &MarkCursor,
    end_cursor: &MarkCursor,
) {
    // Extract anchor from existing selection, or use the old cursor position.
    let (anchor_row, anchor_col) = pane
        .selection()
        .map_or((anchor_cursor.row, anchor_cursor.col), |s| {
            (s.anchor.row, s.anchor.col)
        });

    // Determine direction to set Side correctly for containment.
    let anchor_pos = (anchor_row, anchor_col);
    let end_pos = (end_cursor.row, end_cursor.col);

    let (anchor_side, end_side) = match end_pos.cmp(&anchor_pos) {
        std::cmp::Ordering::Greater => (Side::Left, Side::Right),
        std::cmp::Ordering::Less => (Side::Right, Side::Left),
        std::cmp::Ordering::Equal => (Side::Left, Side::Left),
    };

    let anchor = SelectionPoint {
        row: anchor_row,
        col: anchor_col,
        side: anchor_side,
    };
    let end = SelectionPoint {
        row: end_cursor.row,
        col: end_cursor.col,
        side: end_side,
    };

    pane.set_selection(Selection {
        mode: SelectionMode::Char,
        anchor,
        pivot: anchor,
        end,
    });
}

/// Select the entire buffer (scrollback + visible).
pub(super) fn select_all(pane: &mut Pane) {
    let (start_row, end_row, cols) = {
        let term = pane.terminal().lock();
        let g = term.grid();
        let first = StableRowIndex::from_absolute(g, 0);
        let last_abs = g.scrollback().len() + g.lines().saturating_sub(1);
        let last = StableRowIndex::from_absolute(g, last_abs);
        (first, last, g.cols())
    };

    let anchor = SelectionPoint {
        row: start_row,
        col: 0,
        side: Side::Left,
    };
    let end = SelectionPoint {
        row: end_row,
        col: cols.saturating_sub(1),
        side: Side::Right,
    };

    pane.set_selection(Selection {
        mode: SelectionMode::Char,
        anchor,
        pivot: anchor,
        end,
    });
}

/// Auto-scroll the viewport to keep the mark cursor visible.
fn ensure_visible(pane: &Pane, cursor: &MarkCursor) {
    let delta = {
        let term = pane.terminal().lock();
        let g = term.grid();
        let Some(abs_row) = cursor.row.to_absolute(g) else {
            return;
        };
        let sb_len = g.scrollback().len();
        let offset = g.display_offset();
        let lines = g.lines();
        let first_visible = sb_len.saturating_sub(offset);
        let last_visible = first_visible + lines.saturating_sub(1);

        if abs_row < first_visible {
            // Cursor above viewport — scroll up (into history).
            Some((first_visible - abs_row) as isize)
        } else if abs_row > last_visible {
            // Cursor below viewport — scroll down (toward live).
            Some(-((abs_row - last_visible) as isize))
        } else {
            None
        }
    };

    if let Some(d) = delta {
        pane.scroll_display(d);
    }
}

#[cfg(test)]
mod tests;
