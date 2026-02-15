//! Terminal mode flags (DECSET/DECRST, SM/RM).
//!
//! Each flag corresponds to a terminal mode set/reset via escape sequences.
//! The default mode has `SHOW_CURSOR` and `LINE_WRAP` enabled.

use bitflags::bitflags;

bitflags! {
    /// Bitflags for terminal mode state.
    ///
    /// Modes are toggled by DECSET (`CSI ? n h`), DECRST (`CSI ? n l`),
    /// SM (`CSI n h`), and RM (`CSI n l`) escape sequences.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct TermMode: u32 {
        /// DECTCEM — cursor visible.
        const SHOW_CURSOR        = 1;
        /// DECCKM — application cursor keys.
        const APP_CURSOR         = 1 << 1;
        /// DECKPAM/DECKPNM — application keypad mode.
        const APP_KEYPAD         = 1 << 2;
        /// Mode 1000 — report mouse clicks.
        const MOUSE_REPORT_CLICK = 1 << 3;
        /// Mode 1002 — report mouse button + drag.
        const MOUSE_DRAG         = 1 << 4;
        /// Mode 1003 — report all mouse motion.
        const MOUSE_MOTION       = 1 << 5;
        /// Mode 1006 — SGR mouse encoding.
        const MOUSE_SGR          = 1 << 6;
        /// Mode 1005 — UTF-8 mouse encoding.
        const MOUSE_UTF8         = 1 << 7;
        /// Mode 1049 — alternate screen buffer.
        const ALT_SCREEN         = 1 << 8;
        /// DECAWM — auto-wrap at end of line.
        const LINE_WRAP          = 1 << 9;
        /// DECOM — origin mode (cursor relative to scroll region).
        const ORIGIN             = 1 << 10;
        /// IRM — insert mode.
        const INSERT             = 1 << 11;
        /// Mode 1004 — report focus in/out events.
        const FOCUS_IN_OUT       = 1 << 12;
        /// Mode 2004 — bracketed paste mode.
        const BRACKETED_PASTE    = 1 << 13;
        /// Mode 2026 — synchronized output.
        const SYNC_UPDATE        = 1 << 14;
        /// Mode 1042 — urgency hints on bell.
        const URGENCY_HINTS      = 1 << 15;
        /// Progressive keyboard enhancement (kitty protocol).
        const KITTY_KEYBOARD     = 1 << 16;
        /// ATT610 — cursor blinking.
        const CURSOR_BLINKING    = 1 << 17;
        /// LNM — linefeed/new line mode (LF acts as CR+LF).
        const LINE_FEED_NEW_LINE = 1 << 18;
        /// Computed: any mouse reporting mode is active.
        const ANY_MOUSE = Self::MOUSE_REPORT_CLICK.bits()
                        | Self::MOUSE_DRAG.bits()
                        | Self::MOUSE_MOTION.bits();
    }
}

impl Default for TermMode {
    fn default() -> Self {
        Self::SHOW_CURSOR | Self::LINE_WRAP
    }
}

#[cfg(test)]
mod tests;
