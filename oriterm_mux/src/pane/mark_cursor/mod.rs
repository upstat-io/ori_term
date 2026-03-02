//! Mark cursor — keyboard-driven cursor for mark mode selection.
//!
//! Lives in `pane` because mark mode is per-pane state. Re-exported from
//! `tab` for backward compatibility during the mux transition.
//!
//! The mark cursor uses [`StableRowIndex`] for its row coordinate so it
//! survives scrollback eviction between key presses. Column is zero-based.

use oriterm_core::grid::StableRowIndex;

/// A cursor position for keyboard-driven (mark mode) navigation.
///
/// Uses stable row identity to survive scrollback eviction. Convert to
/// absolute row index under terminal lock for arithmetic, then back to
/// `StableRowIndex` before storing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkCursor {
    /// Stable row identity (survives scrollback eviction).
    pub row: StableRowIndex,
    /// Column (0-based).
    pub col: usize,
}

impl MarkCursor {
    /// Convert to viewport coordinates given the viewport's stable row base.
    ///
    /// Returns `(viewport_line, col)` if the cursor is within the viewport,
    /// `None` if it has scrolled off-screen.
    pub fn to_viewport(self, stable_row_base: u64, max_lines: usize) -> Option<(usize, usize)> {
        let offset = self.row.0.checked_sub(stable_row_base)?;
        let line = offset as usize;
        if line < max_lines {
            Some((line, self.col))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests;
