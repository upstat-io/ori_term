//! Terminal cell types.
//!
//! A `Cell` represents one character position in the terminal grid. Most cells
//! are 24 bytes on the stack. Only cells with combining marks, colored
//! underlines, or hyperlinks allocate heap storage via `CellExtra`. Extra data
//! is stored behind `Arc` so cloning a cell (e.g. from cursor template) is O(1).

use std::fmt;
use std::sync::Arc;

use bitflags::bitflags;
use unicode_width::UnicodeWidthChar;
use vte::ansi::Color;

bitflags! {
    /// Per-cell attribute flags (SGR and internal).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CellFlags: u16 {
        const BOLD              = 1 << 0;
        const DIM               = 1 << 1;
        const ITALIC            = 1 << 2;
        const UNDERLINE         = 1 << 3;
        const BLINK             = 1 << 4;
        const INVERSE           = 1 << 5;
        const HIDDEN            = 1 << 6;
        const STRIKETHROUGH     = 1 << 7;
        const WIDE_CHAR                 = 1 << 8;
        const WIDE_CHAR_SPACER          = 1 << 9;
        const WRAP                      = 1 << 10;
        /// Padding cell before a wide char that wrapped to the next line.
        ///
        /// Inserted at `cols - 1` when a wide char can't fit and wraps.
        /// Skipped during text extraction, selection, search, and reflow
        /// to avoid spurious spaces.
        const LEADING_WIDE_CHAR_SPACER  = 1 << 15;
        const CURLY_UNDERLINE   = 1 << 11;
        const DOTTED_UNDERLINE  = 1 << 12;
        const DASHED_UNDERLINE  = 1 << 13;
        const DOUBLE_UNDERLINE  = 1 << 14;

        /// Union of all underline variants for mutual exclusion.
        const ALL_UNDERLINES = Self::UNDERLINE.bits()
            | Self::DOUBLE_UNDERLINE.bits()
            | Self::CURLY_UNDERLINE.bits()
            | Self::DOTTED_UNDERLINE.bits()
            | Self::DASHED_UNDERLINE.bits();
    }
}

impl Default for CellFlags {
    fn default() -> Self {
        Self::empty()
    }
}

/// Heap-allocated optional data for cells that need it.
///
/// Only allocated when a cell has combining marks, a colored underline,
/// or a hyperlink. Normal cells keep `extra: None` (zero overhead).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellExtra {
    /// Colored underline (SGR 58).
    pub underline_color: Option<Color>,
    /// OSC 8 hyperlink.
    pub hyperlink: Option<Hyperlink>,
    /// Combining marks and zero-width characters appended to this cell.
    pub zerowidth: Vec<char>,
}

impl CellExtra {
    /// Create an empty extra with all fields at their defaults.
    pub fn new() -> Self {
        Self {
            underline_color: None,
            hyperlink: None,
            zerowidth: Vec::new(),
        }
    }
}

impl Default for CellExtra {
    fn default() -> Self {
        Self::new()
    }
}

/// OSC 8 hyperlink data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hyperlink {
    /// Optional link id for grouping.
    pub id: Option<String>,
    /// The URI target.
    pub uri: String,
}

impl fmt::Display for Hyperlink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.uri)
    }
}

impl From<vte::ansi::Hyperlink> for Hyperlink {
    fn from(h: vte::ansi::Hyperlink) -> Self {
        Self {
            id: h.id,
            uri: h.uri,
        }
    }
}

/// One character position in the terminal grid.
///
/// Target size: 24 bytes. Fields are ordered to minimize padding:
/// `char(4) + Color(4) + Color(4) + CellFlags(2) + pad(2) + Option<Arc>(8)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The character stored in this cell.
    pub ch: char,
    /// Foreground color (deferred palette resolution).
    pub fg: Color,
    /// Background color (deferred palette resolution).
    pub bg: Color,
    /// SGR attribute flags.
    pub flags: CellFlags,
    /// Optional heap data for combining marks, underline color, or hyperlinks.
    ///
    /// Uses `Arc` so that cloning a cell with extra data (e.g. propagating
    /// cursor template attributes) is O(1) — a refcount bump instead of a
    /// heap allocation.
    pub extra: Option<Arc<CellExtra>>,
}

const _: () = assert!(size_of::<Cell>() <= 24);

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::Named(vte::ansi::NamedColor::Foreground),
            bg: Color::Named(vte::ansi::NamedColor::Background),
            flags: CellFlags::empty(),
            extra: None,
        }
    }
}

impl From<Color> for Cell {
    /// BCE (Background Color Erase) cell: default cell with a custom background.
    fn from(bg: Color) -> Self {
        Self {
            bg,
            ..Self::default()
        }
    }
}

impl Cell {
    /// Reset this cell to match the given template.
    pub fn reset(&mut self, template: &Self) {
        self.ch = template.ch;
        self.fg = template.fg;
        self.bg = template.bg;
        self.flags = template.flags;
        self.extra.clone_from(&template.extra);
    }

    /// Returns `true` if this cell is visually empty (space, default colors, no flags).
    pub fn is_empty(&self) -> bool {
        self.ch == ' '
            && self.fg == Color::Named(vte::ansi::NamedColor::Foreground)
            && self.bg == Color::Named(vte::ansi::NamedColor::Background)
            && self.flags.is_empty()
            && self.extra.is_none()
    }

    /// Display width of this cell's character.
    ///
    /// Respects the `WIDE_CHAR` flag and falls back to `unicode-width`.
    pub fn width(&self) -> usize {
        if self.flags.contains(CellFlags::WIDE_CHAR) {
            return 2;
        }
        if self
            .flags
            .intersects(CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER)
        {
            return 0;
        }
        UnicodeWidthChar::width(self.ch).unwrap_or(1)
    }

    /// Set or clear the underline color (SGR 58/59).
    ///
    /// `Some(color)` allocates `CellExtra` if needed. `None` clears the
    /// underline color and drops `CellExtra` if it becomes empty.
    pub fn set_underline_color(&mut self, color: Option<Color>) {
        match color {
            Some(c) => {
                let extra = self.extra.get_or_insert_with(Default::default);
                Arc::make_mut(extra).underline_color = Some(c);
            }
            None => {
                if let Some(extra) = &mut self.extra {
                    Arc::make_mut(extra).underline_color = None;
                    if extra.zerowidth.is_empty() && extra.hyperlink.is_none() {
                        self.extra = None;
                    }
                }
            }
        }
    }

    /// Set or clear the hyperlink (OSC 8).
    ///
    /// `Some(link)` allocates `CellExtra` if needed. `None` clears the
    /// hyperlink and drops `CellExtra` if it becomes empty.
    pub fn set_hyperlink(&mut self, link: Option<Hyperlink>) {
        match link {
            Some(l) => {
                let extra = self.extra.get_or_insert_with(Default::default);
                Arc::make_mut(extra).hyperlink = Some(l);
            }
            None => {
                if let Some(extra) = &mut self.extra {
                    Arc::make_mut(extra).hyperlink = None;
                    if extra.zerowidth.is_empty() && extra.underline_color.is_none() {
                        self.extra = None;
                    }
                }
            }
        }
    }

    /// Returns the hyperlink attached to this cell, if any.
    pub fn hyperlink(&self) -> Option<&Hyperlink> {
        self.extra.as_ref().and_then(|e| e.hyperlink.as_ref())
    }

    /// Append a combining mark (zero-width character) to this cell.
    ///
    /// Lazily allocates `CellExtra` on first combining mark. Uses
    /// `Arc::make_mut` for copy-on-write when the extra is shared.
    pub fn push_zerowidth(&mut self, ch: char) {
        let extra = self.extra.get_or_insert_with(Default::default);
        Arc::make_mut(extra).zerowidth.push(ch);
    }
}

#[cfg(test)]
mod tests;
