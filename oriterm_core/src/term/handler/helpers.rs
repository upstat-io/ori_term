//! Helper functions for VTE handler dispatch.
//!
//! Mode number lookups, mode-to-flag mappings, cursor positioning helpers,
//! and version encoding used by the Handler impl and mode dispatch.

use std::cmp;

use vte::ansi::NamedPrivateMode;

use crate::event::EventListener;
use crate::index::Column;
use crate::term::{Term, TermMode};

/// DECRPM value: 1 = set, 2 = reset.
pub(super) fn mode_report_value(is_set: bool) -> u8 {
    if is_set { 1 } else { 2 }
}

/// Map `NamedPrivateMode` to its CSI mode number.
pub(super) fn named_private_mode_number(mode: NamedPrivateMode) -> u16 {
    match mode {
        NamedPrivateMode::CursorKeys => 1,
        NamedPrivateMode::ColumnMode => 3,
        NamedPrivateMode::Origin => 6,
        NamedPrivateMode::LineWrap => 7,
        NamedPrivateMode::BlinkingCursor => 12,
        NamedPrivateMode::ShowCursor => 25,
        NamedPrivateMode::ReportMouseClicks => 1000,
        NamedPrivateMode::ReportCellMouseMotion => 1002,
        NamedPrivateMode::ReportAllMouseMotion => 1003,
        NamedPrivateMode::ReportFocusInOut => 1004,
        NamedPrivateMode::Utf8Mouse => 1005,
        NamedPrivateMode::SgrMouse => 1006,
        NamedPrivateMode::AlternateScroll => 1007,
        NamedPrivateMode::UrgencyHints => 1042,
        NamedPrivateMode::SwapScreenAndSetRestoreCursor => 1049,
        NamedPrivateMode::BracketedPaste => 2004,
        NamedPrivateMode::SyncUpdate => 2026,
    }
}

/// Map `NamedPrivateMode` to the corresponding `TermMode` flag, if supported.
pub(super) fn named_private_mode_flag(mode: NamedPrivateMode) -> Option<TermMode> {
    match mode {
        NamedPrivateMode::CursorKeys => Some(TermMode::APP_CURSOR),
        NamedPrivateMode::Origin => Some(TermMode::ORIGIN),
        NamedPrivateMode::LineWrap => Some(TermMode::LINE_WRAP),
        NamedPrivateMode::BlinkingCursor => Some(TermMode::CURSOR_BLINKING),
        NamedPrivateMode::ShowCursor => Some(TermMode::SHOW_CURSOR),
        NamedPrivateMode::ReportMouseClicks => Some(TermMode::MOUSE_REPORT_CLICK),
        NamedPrivateMode::ReportCellMouseMotion => Some(TermMode::MOUSE_DRAG),
        NamedPrivateMode::ReportAllMouseMotion => Some(TermMode::MOUSE_MOTION),
        NamedPrivateMode::ReportFocusInOut => Some(TermMode::FOCUS_IN_OUT),
        NamedPrivateMode::Utf8Mouse => Some(TermMode::MOUSE_UTF8),
        NamedPrivateMode::SgrMouse => Some(TermMode::MOUSE_SGR),
        NamedPrivateMode::UrgencyHints => Some(TermMode::URGENCY_HINTS),
        NamedPrivateMode::SwapScreenAndSetRestoreCursor => Some(TermMode::ALT_SCREEN),
        NamedPrivateMode::BracketedPaste => Some(TermMode::BRACKETED_PASTE),
        NamedPrivateMode::SyncUpdate => Some(TermMode::SYNC_UPDATE),
        NamedPrivateMode::ColumnMode | NamedPrivateMode::AlternateScroll => None,
    }
}

/// Convert the crate version (semver) to a single integer for DA2 response.
///
/// `"0.1.3"` → `103`.
pub(super) fn crate_version_number() -> usize {
    let mut result = 0usize;
    let version = env!("CARGO_PKG_VERSION");
    // Strip any pre-release suffix (e.g. "-alpha.3").
    let version = version.split('-').next().unwrap_or(version);
    for (i, part) in version.split('.').rev().enumerate() {
        let n = part.parse::<usize>().unwrap_or(0);
        result += n * 100usize.pow(i as u32);
    }
    result
}

impl<T: EventListener> Term<T> {
    /// Origin-aware absolute cursor positioning.
    ///
    /// When ORIGIN mode is active, `line` is relative to the scroll region
    /// and clamped to it. Otherwise, `line` is relative to the screen top
    /// and clamped to the full viewport. Used by `Handler::goto`,
    /// `set_scrolling_region`, and DECSET/DECRST origin-mode toggling.
    pub(super) fn goto_origin_aware(&mut self, line: i32, col: usize) {
        let origin = self.mode.contains(TermMode::ORIGIN);
        let grid = self.grid_mut();
        let region_start = grid.scroll_region().start;
        let region_end = grid.scroll_region().end;

        let (offset, max_line) = if origin {
            (region_start, region_end.saturating_sub(1))
        } else {
            (0, grid.lines().saturating_sub(1))
        };

        let line = cmp::max(0, line) as usize;
        let line = cmp::min(line + offset, max_line);
        let col = Column(col.min(grid.cols().saturating_sub(1)));
        grid.move_to(line, col);
    }
}
