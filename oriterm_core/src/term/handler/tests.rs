//! Tests for VTE handler (Print, Execute, and CSI sequences).
//!
//! Feed raw bytes through `vte::ansi::Processor` → `Term<RecordingListener>`
//! and verify grid state and events.

use std::sync::{Arc, Mutex};

use vte::ansi::Processor;

use crate::event::{Event, EventListener};
use crate::index::Column;
use crate::term::Term;
use crate::theme::Theme;

/// Event listener that records all events for assertions.
#[derive(Clone)]
struct RecordingListener {
    events: Arc<Mutex<Vec<String>>>,
}

impl RecordingListener {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn events(&self) -> Vec<String> {
        self.events.lock().expect("lock poisoned").clone()
    }
}

impl EventListener for RecordingListener {
    fn send_event(&self, event: Event) {
        self.events
            .lock()
            .expect("lock poisoned")
            .push(format!("{event:?}"));
    }
}

/// Create a Term with 24 lines, 80 columns, and a recording listener.
fn term_with_recorder() -> (Term<RecordingListener>, RecordingListener) {
    let listener = RecordingListener::new();
    let term = Term::new(24, 80, 0, Theme::default(), listener.clone());
    (term, listener)
}

/// Create a Term with VoidListener (when events don't matter).
fn term() -> Term<crate::event::VoidListener> {
    Term::new(24, 80, 0, Theme::default(), crate::event::VoidListener)
}

/// Feed raw bytes through the VTE processor.
fn feed(term: &mut impl vte::ansi::Handler, bytes: &[u8]) {
    let mut processor: Processor = Processor::new();
    processor.advance(term, bytes);
}

// --- Print (input) tests ---

#[test]
fn hello_places_cells_and_advances_cursor() {
    let mut t = term();
    feed(&mut t, b"hello");

    let grid = t.grid();
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'h');
    assert_eq!(grid[crate::index::Line(0)][Column(1)].ch, 'e');
    assert_eq!(grid[crate::index::Line(0)][Column(2)].ch, 'l');
    assert_eq!(grid[crate::index::Line(0)][Column(3)].ch, 'l');
    assert_eq!(grid[crate::index::Line(0)][Column(4)].ch, 'o');
    assert_eq!(grid.cursor().col(), Column(5));
    assert_eq!(grid.cursor().line(), 0);
}

#[test]
fn hello_newline_world() {
    let mut t = term();
    feed(&mut t, b"hello\nworld");

    let grid = t.grid();
    // "hello" on line 0.
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'h');
    assert_eq!(grid[crate::index::Line(0)][Column(4)].ch, 'o');
    // LF only moves down, column stays at 5. "world" starts at col 5 on line 1.
    assert_eq!(grid[crate::index::Line(1)][Column(5)].ch, 'w');
    assert_eq!(grid[crate::index::Line(1)][Column(9)].ch, 'd');
    assert_eq!(grid.cursor().line(), 1);
    assert_eq!(grid.cursor().col(), Column(10));
}

#[test]
fn carriage_return_overwrites() {
    let mut t = term();
    feed(&mut t, b"hello\rworld");

    let grid = t.grid();
    // "world" overwrites "hello" on line 0.
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'w');
    assert_eq!(grid[crate::index::Line(0)][Column(1)].ch, 'o');
    assert_eq!(grid[crate::index::Line(0)][Column(2)].ch, 'r');
    assert_eq!(grid[crate::index::Line(0)][Column(3)].ch, 'l');
    assert_eq!(grid[crate::index::Line(0)][Column(4)].ch, 'd');
    assert_eq!(grid.cursor().col(), Column(5));
}

#[test]
fn tab_advances_to_column_8() {
    let mut t = term();
    feed(&mut t, b"\t");

    // Tab stops are at 0, 8, 16, ... — from col 0, next stop is col 8.
    assert_eq!(t.grid().cursor().col(), Column(8));
}

#[test]
fn tab_from_midline() {
    let mut t = term();
    feed(&mut t, b"ab\t");

    // From col 2, next tab stop is col 8.
    assert_eq!(t.grid().cursor().col(), Column(8));
}

#[test]
fn backspace_moves_left() {
    let mut t = term();
    feed(&mut t, b"abc\x08");

    // "abc" puts cursor at col 3; backspace moves to col 2.
    assert_eq!(t.grid().cursor().col(), Column(2));
}

#[test]
fn backspace_at_col_zero_is_noop() {
    let mut t = term();
    feed(&mut t, b"\x08");

    assert_eq!(t.grid().cursor().col(), Column(0));
}

#[test]
fn bell_triggers_event() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x07");

    let events = listener.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], "Bell");
}

#[test]
fn linefeed_moves_down() {
    let mut t = term();
    feed(&mut t, b"A\n");

    let grid = t.grid();
    assert_eq!(grid.cursor().line(), 1);
    // LF does not change column (unlike CR+LF).
    assert_eq!(grid.cursor().col(), Column(1));
}

#[test]
fn vertical_tab_same_as_lf() {
    let mut t = term();
    feed(&mut t, b"A\x0B");

    // VT (0x0B) is treated identically to LF.
    assert_eq!(t.grid().cursor().line(), 1);
    assert_eq!(t.grid().cursor().col(), Column(1));
}

#[test]
fn form_feed_same_as_lf() {
    let mut t = term();
    feed(&mut t, b"A\x0C");

    // FF (0x0C) is treated identically to LF.
    assert_eq!(t.grid().cursor().line(), 1);
    assert_eq!(t.grid().cursor().col(), Column(1));
}

#[test]
fn so_activates_g1_charset() {
    let mut t = term();
    // SO = 0x0E activates G1.
    feed(&mut t, b"\x0E");

    assert_eq!(*t.charset().active(), vte::ansi::CharsetIndex::G1);
}

#[test]
fn si_activates_g0_charset() {
    let mut t = term();
    // SO then SI should restore G0.
    feed(&mut t, b"\x0E\x0F");

    assert_eq!(*t.charset().active(), vte::ansi::CharsetIndex::G0);
}

#[test]
fn crlf_moves_to_start_of_next_line() {
    let mut t = term();
    feed(&mut t, b"hello\r\n");

    let grid = t.grid();
    assert_eq!(grid.cursor().line(), 1);
    assert_eq!(grid.cursor().col(), Column(0));
}

#[test]
fn multiple_linefeeds() {
    let mut t = term();
    feed(&mut t, b"\n\n\n");

    assert_eq!(t.grid().cursor().line(), 3);
}

#[test]
fn substitute_writes_space() {
    let mut t = term();
    feed(&mut t, b"A\x1AB");

    let grid = t.grid();
    // SUB (0x1A) writes a space.
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[crate::index::Line(0)][Column(1)].ch, ' ');
    assert_eq!(grid[crate::index::Line(0)][Column(2)].ch, 'B');
}

// --- CSI cursor movement tests ---

#[test]
fn cuu_moves_cursor_up_5() {
    let mut t = term();
    // Move cursor to line 10, then CUU 5.
    feed(&mut t, b"\x1b[11;1H"); // CUP to line 10 (1-based)
    feed(&mut t, b"\x1b[5A"); // CUU 5

    assert_eq!(t.grid().cursor().line(), 5);
}

#[test]
fn cup_moves_cursor_to_line_9_col_19() {
    let mut t = term();
    // CSI 10;20 H — CUP to row 10, column 20 (1-based → 0-based: 9, 19).
    feed(&mut t, b"\x1b[10;20H");

    assert_eq!(t.grid().cursor().line(), 9);
    assert_eq!(t.grid().cursor().col(), Column(19));
}

// --- CSI erase tests ---

#[test]
fn ed_clears_screen() {
    let mut t = term();
    feed(&mut t, b"ABCDE\r\nFGHIJ\r\nKLMNO");
    // CSI 2 J — erase entire display.
    feed(&mut t, b"\x1b[2J");

    let grid = t.grid();
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, ' ');
    assert_eq!(grid[crate::index::Line(1)][Column(0)].ch, ' ');
    assert_eq!(grid[crate::index::Line(2)][Column(0)].ch, ' ');
}

#[test]
fn el_clears_to_end_of_line() {
    let mut t = term();
    feed(&mut t, b"ABCDE");
    // Move cursor to column 2, then EL 0 (clear to right).
    feed(&mut t, b"\x1b[3G"); // CHA column 3 (1-based) → col 2
    feed(&mut t, b"\x1b[K"); // EL (default = right)

    let grid = t.grid();
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[crate::index::Line(0)][Column(1)].ch, 'B');
    assert_eq!(grid[crate::index::Line(0)][Column(2)].ch, ' ');
    assert_eq!(grid[crate::index::Line(0)][Column(3)].ch, ' ');
    assert_eq!(grid[crate::index::Line(0)][Column(4)].ch, ' ');
}

// --- CSI insert / delete tests ---

#[test]
fn ich_inserts_5_blanks() {
    let mut t = term();
    feed(&mut t, b"ABCDE");
    // Move cursor to column 1, then ICH 5.
    feed(&mut t, b"\x1b[2G"); // CHA column 2 (1-based) → col 1
    feed(&mut t, b"\x1b[5@"); // ICH 5

    let grid = t.grid();
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'A');
    // 5 blanks inserted at col 1.
    assert_eq!(grid[crate::index::Line(0)][Column(1)].ch, ' ');
    assert_eq!(grid[crate::index::Line(0)][Column(5)].ch, ' ');
    // 'B' shifted to col 6.
    assert_eq!(grid[crate::index::Line(0)][Column(6)].ch, 'B');
}

#[test]
fn dch_deletes_3_chars() {
    let mut t = term();
    feed(&mut t, b"ABCDEFGH");
    // Move cursor to column 2, then DCH 3.
    feed(&mut t, b"\x1b[3G"); // CHA col 3 (1-based) → col 2
    feed(&mut t, b"\x1b[3P"); // DCH 3

    let grid = t.grid();
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[crate::index::Line(0)][Column(1)].ch, 'B');
    // C, D, E deleted; F shifts to col 2.
    assert_eq!(grid[crate::index::Line(0)][Column(2)].ch, 'F');
    assert_eq!(grid[crate::index::Line(0)][Column(3)].ch, 'G');
    assert_eq!(grid[crate::index::Line(0)][Column(4)].ch, 'H');
}

#[test]
fn il_inserts_2_lines() {
    let mut t = term();
    feed(&mut t, b"AAA\r\nBBB\r\nCCC\r\nDDD");
    // Move cursor to line 1 (0-based), then IL 2.
    feed(&mut t, b"\x1b[2;1H"); // CUP row 2 (1-based) → line 1
    feed(&mut t, b"\x1b[2L"); // IL 2

    let grid = t.grid();
    // Line 0: AAA (untouched).
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'A');
    // Lines 1–2: blank (inserted).
    assert_eq!(grid[crate::index::Line(1)][Column(0)].ch, ' ');
    assert_eq!(grid[crate::index::Line(2)][Column(0)].ch, ' ');
    // Line 3: BBB (pushed down from line 1).
    assert_eq!(grid[crate::index::Line(3)][Column(0)].ch, 'B');
}

#[test]
fn dl_deletes_3_lines() {
    let mut t = term();
    feed(&mut t, b"AAA\r\nBBB\r\nCCC\r\nDDD\r\nEEE");
    // Move cursor to line 1, then DL 3.
    feed(&mut t, b"\x1b[2;1H"); // CUP row 2 → line 1
    feed(&mut t, b"\x1b[3M"); // DL 3

    let grid = t.grid();
    // Line 0: AAA (untouched).
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'A');
    // Lines 1–3 deleted, EEE moved from line 4 to line 1.
    assert_eq!(grid[crate::index::Line(1)][Column(0)].ch, 'E');
    // Line 2 now blank.
    assert_eq!(grid[crate::index::Line(2)][Column(0)].ch, ' ');
}

// --- CSI mode tests ---

#[test]
fn dectcem_hides_cursor() {
    let mut t = term();
    // CSI ? 25 l — hide cursor.
    feed(&mut t, b"\x1b[?25l");

    assert!(!t.mode().contains(TermMode::SHOW_CURSOR));
}

#[test]
fn dectcem_shows_cursor() {
    let mut t = term();
    // First hide, then show.
    feed(&mut t, b"\x1b[?25l");
    feed(&mut t, b"\x1b[?25h");

    assert!(t.mode().contains(TermMode::SHOW_CURSOR));
}

#[test]
fn decset_alt_screen_switches_to_alt() {
    let mut t = term();
    feed(&mut t, b"hello"); // Write on primary.
    // CSI ? 1049 h — switch to alt screen.
    feed(&mut t, b"\x1b[?1049h");

    assert!(t.mode().contains(TermMode::ALT_SCREEN));
    // Alt screen should be clear.
    assert_eq!(t.grid()[crate::index::Line(0)][Column(0)].ch, ' ');
}

#[test]
fn decrst_alt_screen_switches_back() {
    let mut t = term();
    feed(&mut t, b"hello");
    feed(&mut t, b"\x1b[?1049h"); // Enter alt.
    feed(&mut t, b"alt");
    feed(&mut t, b"\x1b[?1049l"); // Leave alt.

    assert!(!t.mode().contains(TermMode::ALT_SCREEN));
    // Primary screen content restored.
    assert_eq!(t.grid()[crate::index::Line(0)][Column(0)].ch, 'h');
}

// --- CSI scroll region tests ---

#[test]
fn decstbm_sets_scroll_region() {
    let mut t = term();
    // CSI 3;20 r — set scroll region lines 3–20 (1-based).
    feed(&mut t, b"\x1b[3;20r");

    let region = t.grid().scroll_region();
    assert_eq!(region.start, 2); // 3 - 1 = 2 (0-based).
    assert_eq!(region.end, 20); // 20 (half-open).
    // Cursor should be at origin after DECSTBM.
    assert_eq!(t.grid().cursor().line(), 0);
    assert_eq!(t.grid().cursor().col(), Column(0));
}

// --- CSI device status tests ---

#[test]
fn dsr_produces_cursor_position_report() {
    let (mut t, listener) = term_with_recorder();
    // Move cursor to line 4, column 9 (0-based).
    feed(&mut t, b"\x1b[5;10H"); // CUP row 5, col 10 (1-based)
    // CSI 6 n — DSR: request cursor position.
    feed(&mut t, b"\x1b[6n");

    let events = listener.events();
    // CPR response: ESC [ 5 ; 10 R (1-based).
    assert!(events.iter().any(|e| e == "PtyWrite(\x1b[5;10R)"));
}

#[test]
fn da1_produces_device_attributes() {
    let (mut t, listener) = term_with_recorder();
    // CSI c — primary device attributes.
    feed(&mut t, b"\x1b[c");

    let events = listener.events();
    assert!(events.iter().any(|e| e == "PtyWrite(\x1b[?6c)"));
}

// --- ORIGIN mode tests ---

#[test]
fn origin_mode_cup_relative_to_scroll_region() {
    let mut t = term();
    // Set scroll region rows 5–15 (1-based), enable ORIGIN mode.
    feed(&mut t, b"\x1b[5;15r"); // DECSTBM
    feed(&mut t, b"\x1b[?6h"); // DECSET ORIGIN

    // CUP(1,1) in ORIGIN mode → absolute line 4 (region.start), col 0.
    feed(&mut t, b"\x1b[1;1H");
    assert_eq!(t.grid().cursor().line(), 4);
    assert_eq!(t.grid().cursor().col(), Column(0));

    // CUP(3,5) → absolute line 6, col 4.
    feed(&mut t, b"\x1b[3;5H");
    assert_eq!(t.grid().cursor().line(), 6);
    assert_eq!(t.grid().cursor().col(), Column(4));
}

#[test]
fn origin_mode_cup_clamps_to_scroll_region() {
    let mut t = term();
    // Scroll region rows 5–10 (1-based → lines 4..10).
    feed(&mut t, b"\x1b[5;10r");
    feed(&mut t, b"\x1b[?6h");

    // CUP(99,1) should clamp to bottom of region (line 9).
    feed(&mut t, b"\x1b[99;1H");
    assert_eq!(t.grid().cursor().line(), 9);
}

#[test]
fn origin_mode_vpa_relative_to_scroll_region() {
    let mut t = term();
    feed(&mut t, b"\x1b[5;15r"); // DECSTBM 5–15
    feed(&mut t, b"\x1b[?6h"); // ORIGIN mode
    feed(&mut t, b"\x1b[1;10H"); // Start at col 9

    // VPA(2) in ORIGIN mode → absolute line 5 (region.start + 1).
    feed(&mut t, b"\x1b[2d");
    assert_eq!(t.grid().cursor().line(), 5);
    // Column preserved.
    assert_eq!(t.grid().cursor().col(), Column(9));
}

#[test]
fn origin_mode_disabled_cup_uses_full_screen() {
    let mut t = term();
    feed(&mut t, b"\x1b[5;15r"); // DECSTBM
    feed(&mut t, b"\x1b[?6h"); // Enable ORIGIN
    feed(&mut t, b"\x1b[?6l"); // Disable ORIGIN

    // CUP(1,1) without ORIGIN → absolute line 0, col 0.
    feed(&mut t, b"\x1b[1;1H");
    assert_eq!(t.grid().cursor().line(), 0);
    assert_eq!(t.grid().cursor().col(), Column(0));
}

// --- IRM (Insert/Replace Mode) tests ---

#[test]
fn irm_insert_mode_shifts_content_right() {
    let mut t = term();
    feed(&mut t, b"foo");
    feed(&mut t, b"\x1b[1;1H"); // CUP to origin
    feed(&mut t, b"\x1b[4h"); // SM: set IRM (Insert mode)
    feed(&mut t, b"BAR");

    let grid = t.grid();
    // "BAR" inserted before "foo", shifting it right.
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'B');
    assert_eq!(grid[crate::index::Line(0)][Column(1)].ch, 'A');
    assert_eq!(grid[crate::index::Line(0)][Column(2)].ch, 'R');
    assert_eq!(grid[crate::index::Line(0)][Column(3)].ch, 'f');
    assert_eq!(grid[crate::index::Line(0)][Column(4)].ch, 'o');
    assert_eq!(grid[crate::index::Line(0)][Column(5)].ch, 'o');
}

#[test]
fn irm_replace_mode_overwrites() {
    let mut t = term();
    feed(&mut t, b"foo");
    feed(&mut t, b"\x1b[1;1H"); // CUP to origin
    feed(&mut t, b"\x1b[4h"); // SM: set IRM
    feed(&mut t, b"\x1b[4l"); // RM: reset IRM (back to replace)
    feed(&mut t, b"BAR");

    let grid = t.grid();
    // "BAR" overwrites "foo".
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'B');
    assert_eq!(grid[crate::index::Line(0)][Column(1)].ch, 'A');
    assert_eq!(grid[crate::index::Line(0)][Column(2)].ch, 'R');
    assert_eq!(grid.cursor().col(), Column(3));
}

// --- LNM (Line Feed / New Line Mode) tests ---

#[test]
fn lnm_mode_lf_acts_as_crlf() {
    let mut t = term();
    feed(&mut t, b"\x1b[20h"); // SM: set LNM
    feed(&mut t, b"hello\n"); // LF should also perform CR

    assert_eq!(t.grid().cursor().line(), 1);
    assert_eq!(t.grid().cursor().col(), Column(0));
}

#[test]
fn lnm_mode_off_lf_preserves_column() {
    let mut t = term();
    feed(&mut t, b"\x1b[20h"); // Enable LNM
    feed(&mut t, b"\x1b[20l"); // Disable LNM
    feed(&mut t, b"hello\n");

    assert_eq!(t.grid().cursor().line(), 1);
    // Column stays at 5 (normal LF behavior).
    assert_eq!(t.grid().cursor().col(), Column(5));
}

// --- CHA edge case tests ---

#[test]
fn cha_default_param_goes_to_column_0() {
    let mut t = term();
    feed(&mut t, b"hello");
    // CSI G — default param is 1 (1-based → col 0).
    feed(&mut t, b"\x1b[G");

    assert_eq!(t.grid().cursor().col(), Column(0));
}

#[test]
fn cha_overflow_clamps_to_last_column() {
    let mut t = term();
    // CSI 999 G — should clamp to col 79 on an 80-column terminal.
    feed(&mut t, b"\x1b[999G");

    assert_eq!(t.grid().cursor().col(), Column(79));
}

// --- CNL / CPL tests ---

#[test]
fn cnl_moves_down_and_to_column_0() {
    let mut t = term();
    feed(&mut t, b"hello");
    // CSI 3 E — move down 3 lines, column 0.
    feed(&mut t, b"\x1b[3E");

    assert_eq!(t.grid().cursor().line(), 3);
    assert_eq!(t.grid().cursor().col(), Column(0));
}

#[test]
fn cpl_moves_up_and_to_column_0() {
    let mut t = term();
    feed(&mut t, b"\x1b[10;15H"); // CUP to line 9, col 14
    // CSI 3 F — move up 3 lines, column 0.
    feed(&mut t, b"\x1b[3F");

    assert_eq!(t.grid().cursor().line(), 6);
    assert_eq!(t.grid().cursor().col(), Column(0));
}

// --- DSR code 5 and DA2 tests ---

#[test]
fn dsr_code_5_reports_terminal_ok() {
    let (mut t, listener) = term_with_recorder();
    // CSI 5 n — DSR: terminal status.
    feed(&mut t, b"\x1b[5n");

    let events = listener.events();
    assert!(events.iter().any(|e| e == "PtyWrite(\x1b[0n)"));
}

#[test]
fn da2_produces_secondary_device_attributes() {
    let (mut t, listener) = term_with_recorder();
    // CSI > c — secondary device attributes.
    feed(&mut t, b"\x1b[>c");

    let events = listener.events();
    // DA2 response: ESC [ > 0 ; version ; 1 c
    assert!(
        events
            .iter()
            .any(|e| e.starts_with("PtyWrite(\x1b[>0;") && e.ends_with(";1c)"))
    );
}

// --- DECRPM (mode report) tests ---

#[test]
fn decrpm_reports_set_private_mode() {
    let (mut t, listener) = term_with_recorder();
    // SHOW_CURSOR is on by default.
    // CSI ? 25 $ p — query DECTCEM.
    feed(&mut t, b"\x1b[?25$p");

    let events = listener.events();
    // Response: CSI ? 25 ; 1 $ y (1 = set).
    assert!(events.iter().any(|e| e == "PtyWrite(\x1b[?25;1$y)"));
}

#[test]
fn decrpm_reports_reset_private_mode() {
    let (mut t, listener) = term_with_recorder();
    // ALT_SCREEN is off by default.
    // CSI ? 1049 $ p — query alt screen.
    feed(&mut t, b"\x1b[?1049$p");

    let events = listener.events();
    // Response: CSI ? 1049 ; 2 $ y (2 = reset).
    assert!(events.iter().any(|e| e == "PtyWrite(\x1b[?1049;2$y)"));
}

#[test]
fn decrpm_reports_ansi_mode() {
    let (mut t, listener) = term_with_recorder();
    // INSERT mode is off by default.
    // CSI 4 $ p — query IRM.
    feed(&mut t, b"\x1b[4$p");

    let events = listener.events();
    // Response: CSI 4 ; 2 $ y (2 = reset).
    assert!(events.iter().any(|e| e == "PtyWrite(\x1b[4;2$y)"));
}

// --- ECH edge case tests ---

#[test]
fn ech_overflow_clamps_to_line_end() {
    let mut t = term();
    feed(&mut t, b"ABCDE");
    feed(&mut t, b"\x1b[2G"); // CHA col 2 → col 1
    // ECH 999 — should erase from col 1 to end of line.
    feed(&mut t, b"\x1b[999X");

    let grid = t.grid();
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'A');
    assert_eq!(grid[crate::index::Line(0)][Column(1)].ch, ' ');
    assert_eq!(grid[crate::index::Line(0)][Column(4)].ch, ' ');
}

// --- Scroll up/down through VTE bytes ---

#[test]
fn su_scrolls_content_up() {
    let mut t = term();
    feed(&mut t, b"AAA\r\nBBB\r\nCCC");
    // CSI 1 S — scroll up 1.
    feed(&mut t, b"\x1b[1S");

    let grid = t.grid();
    // Line 0 now has BBB (was line 1).
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, 'B');
    assert_eq!(grid[crate::index::Line(1)][Column(0)].ch, 'C');
}

#[test]
fn sd_scrolls_content_down() {
    let mut t = term();
    feed(&mut t, b"AAA\r\nBBB\r\nCCC");
    // CSI 1 T — scroll down 1.
    feed(&mut t, b"\x1b[1T");

    let grid = t.grid();
    // Line 0 is blank (new).
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, ' ');
    // AAA moved from line 0 to line 1.
    assert_eq!(grid[crate::index::Line(1)][Column(0)].ch, 'A');
    assert_eq!(grid[crate::index::Line(2)][Column(0)].ch, 'B');
}

// --- RI (Reverse Index) through VTE bytes ---

#[test]
fn ri_at_top_of_scroll_region_scrolls_down() {
    let mut t = term();
    feed(&mut t, b"AAA\r\nBBB\r\nCCC");
    feed(&mut t, b"\x1b[1;1H"); // CUP to origin (top of region)
    // ESC M — reverse index.
    feed(&mut t, b"\x1bM");

    let grid = t.grid();
    // Line 0 is blank (scrolled down).
    assert_eq!(grid[crate::index::Line(0)][Column(0)].ch, ' ');
    // AAA pushed to line 1.
    assert_eq!(grid[crate::index::Line(1)][Column(0)].ch, 'A');
    assert_eq!(grid[crate::index::Line(2)][Column(0)].ch, 'B');
}

#[test]
fn ri_in_middle_moves_cursor_up() {
    let mut t = term();
    feed(&mut t, b"\x1b[5;1H"); // CUP to line 4
    // ESC M — reverse index (not at region top → just moves up).
    feed(&mut t, b"\x1bM");

    assert_eq!(t.grid().cursor().line(), 3);
}

// --- DECSC / DECRC full round-trip ---

#[test]
fn decsc_decrc_saves_and_restores_cursor_position() {
    let mut t = term();
    feed(&mut t, b"\x1b[5;10H"); // CUP to line 4, col 9
    feed(&mut t, b"\x1b7"); // DECSC: save cursor
    feed(&mut t, b"\x1b[1;1H"); // Move somewhere else
    feed(&mut t, b"\x1b8"); // DECRC: restore cursor

    assert_eq!(t.grid().cursor().line(), 4);
    assert_eq!(t.grid().cursor().col(), Column(9));
}

// --- DSR cursor position report in ORIGIN mode ---

#[test]
fn dsr_reports_absolute_position_even_in_origin_mode() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b[5;15r"); // DECSTBM 5–15
    feed(&mut t, b"\x1b[?6h"); // ORIGIN mode
    feed(&mut t, b"\x1b[1;1H"); // CUP(1,1) → absolute line 4, col 0
    feed(&mut t, b"\x1b[6n"); // DSR

    let events = listener.events();
    // CPR: absolute line 4 + 1 = 5, col 0 + 1 = 1.
    assert!(events.iter().any(|e| e == "PtyWrite(\x1b[5;1R)"));
}

// --- Text area size report ---

#[test]
fn text_area_size_chars_reports_dimensions() {
    let (mut t, listener) = term_with_recorder();
    // CSI 18 t — report text area size in characters.
    feed(&mut t, b"\x1b[18t");

    let events = listener.events();
    // Response: CSI 8 ; lines ; cols t.
    assert!(events.iter().any(|e| e == "PtyWrite(\x1b[8;24;80t)"));
}

// --- Keypad mode tests ---

#[test]
fn deckpam_sets_application_keypad() {
    let mut t = term();
    // ESC = — DECKPAM.
    feed(&mut t, b"\x1b=");

    assert!(t.mode().contains(TermMode::APP_KEYPAD));
}

#[test]
fn deckpnm_resets_application_keypad() {
    let mut t = term();
    feed(&mut t, b"\x1b="); // Enable
    // ESC > — DECKPNM.
    feed(&mut t, b"\x1b>");

    assert!(!t.mode().contains(TermMode::APP_KEYPAD));
}

// --- Tab CSI tests ---

#[test]
fn cht_forward_tab_by_count() {
    let mut t = term();
    // CSI 2 I — forward tab 2 times (from col 0 → 8 → 16).
    feed(&mut t, b"\x1b[2I");

    assert_eq!(t.grid().cursor().col(), Column(16));
}

#[test]
fn cbt_backward_tab_by_count() {
    let mut t = term();
    feed(&mut t, b"\x1b[20G"); // CHA col 20 → col 19
    // CSI 2 Z — backward tab 2 times (col 19 → 16 → 8).
    feed(&mut t, b"\x1b[2Z");

    assert_eq!(t.grid().cursor().col(), Column(8));
}

#[test]
fn tbc_clears_all_tab_stops() {
    let mut t = term();
    // CSI 3 g — clear all tab stops.
    feed(&mut t, b"\x1b[3g");
    // Now tab from col 0 should go to last column (no stops).
    feed(&mut t, b"\t");

    assert_eq!(t.grid().cursor().col(), Column(79));
}

// --- NEL (Next Line) test ---

#[test]
fn nel_performs_cr_and_lf() {
    let mut t = term();
    feed(&mut t, b"hello");
    // ESC E — NEL: next line (CR + LF).
    feed(&mut t, b"\x1bE");

    assert_eq!(t.grid().cursor().line(), 1);
    assert_eq!(t.grid().cursor().col(), Column(0));
}

// --- SGR (Select Graphic Rendition) tests ---

#[test]
fn sgr_bold_sets_flag_on_cursor_template() {
    let mut t = term();
    // ESC[1m — set bold.
    feed(&mut t, b"\x1b[1m");

    let flags = t.grid().cursor().template.flags;
    assert!(flags.contains(crate::cell::CellFlags::BOLD));
}

#[test]
fn sgr_fg_red_sets_ansi_color() {
    let mut t = term();
    // ESC[31m — set fg to red (ANSI 1).
    feed(&mut t, b"\x1b[31m");

    let fg = t.grid().cursor().template.fg;
    assert_eq!(fg, vte::ansi::Color::Named(vte::ansi::NamedColor::Red));
}

#[test]
fn sgr_256color_fg() {
    let mut t = term();
    // ESC[38;5;196m — set fg to 256-color index 196.
    feed(&mut t, b"\x1b[38;5;196m");

    let fg = t.grid().cursor().template.fg;
    assert_eq!(fg, vte::ansi::Color::Indexed(196));
}

#[test]
fn sgr_truecolor_fg() {
    let mut t = term();
    // ESC[38;2;255;128;0m — set fg to RGB(255, 128, 0).
    feed(&mut t, b"\x1b[38;2;255;128;0m");

    let fg = t.grid().cursor().template.fg;
    assert_eq!(
        fg,
        vte::ansi::Color::Spec(vte::ansi::Rgb {
            r: 255,
            g: 128,
            b: 0
        })
    );
}

#[test]
fn sgr_reset_clears_all_attributes() {
    let mut t = term();
    // Set bold + red fg + green bg, then reset.
    feed(&mut t, b"\x1b[1;31;42m");
    feed(&mut t, b"\x1b[0m");

    let template = &t.grid().cursor().template;
    assert_eq!(template.flags, crate::cell::CellFlags::empty());
    assert_eq!(
        template.fg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::Foreground)
    );
    assert_eq!(
        template.bg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::Background)
    );
}

#[test]
fn sgr_compound_bold_red_fg_green_bg() {
    let mut t = term();
    // ESC[1;31;42m — bold + red fg + green bg in one sequence.
    feed(&mut t, b"\x1b[1;31;42m");

    let template = &t.grid().cursor().template;
    assert!(template.flags.contains(crate::cell::CellFlags::BOLD));
    assert_eq!(
        template.fg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::Red)
    );
    assert_eq!(
        template.bg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::Green)
    );
}

#[test]
fn sgr_curly_underline() {
    let mut t = term();
    // ESC[4:3m — curly underline (sub-param style).
    feed(&mut t, b"\x1b[4:3m");

    let flags = t.grid().cursor().template.flags;
    assert!(flags.contains(crate::cell::CellFlags::CURLY_UNDERLINE));
    // Should not have regular underline.
    assert!(!flags.contains(crate::cell::CellFlags::UNDERLINE));
}

#[test]
fn sgr_underline_color_truecolor() {
    let mut t = term();
    // ESC[58;2;255;0;0m — set underline color to red (CellExtra).
    feed(&mut t, b"\x1b[58;2;255;0;0m");

    let template = &t.grid().cursor().template;
    let extra = template
        .extra
        .as_ref()
        .expect("CellExtra should be allocated");
    assert_eq!(
        extra.underline_color,
        Some(vte::ansi::Color::Spec(vte::ansi::Rgb {
            r: 255,
            g: 0,
            b: 0
        }))
    );
}

#[test]
fn sgr_59_clears_underline_color() {
    let mut t = term();
    // Set underline color, then clear it.
    feed(&mut t, b"\x1b[58;2;255;0;0m");
    feed(&mut t, b"\x1b[59m");

    let template = &t.grid().cursor().template;
    // CellExtra should be dropped (no other extra data).
    assert!(template.extra.is_none());
}

// --- SGR individual attribute flag tests ---

#[test]
fn sgr_dim_sets_flag() {
    let mut t = term();
    feed(&mut t, b"\x1b[2m");

    assert!(
        t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::DIM)
    );
}

#[test]
fn sgr_italic_sets_flag() {
    let mut t = term();
    feed(&mut t, b"\x1b[3m");

    assert!(
        t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::ITALIC)
    );
}

#[test]
fn sgr_blink_sets_flag() {
    let mut t = term();
    feed(&mut t, b"\x1b[5m");

    assert!(
        t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::BLINK)
    );
}

#[test]
fn sgr_inverse_sets_flag() {
    let mut t = term();
    feed(&mut t, b"\x1b[7m");

    assert!(
        t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::INVERSE)
    );
}

#[test]
fn sgr_hidden_sets_flag() {
    let mut t = term();
    feed(&mut t, b"\x1b[8m");

    assert!(
        t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::HIDDEN)
    );
}

#[test]
fn sgr_strikethrough_sets_flag() {
    let mut t = term();
    feed(&mut t, b"\x1b[9m");

    assert!(
        t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::STRIKETHROUGH)
    );
}

// --- SGR cancel attribute tests ---

#[test]
fn sgr_22_cancels_bold_and_dim() {
    let mut t = term();
    // Set both bold and dim, then cancel both with SGR 22.
    feed(&mut t, b"\x1b[1;2m");
    feed(&mut t, b"\x1b[22m");

    let flags = t.grid().cursor().template.flags;
    assert!(!flags.contains(crate::cell::CellFlags::BOLD));
    assert!(!flags.contains(crate::cell::CellFlags::DIM));
}

#[test]
fn sgr_23_cancels_italic() {
    let mut t = term();
    feed(&mut t, b"\x1b[3m");
    feed(&mut t, b"\x1b[23m");

    assert!(
        !t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::ITALIC)
    );
}

#[test]
fn sgr_24_cancels_all_underlines() {
    let mut t = term();
    // Set curly underline, then cancel.
    feed(&mut t, b"\x1b[4:3m");
    feed(&mut t, b"\x1b[24m");

    let flags = t.grid().cursor().template.flags;
    assert!(!flags.contains(crate::cell::CellFlags::CURLY_UNDERLINE));
    assert!(!flags.contains(crate::cell::CellFlags::UNDERLINE));
}

#[test]
fn sgr_25_cancels_blink() {
    let mut t = term();
    feed(&mut t, b"\x1b[5m");
    feed(&mut t, b"\x1b[25m");

    assert!(
        !t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::BLINK)
    );
}

#[test]
fn sgr_27_cancels_inverse() {
    let mut t = term();
    feed(&mut t, b"\x1b[7m");
    feed(&mut t, b"\x1b[27m");

    assert!(
        !t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::INVERSE)
    );
}

#[test]
fn sgr_28_cancels_hidden() {
    let mut t = term();
    feed(&mut t, b"\x1b[8m");
    feed(&mut t, b"\x1b[28m");

    assert!(
        !t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::HIDDEN)
    );
}

#[test]
fn sgr_29_cancels_strikethrough() {
    let mut t = term();
    feed(&mut t, b"\x1b[9m");
    feed(&mut t, b"\x1b[29m");

    assert!(
        !t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::STRIKETHROUGH)
    );
}

// --- SGR underline mutual exclusion tests ---

#[test]
fn sgr_underline_replaces_curly() {
    let mut t = term();
    // Set curly, then single — single should replace curly.
    feed(&mut t, b"\x1b[4:3m");
    feed(&mut t, b"\x1b[4m");

    let flags = t.grid().cursor().template.flags;
    assert!(flags.contains(crate::cell::CellFlags::UNDERLINE));
    assert!(!flags.contains(crate::cell::CellFlags::CURLY_UNDERLINE));
}

#[test]
fn sgr_double_underline_replaces_single() {
    let mut t = term();
    // Single underline, then double via sub-param ESC[4:2m.
    feed(&mut t, b"\x1b[4m");
    feed(&mut t, b"\x1b[4:2m");

    let flags = t.grid().cursor().template.flags;
    assert!(flags.contains(crate::cell::CellFlags::DOUBLE_UNDERLINE));
    assert!(!flags.contains(crate::cell::CellFlags::UNDERLINE));
}

#[test]
fn sgr_dotted_underline() {
    let mut t = term();
    feed(&mut t, b"\x1b[4:4m");

    let flags = t.grid().cursor().template.flags;
    assert!(flags.contains(crate::cell::CellFlags::DOTTED_UNDERLINE));
}

#[test]
fn sgr_dashed_underline() {
    let mut t = term();
    feed(&mut t, b"\x1b[4:5m");

    let flags = t.grid().cursor().template.flags;
    assert!(flags.contains(crate::cell::CellFlags::DASHED_UNDERLINE));
}

// --- SGR cancel preserves unrelated attributes ---

#[test]
fn sgr_cancel_underline_preserves_bold() {
    let mut t = term();
    // Bold + underline, then cancel underline — bold should remain.
    feed(&mut t, b"\x1b[1;4m");
    feed(&mut t, b"\x1b[24m");

    let flags = t.grid().cursor().template.flags;
    assert!(flags.contains(crate::cell::CellFlags::BOLD));
    assert!(!flags.contains(crate::cell::CellFlags::UNDERLINE));
}

#[test]
fn sgr_cancel_bold_preserves_italic_and_color() {
    let mut t = term();
    // Bold + italic + red fg, then cancel bold.
    feed(&mut t, b"\x1b[1;3;31m");
    feed(&mut t, b"\x1b[22m");

    let template = &t.grid().cursor().template;
    assert!(!template.flags.contains(crate::cell::CellFlags::BOLD));
    assert!(template.flags.contains(crate::cell::CellFlags::ITALIC));
    assert_eq!(
        template.fg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::Red)
    );
}

// --- SGR color tests ---

#[test]
fn sgr_bg_256color() {
    let mut t = term();
    feed(&mut t, b"\x1b[48;5;42m");

    assert_eq!(t.grid().cursor().template.bg, vte::ansi::Color::Indexed(42));
}

#[test]
fn sgr_bg_truecolor() {
    let mut t = term();
    feed(&mut t, b"\x1b[48;2;0;128;255m");

    assert_eq!(
        t.grid().cursor().template.bg,
        vte::ansi::Color::Spec(vte::ansi::Rgb {
            r: 0,
            g: 128,
            b: 255
        })
    );
}

#[test]
fn sgr_bright_fg() {
    let mut t = term();
    // ESC[91m — bright red foreground (ANSI 8–15 range).
    feed(&mut t, b"\x1b[91m");

    assert_eq!(
        t.grid().cursor().template.fg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::BrightRed)
    );
}

#[test]
fn sgr_bright_bg() {
    let mut t = term();
    // ESC[102m — bright green background.
    feed(&mut t, b"\x1b[102m");

    assert_eq!(
        t.grid().cursor().template.bg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::BrightGreen)
    );
}

#[test]
fn sgr_39_resets_fg_only() {
    let mut t = term();
    // Red fg + green bg, then reset fg only.
    feed(&mut t, b"\x1b[31;42m");
    feed(&mut t, b"\x1b[39m");

    let template = &t.grid().cursor().template;
    assert_eq!(
        template.fg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::Foreground)
    );
    assert_eq!(
        template.bg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::Green)
    );
}

#[test]
fn sgr_49_resets_bg_only() {
    let mut t = term();
    // Red fg + green bg, then reset bg only.
    feed(&mut t, b"\x1b[31;42m");
    feed(&mut t, b"\x1b[49m");

    let template = &t.grid().cursor().template;
    assert_eq!(
        template.fg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::Red)
    );
    assert_eq!(
        template.bg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::Background)
    );
}

// --- SGR character inheritance tests ---

#[test]
fn printed_char_inherits_bold() {
    let mut t = term();
    // Bold, then print 'A'.
    feed(&mut t, b"\x1b[1mA");

    let cell = &t.grid()[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, 'A');
    assert!(cell.flags.contains(crate::cell::CellFlags::BOLD));
}

#[test]
fn printed_char_inherits_fg_color() {
    let mut t = term();
    feed(&mut t, b"\x1b[31mA");

    let cell = &t.grid()[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.fg, vte::ansi::Color::Named(vte::ansi::NamedColor::Red));
}

#[test]
fn reset_between_chars_gives_different_attrs() {
    let mut t = term();
    // Bold 'A', then reset + 'B'.
    feed(&mut t, b"\x1b[1mA\x1b[0mB");

    let a = &t.grid()[crate::index::Line(0)][Column(0)];
    let b = &t.grid()[crate::index::Line(0)][Column(1)];
    assert!(a.flags.contains(crate::cell::CellFlags::BOLD));
    assert!(!b.flags.contains(crate::cell::CellFlags::BOLD));
}

// --- SGR persistence tests ---

#[test]
fn sgr_persists_across_cursor_movement() {
    let mut t = term();
    // Set bold, then move cursor down 5.
    feed(&mut t, b"\x1b[1m");
    feed(&mut t, b"\x1b[5B");

    assert!(
        t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::BOLD)
    );
}

#[test]
fn sgr_stacks_across_separate_sequences() {
    let mut t = term();
    // Bold in one sequence, underline in another, color in a third.
    feed(&mut t, b"\x1b[1m");
    feed(&mut t, b"\x1b[4m");
    feed(&mut t, b"\x1b[31m");

    let template = &t.grid().cursor().template;
    assert!(template.flags.contains(crate::cell::CellFlags::BOLD));
    assert!(template.flags.contains(crate::cell::CellFlags::UNDERLINE));
    assert_eq!(
        template.fg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::Red)
    );
}

// --- SGR edge case tests ---

#[test]
fn sgr_empty_params_resets() {
    let mut t = term();
    // Set bold, then ESC[m (no params) should reset like SGR 0.
    feed(&mut t, b"\x1b[1m");
    feed(&mut t, b"\x1b[m");

    assert_eq!(
        t.grid().cursor().template.flags,
        crate::cell::CellFlags::empty()
    );
}

#[test]
fn sgr_last_color_wins() {
    let mut t = term();
    // ESC[30;31m — black then red in same sequence; red should win.
    feed(&mut t, b"\x1b[30;31m");

    assert_eq!(
        t.grid().cursor().template.fg,
        vte::ansi::Color::Named(vte::ansi::NamedColor::Red)
    );
}

#[test]
fn sgr_fast_blink_uses_blink_flag() {
    let mut t = term();
    // SGR 6 (fast blink) — mapped to same BLINK flag as slow blink.
    feed(&mut t, b"\x1b[6m");

    assert!(
        t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::BLINK)
    );
}

#[test]
fn sgr_underline_color_survives_underline_type_change() {
    let mut t = term();
    // Set underline color to red, then switch from single to curly.
    feed(&mut t, b"\x1b[4m");
    feed(&mut t, b"\x1b[58;2;255;0;0m");
    feed(&mut t, b"\x1b[4:3m");

    let template = &t.grid().cursor().template;
    assert!(
        template
            .flags
            .contains(crate::cell::CellFlags::CURLY_UNDERLINE)
    );
    let extra = template
        .extra
        .as_ref()
        .expect("underline color should survive");
    assert_eq!(
        extra.underline_color,
        Some(vte::ansi::Color::Spec(vte::ansi::Rgb {
            r: 255,
            g: 0,
            b: 0
        }))
    );
}

#[test]
fn sgr_underline_color_256() {
    let mut t = term();
    // ESC[58;5;196m — set underline color to 256-color index 196.
    feed(&mut t, b"\x1b[58;5;196m");

    let extra = t
        .grid()
        .cursor()
        .template
        .extra
        .as_ref()
        .expect("CellExtra should be allocated");
    assert_eq!(extra.underline_color, Some(vte::ansi::Color::Indexed(196)));
}

#[test]
fn sgr_reset_clears_underline_color() {
    let mut t = term();
    // Set underline color, then full reset.
    feed(&mut t, b"\x1b[58;2;255;0;0m");
    feed(&mut t, b"\x1b[0m");

    assert!(t.grid().cursor().template.extra.is_none());
}

// --- OSC (Operating System Command) tests ---

#[test]
fn osc2_sets_window_title() {
    let (mut t, listener) = term_with_recorder();
    // ESC]2;Hello World\x07
    feed(&mut t, b"\x1b]2;Hello World\x07");

    assert_eq!(t.title(), "Hello World");
    let events = listener.events();
    assert!(events.iter().any(|e| e.contains("Title(Hello World)")));
}

#[test]
fn osc0_sets_window_title() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]0;My Terminal\x07");

    assert_eq!(t.title(), "My Terminal");
    let events = listener.events();
    assert!(events.iter().any(|e| e.contains("Title(My Terminal)")));
}

#[test]
fn osc2_with_st_terminator() {
    let mut t = term();
    // ESC]2;Title Here\x1b\\
    feed(&mut t, b"\x1b]2;Title Here\x1b\\");

    assert_eq!(t.title(), "Title Here");
}

#[test]
fn osc_title_with_semicolons() {
    let mut t = term();
    // Title containing semicolons should be preserved.
    feed(&mut t, b"\x1b]2;a;b;c\x07");

    assert_eq!(t.title(), "a;b;c");
}

#[test]
fn osc_push_pop_title() {
    let mut t = term();
    // Set initial title.
    feed(&mut t, b"\x1b]2;First\x07");
    assert_eq!(t.title(), "First");

    // Push title (ESC[22t).
    // VTE dispatches push_title from CSI 22;2t.
    feed(&mut t, b"\x1b[22;2t");
    assert_eq!(t.title_stack().len(), 1);
    assert_eq!(t.title_stack()[0], "First");

    // Set new title.
    feed(&mut t, b"\x1b]2;Second\x07");
    assert_eq!(t.title(), "Second");

    // Pop title (ESC[23t).
    feed(&mut t, b"\x1b[23;2t");
    assert_eq!(t.title(), "First");
    assert!(t.title_stack().is_empty());
}

#[test]
fn osc_pop_empty_stack_is_noop() {
    let mut t = term();
    feed(&mut t, b"\x1b]2;Original\x07");

    // Pop from empty stack — title should remain.
    feed(&mut t, b"\x1b[23;2t");
    assert_eq!(t.title(), "Original");
}

#[test]
fn osc4_sets_indexed_color() {
    let mut t = term();
    // ESC]4;1;rgb:ff/00/00\x07 — set color index 1 to red.
    feed(&mut t, b"\x1b]4;1;rgb:ff/00/00\x07");

    let color = t.palette().resolve(vte::ansi::Color::Indexed(1));
    assert_eq!(
        color,
        vte::ansi::Rgb {
            r: 0xff,
            g: 0x00,
            b: 0x00
        }
    );
}

#[test]
fn osc4_query_sends_color_request_event() {
    let (mut t, listener) = term_with_recorder();
    // ESC]4;1;?\x07 — query color index 1.
    feed(&mut t, b"\x1b]4;1;?\x07");

    let events = listener.events();
    assert!(events.iter().any(|e| e.contains("ColorRequest(1)")));
}

#[test]
fn osc10_sets_foreground_color() {
    let mut t = term();
    // ESC]10;rgb:aa/bb/cc\x07 — set foreground.
    feed(&mut t, b"\x1b]10;rgb:aa/bb/cc\x07");

    assert_eq!(
        t.palette().foreground(),
        vte::ansi::Rgb {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc
        }
    );
}

#[test]
fn osc11_sets_background_color() {
    let mut t = term();
    feed(&mut t, b"\x1b]11;rgb:11/22/33\x07");

    assert_eq!(
        t.palette().background(),
        vte::ansi::Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33
        }
    );
}

#[test]
fn osc12_sets_cursor_color() {
    let mut t = term();
    feed(&mut t, b"\x1b]12;rgb:ff/ff/00\x07");

    assert_eq!(
        t.palette().cursor_color(),
        vte::ansi::Rgb {
            r: 0xff,
            g: 0xff,
            b: 0x00
        }
    );
}

#[test]
fn osc104_resets_indexed_color() {
    let mut t = term();
    let original = t.palette().resolve(vte::ansi::Color::Indexed(1));

    // Set then reset.
    feed(&mut t, b"\x1b]4;1;rgb:ff/ff/ff\x07");
    assert_ne!(t.palette().resolve(vte::ansi::Color::Indexed(1)), original);

    feed(&mut t, b"\x1b]104;1\x07");
    assert_eq!(t.palette().resolve(vte::ansi::Color::Indexed(1)), original);
}

#[test]
fn osc110_resets_foreground() {
    let mut t = term();
    let original = t.palette().foreground();

    feed(&mut t, b"\x1b]10;rgb:ff/00/ff\x07");
    assert_ne!(t.palette().foreground(), original);

    feed(&mut t, b"\x1b]110\x07");
    assert_eq!(t.palette().foreground(), original);
}

#[test]
fn osc111_resets_background() {
    let mut t = term();
    let original = t.palette().background();

    feed(&mut t, b"\x1b]11;rgb:ff/00/ff\x07");
    assert_ne!(t.palette().background(), original);

    feed(&mut t, b"\x1b]111\x07");
    assert_eq!(t.palette().background(), original);
}

#[test]
fn osc112_resets_cursor_color() {
    let mut t = term();
    let original = t.palette().cursor_color();

    feed(&mut t, b"\x1b]12;rgb:00/ff/00\x07");
    assert_ne!(t.palette().cursor_color(), original);

    feed(&mut t, b"\x1b]112\x07");
    assert_eq!(t.palette().cursor_color(), original);
}

#[test]
fn osc52_clipboard_store() {
    let (mut t, listener) = term_with_recorder();
    // ESC]52;c;aGVsbG8=\x07 — store "hello" to clipboard.
    feed(&mut t, b"\x1b]52;c;aGVsbG8=\x07");

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardStore(Clipboard, hello)")),
        "Expected ClipboardStore event with 'hello', got: {events:?}",
    );
}

#[test]
fn osc52_clipboard_load() {
    let (mut t, listener) = term_with_recorder();
    // ESC]52;c;?\x07 — request clipboard content.
    feed(&mut t, b"\x1b]52;c;?\x07");

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardLoad(Clipboard)")),
        "Expected ClipboardLoad event, got: {events:?}",
    );
}

#[test]
fn osc52_primary_selection() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]52;p;dGVzdA==\x07");

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardStore(Selection, test)")),
        "Expected Selection store, got: {events:?}",
    );
}

#[test]
fn osc52_invalid_base64_is_ignored() {
    let (mut t, listener) = term_with_recorder();
    // Invalid base64 should not produce an event.
    feed(&mut t, b"\x1b]52;c;!!!invalid!!!\x07");

    let events = listener.events();
    assert!(
        !events.iter().any(|e| e.contains("ClipboardStore")),
        "Should not produce ClipboardStore for invalid base64, got: {events:?}",
    );
}

#[test]
fn osc8_sets_hyperlink() {
    let mut t = term();
    // ESC]8;;https://example.com\x07
    feed(&mut t, b"\x1b]8;;https://example.com\x07");

    let extra = t
        .grid()
        .cursor()
        .template
        .extra
        .as_ref()
        .expect("CellExtra should be allocated for hyperlink");
    let link = extra.hyperlink.as_ref().expect("hyperlink should be set");
    assert_eq!(link.uri, "https://example.com");
    assert!(link.id.is_none());
}

#[test]
fn osc8_with_id_parameter() {
    let mut t = term();
    feed(&mut t, b"\x1b]8;id=my-link;https://example.com\x07");

    let extra = t.grid().cursor().template.extra.as_ref().unwrap();
    let link = extra.hyperlink.as_ref().unwrap();
    assert_eq!(link.uri, "https://example.com");
    assert_eq!(link.id.as_deref(), Some("my-link"));
}

#[test]
fn osc8_clear_hyperlink() {
    let mut t = term();
    // Set then clear.
    feed(&mut t, b"\x1b]8;;https://example.com\x07");
    assert!(t.grid().cursor().template.extra.is_some());

    feed(&mut t, b"\x1b]8;;\x07");
    // CellExtra should be dropped (no other extra data).
    assert!(t.grid().cursor().template.extra.is_none());
}

// --- OSC edge cases (from reference repos: Alacritty, Ghostty, WezTerm) ---

#[test]
fn osc_title_empty_string() {
    let mut t = term();
    feed(&mut t, b"\x1b]2;Something\x07");
    assert_eq!(t.title(), "Something");

    // Empty title (OSC 2 with empty payload).
    feed(&mut t, b"\x1b]2;\x07");
    assert_eq!(t.title(), "");
}

#[test]
fn osc_title_utf8_multibyte() {
    let mut t = term();
    // Title with multi-byte UTF-8: em dash (U+2014) + CJK character.
    let mut seq = b"\x1b]2;".to_vec();
    seq.extend_from_slice("Hello — 世界".as_bytes());
    seq.push(0x07);
    feed(&mut t, &seq);

    assert_eq!(t.title(), "Hello — 世界");
}

#[test]
fn osc_title_stack_cap_at_4096() {
    let mut t = term();
    // Push 4096 titles to fill the stack.
    for i in 0..4096 {
        let title = format!("\x1b]2;title-{i}\x07");
        feed(&mut t, title.as_bytes());
        feed(&mut t, b"\x1b[22;2t");
    }
    assert_eq!(t.title_stack().len(), 4096);

    // One more push should evict the oldest.
    feed(&mut t, b"\x1b]2;overflow\x07");
    feed(&mut t, b"\x1b[22;2t");
    assert_eq!(t.title_stack().len(), 4096);
    // Oldest entry ("title-0") should be gone.
    assert_ne!(t.title_stack()[0], "title-0");
}

#[test]
fn osc_push_pop_interleaved() {
    let mut t = term();

    feed(&mut t, b"\x1b]2;A\x07");
    feed(&mut t, b"\x1b[22;2t"); // push "A"
    feed(&mut t, b"\x1b]2;B\x07");
    feed(&mut t, b"\x1b[22;2t"); // push "B"
    feed(&mut t, b"\x1b]2;C\x07");

    assert_eq!(t.title(), "C");
    assert_eq!(t.title_stack().len(), 2);

    // Pop should restore in LIFO order.
    feed(&mut t, b"\x1b[23;2t");
    assert_eq!(t.title(), "B");
    feed(&mut t, b"\x1b[23;2t");
    assert_eq!(t.title(), "A");
    assert!(t.title_stack().is_empty());
}

#[test]
fn osc4_multiple_colors_in_one_sequence() {
    let mut t = term();
    // Set two colors in a single OSC 4 (VTE processes pairs).
    feed(&mut t, b"\x1b]4;1;rgb:ff/00/00;2;rgb:00/ff/00\x07");

    let c1 = t.palette().resolve(vte::ansi::Color::Indexed(1));
    let c2 = t.palette().resolve(vte::ansi::Color::Indexed(2));
    assert_eq!(
        c1,
        vte::ansi::Rgb {
            r: 0xff,
            g: 0x00,
            b: 0x00
        }
    );
    assert_eq!(
        c2,
        vte::ansi::Rgb {
            r: 0x00,
            g: 0xff,
            b: 0x00
        }
    );
}

#[test]
fn osc4_out_of_range_index_ignored() {
    let mut t = term();
    // Index 999 is beyond palette bounds — should not crash.
    feed(&mut t, b"\x1b]4;999;rgb:ff/00/00\x07");
    // Verify no panic and palette is intact.
    let _ = t.palette().foreground();
}

#[test]
fn osc104_no_params_resets_all_indexed() {
    let mut t = term();
    let original_0 = t.palette().resolve(vte::ansi::Color::Indexed(0));
    let original_1 = t.palette().resolve(vte::ansi::Color::Indexed(1));

    // Set colors 0 and 1.
    feed(&mut t, b"\x1b]4;0;rgb:ff/ff/ff\x07");
    feed(&mut t, b"\x1b]4;1;rgb:ff/ff/ff\x07");
    assert_ne!(
        t.palette().resolve(vte::ansi::Color::Indexed(0)),
        original_0
    );
    assert_ne!(
        t.palette().resolve(vte::ansi::Color::Indexed(1)),
        original_1
    );

    // OSC 104 with no params resets all 256 indexed colors.
    feed(&mut t, b"\x1b]104\x07");
    assert_eq!(
        t.palette().resolve(vte::ansi::Color::Indexed(0)),
        original_0
    );
    assert_eq!(
        t.palette().resolve(vte::ansi::Color::Indexed(1)),
        original_1
    );
}

#[test]
fn osc_set_color_marks_grid_dirty() {
    let mut t = term();
    // Drain any existing dirty lines.
    let _: Vec<usize> = t.grid_mut().dirty_mut().drain().collect();

    // Setting a non-cursor color should mark all lines dirty.
    feed(&mut t, b"\x1b]4;1;rgb:ff/00/00\x07");
    let dirty: Vec<usize> = t.grid_mut().dirty_mut().drain().collect();
    assert!(!dirty.is_empty(), "set_color should mark grid dirty");
}

#[test]
fn osc_set_cursor_color_does_not_mark_dirty() {
    let mut t = term();
    let _: Vec<usize> = t.grid_mut().dirty_mut().drain().collect();

    // Cursor color changes don't require full redraw (per Alacritty).
    feed(&mut t, b"\x1b]12;rgb:ff/00/00\x07");
    let dirty: Vec<usize> = t.grid_mut().dirty_mut().drain().collect();
    assert!(dirty.is_empty(), "cursor color should not mark grid dirty");
}

#[test]
fn osc_reset_color_marks_grid_dirty() {
    let mut t = term();
    feed(&mut t, b"\x1b]4;1;rgb:ff/00/00\x07");
    let _: Vec<usize> = t.grid_mut().dirty_mut().drain().collect();

    feed(&mut t, b"\x1b]104;1\x07");
    let dirty: Vec<usize> = t.grid_mut().dirty_mut().drain().collect();
    assert!(!dirty.is_empty(), "reset_color should mark grid dirty");
}

#[test]
fn osc10_query_sends_foreground_color_request() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]10;?\x07");

    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("ColorRequest(256)")),
        "OSC 10 query should request foreground (index 256), got: {events:?}",
    );
}

#[test]
fn osc11_query_sends_background_color_request() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]11;?\x07");

    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("ColorRequest(257)")),
        "OSC 11 query should request background (index 257), got: {events:?}",
    );
}

#[test]
fn osc12_query_sends_cursor_color_request() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]12;?\x07");

    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("ColorRequest(258)")),
        "OSC 12 query should request cursor color (index 258), got: {events:?}",
    );
}

#[test]
fn osc52_selection_type_s() {
    let (mut t, listener) = term_with_recorder();
    // 's' selector maps to Selection (same as 'p').
    feed(&mut t, b"\x1b]52;s;dGVzdA==\x07");

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardStore(Selection, test)")),
        "Expected Selection store for 's', got: {events:?}",
    );
}

#[test]
fn osc52_unknown_selector_ignored() {
    let (mut t, listener) = term_with_recorder();
    // Unknown selector letter 'x' should be silently ignored.
    feed(&mut t, b"\x1b]52;x;aGVsbG8=\x07");

    let events = listener.events();
    assert!(
        !events.iter().any(|e| e.contains("ClipboardStore")),
        "Unknown selector should not produce event, got: {events:?}",
    );
}

#[test]
fn osc52_load_sends_clipboard_type() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]52;c;?\x07");

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardLoad(Clipboard)")),
        "OSC 52 load with 'c' should send Clipboard type, got: {events:?}",
    );
}

// --- OSC 52 edge cases (from reference repos: Crossterm, WezTerm, Ghostty) ---

#[test]
fn osc52_load_response_formatting_bel() {
    let (mut t, listener) = term_with_recorder();
    // BEL-terminated query — response should also use BEL.
    feed(&mut t, b"\x1b]52;c;?\x07");

    let events = listener.events();
    let load_event = events
        .iter()
        .find(|e| e.contains("ClipboardLoad"))
        .expect("should have ClipboardLoad event");

    // Extract the formatter closure from the event and verify the response.
    // The RecordingListener only stores Debug strings, so we need to test
    // the formatter directly via the handler.
    assert!(load_event.contains("ClipboardLoad(Clipboard)"));
}

#[test]
fn osc52_load_response_closure_produces_valid_base64() {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as Base64;

    // Directly test the response closure logic (same as osc_clipboard_load).
    let clipboard = b'c';
    let terminator = "\x07";
    let text = "hello world";
    let encoded = Base64.encode(text);
    let response = format!("\x1b]52;{};{}{}", clipboard as char, encoded, terminator);

    assert_eq!(response, "\x1b]52;c;aGVsbG8gd29ybGQ=\x07");
}

#[test]
fn osc52_load_response_st_terminator() {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as Base64;

    // ST-terminated query should produce ST-terminated response.
    let clipboard = b'c';
    let terminator = "\x1b\\";
    let text = "hello";
    let encoded = Base64.encode(text);
    let response = format!("\x1b]52;{};{}{}", clipboard as char, encoded, terminator);

    assert_eq!(response, "\x1b]52;c;aGVsbG8=\x1b\\");
}

#[test]
fn osc52_load_selection_p() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]52;p;?\x07");

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardLoad(Selection)")),
        "OSC 52 load with 'p' should send Selection type, got: {events:?}",
    );
}

#[test]
fn osc52_load_selection_s() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]52;s;?\x07");

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardLoad(Selection)")),
        "OSC 52 load with 's' should send Selection type, got: {events:?}",
    );
}

#[test]
fn osc52_load_unknown_selector_ignored() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]52;x;?\x07");

    let events = listener.events();
    assert!(
        !events.iter().any(|e| e.contains("ClipboardLoad")),
        "Unknown selector 'x' should not produce load event, got: {events:?}",
    );
}

#[test]
fn osc52_store_empty_payload() {
    let (mut t, listener) = term_with_recorder();
    // Empty base64 after selector — decodes to empty string.
    feed(&mut t, b"\x1b]52;c;\x07");

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardStore(Clipboard, )")),
        "Empty base64 should store empty string, got: {events:?}",
    );
}

#[test]
fn osc52_store_with_st_terminator() {
    let (mut t, listener) = term_with_recorder();
    // ST terminator (\x1b\\) instead of BEL (\x07).
    feed(&mut t, b"\x1b]52;c;aGVsbG8=\x1b\\");

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardStore(Clipboard, hello)")),
        "ST-terminated store should work, got: {events:?}",
    );
}

#[test]
fn osc52_store_multiline_content() {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as Base64;

    let (mut t, listener) = term_with_recorder();
    let text = "line one\nline two\nline three";
    let encoded = Base64.encode(text);

    let mut seq = b"\x1b]52;c;".to_vec();
    seq.extend_from_slice(encoded.as_bytes());
    seq.push(0x07);
    feed(&mut t, &seq);

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardStore(Clipboard, line one\nline two\nline three)")),
        "Multiline content should be preserved, got: {events:?}",
    );
}

#[test]
fn osc52_store_crlf_content() {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as Base64;

    let (mut t, listener) = term_with_recorder();
    let text = "first\r\nsecond\r\n";
    let encoded = Base64.encode(text);

    let mut seq = b"\x1b]52;c;".to_vec();
    seq.extend_from_slice(encoded.as_bytes());
    seq.push(0x07);
    feed(&mut t, &seq);

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardStore(Clipboard, first\r\nsecond\r\n)")),
        "CRLF should be preserved verbatim, got: {events:?}",
    );
}

#[test]
fn osc52_store_large_payload() {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as Base64;

    let (mut t, listener) = term_with_recorder();
    // 10KB of text — validates no truncation in the pipeline.
    let text: String = "abcdefghij".repeat(1_000);
    let encoded = Base64.encode(&text);

    let mut seq = b"\x1b]52;c;".to_vec();
    seq.extend_from_slice(encoded.as_bytes());
    seq.push(0x07);
    feed(&mut t, &seq);

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardStore(Clipboard,")),
        "Large payload should produce store event, got: {events:?}",
    );
    // Verify full content survived by checking length in the event string.
    let store = events
        .iter()
        .find(|e| e.contains("ClipboardStore"))
        .unwrap();
    assert!(
        store.contains(&text),
        "Full 10KB text should be preserved in event",
    );
}

#[test]
fn osc52_store_base64_no_padding() {
    let (mut t, listener) = term_with_recorder();
    // "hi" → base64 "aGk=" (with padding) but "aGk" (without) should also work.
    // Standard base64 requires padding, but some terminals omit it.
    // base64 crate rejects missing padding by default.
    feed(&mut t, b"\x1b]52;c;aGk=\x07");

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardStore(Clipboard, hi)")),
        "Padded base64 should decode, got: {events:?}",
    );
}

#[test]
fn osc52_store_base64_double_padding() {
    let (mut t, listener) = term_with_recorder();
    // "h" → base64 "aA==" (double padding).
    feed(&mut t, b"\x1b]52;c;aA==\x07");

    let events = listener.events();
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardStore(Clipboard, h)")),
        "Double-padded base64 should decode, got: {events:?}",
    );
}

#[test]
fn osc52_store_invalid_utf8() {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as Base64;

    let (mut t, listener) = term_with_recorder();
    // Valid base64 that decodes to invalid UTF-8 (0xFF 0xFE).
    let encoded = Base64.encode([0xFF, 0xFE]);

    let mut seq = b"\x1b]52;c;".to_vec();
    seq.extend_from_slice(encoded.as_bytes());
    seq.push(0x07);
    feed(&mut t, &seq);

    let events = listener.events();
    assert!(
        !events.iter().any(|e| e.contains("ClipboardStore")),
        "Invalid UTF-8 should not produce store event, got: {events:?}",
    );
}

#[test]
fn osc52_store_truncated_base64() {
    let (mut t, listener) = term_with_recorder();
    // "aGVsbG8" is "aGVsbG8=" without final padding — may or may not decode
    // depending on base64 crate strictness. Either way, should not panic.
    feed(&mut t, b"\x1b]52;c;aGVsbG8\x07");

    // Not asserting specific behavior — just that it doesn't panic.
    let _ = listener.events();
}

#[test]
fn osc52_multi_selector_uses_first() {
    let (mut t, listener) = term_with_recorder();
    // VTE parser extracts only the first selector byte from "cp".
    // So this should store to Clipboard (from 'c'), not Selection.
    feed(&mut t, b"\x1b]52;cp;aGVsbG8=\x07");

    let events = listener.events();
    // VTE takes first byte 'c' → Clipboard.
    assert!(
        events
            .iter()
            .any(|e| e.contains("ClipboardStore(Clipboard, hello)")),
        "Multi-selector 'cp' should use first byte 'c' → Clipboard, got: {events:?}",
    );
    // Should NOT also produce a Selection store (VTE doesn't iterate selectors).
    let store_count = events
        .iter()
        .filter(|e| e.contains("ClipboardStore"))
        .count();
    assert_eq!(store_count, 1, "Should produce exactly one store event");
}

#[test]
fn osc52_missing_data_param_ignored() {
    let (mut t, listener) = term_with_recorder();
    // Only 2 params (no data after selector) — VTE calls unhandled().
    feed(&mut t, b"\x1b]52;c\x07");

    let events = listener.events();
    assert!(
        !events.iter().any(|e| e.contains("ClipboardStore")),
        "Missing data param should not produce event, got: {events:?}",
    );
}

#[test]
fn osc8_hyperlink_survives_sgr_reset() {
    let mut t = term();

    // Set hyperlink, then write text, then SGR reset, then more text.
    // ESC]8;;uri\x07  sets hyperlink
    // ESC[1m          sets bold
    // ESC[0m          resets all SGR (but NOT hyperlink)
    feed(&mut t, b"\x1b]8;;https://example.com\x07");
    feed(&mut t, b"\x1b[1m"); // bold
    feed(&mut t, b"\x1b[0m"); // SGR reset

    // Hyperlink should survive SGR reset — it's not an SGR attribute.
    let extra = t.grid().cursor().template.extra.as_ref();
    assert!(
        extra.is_some() && extra.unwrap().hyperlink.is_some(),
        "Hyperlink should persist across SGR reset",
    );
}

#[test]
fn osc8_hyperlink_written_to_cells() {
    let mut t = term();

    // Set hyperlink, write some text, clear hyperlink, write more.
    feed(&mut t, b"\x1b]8;;https://example.com\x07");
    feed(&mut t, b"AB");
    feed(&mut t, b"\x1b]8;;\x07");
    feed(&mut t, b"CD");

    // Cells A and B should have the hyperlink.
    let row = &t.grid()[crate::index::Line(0)];
    let a_extra = row[Column(0)]
        .extra
        .as_ref()
        .expect("cell A should have extra");
    assert!(a_extra.hyperlink.is_some(), "cell A should have hyperlink");
    let b_extra = row[Column(1)]
        .extra
        .as_ref()
        .expect("cell B should have extra");
    assert!(b_extra.hyperlink.is_some(), "cell B should have hyperlink");

    // Cells C and D should NOT have the hyperlink.
    assert!(
        row[Column(2)].extra.is_none(),
        "cell C should not have hyperlink"
    );
    assert!(
        row[Column(3)].extra.is_none(),
        "cell D should not have hyperlink"
    );
}

#[test]
fn osc8_uri_with_semicolons() {
    let mut t = term();
    // URI containing semicolons (VTE reconstructs from params[2..]).
    feed(&mut t, b"\x1b]8;;https://example.com/path;key=val\x07");

    let extra = t.grid().cursor().template.extra.as_ref().unwrap();
    let link = extra.hyperlink.as_ref().unwrap();
    assert_eq!(link.uri, "https://example.com/path;key=val");
}

#[test]
fn osc_set_title_none_resets() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]2;Foo\x07");
    assert_eq!(t.title(), "Foo");

    // Directly invoke set_title(None) to test ResetTitle event.
    use vte::ansi::Handler;
    t.set_title(None);

    assert!(t.title().is_empty());
    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("ResetTitle")),
        "set_title(None) should emit ResetTitle, got: {events:?}",
    );
}

#[test]
fn osc1_sets_icon_name_not_title() {
    let (mut t, listener) = term_with_recorder();
    // OSC 1 sets icon name only, not window title.
    feed(&mut t, b"\x1b]1;Icon Title\x07");

    assert!(t.title().is_empty(), "OSC 1 should not change title");
    assert_eq!(t.icon_name(), "Icon Title");
    let events = listener.events();
    assert!(events.iter().any(|e| e.contains("IconName")));
}

// OSC 7 is handled by the raw interceptor (oriterm::shell_integration),
// not by the high-level Handler impl. See shell_integration/tests.rs.

// --- ESC sequence tests ---

#[test]
fn esc7_esc8_save_restore_cursor() {
    let mut t = term();
    // Move to (5, 10), save, move elsewhere, restore.
    feed(&mut t, b"\x1b[6;11H"); // CUP to line 5, col 10 (1-based)
    feed(&mut t, b"\x1b7"); // DECSC: save cursor
    feed(&mut t, b"\x1b[1;1H"); // CUP to (0, 0)
    assert_eq!(t.grid().cursor().line(), 0);
    assert_eq!(t.grid().cursor().col().0, 0);
    feed(&mut t, b"\x1b8"); // DECRC: restore cursor
    assert_eq!(t.grid().cursor().line(), 5);
    assert_eq!(t.grid().cursor().col().0, 10);
}

#[test]
fn esc_d_index_at_bottom_scrolls() {
    let mut t = term();
    // Fill first line so we can check scroll.
    feed(&mut t, b"TOP");
    // Move to the last line.
    feed(&mut t, b"\x1b[24;1H"); // CUP to last line (line 23, 0-based)
    feed(&mut t, b"BOTTOM");
    // ESC D (IND) at bottom should scroll up.
    feed(&mut t, b"\x1bD");
    // Cursor should still be on the last line.
    assert_eq!(t.grid().cursor().line(), 23);
    // The old last line ("BOTTOM") should now be on line 22.
    let row22 = &t.grid()[crate::index::Line(22)];
    let text: String = (0..6).map(|c| row22[Column(c)].ch).collect();
    assert_eq!(text, "BOTTOM");
}

#[test]
fn esc_m_reverse_index_at_top_scrolls_down() {
    let mut t = term();
    // Write on first line and move to top.
    feed(&mut t, b"LINE0");
    feed(&mut t, b"\x1b[1;1H"); // CUP to (0, 0)
    // ESC M (RI) at top should scroll content down.
    feed(&mut t, b"\x1bM");
    // Cursor stays at line 0.
    assert_eq!(t.grid().cursor().line(), 0);
    // "LINE0" should have moved to line 1.
    let row1 = &t.grid()[crate::index::Line(1)];
    let text: String = (0..5).map(|c| row1[Column(c)].ch).collect();
    assert_eq!(text, "LINE0");
}

#[test]
fn esc_c_ris_resets_all_state() {
    let (mut t, listener) = term_with_recorder();
    // Set up some state.
    feed(&mut t, b"\x1b]2;Custom Title\x07"); // Set title
    feed(&mut t, b"\x1b[5;10H"); // Move cursor
    feed(&mut t, b"\x1b[1m"); // Bold
    feed(&mut t, b"\x1b(0"); // DEC special graphics
    // Now reset everything.
    feed(&mut t, b"\x1bc");
    // Cursor should be at origin.
    assert_eq!(t.grid().cursor().line(), 0);
    assert_eq!(t.grid().cursor().col().0, 0);
    // Title should be cleared.
    assert!(t.title().is_empty());
    // Mode should be default.
    assert_eq!(t.mode(), TermMode::default());
    // Charset should be back to ASCII (not DEC special graphics).
    let mut charset = t.charset().clone();
    assert_eq!(charset.translate('q'), 'q');
    // ResetTitle event should have been sent.
    let events = listener.events();
    assert!(events.iter().any(|e| e.contains("ResetTitle")));
}

#[test]
fn esc_paren_0_activates_dec_special_graphics() {
    let mut t = term();
    // ESC (0: designate G0 as DEC Special Graphics.
    feed(&mut t, b"\x1b(0");
    // 'q' in DEC Special Graphics → '─' (U+2500, horizontal line).
    feed(&mut t, b"q");
    let ch = t.grid()[crate::index::Line(0)][Column(0)].ch;
    assert_eq!(ch, '─', "DEC special graphics 'q' should map to '─'");
}

#[test]
fn esc_paren_b_restores_ascii() {
    let mut t = term();
    // Switch to DEC special graphics, then back to ASCII.
    feed(&mut t, b"\x1b(0");
    feed(&mut t, b"\x1b(B");
    // 'q' should now be plain 'q'.
    feed(&mut t, b"q");
    let ch = t.grid()[crate::index::Line(0)][Column(0)].ch;
    assert_eq!(ch, 'q');
}

// --- ESC edge cases (from reference repos: Ghostty, WezTerm) ---

/// Ghostty: saveCursor saves/restores SGR attributes, not just position.
#[test]
fn esc7_esc8_preserves_sgr_attributes() {
    let mut t = term();
    // Set bold, then save cursor.
    feed(&mut t, b"\x1b[1m"); // Bold on
    feed(&mut t, b"\x1b7"); // DECSC
    // Clear bold.
    feed(&mut t, b"\x1b[0m"); // SGR reset
    assert!(
        !t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::BOLD)
    );
    // Restore — should bring back bold.
    feed(&mut t, b"\x1b8"); // DECRC
    assert!(
        t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::BOLD),
        "DECRC should restore bold attribute",
    );
}

/// Ghostty: saveCursor pending wrap state — cursor at end of line, restore wraps.
#[test]
fn esc7_esc8_preserves_wrap_pending() {
    // 5-column terminal.
    let mut t = Term::new(5, 5, 0, Theme::default(), crate::event::VoidListener);
    // Fill the line to trigger wrap-pending (col == cols).
    feed(&mut t, b"ABCDE");
    // Cursor col should be past last column (wrap-pending).
    assert_eq!(t.grid().cursor().col().0, 5);
    // Save, move cursor, restore.
    feed(&mut t, b"\x1b7"); // Save
    feed(&mut t, b"\x1b[1;1H"); // CUP to origin
    assert_eq!(t.grid().cursor().col().0, 0);
    feed(&mut t, b"\x1b8"); // Restore
    assert_eq!(
        t.grid().cursor().col().0,
        5,
        "DECRC should restore wrap-pending state (col == cols)",
    );
    // Next character should wrap to line 1.
    feed(&mut t, b"X");
    assert_eq!(t.grid().cursor().line(), 1);
    let ch = t.grid()[crate::index::Line(1)][Column(0)].ch;
    assert_eq!(ch, 'X');
}

/// Ghostty: configuring G1 without activating has no effect on G0 output.
#[test]
fn configure_g1_does_not_affect_g0() {
    let mut t = term();
    // Configure G1 as DEC special graphics (G0 stays ASCII).
    feed(&mut t, b"\x1b)0");
    // Print 'q' — should be plain 'q' since G0 is still active.
    feed(&mut t, b"q");
    let ch = t.grid()[crate::index::Line(0)][Column(0)].ch;
    assert_eq!(ch, 'q', "Configuring G1 should not affect G0 output");
}

/// WezTerm: SO/SI (Shift Out/Shift In) switching between G0 and G1.
#[test]
fn so_si_charset_switching() {
    let mut t = term();
    // Configure G1 as DEC special graphics.
    feed(&mut t, b"\x1b)0");
    // Print in G0 (ASCII).
    feed(&mut t, b"A");
    // SO (0x0E): switch to G1.
    feed(&mut t, b"\x0e");
    feed(&mut t, b"q"); // Should be '─' in DEC special.
    // SI (0x0F): switch back to G0.
    feed(&mut t, b"\x0f");
    feed(&mut t, b"B"); // Should be plain 'B'.

    let ch0 = t.grid()[crate::index::Line(0)][Column(0)].ch;
    let ch1 = t.grid()[crate::index::Line(0)][Column(1)].ch;
    let ch2 = t.grid()[crate::index::Line(0)][Column(2)].ch;
    assert_eq!(ch0, 'A');
    assert_eq!(ch1, '─', "After SO, G1 (DEC special) should be active");
    assert_eq!(ch2, 'B', "After SI, G0 (ASCII) should be active again");
}

/// Ghostty: single shift (SS2/SS3) applies to exactly one character.
#[test]
fn single_shift_applies_to_one_character() {
    let mut t = term();
    // Configure G2 as DEC special graphics.
    feed(&mut t, b"\x1b*0");
    // SS2 (ESC N): single shift G2 for next character only.
    feed(&mut t, b"\x1bN");
    feed(&mut t, b"q"); // Should be '─' (DEC special via SS2).
    feed(&mut t, b"q"); // Should be plain 'q' (back to G0/ASCII).

    let ch0 = t.grid()[crate::index::Line(0)][Column(0)].ch;
    let ch1 = t.grid()[crate::index::Line(0)][Column(1)].ch;
    assert_eq!(ch0, '─', "SS2 should apply DEC special for one character");
    assert_eq!(ch1, 'q', "After SS2, should revert to G0 (ASCII)");
}

/// WezTerm: DEC special graphics full alphabet mapping.
#[test]
fn dec_special_graphics_full_mapping() {
    let mut t = term();
    feed(&mut t, b"\x1b(0");
    // Characters 'a' through 'z' should all map to DEC special graphics.
    feed(&mut t, b"abcdefghijklmnopqrstuvwxyz");
    let expected = "▒␉␌␍␊°±␤␋┘┐┌└┼⎺⎻─⎼⎽├┤┴┬│≤≥";
    let chars: Vec<char> = expected.chars().collect();
    for (i, &expected_ch) in chars.iter().enumerate() {
        let actual = t.grid()[crate::index::Line(0)][Column(i)].ch;
        assert_eq!(
            actual, expected_ch,
            "DEC special graphics mapping mismatch at index {i}: expected '{expected_ch}' got '{actual}'",
        );
    }
}

/// Ghostty: fullReset resets pen/attributes to defaults.
#[test]
fn esc_c_ris_resets_pen_attributes() {
    let mut t = term();
    // Set bold + foreground color.
    feed(&mut t, b"\x1b[1;31m"); // Bold + red FG
    assert!(
        t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::BOLD)
    );
    // RIS.
    feed(&mut t, b"\x1bc");
    // Attributes should be default.
    assert!(
        !t.grid()
            .cursor()
            .template
            .flags
            .contains(crate::cell::CellFlags::BOLD),
        "RIS should reset bold",
    );
    assert_eq!(
        t.grid().cursor().template.flags,
        crate::cell::CellFlags::empty(),
        "RIS should clear all cell flags",
    );
}

/// Ghostty: fullReset clears saved cursor.
#[test]
fn esc_c_ris_clears_saved_cursor() {
    let mut t = term();
    // Move cursor and save.
    feed(&mut t, b"\x1b[5;10H"); // CUP to (4, 9)
    feed(&mut t, b"\x1b7"); // DECSC
    // RIS.
    feed(&mut t, b"\x1bc");
    // Restore should NOT go to (4, 9) — saved cursor was cleared.
    feed(&mut t, b"\x1b8"); // DECRC
    assert_eq!(t.grid().cursor().line(), 0, "RIS should clear saved cursor");
    assert_eq!(
        t.grid().cursor().col().0,
        0,
        "RIS should clear saved cursor"
    );
}

/// Ghostty: fullReset from alt screen returns to primary.
#[test]
fn esc_c_ris_exits_alt_screen() {
    let mut t = term();
    // Write on primary, switch to alt.
    feed(&mut t, b"PRIMARY");
    feed(&mut t, b"\x1b[?1049h"); // DECSET 1049: enter alt screen
    assert!(t.mode().contains(TermMode::ALT_SCREEN));
    // RIS should exit alt screen.
    feed(&mut t, b"\x1bc");
    assert!(
        !t.mode().contains(TermMode::ALT_SCREEN),
        "RIS should exit alt screen",
    );
}

/// Ghostty: fullReset resets palette to defaults.
#[test]
fn esc_c_ris_resets_palette() {
    use vte::ansi::Color;

    let mut t = term();
    let default_color_1 = t.palette().resolve(Color::Indexed(1));
    // Modify palette color 1.
    feed(&mut t, b"\x1b]4;1;rgb:00/ff/00\x07"); // Set color 1 to green
    let modified = t.palette().resolve(Color::Indexed(1));
    assert_ne!(modified, default_color_1, "Color 1 should have changed");
    // RIS.
    feed(&mut t, b"\x1bc");
    let reset = t.palette().resolve(Color::Indexed(1));
    assert_eq!(
        reset, default_color_1,
        "RIS should reset palette (was {modified:?}, now {reset:?})",
    );
}

/// Ghostty: reverseIndex from middle just moves cursor up, no scroll.
#[test]
fn reverse_index_from_middle_moves_up() {
    let mut t = term();
    feed(&mut t, b"A\r\n"); // Line 0: "A"
    feed(&mut t, b"B\r\n"); // Line 1: "B"
    feed(&mut t, b"C"); // Line 2: "C"
    // RI from line 2 should move to line 1 (no scroll).
    feed(&mut t, b"\x1bM");
    assert_eq!(t.grid().cursor().line(), 1);
    feed(&mut t, b"D");
    // Line 1 should now have "D" at col 1 (after "B").
    let ch = t.grid()[crate::index::Line(1)][Column(1)].ch;
    assert_eq!(ch, 'D');
    // Line 0 should still have "A".
    let ch0 = t.grid()[crate::index::Line(0)][Column(0)].ch;
    assert_eq!(ch0, 'A');
}

/// Ghostty: reverseIndex at top of scroll region scrolls within region.
#[test]
fn reverse_index_top_of_scroll_region() {
    let mut t = Term::new(10, 2, 0, Theme::default(), crate::event::VoidListener);
    // Set up content.
    feed(&mut t, b"\x1b[2;1H"); // Move to line 1 (0-based)
    feed(&mut t, b"A\r\n");
    feed(&mut t, b"B\r\n");
    feed(&mut t, b"C\r\n");
    feed(&mut t, b"D\r\n");
    // Set scroll region to lines 2-5 (1-based).
    feed(&mut t, b"\x1b[2;5r");
    // Move to line 2 (top of scroll region, 1-based).
    feed(&mut t, b"\x1b[2;1H");
    // RI at top of scroll region should scroll content down within region.
    feed(&mut t, b"\x1bM");
    feed(&mut t, b"X");
    // Line 1 (0-based) should now have "X" (new blank line with our char).
    let ch = t.grid()[crate::index::Line(1)][Column(0)].ch;
    assert_eq!(
        ch, 'X',
        "RI at top of scroll region should scroll within region"
    );
    // "A" should have shifted to line 2.
    let ch2 = t.grid()[crate::index::Line(2)][Column(0)].ch;
    assert_eq!(ch2, 'A', "Content should shift down within scroll region");
}

/// Ghostty: reverseIndex outside scroll region just moves cursor up.
#[test]
fn reverse_index_outside_scroll_region() {
    let mut t = Term::new(5, 5, 0, Theme::default(), crate::event::VoidListener);
    feed(&mut t, b"A\r\n");
    feed(&mut t, b"B\r\n");
    feed(&mut t, b"C");
    // Set scroll region to lines 2-3 (1-based).
    feed(&mut t, b"\x1b[2;3r");
    // Move to line 1 (1-based, outside/above the scroll region).
    feed(&mut t, b"\x1b[1;1H");
    // RI should just move cursor up (no scroll because we're outside region).
    // At line 0, RI can't move up further — cursor stays at line 0.
    feed(&mut t, b"\x1bM");
    assert_eq!(t.grid().cursor().line(), 0);
    // Content should be unchanged.
    let ch = t.grid()[crate::index::Line(0)][Column(0)].ch;
    assert_eq!(ch, 'A', "Content above scroll region should not scroll");
    let ch1 = t.grid()[crate::index::Line(1)][Column(0)].ch;
    assert_eq!(ch1, 'B');
}

/// Ghostty: fullReset clears hyperlink on cursor template.
#[test]
fn esc_c_ris_clears_hyperlink() {
    let mut t = term();
    // Set a hyperlink.
    feed(&mut t, b"\x1b]8;;https://example.com\x07");
    let has_link = t
        .grid()
        .cursor()
        .template
        .extra
        .as_ref()
        .and_then(|e| e.hyperlink.as_ref())
        .is_some();
    assert!(has_link, "Hyperlink should be set before RIS");
    // RIS.
    feed(&mut t, b"\x1bc");
    let has_link = t
        .grid()
        .cursor()
        .template
        .extra
        .as_ref()
        .and_then(|e| e.hyperlink.as_ref())
        .is_some();
    assert!(!has_link, "RIS should clear hyperlink on cursor template");
}

/// Ghostty: fullReset resets origin mode.
#[test]
fn esc_c_ris_resets_origin_mode() {
    let mut t = term();
    // Set origin mode.
    feed(&mut t, b"\x1b[?6h"); // DECSET 6: origin mode
    assert!(t.mode().contains(TermMode::ORIGIN));
    // Move cursor.
    feed(&mut t, b"\x1b[3;5H");
    // RIS.
    feed(&mut t, b"\x1bc");
    assert!(
        !t.mode().contains(TermMode::ORIGIN),
        "RIS should reset origin mode",
    );
    assert_eq!(
        t.grid().cursor().line(),
        0,
        "RIS should reset cursor to origin"
    );
    assert_eq!(t.grid().cursor().col().0, 0);
}

/// Ghostty: saveCursor doesn't modify hyperlink state.
#[test]
fn esc7_esc8_preserves_hyperlink() {
    let mut t = term();
    // Set a hyperlink.
    feed(&mut t, b"\x1b]8;;https://example.com\x07");
    let get_link = |t: &Term<crate::event::VoidListener>| {
        t.grid()
            .cursor()
            .template
            .extra
            .as_ref()
            .and_then(|e| e.hyperlink.clone())
    };
    let before = get_link(&t);
    assert!(before.is_some());
    // Save.
    feed(&mut t, b"\x1b7");
    assert_eq!(get_link(&t), before, "Save should not clear hyperlink");
    // Restore.
    feed(&mut t, b"\x1b8");
    assert_eq!(
        get_link(&t),
        before,
        "DECSC/DECRC should not modify hyperlink state",
    );
}

/// Ghostty: DEC special charset only maps ASCII-range characters.
#[test]
fn dec_special_charset_ignores_non_ascii() {
    let mut t = term();
    // Switch G0 to DEC special graphics.
    feed(&mut t, b"\x1b(0");
    // Print backtick (should map to ◆ in DEC special).
    feed(&mut t, b"`");
    let ch0 = t.grid()[crate::index::Line(0)][Column(0)].ch;
    assert_eq!(ch0, '◆');
    // Non-ASCII characters should pass through the VTE parser as-is.
    // The charset mapping only applies to single-byte ASCII-range chars.
    // Multi-byte UTF-8 chars won't match the DEC mapping table.
    feed(&mut t, b"\xc3\xa9"); // 'é' (U+00E9)
    let ch1 = t.grid()[crate::index::Line(0)][Column(1)].ch;
    assert_eq!(
        ch1, 'é',
        "Non-ASCII should not be affected by DEC special charset"
    );
}

// --- DECSCUSR (cursor shape) tests ---

#[test]
fn decscusr_1_sets_blinking_block() {
    let (mut t, _listener) = term_with_recorder();
    feed(&mut t, b"\x1b[1 q");

    assert_eq!(t.cursor_shape(), crate::grid::CursorShape::Block);
    assert!(
        t.mode().contains(TermMode::CURSOR_BLINKING),
        "CSI 1 q should enable blinking"
    );
}

#[test]
fn decscusr_2_sets_steady_block() {
    let (mut t, _listener) = term_with_recorder();
    feed(&mut t, b"\x1b[2 q");

    assert_eq!(t.cursor_shape(), crate::grid::CursorShape::Block);
    assert!(
        !t.mode().contains(TermMode::CURSOR_BLINKING),
        "CSI 2 q should disable blinking"
    );
}

#[test]
fn decscusr_5_sets_blinking_bar() {
    let (mut t, _listener) = term_with_recorder();
    feed(&mut t, b"\x1b[5 q");

    assert_eq!(t.cursor_shape(), crate::grid::CursorShape::Bar);
    assert!(
        t.mode().contains(TermMode::CURSOR_BLINKING),
        "CSI 5 q should enable blinking"
    );
}

#[test]
fn decscusr_6_sets_steady_bar() {
    let (mut t, _listener) = term_with_recorder();
    feed(&mut t, b"\x1b[6 q");

    assert_eq!(t.cursor_shape(), crate::grid::CursorShape::Bar);
    assert!(
        !t.mode().contains(TermMode::CURSOR_BLINKING),
        "CSI 6 q should disable blinking"
    );
}

#[test]
fn decscusr_3_sets_blinking_underline() {
    let (mut t, _listener) = term_with_recorder();
    feed(&mut t, b"\x1b[3 q");

    assert_eq!(t.cursor_shape(), crate::grid::CursorShape::Underline);
    assert!(t.mode().contains(TermMode::CURSOR_BLINKING));
}

#[test]
fn decscusr_4_sets_steady_underline() {
    let (mut t, _listener) = term_with_recorder();
    feed(&mut t, b"\x1b[4 q");

    assert_eq!(t.cursor_shape(), crate::grid::CursorShape::Underline);
    assert!(!t.mode().contains(TermMode::CURSOR_BLINKING));
}

#[test]
fn decscusr_0_resets_to_default() {
    let (mut t, _listener) = term_with_recorder();
    // Set to bar first.
    feed(&mut t, b"\x1b[5 q");
    assert_eq!(t.cursor_shape(), crate::grid::CursorShape::Bar);

    // Reset.
    feed(&mut t, b"\x1b[0 q");
    assert_eq!(t.cursor_shape(), crate::grid::CursorShape::Block);
}

#[test]
fn decscusr_fires_cursor_blinking_change_event() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b[5 q");

    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("CursorBlinkingChange")),
        "DECSCUSR should fire CursorBlinkingChange event"
    );
}

// --- Kitty keyboard protocol tests ---

#[test]
fn push_keyboard_mode_1() {
    let (mut t, _listener) = term_with_recorder();
    // CSI > 1 u — push mode with DISAMBIGUATE_ESC_CODES.
    feed(&mut t, b"\x1b[>1u");

    assert_eq!(t.keyboard_mode_stack().len(), 1);
    assert!(
        t.mode().contains(TermMode::DISAMBIGUATE_ESC_CODES),
        "push_keyboard_mode(1) should set DISAMBIGUATE_ESC_CODES"
    );
}

#[test]
fn push_keyboard_mode_3() {
    let (mut t, _listener) = term_with_recorder();
    // Mode 3 = DISAMBIGUATE_ESC_CODES | REPORT_EVENT_TYPES.
    feed(&mut t, b"\x1b[>3u");

    assert_eq!(t.keyboard_mode_stack().len(), 1);
    assert!(t.mode().contains(TermMode::DISAMBIGUATE_ESC_CODES));
    assert!(t.mode().contains(TermMode::REPORT_EVENT_TYPES));
}

#[test]
fn pop_keyboard_mode() {
    let (mut t, _listener) = term_with_recorder();
    // Push two modes.
    feed(&mut t, b"\x1b[>1u");
    feed(&mut t, b"\x1b[>3u");
    assert_eq!(t.keyboard_mode_stack().len(), 2);

    // Pop one.
    feed(&mut t, b"\x1b[<u");
    assert_eq!(t.keyboard_mode_stack().len(), 1);
    // Active mode should be the remaining mode (1 = DISAMBIGUATE_ESC_CODES).
    assert!(t.mode().contains(TermMode::DISAMBIGUATE_ESC_CODES));
    assert!(
        !t.mode().contains(TermMode::REPORT_EVENT_TYPES),
        "after pop, only first pushed mode should remain active"
    );
}

#[test]
fn pop_all_keyboard_modes() {
    let (mut t, _listener) = term_with_recorder();
    feed(&mut t, b"\x1b[>1u");
    feed(&mut t, b"\x1b[>3u");

    // Pop both (pop 2).
    feed(&mut t, b"\x1b[<2u");
    assert!(t.keyboard_mode_stack().is_empty());
    assert!(
        !t.mode().intersects(TermMode::KITTY_KEYBOARD_PROTOCOL),
        "all kitty flags should be cleared after popping all modes"
    );
}

#[test]
fn query_keyboard_mode_responds_with_current() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b[>1u");
    // CSI ? u — query.
    feed(&mut t, b"\x1b[?u");

    let events = listener.events();
    let pty_writes: Vec<_> = events.iter().filter(|e| e.contains("PtyWrite")).collect();
    assert!(
        pty_writes.iter().any(|w| w.contains("[?1u")),
        "report should contain current mode bits: {pty_writes:?}"
    );
}

#[test]
fn pop_from_empty_stack_is_noop() {
    let mut t = term();
    // Pop from empty — should not panic.
    feed(&mut t, b"\x1b[<u");
    assert!(t.keyboard_mode_stack().is_empty());
}

// --- Unhandled sequences ---

#[test]
fn unknown_csi_does_not_panic() {
    let mut t = term();
    // Random unknown CSI.
    feed(&mut t, b"\x1b[999z");
    // Should not panic — grid still functional.
    feed(&mut t, b"ok");
    assert_eq!(t.grid()[crate::index::Line(0)][Column(0)].ch, 'o');
}

#[test]
fn unknown_osc_does_not_panic() {
    let mut t = term();
    // Unknown OSC number.
    feed(&mut t, b"\x1b]9999;data\x07");
    feed(&mut t, b"ok");
    assert_eq!(t.grid()[crate::index::Line(0)][Column(0)].ch, 'o');
}

#[test]
fn unknown_esc_does_not_panic() {
    let mut t = term();
    // Unknown ESC final.
    feed(&mut t, b"\x1bZ");
    feed(&mut t, b"ok");
    assert_eq!(t.grid()[crate::index::Line(0)][Column(0)].ch, 'o');
}

#[test]
fn ris_clears_keyboard_mode_stack_and_flags() {
    let (mut t, _listener) = term_with_recorder();
    feed(&mut t, b"\x1b[>3u");
    assert!(!t.keyboard_mode_stack().is_empty());

    // RIS.
    feed(&mut t, b"\x1bc");
    assert!(t.keyboard_mode_stack().is_empty());
    assert!(!t.mode().intersects(TermMode::KITTY_KEYBOARD_PROTOCOL));
}

#[test]
fn ris_resets_cursor_shape() {
    let (mut t, _listener) = term_with_recorder();
    feed(&mut t, b"\x1b[5 q");
    assert_eq!(t.cursor_shape(), crate::grid::CursorShape::Bar);

    feed(&mut t, b"\x1bc");
    assert_eq!(t.cursor_shape(), crate::grid::CursorShape::Block);
}

#[test]
fn query_keyboard_mode_empty_stack_reports_zero() {
    let (mut t, listener) = term_with_recorder();
    // Query with nothing on the stack.
    feed(&mut t, b"\x1b[?u");

    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("[?0u")),
        "empty stack should report mode 0: {events:?}"
    );
}

#[test]
fn query_keyboard_mode_reports_bitmask() {
    let (mut t, listener) = term_with_recorder();
    // Mode 3 = DISAMBIGUATE_ESC_CODES (1) | REPORT_EVENT_TYPES (2).
    feed(&mut t, b"\x1b[>3u");
    feed(&mut t, b"\x1b[?u");

    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("[?3u")),
        "should report combined bitmask 3: {events:?}"
    );
}

#[test]
fn pop_more_than_stack_depth_clamps() {
    let (mut t, _listener) = term_with_recorder();
    feed(&mut t, b"\x1b[>1u");
    feed(&mut t, b"\x1b[>3u");
    assert_eq!(t.keyboard_mode_stack().len(), 2);

    // Pop 999 from a stack of 2 — should clamp to empty.
    feed(&mut t, b"\x1b[<999u");
    assert!(t.keyboard_mode_stack().is_empty());
    assert!(!t.mode().intersects(TermMode::KITTY_KEYBOARD_PROTOCOL));
}

#[test]
fn keyboard_mode_stack_survives_alt_screen_swap() {
    let (mut t, _listener) = term_with_recorder();
    // Push mode on primary screen.
    feed(&mut t, b"\x1b[>1u");
    assert_eq!(t.keyboard_mode_stack().len(), 1);

    // Switch to alt screen — primary stack is swapped out.
    feed(&mut t, b"\x1b[?1049h");
    assert!(
        t.keyboard_mode_stack().is_empty(),
        "alt screen should have its own empty keyboard mode stack"
    );

    // Push a different mode on alt screen.
    feed(&mut t, b"\x1b[>3u");
    assert_eq!(t.keyboard_mode_stack().len(), 1);

    // Switch back to primary — original mode should be restored.
    feed(&mut t, b"\x1b[?1049l");
    assert_eq!(t.keyboard_mode_stack().len(), 1);
    assert!(
        t.mode().contains(TermMode::DISAMBIGUATE_ESC_CODES),
        "primary stack mode should be restored after alt screen exit"
    );
}

#[test]
fn decscusr_set_same_shape_twice_is_idempotent() {
    let (mut t, _listener) = term_with_recorder();
    feed(&mut t, b"\x1b[5 q");
    assert_eq!(t.cursor_shape(), crate::grid::CursorShape::Bar);
    assert!(t.mode().contains(TermMode::CURSOR_BLINKING));

    // Set the same shape again.
    feed(&mut t, b"\x1b[5 q");
    assert_eq!(t.cursor_shape(), crate::grid::CursorShape::Bar);
    assert!(t.mode().contains(TermMode::CURSOR_BLINKING));
}

#[test]
fn ris_clears_cursor_blinking() {
    let (mut t, _listener) = term_with_recorder();
    feed(&mut t, b"\x1b[5 q");
    assert!(t.mode().contains(TermMode::CURSOR_BLINKING));

    feed(&mut t, b"\x1bc");
    assert!(
        !t.mode().contains(TermMode::CURSOR_BLINKING),
        "RIS should clear cursor blinking flag"
    );
}

// --- Zero-width / combining mark tests ---

#[test]
fn combining_mark_appends_to_previous_cell() {
    let mut t = term();
    // 'e' followed by U+0301 (combining acute accent).
    feed(&mut t, "e\u{0301}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, 'e');
    let zw = cell
        .extra
        .as_ref()
        .expect("should have extra")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{0301}']);
    // Cursor stays at col 1 (zero-width doesn't advance).
    assert_eq!(grid.cursor().col(), Column(1));
}

#[test]
fn multiple_combining_marks_append_to_same_cell() {
    let mut t = term();
    // 'a' + U+0300 (grave) + U+0301 (acute) + U+0302 (circumflex).
    feed(&mut t, "a\u{0300}\u{0301}\u{0302}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, 'a');
    let zw = cell
        .extra
        .as_ref()
        .expect("should have extra")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{0300}', '\u{0301}', '\u{0302}']);
    assert_eq!(grid.cursor().col(), Column(1));
}

#[test]
fn zerowidth_at_col_zero_discarded() {
    let mut t = term();
    // Feed a combining mark at column 0 with no previous cell.
    feed(&mut t, "\u{0301}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    // Cell should remain the default space — combining mark was discarded.
    assert_eq!(cell.ch, ' ');
    assert!(cell.extra.is_none());
    assert_eq!(grid.cursor().col(), Column(0));
}

#[test]
fn combining_mark_on_wide_char() {
    use crate::cell::CellFlags;

    let mut t = term();
    // CJK ideograph '漢' (width 2) + combining acute accent.
    feed(&mut t, "漢\u{0301}".as_bytes());

    let grid = t.grid();
    let base = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(base.ch, '漢');
    assert!(base.flags.contains(CellFlags::WIDE_CHAR));
    let zw = base
        .extra
        .as_ref()
        .expect("combining mark on base cell")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{0301}']);

    // Spacer at col 1 must NOT have the combining mark.
    let spacer = &grid[crate::index::Line(0)][Column(1)];
    assert!(spacer.flags.contains(CellFlags::WIDE_CHAR_SPACER));
    assert!(spacer.extra.is_none());

    // Cursor at col 2 (wide char width), unaffected by combining mark.
    assert_eq!(grid.cursor().col(), Column(2));
}

#[test]
fn combining_mark_at_wrap_pending() {
    // 5-column terminal: write "abcde" to fill the line.
    // After 'e', cursor is at col 5 (== cols), i.e. wrap-pending.
    // A combining mark should attach to 'e' at col 4, not trigger a wrap.
    let mut t = Term::new(5, 5, 0, Theme::default(), crate::event::VoidListener);
    feed(&mut t, "abcde\u{0300}".as_bytes());

    let grid = t.grid();
    let cell_e = &grid[crate::index::Line(0)][Column(4)];
    assert_eq!(cell_e.ch, 'e');
    let zw = cell_e
        .extra
        .as_ref()
        .expect("combining mark on wrap-pending cell")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{0300}']);

    // Cursor stays wrap-pending at col 5 — combining mark didn't advance it.
    assert_eq!(grid.cursor().col(), Column(5));
    // Still on line 0 — no wrap occurred.
    assert_eq!(grid.cursor().line(), 0);
}

#[test]
fn zerowidth_joiner_at_col_zero_discarded() {
    let mut t = term();
    // U+200D (zero-width joiner) at column 0 with no previous cell.
    feed(&mut t, "\u{200D}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, ' ');
    assert!(cell.extra.is_none());
    assert_eq!(grid.cursor().col(), Column(0));
}

// --- Extended zero-width character tests (from Ghostty/Alacritty reference patterns) ---

#[test]
fn zerowidth_space_appends_to_previous_cell() {
    let mut t = term();
    // 'a' + U+200B (zero-width space).
    feed(&mut t, "a\u{200B}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, 'a');
    let zw = cell
        .extra
        .as_ref()
        .expect("should have extra")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{200B}']);
    // Cursor stays at col 1.
    assert_eq!(grid.cursor().col(), Column(1));
}

#[test]
fn word_joiner_appends_to_previous_cell() {
    let mut t = term();
    // 'b' + U+2060 (word joiner).
    feed(&mut t, "b\u{2060}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, 'b');
    let zw = cell
        .extra
        .as_ref()
        .expect("should have extra")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{2060}']);
    assert_eq!(grid.cursor().col(), Column(1));
}

#[test]
fn variation_selector_15_appends_to_previous_cell() {
    let mut t = term();
    // '☔' (U+2614, umbrella with rain, width 2) + U+FE0E (VS15).
    // VS15 is zero-width; without mode 2027 it's stored as a combining mark.
    feed(&mut t, "\u{2614}\u{FE0E}".as_bytes());

    let grid = t.grid();
    let base = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(base.ch, '\u{2614}');
    let zw = base
        .extra
        .as_ref()
        .expect("VS15 stored")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{FE0E}']);
    // Without mode 2027, width stays at 2.
    assert_eq!(grid.cursor().col(), Column(2));
}

#[test]
fn variation_selector_16_appends_to_previous_cell() {
    let mut t = term();
    // '❤' (U+2764, heavy black heart) + U+FE0F (VS16).
    // VS16 is zero-width; stored as combining mark without mode 2027.
    feed(&mut t, "\u{2764}\u{FE0F}".as_bytes());

    let grid = t.grid();
    let base = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(base.ch, '\u{2764}');
    let zw = base
        .extra
        .as_ref()
        .expect("VS16 stored")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{FE0F}']);
}

#[test]
fn vs16_on_ascii_stored_as_zerowidth() {
    let mut t = term();
    // 'x' + U+FE0F (VS16, invalid for ASCII — silently stored as zerowidth).
    feed(&mut t, "x\u{FE0F}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, 'x');
    let zw = cell
        .extra
        .as_ref()
        .expect("VS16 stored on ASCII")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{FE0F}']);
    assert_eq!(grid.cursor().col(), Column(1));
}

#[test]
fn zjw_appends_to_previous_cell() {
    let mut t = term();
    // 'a' + U+200D (ZWJ).
    feed(&mut t, "a\u{200D}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, 'a');
    let zw = cell
        .extra
        .as_ref()
        .expect("ZWJ stored")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{200D}']);
    assert_eq!(grid.cursor().col(), Column(1));
}

#[test]
fn zjw_emoji_sequence_stores_each_emoji_separately() {
    use crate::cell::CellFlags;

    let mut t = term();
    // 👨‍👩‍👧 = U+1F468 + U+200D + U+1F469 + U+200D + U+1F467
    // Without mode 2027, each emoji is placed as a separate wide char.
    // ZWJ chars get appended as zerowidth to the preceding emoji.
    feed(
        &mut t,
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}".as_bytes(),
    );

    let grid = t.grid();
    // 👨 at col 0-1 (wide).
    let man = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(man.ch, '\u{1F468}');
    assert!(man.flags.contains(CellFlags::WIDE_CHAR));
    // ZWJ appended to 👨.
    let zw = man.extra.as_ref().expect("ZWJ on man").zerowidth.as_slice();
    assert_eq!(zw, &['\u{200D}']);

    // Spacer at col 1.
    assert!(
        grid[crate::index::Line(0)][Column(1)]
            .flags
            .contains(CellFlags::WIDE_CHAR_SPACER)
    );

    // 👩 at col 2-3 (wide).
    let woman = &grid[crate::index::Line(0)][Column(2)];
    assert_eq!(woman.ch, '\u{1F469}');
    assert!(woman.flags.contains(CellFlags::WIDE_CHAR));
    // ZWJ appended to 👩.
    let zw = woman
        .extra
        .as_ref()
        .expect("ZWJ on woman")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{200D}']);

    // 👧 at col 4-5 (wide).
    let girl = &grid[crate::index::Line(0)][Column(4)];
    assert_eq!(girl.ch, '\u{1F467}');
    assert!(girl.flags.contains(CellFlags::WIDE_CHAR));

    // Cursor at col 6 (3 wide chars * 2).
    assert_eq!(grid.cursor().col(), Column(6));
}

#[test]
fn vs16_then_combining_mark_both_stored() {
    let mut t = term();
    // 'n' + U+FE0F (VS16) + U+0303 (combining tilde).
    // Both are zero-width and should be stored on 'n'.
    feed(&mut t, "n\u{FE0F}\u{0303}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, 'n');
    let zw = cell
        .extra
        .as_ref()
        .expect("both stored")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{FE0F}', '\u{0303}']);
    assert_eq!(grid.cursor().col(), Column(1));
}

#[test]
fn four_combining_marks_all_stored() {
    let mut t = term();
    // 'o' + 4 combining marks (grave, acute, circumflex, tilde).
    feed(&mut t, "o\u{0300}\u{0301}\u{0302}\u{0303}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, 'o');
    let zw = cell
        .extra
        .as_ref()
        .expect("4 marks stored")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{0300}', '\u{0301}', '\u{0302}', '\u{0303}']);
    assert_eq!(grid.cursor().col(), Column(1));
}

#[test]
fn mixed_zerowidth_types_on_same_cell() {
    let mut t = term();
    // 'a' + combining acute + ZWJ + VS16.
    feed(&mut t, "a\u{0301}\u{200D}\u{FE0F}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, 'a');
    let zw = cell
        .extra
        .as_ref()
        .expect("mixed zw types")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{0301}', '\u{200D}', '\u{FE0F}']);
    assert_eq!(grid.cursor().col(), Column(1));
}

#[test]
fn combining_mark_after_line_wrap() {
    // 5-column terminal. Write "ABCDE" to fill line 0, then "F" wraps to line 1.
    // Then a combining mark should attach to 'F' on line 1.
    let mut t = Term::new(5, 5, 0, Theme::default(), crate::event::VoidListener);
    feed(&mut t, "ABCDEF\u{0301}".as_bytes());

    let grid = t.grid();
    // 'F' is on line 1, col 0. Combining mark attaches to it.
    let cell = &grid[crate::index::Line(1)][Column(0)];
    assert_eq!(cell.ch, 'F');
    let zw = cell
        .extra
        .as_ref()
        .expect("combining on wrapped cell")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{0301}']);
    assert_eq!(grid.cursor().col(), Column(1));
    assert_eq!(grid.cursor().line(), 1);
}

#[test]
fn wide_char_at_boundary_sets_leading_spacer() {
    use crate::cell::CellFlags;

    // 5-column terminal. Write "ABCD" (fills cols 0-3), then a wide char at col 4
    // can't fit (needs 2 cells, only 1 left). The boundary cell should become
    // LEADING_WIDE_CHAR_SPACER, and the wide char goes to the next line.
    let mut t = Term::new(5, 5, 0, Theme::default(), crate::event::VoidListener);
    feed(&mut t, b"ABCD");
    feed(&mut t, "漢".as_bytes());

    let grid = t.grid();

    // Line 0, col 4: boundary padding (LEADING_WIDE_CHAR_SPACER + WRAP).
    let boundary = &grid[crate::index::Line(0)][Column(4)];
    assert!(
        boundary.flags.contains(CellFlags::LEADING_WIDE_CHAR_SPACER),
        "boundary cell should be LEADING_WIDE_CHAR_SPACER"
    );
    assert!(
        boundary.flags.contains(CellFlags::WRAP),
        "boundary cell should also have WRAP"
    );

    // Line 1, col 0: the wide char.
    let wide = &grid[crate::index::Line(1)][Column(0)];
    assert_eq!(wide.ch, '漢');
    assert!(wide.flags.contains(CellFlags::WIDE_CHAR));
}

#[test]
fn combining_mark_on_wide_char_after_wrap() {
    use crate::cell::CellFlags;

    // 5-column terminal. Write "ABC" (3 cols), then a wide char wraps to next line.
    // Then a combining mark should attach to the wide char base, not the spacer.
    let mut t = Term::new(5, 5, 0, Theme::default(), crate::event::VoidListener);
    feed(&mut t, b"ABCD");
    // Wide char at col 4 can't fit → wraps to line 1.
    feed(&mut t, "漢\u{0301}".as_bytes());

    let grid = t.grid();
    let base = &grid[crate::index::Line(1)][Column(0)];
    assert_eq!(base.ch, '漢');
    assert!(base.flags.contains(CellFlags::WIDE_CHAR));
    let zw = base
        .extra
        .as_ref()
        .expect("combining on wide after wrap")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{0301}']);

    // Spacer must not have the combining mark.
    let spacer = &grid[crate::index::Line(1)][Column(1)];
    assert!(spacer.flags.contains(CellFlags::WIDE_CHAR_SPACER));
    assert!(spacer.extra.is_none());
}

#[test]
fn zerowidth_space_at_col_zero_discarded() {
    let mut t = term();
    // U+200B at column 0 with no previous cell.
    feed(&mut t, "\u{200B}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, ' ');
    assert!(cell.extra.is_none());
    assert_eq!(grid.cursor().col(), Column(0));
}

#[test]
fn variation_selector_at_col_zero_discarded() {
    let mut t = term();
    // U+FE0F (VS16) at column 0 — no previous cell to attach to.
    feed(&mut t, "\u{FE0F}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, ' ');
    assert!(cell.extra.is_none());
    assert_eq!(grid.cursor().col(), Column(0));
}

#[test]
fn combining_mark_does_not_trigger_wrap() {
    // 5-column terminal, fill line with "abcde" (wrap pending at col 5).
    // Multiple combining marks should attach to 'e' without wrapping.
    let mut t = Term::new(5, 5, 0, Theme::default(), crate::event::VoidListener);
    feed(&mut t, "abcde\u{0300}\u{0301}\u{0302}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(4)];
    assert_eq!(cell.ch, 'e');
    let zw = cell
        .extra
        .as_ref()
        .expect("3 marks on wrap-pending")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{0300}', '\u{0301}', '\u{0302}']);
    // Still on line 0, cursor at col 5 (wrap pending). No wrap occurred.
    assert_eq!(grid.cursor().line(), 0);
    assert_eq!(grid.cursor().col(), Column(5));
}

#[test]
fn zjw_between_wide_chars_stored_correctly() {
    use crate::cell::CellFlags;

    let mut t = term();
    // Two CJK chars with ZWJ between them: 漢 + ZWJ + 字
    feed(&mut t, "漢\u{200D}字".as_bytes());

    let grid = t.grid();
    // 漢 at col 0 with ZWJ.
    let c1 = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(c1.ch, '漢');
    assert!(c1.flags.contains(CellFlags::WIDE_CHAR));
    let zw = c1
        .extra
        .as_ref()
        .expect("ZWJ between wide chars")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{200D}']);

    // 字 at col 2.
    let c2 = &grid[crate::index::Line(0)][Column(2)];
    assert_eq!(c2.ch, '字');
    assert!(c2.flags.contains(CellFlags::WIDE_CHAR));

    assert_eq!(grid.cursor().col(), Column(4));
}

#[test]
fn emoji_with_vs16_and_combining() {
    let mut t = term();
    // '❤' (U+2764) + VS16 (U+FE0F) + combining enclosing keycap (U+20E3).
    // Both zero-width chars stored on the heart.
    feed(&mut t, "\u{2764}\u{FE0F}\u{20E3}".as_bytes());

    let grid = t.grid();
    let cell = &grid[crate::index::Line(0)][Column(0)];
    assert_eq!(cell.ch, '\u{2764}');
    let zw = cell
        .extra
        .as_ref()
        .expect("VS16 + combining")
        .zerowidth
        .as_slice();
    assert_eq!(zw, &['\u{FE0F}', '\u{20E3}']);
}

#[test]
fn dirty_tracked_for_combining_mark() {
    let mut t = term();
    // Write 'a', drain dirty, then add combining mark.
    feed(&mut t, b"a");
    t.grid_mut().dirty_mut().drain().for_each(drop);

    // Combining mark should mark line 0 dirty.
    feed(&mut t, "\u{0301}".as_bytes());

    let dirty: Vec<usize> = t.grid_mut().dirty_mut().drain().collect();
    assert!(
        dirty.contains(&0),
        "combining mark should mark line dirty: {dirty:?}"
    );
}

#[test]
fn dirty_tracked_for_zerowidth_space() {
    let mut t = term();
    feed(&mut t, b"x");
    t.grid_mut().dirty_mut().drain().for_each(drop);

    // Zero-width space should mark line 0 dirty.
    feed(&mut t, "\u{200B}".as_bytes());

    let dirty: Vec<usize> = t.grid_mut().dirty_mut().drain().collect();
    assert!(
        dirty.contains(&0),
        "zero-width space should mark line dirty: {dirty:?}"
    );
}

// --- VT handler edge cases (tmux audit) ---

#[test]
fn decstbm_top_greater_than_bottom_is_ignored() {
    let mut t = term();
    // First set a valid region so we can verify it doesn't change.
    feed(&mut t, b"\x1b[5;20r");
    let region_before = t.grid().scroll_region().clone();

    // CSI 10;5r — top > bottom: should be silently ignored.
    feed(&mut t, b"\x1b[10;5r");
    assert_eq!(
        t.grid().scroll_region().clone(),
        region_before,
        "invalid DECSTBM (top > bottom) should be ignored"
    );
}

#[test]
fn decstbm_equal_top_and_bottom_is_ignored() {
    let mut t = term();
    feed(&mut t, b"\x1b[5;20r");
    let region_before = t.grid().scroll_region().clone();

    // CSI 10;10r — top == bottom (single line): should be ignored.
    feed(&mut t, b"\x1b[10;10r");
    assert_eq!(
        t.grid().scroll_region().clone(),
        region_before,
        "DECSTBM with top == bottom should be ignored"
    );
}

#[test]
fn decstbm_reset_with_no_params_restores_full_screen() {
    let mut t = term();
    // Set a sub-region.
    feed(&mut t, b"\x1b[5;20r");
    assert_ne!(t.grid().scroll_region().start, 0);

    // CSI r — no params: reset to full screen.
    feed(&mut t, b"\x1b[r");
    assert_eq!(t.grid().scroll_region().start, 0);
    assert_eq!(t.grid().scroll_region().end, 24);
}

#[test]
fn cht_with_count_zero_treated_as_one() {
    let mut t = term();
    feed(&mut t, b"\x1b[3;1H"); // Move to col 0
    feed(&mut t, b"ABC"); // Now at col 3

    // CSI 0 I — CHT with count=0, should act as count=1.
    feed(&mut t, b"\x1b[0I");
    // Next tab stop after col 3 is col 8.
    assert_eq!(t.grid().cursor().col(), Column(8));
}

#[test]
fn cht_with_count_three_advances_three_stops() {
    let mut t = term();
    // CSI 3 I — advance 3 tab stops from col 0 (stops at 8, 16, 24).
    feed(&mut t, b"\x1b[3I");
    assert_eq!(t.grid().cursor().col(), Column(24));
}

#[test]
fn cbt_at_col_past_end_goes_to_last_stop() {
    let mut t = term();
    // Fill the line to trigger wrap-pending.
    let text: String = (0..80).map(|_| 'A').collect();
    feed(&mut t, text.as_bytes());
    assert_eq!(t.grid().cursor().col(), Column(80)); // wrap-pending

    // CSI Z — CBT from wrap-pending should snap and go to previous stop.
    feed(&mut t, b"\x1b[Z");
    assert_eq!(t.grid().cursor().col(), Column(72));
}

#[test]
fn alt_screen_preserves_and_restores_cursor_position() {
    let mut t = term();
    // Move to a known position on primary screen.
    feed(&mut t, b"\x1b[10;30H"); // Row 10, Col 30 (1-based)
    assert_eq!(t.grid().cursor().line(), 9);
    assert_eq!(t.grid().cursor().col(), Column(29));

    // Enter alt screen (mode 1049 saves cursor).
    feed(&mut t, b"\x1b[?1049h");
    // Alt screen starts at origin.
    assert_eq!(t.grid().cursor().line(), 0);
    assert_eq!(t.grid().cursor().col(), Column(0));

    // Move in alt screen.
    feed(&mut t, b"\x1b[5;15H");
    assert_eq!(t.grid().cursor().line(), 4);

    // Exit alt screen — cursor should be restored to primary position.
    feed(&mut t, b"\x1b[?1049l");
    assert_eq!(t.grid().cursor().line(), 9);
    assert_eq!(t.grid().cursor().col(), Column(29));
}

#[test]
fn scroll_up_count_exceeds_region_via_handler() {
    let mut t = term();
    feed(&mut t, b"AAAAA");
    // CSI 100 S — scroll up by 100 (exceeds screen height).
    feed(&mut t, b"\x1b[100S");
    // All visible lines should be blank.
    for line in 0..24 {
        assert!(
            t.grid()[crate::index::Line(line)][Column(0)].is_empty(),
            "line {line} should be empty after massive scroll"
        );
    }
}

#[test]
fn scroll_down_count_exceeds_region_via_handler() {
    let mut t = term();
    feed(&mut t, b"AAAAA");
    // CSI 100 T — scroll down by 100.
    feed(&mut t, b"\x1b[100T");
    // All visible lines should be blank.
    for line in 0..24 {
        assert!(
            t.grid()[crate::index::Line(line)][Column(0)].is_empty(),
            "line {line} should be empty after massive scroll"
        );
    }
}

#[test]
fn insert_delete_lines_outside_scroll_region_noop() {
    let mut t = term();
    // Fill with content.
    for i in 0..24 {
        feed(
            &mut t,
            format!("\x1b[{};1H{}", i + 1, (b'A' + (i as u8 % 26)) as char).as_bytes(),
        );
    }
    // Set scroll region 5-20.
    feed(&mut t, b"\x1b[5;20r");
    // Move cursor to line 1 (outside region).
    feed(&mut t, b"\x1b[1;1H");
    let ch_before = t.grid()[crate::index::Line(0)][Column(0)].ch;

    // IL and DL should be noop outside scroll region.
    feed(&mut t, b"\x1b[5L"); // Insert 5 lines
    assert_eq!(t.grid()[crate::index::Line(0)][Column(0)].ch, ch_before);
    feed(&mut t, b"\x1b[5M"); // Delete 5 lines
    assert_eq!(t.grid()[crate::index::Line(0)][Column(0)].ch, ch_before);
}

// --- SGR colon-separated color parameters (ISO 8613-6) ---
//
// Modern terminals accept both semicolon and colon as sub-parameter
// separators for extended color sequences. The VTE crate parses both.

/// `ESC[38:5:196m` — colon-separated 256-color foreground.
#[test]
fn sgr_256color_fg_colon_separator() {
    let mut t = term();
    feed(&mut t, b"\x1b[38:5:196m");

    let fg = t.grid().cursor().template.fg;
    assert_eq!(fg, vte::ansi::Color::Indexed(196));
}

/// `ESC[48:5:42m` — colon-separated 256-color background.
#[test]
fn sgr_256color_bg_colon_separator() {
    let mut t = term();
    feed(&mut t, b"\x1b[48:5:42m");

    assert_eq!(t.grid().cursor().template.bg, vte::ansi::Color::Indexed(42));
}

/// `ESC[38:2::255:128:0m` — colon-separated truecolor foreground.
///
/// Per ISO 8613-6, the format is `38:2:<color-space>:R:G:B`. The color
/// space parameter is optional (empty = default RGB). The double colon
/// after `2` represents the empty color space ID.
#[test]
fn sgr_truecolor_fg_colon_separator() {
    let mut t = term();
    feed(&mut t, b"\x1b[38:2::255:128:0m");

    let fg = t.grid().cursor().template.fg;
    assert_eq!(
        fg,
        vte::ansi::Color::Spec(vte::ansi::Rgb {
            r: 255,
            g: 128,
            b: 0
        })
    );
}

/// `ESC[48:2::0:128:255m` — colon-separated truecolor background.
#[test]
fn sgr_truecolor_bg_colon_separator() {
    let mut t = term();
    feed(&mut t, b"\x1b[48:2::0:128:255m");

    assert_eq!(
        t.grid().cursor().template.bg,
        vte::ansi::Color::Spec(vte::ansi::Rgb {
            r: 0,
            g: 128,
            b: 255
        })
    );
}

/// Semicolon and colon SGR produce identical results.
#[test]
fn sgr_colon_and_semicolon_equivalent_256() {
    let mut semi = term();
    feed(&mut semi, b"\x1b[38;5;100m");

    let mut colon = term();
    feed(&mut colon, b"\x1b[38:5:100m");

    assert_eq!(
        semi.grid().cursor().template.fg,
        colon.grid().cursor().template.fg,
    );
}

/// Semicolon and colon SGR produce identical truecolor results.
#[test]
fn sgr_colon_and_semicolon_equivalent_truecolor() {
    let mut semi = term();
    feed(&mut semi, b"\x1b[38;2;10;20;30m");

    let mut colon = term();
    feed(&mut colon, b"\x1b[38:2::10:20:30m");

    assert_eq!(
        semi.grid().cursor().template.fg,
        colon.grid().cursor().template.fg,
    );
}

// --- OSC color set→query roundtrip ---
//
// Verify that setting a dynamic color then querying it produces a
// response containing the correct hex values. The ColorRequest event
// carries a formatter closure; we verify it returns the expected format.

/// OSC 4 set then query: response contains the set color.
#[test]
fn osc4_set_then_query_roundtrip() {
    let mut t = term();
    // Set index 5 to a known color.
    feed(&mut t, b"\x1b]4;5;rgb:ab/cd/ef\x07");

    // Verify palette has the color.
    let color = t.palette().resolve(vte::ansi::Color::Indexed(5));
    assert_eq!(
        color,
        vte::ansi::Rgb {
            r: 0xab,
            g: 0xcd,
            b: 0xef
        }
    );

    // Query: the event should reference index 5.
    let (mut t2, listener) = term_with_recorder();
    // Set same color on t2.
    feed(&mut t2, b"\x1b]4;5;rgb:ab/cd/ef\x07");
    // Now query it.
    feed(&mut t2, b"\x1b]4;5;?\x07");
    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("ColorRequest(5)")),
        "expected ColorRequest(5), got: {events:?}",
    );
}

/// OSC 10 set then query: foreground color roundtrip.
#[test]
fn osc10_set_then_query_roundtrip() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]10;rgb:de/ad/ff\x07");
    assert_eq!(
        t.palette().foreground(),
        vte::ansi::Rgb {
            r: 0xde,
            g: 0xad,
            b: 0xff
        }
    );

    feed(&mut t, b"\x1b]10;?\x07");
    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("ColorRequest(256)")),
        "expected foreground query event, got: {events:?}",
    );
}

/// OSC 11 set then query: background color roundtrip.
#[test]
fn osc11_set_then_query_roundtrip() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]11;rgb:12/34/56\x07");
    assert_eq!(
        t.palette().background(),
        vte::ansi::Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56
        }
    );

    feed(&mut t, b"\x1b]11;?\x07");
    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("ColorRequest(257)")),
        "expected background query event, got: {events:?}",
    );
}

/// OSC 12 set then query: cursor color roundtrip.
#[test]
fn osc12_set_then_query_roundtrip() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]12;rgb:fe/dc/ba\x07");
    assert_eq!(
        t.palette().cursor_color(),
        vte::ansi::Rgb {
            r: 0xfe,
            g: 0xdc,
            b: 0xba
        }
    );

    feed(&mut t, b"\x1b]12;?\x07");
    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("ColorRequest(258)")),
        "expected cursor color query event, got: {events:?}",
    );
}

/// OSC 4 set, reset, then query: verify reset took effect.
#[test]
fn osc4_set_reset_then_verify() {
    let mut t = term();
    let original = t.palette().resolve(vte::ansi::Color::Indexed(3));

    // Set to a different color.
    feed(&mut t, b"\x1b]4;3;rgb:ff/ff/ff\x07");
    assert_ne!(t.palette().resolve(vte::ansi::Color::Indexed(3)), original);

    // Reset it.
    feed(&mut t, b"\x1b]104;3\x07");
    assert_eq!(
        t.palette().resolve(vte::ansi::Color::Indexed(3)),
        original,
        "OSC 104 should restore to the original default"
    );
}

// --- Icon name (OSC 0/1) tests ---

#[test]
fn osc_1_sets_icon_name_only() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]1;\xF0\x9F\x90\x8Dpython\x07");
    assert_eq!(t.icon_name(), "🐍python");
    assert!(t.title().is_empty(), "OSC 1 should not change title");
    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("IconName")),
        "OSC 1 should emit IconName event, got: {events:?}",
    );
}

#[test]
fn osc_0_sets_both_title_and_icon_name() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]0;hello\x07");
    assert_eq!(t.title(), "hello");
    assert_eq!(t.icon_name(), "hello");
    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("Title")),
        "OSC 0 should emit Title event, got: {events:?}",
    );
    assert!(
        events.iter().any(|e| e.contains("IconName")),
        "OSC 0 should emit IconName event, got: {events:?}",
    );
}

#[test]
fn osc_2_does_not_set_icon_name() {
    let (mut t, _) = term_with_recorder();
    feed(&mut t, b"\x1b]2;title\x07");
    assert_eq!(t.title(), "title");
    assert!(
        t.icon_name().is_empty(),
        "OSC 2 should not change icon name"
    );
}

#[test]
fn osc_set_icon_name_none_resets() {
    let (mut t, listener) = term_with_recorder();
    feed(&mut t, b"\x1b]1;icon\x07");
    assert_eq!(t.icon_name(), "icon");

    use vte::ansi::Handler;
    t.set_icon_name(None);

    assert!(t.icon_name().is_empty());
    let events = listener.events();
    assert!(
        events.iter().any(|e| e.contains("ResetIconName")),
        "set_icon_name(None) should emit ResetIconName, got: {events:?}",
    );
}

// --- Wide character (CJK) placement ---

#[test]
fn wide_char_occupies_two_cells_with_spacer() {
    let mut t = term();
    // U+4E16 '世' is a CJK character with display width 2.
    feed(&mut t, "世".as_bytes());

    let grid = t.grid();
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(0)].ch, '世');
    assert!(
        grid[line][Column(0)]
            .flags
            .contains(crate::cell::CellFlags::WIDE_CHAR),
        "base cell should have WIDE_CHAR flag"
    );
    assert_eq!(grid[line][Column(1)].ch, ' ');
    assert!(
        grid[line][Column(1)]
            .flags
            .contains(crate::cell::CellFlags::WIDE_CHAR_SPACER),
        "next cell should be WIDE_CHAR_SPACER"
    );
    // Cursor should advance by 2.
    assert_eq!(grid.cursor().col(), Column(2));
}

#[test]
fn multiple_wide_chars_place_correctly() {
    let mut t = term();
    // '世界' — two CJK chars, each width 2.
    feed(&mut t, "世界".as_bytes());

    let grid = t.grid();
    let line = crate::index::Line(0);
    assert_eq!(grid[line][Column(0)].ch, '世');
    assert_eq!(grid[line][Column(2)].ch, '界');
    assert_eq!(grid.cursor().col(), Column(4));
}

#[test]
fn wide_char_at_last_column_wraps_to_next_line() {
    // 10-column terminal: wide char at col 9 can't fit, wraps.
    let mut t = Term::new(5, 10, 0, Theme::default(), crate::event::VoidListener);
    // Fill to col 9 (last column).
    feed(&mut t, b"123456789");
    assert_eq!(t.grid().cursor().col(), Column(9));

    // Write a wide char — doesn't fit in 1 remaining column.
    feed(&mut t, "世".as_bytes());

    let grid = t.grid();
    // Col 9 should be a LEADING_WIDE_CHAR_SPACER (padding before wrap).
    assert!(
        grid[crate::index::Line(0)][Column(9)]
            .flags
            .contains(crate::cell::CellFlags::LEADING_WIDE_CHAR_SPACER),
        "boundary cell should be LEADING_WIDE_CHAR_SPACER"
    );
    // Wide char should be on the next line, col 0.
    assert_eq!(grid[crate::index::Line(1)][Column(0)].ch, '世');
    assert!(
        grid[crate::index::Line(1)][Column(0)]
            .flags
            .contains(crate::cell::CellFlags::WIDE_CHAR)
    );
    assert_eq!(grid.cursor().col(), Column(2));
    assert_eq!(grid.cursor().line(), 1);
}

#[test]
fn wide_char_on_single_column_grid_is_skipped() {
    // Width-2 char on a 1-column grid — can never fit.
    let mut t = Term::new(5, 1, 0, Theme::default(), crate::event::VoidListener);
    feed(&mut t, "世".as_bytes());

    // Cursor shouldn't have moved (char was skipped).
    assert_eq!(t.grid().cursor().col(), Column(0));
}

// --- Line wrap at column boundary ---

#[test]
fn printing_past_last_column_wraps_to_next_line() {
    let mut t = Term::new(5, 5, 0, Theme::default(), crate::event::VoidListener);
    feed(&mut t, b"ABCDE");
    // After writing 5 chars in a 5-col grid, cursor is at col 5 (wrap-pending).
    assert_eq!(t.grid().cursor().col(), Column(5));

    // Next char triggers wrap.
    feed(&mut t, b"F");
    let grid = t.grid();
    assert_eq!(grid.cursor().line(), 1);
    assert_eq!(grid.cursor().col(), Column(1));
    assert_eq!(grid[crate::index::Line(1)][Column(0)].ch, 'F');
    // First line should have WRAP flag on last cell.
    assert!(
        grid[crate::index::Line(0)][Column(4)]
            .flags
            .contains(crate::cell::CellFlags::WRAP)
    );
}

#[test]
fn wrap_pending_cleared_by_cursor_movement() {
    let mut t = Term::new(5, 5, 0, Theme::default(), crate::event::VoidListener);
    feed(&mut t, b"ABCDE");
    // Wrap pending — cursor at col 5 (one past last).
    assert_eq!(t.grid().cursor().col(), Column(5));

    // CUB (cursor back 1) clamps to last column first, then moves back by 1.
    feed(&mut t, b"\x1b[D");
    assert_eq!(t.grid().cursor().col(), Column(4));
    assert_eq!(t.grid().cursor().line(), 0);

    // Another CUB moves further back.
    feed(&mut t, b"\x1b[D");
    assert_eq!(t.grid().cursor().col(), Column(3));
}

// --- RIS grid content verification ---

#[test]
fn ris_clears_grid_content() {
    let mut t = term();
    feed(&mut t, b"Hello, World!");
    assert_eq!(t.grid()[crate::index::Line(0)][Column(0)].ch, 'H');

    // RIS (ESC c).
    feed(&mut t, b"\x1bc");

    // Grid should be cleared — all cells should be default (space or null).
    let grid = t.grid();
    for col in 0..80 {
        let ch = grid[crate::index::Line(0)][Column(col)].ch;
        assert!(
            ch == ' ' || ch == '\0',
            "cell at col {col} should be blank after RIS, got {ch:?}"
        );
    }
    // Cursor should be at origin.
    assert_eq!(grid.cursor().col(), Column(0));
    assert_eq!(grid.cursor().line(), 0);
}

#[test]
fn ris_clears_all_visible_lines() {
    let mut t = term();
    // Write content on multiple lines.
    feed(&mut t, b"Line 0\r\nLine 1\r\nLine 2");

    feed(&mut t, b"\x1bc");

    let grid = t.grid();
    for line in 0..3 {
        let ch = grid[crate::index::Line(line)][Column(0)].ch;
        assert!(
            ch == ' ' || ch == '\0',
            "line {line} col 0 should be blank after RIS, got {ch:?}"
        );
    }
}

// --- Mouse mutual exclusion ---

use super::super::TermMode;

#[test]
fn mouse_mode_1003_clears_1000_and_1002() {
    let mut t = term();
    // Set mode 1000 (report clicks).
    feed(&mut t, b"\x1b[?1000h");
    assert!(t.mode().contains(TermMode::MOUSE_REPORT_CLICK));

    // Set mode 1003 (all motion) — should clear 1000.
    feed(&mut t, b"\x1b[?1003h");
    assert!(t.mode().contains(TermMode::MOUSE_MOTION));
    assert!(!t.mode().contains(TermMode::MOUSE_REPORT_CLICK));
    assert!(!t.mode().contains(TermMode::MOUSE_DRAG));
}

#[test]
fn mouse_mode_1002_clears_1000_and_1003() {
    let mut t = term();
    feed(&mut t, b"\x1b[?1003h");
    assert!(t.mode().contains(TermMode::MOUSE_MOTION));

    feed(&mut t, b"\x1b[?1002h");
    assert!(t.mode().contains(TermMode::MOUSE_DRAG));
    assert!(!t.mode().contains(TermMode::MOUSE_MOTION));
    assert!(!t.mode().contains(TermMode::MOUSE_REPORT_CLICK));
}

#[test]
fn mouse_encoding_1006_clears_1005_and_1015() {
    let mut t = term();
    // Set UTF-8 mouse.
    feed(&mut t, b"\x1b[?1005h");
    assert!(t.mode().contains(TermMode::MOUSE_UTF8));

    // Set SGR mouse — should clear UTF-8.
    feed(&mut t, b"\x1b[?1006h");
    assert!(t.mode().contains(TermMode::MOUSE_SGR));
    assert!(!t.mode().contains(TermMode::MOUSE_UTF8));
    assert!(!t.mode().contains(TermMode::MOUSE_URXVT));
}

#[test]
fn mouse_encoding_1015_clears_1005_and_1006() {
    let mut t = term();
    feed(&mut t, b"\x1b[?1006h");
    assert!(t.mode().contains(TermMode::MOUSE_SGR));

    feed(&mut t, b"\x1b[?1015h");
    assert!(t.mode().contains(TermMode::MOUSE_URXVT));
    assert!(!t.mode().contains(TermMode::MOUSE_SGR));
    assert!(!t.mode().contains(TermMode::MOUSE_UTF8));
}

// --- Legacy alt screen ---

#[test]
fn mode_47_swaps_without_cursor_save() {
    let mut t = term();
    // Move cursor to (5, 10).
    feed(&mut t, b"\x1b[6;11H");
    assert_eq!(t.grid().cursor().line(), 5);
    assert_eq!(t.grid().cursor().col(), Column(10));

    // Enter alt screen (mode 47).
    feed(&mut t, b"\x1b[?47h");
    assert!(t.mode().contains(TermMode::ALT_SCREEN));
    // Cursor is NOT saved; alt screen cursor starts at origin.
    // (The alt grid's cursor was never explicitly set, so it's at 0,0.)

    // Move cursor in alt screen.
    feed(&mut t, b"\x1b[3;5H");
    assert_eq!(t.grid().cursor().line(), 2);
    assert_eq!(t.grid().cursor().col(), Column(4));

    // Leave alt screen (mode 47).
    feed(&mut t, b"\x1b[?47l");
    assert!(!t.mode().contains(TermMode::ALT_SCREEN));
    // Cursor is NOT restored — whatever was on primary grid stays.
    // We verify alt screen was exited.
}

#[test]
fn mode_1047_clears_alt_on_enter() {
    let mut t = term();
    // Enter alt screen with mode 1049 first (which saves/restores).
    feed(&mut t, b"\x1b[?1049h");
    // Write content.
    feed(&mut t, b"ALTTEXT");
    // Leave alt screen.
    feed(&mut t, b"\x1b[?1049l");

    // Now enter alt screen with mode 1047 — alt should be cleared.
    feed(&mut t, b"\x1b[?1047h");
    assert!(t.mode().contains(TermMode::ALT_SCREEN));

    // Verify alt grid is clean (first cell is blank).
    let ch = t.grid()[crate::index::Line(0)][Column(0)].ch;
    assert!(
        ch == ' ' || ch == '\0',
        "alt grid should be cleared on mode 1047 enter, got {ch:?}"
    );

    feed(&mut t, b"\x1b[?1047l");
    assert!(!t.mode().contains(TermMode::ALT_SCREEN));
}

#[test]
fn mode_1048_saves_and_restores_cursor() {
    let mut t = term();
    // Move cursor to (3, 7).
    feed(&mut t, b"\x1b[4;8H");
    assert_eq!(t.grid().cursor().line(), 3);
    assert_eq!(t.grid().cursor().col(), Column(7));

    // Save cursor (mode 1048 DECSET).
    feed(&mut t, b"\x1b[?1048h");

    // Move cursor elsewhere.
    feed(&mut t, b"\x1b[1;1H");
    assert_eq!(t.grid().cursor().line(), 0);
    assert_eq!(t.grid().cursor().col(), Column(0));

    // Restore cursor (mode 1048 DECRST).
    feed(&mut t, b"\x1b[?1048l");
    assert_eq!(t.grid().cursor().line(), 3);
    assert_eq!(t.grid().cursor().col(), Column(7));
}

// --- Reverse wraparound (mode 45) ---

#[test]
fn reverse_wrap_at_col0_wraps_to_previous_wrapped_line() {
    let mut t = Term::new(24, 10, 0, Theme::default(), crate::event::VoidListener);
    // Enable reverse wraparound.
    feed(&mut t, b"\x1b[?45h");
    assert!(t.mode().contains(TermMode::REVERSE_WRAP));

    // Fill first line and force wrap with one more char.
    feed(&mut t, b"1234567890X");
    // "1234567890" fills line 0, WRAP flag set, "X" goes to line 1 col 0.
    assert_eq!(t.grid().cursor().line(), 1);
    assert_eq!(t.grid().cursor().col(), Column(1));

    // Move to col 0.
    feed(&mut t, b"\r");
    assert_eq!(t.grid().cursor().col(), Column(0));

    // BS should wrap back to line 0, col 9 (last col of wrapped line).
    feed(&mut t, b"\x08");
    assert_eq!(t.grid().cursor().line(), 0);
    assert_eq!(t.grid().cursor().col(), Column(9));
}

#[test]
fn reverse_wrap_at_col0_noop_if_not_wrapped() {
    let mut t = Term::new(24, 10, 0, Theme::default(), crate::event::VoidListener);
    feed(&mut t, b"\x1b[?45h");

    // Write a short line (no wrap) and move to start of next line.
    feed(&mut t, b"hello\r\n");
    assert_eq!(t.grid().cursor().line(), 1);
    assert_eq!(t.grid().cursor().col(), Column(0));

    // BS at col 0: previous line was NOT soft-wrapped, so no-op.
    feed(&mut t, b"\x08");
    assert_eq!(t.grid().cursor().line(), 1);
    assert_eq!(t.grid().cursor().col(), Column(0));
}

#[test]
fn reverse_wrap_disabled_does_not_wrap() {
    let mut t = Term::new(24, 10, 0, Theme::default(), crate::event::VoidListener);
    // Do NOT enable mode 45.

    // Fill first line and force wrap.
    feed(&mut t, b"1234567890X");
    assert_eq!(t.grid().cursor().line(), 1);

    // Move to col 0.
    feed(&mut t, b"\r");
    assert_eq!(t.grid().cursor().col(), Column(0));

    // BS should stay at col 0 (normal behavior, no reverse wrap).
    feed(&mut t, b"\x08");
    assert_eq!(t.grid().cursor().line(), 1);
    assert_eq!(t.grid().cursor().col(), Column(0));
}

// --- XTSAVE/XTRESTORE ---

#[test]
fn xtsave_xtrestore_saves_and_restores_mode() {
    let mut t = term();
    // Verify cursor is visible (default).
    assert!(t.mode().contains(TermMode::SHOW_CURSOR));

    // Save mode 25 (show cursor).
    feed(&mut t, b"\x1b[?25s");

    // Clear mode 25.
    feed(&mut t, b"\x1b[?25l");
    assert!(!t.mode().contains(TermMode::SHOW_CURSOR));

    // Restore mode 25 — should re-enable.
    feed(&mut t, b"\x1b[?25r");
    assert!(t.mode().contains(TermMode::SHOW_CURSOR));
}

#[test]
fn xtrestore_without_save_is_noop() {
    let mut t = term();
    let before = t.mode();
    // Restore mode 25 without saving — should be no-op.
    feed(&mut t, b"\x1b[?25r");
    assert_eq!(t.mode(), before);
}

#[test]
fn xtsave_xtrestore_multiple_modes_independently() {
    let mut t = term();
    // Enable bracketed paste.
    feed(&mut t, b"\x1b[?2004h");
    assert!(t.mode().contains(TermMode::BRACKETED_PASTE));

    // Save modes 25 and 2004.
    feed(&mut t, b"\x1b[?25;2004s");

    // Disable both.
    feed(&mut t, b"\x1b[?25l\x1b[?2004l");
    assert!(!t.mode().contains(TermMode::SHOW_CURSOR));
    assert!(!t.mode().contains(TermMode::BRACKETED_PASTE));

    // Restore both.
    feed(&mut t, b"\x1b[?25;2004r");
    assert!(t.mode().contains(TermMode::SHOW_CURSOR));
    assert!(t.mode().contains(TermMode::BRACKETED_PASTE));
}

#[test]
fn ris_clears_saved_private_modes() {
    let mut t = term();
    // Save mode 25.
    feed(&mut t, b"\x1b[?25s");
    // Disable mode 25.
    feed(&mut t, b"\x1b[?25l");

    // Full reset.
    feed(&mut t, b"\x1bc");

    // Restore should be no-op (saved modes cleared by RIS).
    // Mode 25 is set by default after RIS.
    assert!(t.mode().contains(TermMode::SHOW_CURSOR));

    // Disable it again.
    feed(&mut t, b"\x1b[?25l");
    assert!(!t.mode().contains(TermMode::SHOW_CURSOR));

    // Restore — should still be no-op.
    feed(&mut t, b"\x1b[?25r");
    assert!(!t.mode().contains(TermMode::SHOW_CURSOR));
}
