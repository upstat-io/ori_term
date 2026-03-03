//! Wire-friendly types for pane state transfer.
//!
//! These are separate from internal types (`Cell`, `Palette`, `TermMode`) to
//! decouple the wire format from internal representation. Internal types may
//! use `Arc`, external crate types (`vte::ansi::Color`), or bitflags that
//! aren't directly serializable — wire types are flat and self-contained.

use serde::{Deserialize, Serialize};

use crate::id::{PaneId, TabId, WindowId};

/// RGB color on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

/// Terminal color on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireColor {
    /// Named color (0–15).
    Named(u8),
    /// Indexed color (0–255).
    Indexed(u8),
    /// 24-bit true color.
    Rgb(WireRgb),
}

/// Cell SGR flags as raw bits.
///
/// Maps 1:1 to `oriterm_core::CellFlags` bits. Using raw `u16` avoids
/// coupling the wire format to the bitflags type.
pub type WireCellFlags = u16;

/// A single terminal cell on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireCell {
    /// Displayed character.
    pub ch: char,
    /// Foreground color.
    pub fg: WireColor,
    /// Background color.
    pub bg: WireColor,
    /// SGR attribute flags (bold, italic, etc.).
    pub flags: WireCellFlags,
    /// Combining marks / zero-width characters.
    pub zerowidth: Vec<char>,
}

/// Cursor shape on the wire.
///
/// Stable `#[repr(u8)]` encoding decoupled from `oriterm_core::CursorShape`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum WireCursorShape {
    /// Filled block cursor.
    Block = 0,
    /// Underline cursor.
    Underline = 1,
    /// Vertical bar cursor.
    Bar = 2,
    /// Hollow (outline) block cursor.
    HollowBlock = 3,
    /// Cursor hidden.
    Hidden = 4,
}

/// Cursor state on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireCursor {
    /// Column (0-indexed).
    pub col: u16,
    /// Row (0-indexed, within viewport).
    pub row: u16,
    /// Cursor shape.
    pub shape: WireCursorShape,
    /// Whether the cursor is visible.
    pub visible: bool,
}

/// Full snapshot of a pane's visible state.
///
/// Transferred when a client subscribes to a pane or explicitly requests
/// a snapshot. Contains everything needed to render the pane from scratch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneSnapshot {
    /// Visible grid contents (rows × cols).
    pub cells: Vec<Vec<WireCell>>,
    /// Cursor position and shape.
    pub cursor: WireCursor,
    /// Color palette as 270 RGB triplets.
    pub palette: Vec<[u8; 3]>,
    /// Pane title (from OSC 0/2).
    pub title: String,
    /// Terminal mode flags as raw bits.
    ///
    /// # Wire format
    ///
    /// | Bit | Mode |
    /// |-----|------|
    /// | 0   | `SHOW_CURSOR` (DECTCEM) |
    /// | 1   | `APP_CURSOR` (DECCKM) |
    /// | 2   | `APP_KEYPAD` (DECKPAM/DECKPNM) |
    /// | 3   | `MOUSE_REPORT_CLICK` (mode 1000) |
    /// | 4   | `MOUSE_DRAG` (mode 1002) |
    /// | 5   | `MOUSE_MOTION` (mode 1003) |
    /// | 6   | `MOUSE_SGR` (mode 1006) |
    /// | 7   | `MOUSE_UTF8` (mode 1005) |
    /// | 8   | `ALT_SCREEN` (mode 1049) |
    /// | 9   | `LINE_WRAP` (DECAWM) |
    /// | 10  | `ORIGIN` (DECOM) |
    /// | 11  | `INSERT` (IRM) |
    /// | 12  | `FOCUS_IN_OUT` (mode 1004) |
    /// | 13  | `BRACKETED_PASTE` (mode 2004) |
    /// | 14  | `SYNC_UPDATE` (mode 2026) |
    /// | 15  | `URGENCY_HINTS` (mode 1042) |
    /// | 16  | `CURSOR_BLINKING` (ATT610) |
    /// | 17  | `LINE_FEED_NEW_LINE` (LNM) |
    /// | 18  | `DISAMBIGUATE_ESC_CODES` (Kitty) |
    /// | 19  | `REPORT_EVENT_TYPES` (Kitty) |
    /// | 20  | `REPORT_ALTERNATE_KEYS` (Kitty) |
    /// | 21  | `REPORT_ALL_KEYS_AS_ESC` (Kitty) |
    /// | 22  | `REPORT_ASSOCIATED_TEXT` (Kitty) |
    /// | 23  | `ALTERNATE_SCROLL` (mode 1007) |
    /// | 24  | `REVERSE_WRAP` (mode 45) |
    /// | 25  | `MOUSE_URXVT` (mode 1015) |
    /// | 26  | `MOUSE_X10` (mode 9) |
    pub modes: u32,
    /// Number of scrollback rows above the viewport.
    pub scrollback_len: u32,
    /// Current scroll position (0 = bottom, `scrollback_len` = top).
    pub display_offset: u32,
}

/// Summary info for a mux window (used in `ListWindows` response).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MuxWindowInfo {
    /// Window identity.
    pub window_id: WindowId,
    /// Number of tabs in the window.
    pub tab_count: u32,
    /// Currently active tab (`None` for empty windows).
    pub active_tab_id: Option<TabId>,
}

/// Summary info for a mux tab (used in `ListTabs` response).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MuxTabInfo {
    /// Tab identity.
    pub tab_id: TabId,
    /// Currently focused pane.
    pub active_pane_id: PaneId,
    /// Number of panes in the tab.
    pub pane_count: u32,
    /// Tab title (derived from the active pane's title).
    pub title: String,
}
