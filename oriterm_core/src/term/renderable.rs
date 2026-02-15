//! Renderable snapshot types for the GPU renderer.
//!
//! `RenderableContent` captures everything the renderer needs from a locked
//! `Term`: visible cells with resolved colors, cursor state, and damage info.
//! Extracted under lock, consumed without lock — no back-references into `Term`.

use vte::ansi::Color;

use crate::cell::CellFlags;
use crate::color::palette::dim_rgb;
use crate::color::{Palette, Rgb};
use crate::grid::cursor::CursorShape;
use crate::index::Column;
use crate::term::mode::TermMode;

/// A single cell ready for rendering.
///
/// Colors are fully resolved (palette lookups, bold-as-bright, dim,
/// inverse all applied). The renderer never needs the raw `Color` enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderableCell {
    /// Line index in the visible viewport (0 = top).
    pub line: usize,
    /// Column index (0-based).
    pub column: Column,
    /// Display character.
    pub ch: char,
    /// Resolved foreground color.
    pub fg: Rgb,
    /// Resolved background color.
    pub bg: Rgb,
    /// Cell attribute flags.
    pub flags: CellFlags,
    /// Resolved underline color (if custom underline color is set).
    pub underline_color: Option<Rgb>,
    /// Zero-width combining characters appended to this cell.
    pub zerowidth: Vec<char>,
}

/// Cursor state for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderableCursor {
    /// Cursor line in the visible viewport (0 = top).
    pub line: usize,
    /// Cursor column.
    pub column: Column,
    /// Cursor shape (block, underline, bar, etc.).
    pub shape: CursorShape,
    /// Whether the cursor is visible (DECTCEM and not scrolled back).
    pub visible: bool,
}

/// A damaged (changed) line region for incremental rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageLine {
    /// Visible line index (0 = top).
    pub line: usize,
    /// Leftmost changed column (inclusive).
    pub left: Column,
    /// Rightmost changed column (inclusive).
    pub right: Column,
}

/// Complete renderer snapshot extracted from `Term`.
///
/// Contains everything the GPU renderer needs for one frame. Extracted
/// under lock in one pass — the renderer works with this without holding
/// any lock on `Term`.
#[derive(Debug, Clone)]
pub struct RenderableContent {
    /// Visible cells with resolved colors, row-major order.
    pub cells: Vec<RenderableCell>,
    /// Cursor rendering state.
    pub cursor: RenderableCursor,
    /// How far scrolled back into history (0 = live view).
    pub display_offset: usize,
    /// Terminal mode flags snapshot.
    pub mode: TermMode,
    /// Whether the entire viewport needs redrawing.
    pub all_dirty: bool,
    /// Per-line damage (empty when `all_dirty` is true).
    pub damage: Vec<DamageLine>,
}

/// Resolve a cell's foreground color, applying bold-as-bright and dim.
///
/// When both BOLD and DIM are set, DIM takes priority — the base color is
/// dimmed without bright promotion. This matches Alacritty's behavior and
/// avoids the inconsistency where BOLD and DIM would cancel each other on
/// Named colors but stack on Indexed colors.
pub(super) fn resolve_fg(color: Color, flags: CellFlags, palette: &Palette) -> Rgb {
    let is_bold = flags.contains(CellFlags::BOLD);
    let is_dim = flags.contains(CellFlags::DIM);

    match color {
        Color::Spec(rgb) => {
            if is_dim { dim_rgb(rgb) } else { rgb }
        }
        Color::Indexed(idx) => {
            if is_dim {
                // DIM takes priority — dim the base color, no bright promotion.
                dim_rgb(palette.resolve(color))
            } else if is_bold && idx < 8 {
                // Bold-as-bright: promote ANSI 0–7 to 8–15.
                palette.resolve(Color::Indexed(idx + 8))
            } else {
                palette.resolve(color)
            }
        }
        Color::Named(name) => {
            if is_dim {
                // DIM takes priority over BOLD-as-bright.
                palette.resolve(Color::Named(name.to_dim()))
            } else if is_bold {
                palette.resolve(Color::Named(name.to_bright()))
            } else {
                palette.resolve(Color::Named(name))
            }
        }
    }
}

/// Resolve a cell's background color.
pub(super) fn resolve_bg(color: Color, palette: &Palette) -> Rgb {
    palette.resolve(color)
}

/// Apply inverse (swap fg/bg) when the INVERSE flag is set.
pub(super) fn apply_inverse(fg: Rgb, bg: Rgb, flags: CellFlags) -> (Rgb, Rgb) {
    if flags.contains(CellFlags::INVERSE) {
        (bg, fg)
    } else {
        (fg, bg)
    }
}

#[cfg(test)]
mod tests;
