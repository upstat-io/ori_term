//! Mark mode — keyboard-driven cursor navigation and selection.
//!
//! Modeled after Windows Terminal's mark mode: Ctrl+Shift+M enters, arrow
//! keys move a visible cursor, Shift+arrows extend selection, Escape cancels.
//! Uses the existing 3-point [`Selection`] model for selection state.
//!
//! All grid queries operate on [`SnapshotGrid`] — no terminal lock required.
//! Mark cursor and selection state live on [`App`](super::App), not on `Pane`.

pub(crate) mod motion;

use winit::event::KeyEvent;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};

use oriterm_core::{Selection, SelectionMode, SelectionPoint, Side};
use oriterm_mux::MarkCursor;

use self::motion::{AbsCursor, GridBounds, WordContext};
use super::snapshot_grid::SnapshotGrid;

/// Result of processing a key event in mark mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarkAction {
    /// Key was handled (consumed by mark mode).
    ///
    /// `scroll_delta` is `Some(delta)` when the viewport needs to scroll
    /// to keep the mark cursor visible. The caller applies the scroll via
    /// `MuxBackend::scroll_display`.
    Handled { scroll_delta: Option<isize> },
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

/// Result of mark mode key dispatch.
///
/// Contains state mutations that the caller (App) applies.
pub(crate) struct MarkModeResult {
    /// The action to perform.
    pub action: MarkAction,
    /// Updated mark cursor position (if motion occurred).
    pub new_cursor: Option<MarkCursor>,
    /// Updated selection (if shift+motion or select-all occurred).
    pub new_selection: Option<SelectionUpdate>,
}

/// Selection state update from mark mode.
pub(crate) enum SelectionUpdate {
    /// Set or replace the selection.
    Set(Selection),
    /// Clear the selection.
    Clear,
}

/// Dispatch a key event while mark mode is active.
///
/// Pure function: reads grid state from `SnapshotGrid`, mark cursor and
/// selection from parameters. Returns a [`MarkModeResult`] describing
/// state mutations for the caller to apply.
#[expect(
    clippy::too_many_arguments,
    reason = "mark mode dispatch: grid, cursor, selection, event, mods, delimiters"
)]
pub(crate) fn handle_mark_mode_key(
    grid: &SnapshotGrid<'_>,
    cursor: MarkCursor,
    selection: Option<&Selection>,
    event: &KeyEvent,
    mods: ModifiersState,
    word_delimiters: &str,
) -> MarkModeResult {
    let noop = MarkModeResult {
        action: MarkAction::Ignored,
        new_cursor: None,
        new_selection: None,
    };

    // Ctrl+Shift+M toggles mark mode off.
    if mods.control_key()
        && mods.shift_key()
        && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyM))
    {
        return MarkModeResult {
            action: MarkAction::Exit { copy: false },
            new_cursor: None,
            new_selection: Some(SelectionUpdate::Clear),
        };
    }

    // Ctrl+A selects the entire buffer.
    if mods.control_key()
        && !mods.shift_key()
        && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyA))
    {
        let sel = select_all(grid);
        return MarkModeResult {
            action: MarkAction::Handled { scroll_delta: None },
            new_cursor: None,
            new_selection: Some(SelectionUpdate::Set(sel)),
        };
    }

    // Named keys: Escape, Enter, arrow/page/home/end navigation.
    if let Key::Named(named) = &event.logical_key {
        match named {
            NamedKey::Escape => {
                return MarkModeResult {
                    action: MarkAction::Exit { copy: false },
                    new_cursor: None,
                    new_selection: Some(SelectionUpdate::Clear),
                };
            }
            NamedKey::Enter => {
                return MarkModeResult {
                    action: MarkAction::Exit { copy: true },
                    new_cursor: None,
                    new_selection: None,
                };
            }
            _ => {
                if let Some(m) = resolve_motion(*named, mods) {
                    return apply_motion(
                        grid,
                        cursor,
                        selection,
                        m,
                        mods.shift_key(),
                        word_delimiters,
                    );
                }
            }
        }
    }

    noop
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
///
/// Returns a [`MarkModeResult`] with the new cursor, selection update,
/// and scroll delta needed to keep the mark cursor visible.
#[expect(
    clippy::too_many_arguments,
    reason = "motion apply: grid, cursor, selection, motion, shift, delimiters"
)]
fn apply_motion(
    grid: &SnapshotGrid<'_>,
    old_cursor: MarkCursor,
    selection: Option<&Selection>,
    m: Motion,
    shift: bool,
    word_delimiters: &str,
) -> MarkModeResult {
    let Some(abs_row) = grid.stable_to_absolute(old_cursor.row) else {
        return MarkModeResult {
            action: MarkAction::Handled { scroll_delta: None },
            new_cursor: None,
            new_selection: None,
        };
    };

    let bounds = GridBounds {
        total_rows: grid.total_rows(),
        cols: grid.cols(),
        visible_lines: grid.lines(),
    };
    let cur = AbsCursor {
        abs_row,
        col: old_cursor.col,
    };

    let word_ctx = matches!(m, Motion::WordLeft | Motion::WordRight)
        .then(|| extract_word_context(grid, abs_row, cur.col, word_delimiters));

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

    let new_cursor = MarkCursor {
        row: grid.absolute_to_stable(new_abs.abs_row),
        col: new_abs.col,
    };

    let sel_update = if shift {
        Some(SelectionUpdate::Set(extend_or_create_selection(
            selection,
            &old_cursor,
            &new_cursor,
        )))
    } else {
        Some(SelectionUpdate::Clear)
    };

    let scroll_delta = ensure_visible(grid, &new_cursor);

    MarkModeResult {
        action: MarkAction::Handled { scroll_delta },
        new_cursor: Some(new_cursor),
        new_selection: sel_update,
    }
}

/// Extract word boundary data from `SnapshotGrid` for pure word motions.
fn extract_word_context(
    grid: &SnapshotGrid<'_>,
    abs_row: usize,
    col: usize,
    word_delimiters: &str,
) -> WordContext {
    let first_visible = grid.first_visible_absolute();
    let last_visible = first_visible + grid.lines().saturating_sub(1);
    let cols = grid.cols();
    let total_rows = grid.total_rows();

    // Convert absolute row to viewport row for word_boundaries call.
    // If the row is outside the viewport, return a fallback context.
    let vp_row = if abs_row >= first_visible && abs_row <= last_visible {
        abs_row - first_visible
    } else {
        return WordContext {
            ws: col,
            we: col,
            prev_same_row_ws: None,
            prev_row_ws: None,
            next_same_row_we: None,
            next_row_we: None,
        };
    };

    let (ws, we) = grid.word_boundaries(vp_row, col, word_delimiters);

    let prev_same_row_ws = if ws > 0 {
        Some(grid.word_boundaries(vp_row, ws - 1, word_delimiters).0)
    } else {
        None
    };
    let prev_row_ws = if abs_row > 0 && abs_row > first_visible {
        let prev_vp = abs_row - 1 - first_visible;
        let end = cols.saturating_sub(1);
        Some(grid.word_boundaries(prev_vp, end, word_delimiters).0)
    } else {
        None
    };
    let next_same_row_we = if we + 1 < cols {
        Some(grid.word_boundaries(vp_row, we + 1, word_delimiters).1)
    } else {
        None
    };
    let next_row_we = if abs_row + 1 < total_rows && abs_row < last_visible {
        let next_vp = abs_row + 1 - first_visible;
        Some(grid.word_boundaries(next_vp, 0, word_delimiters).1)
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
    selection: Option<&Selection>,
    anchor_cursor: &MarkCursor,
    end_cursor: &MarkCursor,
) -> Selection {
    // Extract anchor from existing selection, or use the old cursor position.
    let (anchor_row, anchor_col) = selection.map_or((anchor_cursor.row, anchor_cursor.col), |s| {
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

    Selection {
        mode: SelectionMode::Char,
        anchor,
        pivot: anchor,
        end,
    }
}

/// Select the entire buffer (scrollback + visible).
///
/// Computes stable row indices from snapshot metadata without locking
/// the terminal.
pub(crate) fn select_all(grid: &SnapshotGrid<'_>) -> Selection {
    let first = grid.absolute_to_stable(0);
    let last_abs = grid.total_rows().saturating_sub(1);
    let last = grid.absolute_to_stable(last_abs);
    let cols = grid.cols();

    let anchor = SelectionPoint {
        row: first,
        col: 0,
        side: Side::Left,
    };
    let end = SelectionPoint {
        row: last,
        col: cols.saturating_sub(1),
        side: Side::Right,
    };

    Selection {
        mode: SelectionMode::Char,
        anchor,
        pivot: anchor,
        end,
    }
}

/// Compute the scroll delta to keep the mark cursor visible.
///
/// Returns `Some(delta)` if scrolling is needed, `None` if the cursor
/// is already within the viewport. The caller applies the scroll via
/// `MuxBackend::scroll_display`.
fn ensure_visible(grid: &SnapshotGrid<'_>, cursor: &MarkCursor) -> Option<isize> {
    let abs_row = grid.stable_to_absolute(cursor.row)?;
    let first_visible = grid.first_visible_absolute();
    let last_visible = first_visible + grid.lines().saturating_sub(1);

    if abs_row < first_visible {
        // Cursor above viewport — scroll up (into history).
        Some((first_visible - abs_row) as isize)
    } else if abs_row > last_visible {
        // Cursor below viewport — scroll down (toward live).
        Some(-((abs_row - last_visible) as isize))
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
