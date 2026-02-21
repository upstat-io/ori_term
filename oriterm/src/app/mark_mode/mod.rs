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

use self::motion::{AbsCursor, GridBounds};
use crate::tab::{MarkCursor, Tab};

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
    tab: &mut Tab,
    event: &KeyEvent,
    mods: ModifiersState,
) -> MarkAction {
    // Ctrl+Shift+M toggles mark mode off.
    if mods.control_key()
        && mods.shift_key()
        && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyM))
    {
        tab.clear_selection();
        tab.exit_mark_mode();
        return MarkAction::Exit { copy: false };
    }

    // Ctrl+A selects the entire buffer.
    if mods.control_key()
        && !mods.shift_key()
        && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyA))
    {
        select_all(tab);
        return MarkAction::Handled;
    }

    // Named keys: Escape, Enter, arrow/page/home/end navigation.
    if let Key::Named(named) = &event.logical_key {
        match named {
            NamedKey::Escape => {
                tab.clear_selection();
                tab.exit_mark_mode();
                return MarkAction::Exit { copy: false };
            }
            NamedKey::Enter => {
                tab.exit_mark_mode();
                return MarkAction::Exit { copy: true };
            }
            _ => {
                if let Some(m) = resolve_motion(*named, mods) {
                    apply_motion(tab, m, mods.shift_key());
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
fn apply_motion(tab: &mut Tab, m: Motion, shift: bool) {
    let Some(old_cursor) = tab.mark_cursor() else {
        return;
    };

    // Compute the new cursor position under terminal lock.
    let new_cursor = {
        let term = tab.terminal().lock();
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
            Motion::WordLeft => word_left(g, abs_row, cur.col),
            Motion::WordRight => word_right(g, abs_row, cur.col, bounds.total_rows),
        };

        let stable = StableRowIndex::from_absolute(g, new_abs.abs_row);
        MarkCursor {
            row: stable,
            col: new_abs.col,
        }
    };

    // Update selection or clear it.
    if shift {
        extend_or_create_selection(tab, &old_cursor, &new_cursor);
    } else {
        tab.clear_selection();
    }

    tab.set_mark_cursor(new_cursor);
    ensure_visible(tab, &new_cursor);
}

/// Move cursor to the start of the current or previous word.
fn word_left(g: &Grid, abs_row: usize, col: usize) -> AbsCursor {
    let (ws, _) = word_boundaries(g, abs_row, col);
    if col > ws {
        AbsCursor { abs_row, col: ws }
    } else if ws > 0 {
        let (prev_ws, _) = word_boundaries(g, abs_row, ws - 1);
        AbsCursor {
            abs_row,
            col: prev_ws,
        }
    } else if abs_row > 0 {
        let prev_end = g.cols().saturating_sub(1);
        let (prev_ws, _) = word_boundaries(g, abs_row - 1, prev_end);
        AbsCursor {
            abs_row: abs_row - 1,
            col: prev_ws,
        }
    } else {
        AbsCursor { abs_row: 0, col: 0 }
    }
}

/// Move cursor to the end of the current or next word.
fn word_right(g: &Grid, abs_row: usize, col: usize, total_rows: usize) -> AbsCursor {
    let cols = g.cols();
    let (_, we) = word_boundaries(g, abs_row, col);
    if col < we {
        AbsCursor { abs_row, col: we }
    } else if we + 1 < cols {
        let (_, next_we) = word_boundaries(g, abs_row, we + 1);
        AbsCursor {
            abs_row,
            col: next_we,
        }
    } else if abs_row + 1 < total_rows {
        let (_, next_we) = word_boundaries(g, abs_row + 1, 0);
        AbsCursor {
            abs_row: abs_row + 1,
            col: next_we,
        }
    } else {
        AbsCursor {
            abs_row,
            col: cols.saturating_sub(1),
        }
    }
}

/// Extend an existing selection or create a new one toward `end_cursor`.
///
/// If a selection exists, updates its endpoint. Otherwise creates a new
/// char-mode selection anchored at `anchor_cursor`.
fn extend_or_create_selection(tab: &mut Tab, anchor_cursor: &MarkCursor, end_cursor: &MarkCursor) {
    // Extract anchor from existing selection, or use the old cursor position.
    let (anchor_row, anchor_col) = tab
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

    tab.set_selection(Selection {
        mode: SelectionMode::Char,
        anchor,
        pivot: anchor,
        end,
    });
}

/// Select the entire buffer (scrollback + visible).
fn select_all(tab: &mut Tab) {
    let (start_row, end_row, cols) = {
        let term = tab.terminal().lock();
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

    tab.set_selection(Selection {
        mode: SelectionMode::Char,
        anchor,
        pivot: anchor,
        end,
    });
}

/// Auto-scroll the viewport to keep the mark cursor visible.
fn ensure_visible(tab: &Tab, cursor: &MarkCursor) {
    let delta = {
        let term = tab.terminal().lock();
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
        tab.scroll_display(d);
    }
}

#[cfg(test)]
mod tests;
