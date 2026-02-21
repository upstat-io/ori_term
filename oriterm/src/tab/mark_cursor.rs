//! Mark cursor — keyboard-driven cursor for mark mode selection.
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
