//! Tests for Term<T> struct.

use std::collections::VecDeque;

use vte::ansi::{KeyboardModes, Processor};

use crate::color::Rgb;
use crate::event::VoidListener;
use crate::grid::CursorShape;
use crate::index::{Column, Line};
use crate::theme::Theme;

use super::{Term, TermMode};

fn make_term() -> Term<VoidListener> {
    Term::new(24, 80, 1000, Theme::default(), VoidListener)
}

/// Feed raw bytes through the VTE processor.
fn feed(term: &mut impl vte::ansi::Handler, bytes: &[u8]) {
    let mut processor: Processor = Processor::new();
    processor.advance(term, bytes);
}

#[test]
fn new_creates_working_terminal() {
    let term = make_term();
    assert_eq!(term.grid().lines(), 24);
    assert_eq!(term.grid().cols(), 80);
}

#[test]
fn grid_returns_primary_by_default() {
    let mut term = make_term();
    // Write to primary grid.
    term.grid_mut().put_char('A');
    assert_eq!(term.grid()[Line(0)][Column(0)].ch, 'A');
    assert!(!term.mode().contains(TermMode::ALT_SCREEN));
}

#[test]
fn swap_alt_switches_to_alt_grid_and_back() {
    let mut term = make_term();
    // Write 'A' on primary.
    term.grid_mut().put_char('A');

    // Switch to alt screen.
    term.swap_alt();
    assert!(term.mode().contains(TermMode::ALT_SCREEN));

    // Alt grid should be clean.
    assert_eq!(term.grid()[Line(0)][Column(0)].ch, ' ');

    // Write 'B' on alt.
    term.grid_mut().put_char('B');

    // Switch back to primary.
    term.swap_alt();
    assert!(!term.mode().contains(TermMode::ALT_SCREEN));

    // Primary still has 'A'.
    assert_eq!(term.grid()[Line(0)][Column(0)].ch, 'A');
}

#[test]
fn mode_defaults_include_show_cursor_and_line_wrap() {
    let term = make_term();
    let mode = term.mode();
    assert!(mode.contains(TermMode::SHOW_CURSOR));
    assert!(mode.contains(TermMode::LINE_WRAP));
}

#[test]
fn default_title_is_empty() {
    let term = make_term();
    assert_eq!(term.title(), "");
}

#[test]
fn default_cursor_shape_is_block() {
    let term = make_term();
    assert_eq!(term.cursor_shape(), CursorShape::Block);
}

#[test]
fn alt_grid_has_no_scrollback() {
    let mut term = make_term();
    term.swap_alt();
    assert_eq!(term.grid().scrollback().max_scrollback(), 0);
}

#[test]
fn primary_grid_has_scrollback() {
    let term = make_term();
    assert_eq!(term.grid().scrollback().max_scrollback(), 1000);
}

#[test]
fn swap_alt_preserves_keyboard_mode_stacks() {
    let mut term = make_term();
    let mode1 = KeyboardModes::DISAMBIGUATE_ESC_CODES;
    let mode3 = KeyboardModes::DISAMBIGUATE_ESC_CODES | KeyboardModes::REPORT_EVENT_TYPES;
    term.keyboard_mode_stack.push_back(mode1);
    term.keyboard_mode_stack.push_back(mode3);

    // After swap, the active stack should be the (empty) inactive stack.
    term.swap_alt();
    assert!(term.keyboard_mode_stack.is_empty());
    assert_eq!(
        term.inactive_keyboard_mode_stack,
        VecDeque::from(vec![mode1, mode3])
    );

    // Swap back: stacks return.
    term.swap_alt();
    assert_eq!(term.keyboard_mode_stack, VecDeque::from(vec![mode1, mode3]));
    assert!(term.inactive_keyboard_mode_stack.is_empty());
}

// --- Damage tracking integration (Term::damage / Term::reset_damage) ---

/// Create a small terminal and clear initial damage.
fn damage_term() -> Term<VoidListener> {
    let mut t = Term::new(6, 10, 100, Theme::default(), VoidListener);
    t.reset_damage();
    t
}

/// Collect damaged line indices from a term.
fn damaged_lines(term: &mut Term<VoidListener>) -> Vec<usize> {
    term.damage().map(|d| d.line).collect()
}

// Basic damage semantics

#[test]
fn damage_write_char_marks_line() {
    let mut t = damage_term();
    feed(&mut t, b"X");

    let dmg: Vec<_> = t.damage().collect();
    assert!(dmg.iter().any(|d| d.line == 0));
    assert!(dmg.iter().all(|d| d.line == 0));
    assert_eq!(dmg[0].left, Column(0));
    assert_eq!(dmg[0].right, Column(9));
}

#[test]
fn damage_drain_clears_marks() {
    let mut t = damage_term();
    feed(&mut t, b"A");

    let first: Vec<_> = t.damage().collect();
    assert!(!first.is_empty(), "first drain should report damage");

    let second: Vec<_> = t.damage().collect();
    assert!(second.is_empty(), "second drain should be empty");
}

#[test]
fn damage_no_changes_empty() {
    let mut t = damage_term();
    let dmg: Vec<_> = t.damage().collect();
    assert!(dmg.is_empty());
}

#[test]
fn damage_scroll_marks_all_dirty() {
    let mut t = damage_term();
    // Push enough lines to trigger scroll in a 6-line terminal.
    feed(&mut t, b"\r\n\r\n\r\n\r\n\r\n\r\n\r\n");

    let dmg = t.damage();
    assert!(dmg.is_all_dirty());
    let lines: Vec<_> = dmg.collect();
    assert_eq!(lines.len(), 6);
}

// Cursor movement damage

#[test]
fn damage_goto_marks_old_and_new_lines() {
    let mut t = damage_term();
    // Cursor starts at (0, 0). Move to line 3, col 5.
    feed(&mut t, b"\x1b[4;6H"); // CSI 4;6 H (1-based)

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&0), "old cursor line 0 should be damaged");
    assert!(lines.contains(&3), "new cursor line 3 should be damaged");
}

#[test]
fn damage_move_forward() {
    let mut t = damage_term();
    // Move to (1, 2) then clear damage.
    feed(&mut t, b"\x1b[2;3H");
    t.reset_damage();

    // CUF: move forward 3 columns.
    feed(&mut t, b"\x1b[3C");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&1), "cursor line should be damaged");
    // Only line 1 should be damaged (same line).
    assert!(lines.iter().all(|&l| l == 1));
}

#[test]
fn damage_move_backward() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[2;8H"); // Line 2, col 8.
    t.reset_damage();

    // CUB: move backward 5 columns.
    feed(&mut t, b"\x1b[5D");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&1));
    assert!(lines.iter().all(|&l| l == 1));
}

#[test]
fn damage_move_up() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[4;1H"); // Line 4.
    t.reset_damage();

    // CUU: move up 2 lines.
    feed(&mut t, b"\x1b[2A");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&3), "old line 3 should be damaged");
    assert!(lines.contains(&1), "new line 1 should be damaged");
}

#[test]
fn damage_move_down() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[2;1H"); // Line 2.
    t.reset_damage();

    // CUD: move down 3 lines.
    feed(&mut t, b"\x1b[3B");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&1), "old line 1 should be damaged");
    assert!(lines.contains(&4), "new line 4 should be damaged");
}

#[test]
fn damage_carriage_return() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[1;6H"); // Line 1, col 6.
    t.reset_damage();

    feed(&mut t, b"\r");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&0), "CR damages cursor line");
    assert!(lines.iter().all(|&l| l == 0));
}

#[test]
fn damage_linefeed_two_lines() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[3;1H"); // Line 3.
    t.reset_damage();

    feed(&mut t, b"\n");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&2), "old line should be damaged");
    assert!(lines.contains(&3), "new line should be damaged");
}

#[test]
fn damage_backspace() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[1;5H"); // Line 1, col 5.
    t.reset_damage();

    feed(&mut t, b"\x08"); // BS.

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&0));
    assert!(lines.iter().all(|&l| l == 0));
}

#[test]
fn damage_wrapline() {
    let mut t = damage_term();
    // Fill line 0 to column 9 (last col in 10-col term).
    feed(&mut t, b"0123456789");
    t.reset_damage();

    // Next char triggers wrap to line 1.
    feed(&mut t, b"X");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&0), "wrapped-from line should be damaged");
    assert!(lines.contains(&1), "wrapped-to line should be damaged");
}

#[test]
fn damage_reverse_index_scrolls() {
    let mut t = damage_term();
    // Set scroll region top=1..bottom=4 (1-based: 1..4).
    feed(&mut t, b"\x1b[1;4r");
    // Cursor at top of scroll region (line 0).
    feed(&mut t, b"\x1b[1;1H");
    t.reset_damage();

    // RI: reverse index at top of region → scroll region down.
    feed(&mut t, b"\x1bM");

    let lines = damaged_lines(&mut t);
    // Scroll region damage covers the region (lines 0..3).
    for l in 0..4 {
        assert!(
            lines.contains(&l),
            "line {l} in scroll region should be damaged"
        );
    }
}

#[test]
fn damage_reverse_index_no_scroll() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[3;1H"); // Line 3 (not at top of region).
    t.reset_damage();

    // RI: reverse index from middle → just moves cursor up.
    feed(&mut t, b"\x1bM");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&2), "old line 2 should be damaged");
    assert!(lines.contains(&1), "new line 1 should be damaged");
}

#[test]
fn damage_tab_forward() {
    let mut t = damage_term();
    t.reset_damage();

    // HT: tab forward.
    feed(&mut t, b"\t");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&0), "tab forward should damage cursor line");
}

#[test]
fn damage_tab_backward() {
    let mut t = damage_term();
    // Move cursor to col 9.
    feed(&mut t, b"\x1b[1;10H");
    t.reset_damage();

    // CBT: cursor backward tab.
    feed(&mut t, b"\x1b[Z");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&0), "tab backward should damage cursor line");
}

#[test]
fn damage_save_restore_cursor() {
    let mut t = damage_term();
    // Move to line 2, save cursor.
    feed(&mut t, b"\x1b[3;5H"); // Line 3, col 5.
    feed(&mut t, b"\x1b7"); // DECSC: save cursor.

    // Move to line 5.
    feed(&mut t, b"\x1b[6;1H");
    t.reset_damage();

    // DECRC: restore cursor (back to line 2).
    feed(&mut t, b"\x1b8");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&5), "old cursor line 5 should be damaged");
    assert!(
        lines.contains(&2),
        "restored cursor line 2 should be damaged"
    );
}

// Erase operation damage

#[test]
fn damage_erase_chars() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[3;1H"); // Line 3.
    t.reset_damage();

    // ECH: erase 5 chars.
    feed(&mut t, b"\x1b[5X");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&2));
    assert!(lines.iter().all(|&l| l == 2));
}

#[test]
fn damage_delete_chars() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[2;3H"); // Line 2, col 3.
    t.reset_damage();

    // DCH: delete 3 chars.
    feed(&mut t, b"\x1b[3P");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&1));
    assert!(lines.iter().all(|&l| l == 1));
}

#[test]
fn damage_clear_line_all() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[4;5H"); // Line 4, col 5.
    t.reset_damage();

    // EL 2: erase entire line.
    feed(&mut t, b"\x1b[2K");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&3));
}

#[test]
fn damage_clear_line_right() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[2;5H");
    t.reset_damage();

    // EL 0: erase to right.
    feed(&mut t, b"\x1b[0K");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&1));
}

#[test]
fn damage_clear_line_left() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[3;5H");
    t.reset_damage();

    // EL 1: erase to left.
    feed(&mut t, b"\x1b[1K");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&2));
}

#[test]
fn damage_clear_screen_below() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[3;1H"); // Cursor on line 3.
    t.reset_damage();

    // ED 0: erase below (lines 2..5 in 0-based).
    feed(&mut t, b"\x1b[0J");

    let lines = damaged_lines(&mut t);
    // Lines 2 through 5 should be damaged.
    for l in 2..6 {
        assert!(lines.contains(&l), "line {l} should be damaged");
    }
}

#[test]
fn damage_clear_screen_above() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[4;1H"); // Cursor on line 4.
    t.reset_damage();

    // ED 1: erase above (lines 0..3 in 0-based).
    feed(&mut t, b"\x1b[1J");

    let lines = damaged_lines(&mut t);
    for l in 0..4 {
        assert!(lines.contains(&l), "line {l} should be damaged");
    }
}

#[test]
fn damage_clear_screen_all() {
    let mut t = damage_term();
    t.reset_damage();

    // ED 2: erase entire display.
    feed(&mut t, b"\x1b[2J");

    let dmg = t.damage();
    assert!(dmg.is_all_dirty(), "clear screen should mark all dirty");
    drop(dmg);
}

// Scroll operations

#[test]
fn damage_scroll_up_csi() {
    let mut t = damage_term();
    t.reset_damage();

    // SU: scroll up 2 lines.
    feed(&mut t, b"\x1b[2S");

    let dmg = t.damage();
    // Full-screen scroll marks all dirty via mark_range(0..lines).
    assert!(dmg.is_all_dirty());
    drop(dmg);
}

#[test]
fn damage_scroll_down_csi() {
    let mut t = damage_term();
    t.reset_damage();

    // SD: scroll down 1 line.
    feed(&mut t, b"\x1b[1T");

    let dmg = t.damage();
    assert!(dmg.is_all_dirty());
    drop(dmg);
}

#[test]
fn damage_scroll_up_in_region() {
    let mut t = damage_term();
    // Set scroll region to lines 2..5 (1-based: 2..5).
    feed(&mut t, b"\x1b[2;5r");
    t.reset_damage();

    // SU: scroll up 1.
    feed(&mut t, b"\x1b[1S");

    let lines = damaged_lines(&mut t);
    // Lines 1..4 (0-based) in the scroll region should be damaged.
    for l in 1..5 {
        assert!(
            lines.contains(&l),
            "line {l} in scroll region should be damaged"
        );
    }
    // Lines outside the region should not be damaged.
    assert!(
        !lines.contains(&0),
        "line 0 above region should not be damaged"
    );
    assert!(
        !lines.contains(&5),
        "line 5 below region should not be damaged"
    );
}

#[test]
fn damage_insert_lines() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[3;1H"); // Cursor on line 3.
    t.reset_damage();

    // IL: insert 2 blank lines at cursor.
    feed(&mut t, b"\x1b[2L");

    let lines = damaged_lines(&mut t);
    // Lines from cursor (2) through bottom of scroll region should be damaged.
    for l in 2..6 {
        assert!(
            lines.contains(&l),
            "line {l} should be damaged by insert_lines"
        );
    }
}

#[test]
fn damage_delete_lines() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[2;1H"); // Cursor on line 2.
    t.reset_damage();

    // DL: delete 1 line at cursor.
    feed(&mut t, b"\x1b[1M");

    let lines = damaged_lines(&mut t);
    // Lines from cursor (1) through bottom should be damaged.
    for l in 1..6 {
        assert!(
            lines.contains(&l),
            "line {l} should be damaged by delete_lines"
        );
    }
}

// Full damage triggers

#[test]
fn damage_swap_alt_marks_all_dirty() {
    let mut t = damage_term();

    // Enter alt screen.
    feed(&mut t, b"\x1b[?1049h");

    let dmg = t.damage();
    assert!(dmg.is_all_dirty(), "swap_alt should mark all dirty");
    drop(dmg);
}

#[test]
fn damage_swap_alt_back_marks_all_dirty() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[?1049h");
    t.reset_damage();

    // Leave alt screen.
    feed(&mut t, b"\x1b[?1049l");

    let dmg = t.damage();
    assert!(dmg.is_all_dirty(), "swap_alt back should mark all dirty");
    drop(dmg);
}

#[test]
fn damage_palette_set_color_marks_all_dirty() {
    let mut t = damage_term();

    // OSC 4;1;rgb:ff/00/00 ST — set palette index 1 to red.
    feed(&mut t, b"\x1b]4;1;rgb:ff/00/00\x1b\\");

    let dmg = t.damage();
    assert!(dmg.is_all_dirty(), "palette change should mark all dirty");
    drop(dmg);
}

#[test]
fn damage_palette_reset_color_marks_all_dirty() {
    let mut t = damage_term();

    // OSC 104;1 ST — reset palette index 1.
    feed(&mut t, b"\x1b]104;1\x1b\\");

    let dmg = t.damage();
    assert!(dmg.is_all_dirty(), "palette reset should mark all dirty");
    drop(dmg);
}

#[test]
fn damage_resize_marks_all_dirty() {
    let mut t = damage_term();

    // DirtyTracker::resize marks all dirty.
    t.grid_mut().dirty_mut().resize(8);

    let dmg = t.damage();
    assert!(dmg.is_all_dirty(), "resize should mark all dirty");
    drop(dmg);
}

#[test]
fn damage_scroll_display_marks_all_dirty() {
    let mut t = damage_term();
    // Generate some scrollback.
    feed(&mut t, b"\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n");
    t.reset_damage();

    // Scroll display back.
    t.grid_mut().scroll_display(2);

    let dmg = t.damage();
    assert!(dmg.is_all_dirty(), "scroll_display should mark all dirty");
    drop(dmg);
}

// Edge cases

#[test]
fn damage_multiple_writes_same_line_single_entry() {
    let mut t = damage_term();

    // Write several chars on line 0.
    feed(&mut t, b"ABCDE");

    let dmg: Vec<_> = t.damage().collect();
    let line0_count = dmg.iter().filter(|d| d.line == 0).count();
    assert_eq!(line0_count, 1, "same line should appear once in damage");
}

#[test]
fn damage_writes_different_lines_separate_entries() {
    let mut t = damage_term();

    // Write on line 0 then jump to line 3.
    feed(&mut t, b"A\x1b[4;1HB");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&0), "line 0 should be damaged");
    assert!(lines.contains(&3), "line 3 should be damaged");
}

#[test]
fn damage_wide_char_marks_line() {
    let mut t = damage_term();

    // Write a CJK character (width 2).
    feed(&mut t, "世".as_bytes());

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&0));
}

#[test]
fn damage_combining_mark_marks_line() {
    let mut t = damage_term();
    feed(&mut t, b"e");
    t.reset_damage();

    // Combining acute accent on existing char.
    feed(&mut t, "\u{0301}".as_bytes());

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&0), "combining mark should damage its line");
}

#[test]
fn damage_insert_blank_chars() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[2;3H"); // Line 2, col 3.
    t.reset_damage();

    // ICH: insert 2 blank chars.
    feed(&mut t, b"\x1b[2@");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&1));
}

#[test]
fn damage_newline_cr_plus_lf() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[3;5H"); // Line 3, col 5.
    t.reset_damage();

    // CR + LF.
    feed(&mut t, b"\r\n");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&2), "CR should damage line 2");
    assert!(lines.contains(&3), "LF should damage line 3");
}

#[test]
fn damage_set_scroll_region_damages_via_goto() {
    let mut t = damage_term();
    feed(&mut t, b"\x1b[3;1H"); // Start on line 3.
    t.reset_damage();

    // DECSTBM resets cursor to origin, which damages old + new lines.
    feed(&mut t, b"\x1b[2;5r");

    let lines = damaged_lines(&mut t);
    assert!(lines.contains(&0), "cursor-to-origin damages line 0");
    assert!(lines.contains(&2), "old cursor line 2 should be damaged");
}

// --- Theme integration ---

#[test]
fn new_with_dark_theme_uses_dark_palette() {
    let t = Term::new(4, 10, 0, Theme::Dark, VoidListener);
    assert_eq!(
        t.palette().foreground(),
        Rgb {
            r: 0xcc,
            g: 0xcc,
            b: 0xcc
        }
    );
    assert_eq!(
        t.palette().background(),
        Rgb {
            r: 0x00,
            g: 0x00,
            b: 0x00
        }
    );
    assert_eq!(t.theme(), Theme::Dark);
}

#[test]
fn new_with_light_theme_uses_light_palette() {
    let t = Term::new(4, 10, 0, Theme::Light, VoidListener);
    assert_eq!(
        t.palette().foreground(),
        Rgb {
            r: 0x2e,
            g: 0x34,
            b: 0x36
        }
    );
    assert_eq!(
        t.palette().background(),
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff
        }
    );
    assert_eq!(t.theme(), Theme::Light);
}

#[test]
fn set_theme_switches_palette() {
    let mut t = Term::new(4, 10, 0, Theme::Dark, VoidListener);
    t.reset_damage();

    t.set_theme(Theme::Light);

    assert_eq!(t.theme(), Theme::Light);
    assert_eq!(
        t.palette().foreground(),
        Rgb {
            r: 0x2e,
            g: 0x34,
            b: 0x36
        }
    );
    assert_eq!(
        t.palette().background(),
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff
        }
    );

    let dmg = t.damage();
    assert!(dmg.is_all_dirty(), "set_theme should mark all dirty");
    drop(dmg);
}

#[test]
fn set_theme_same_theme_is_noop() {
    let mut t = Term::new(4, 10, 0, Theme::Dark, VoidListener);
    t.reset_damage();

    t.set_theme(Theme::Dark);

    assert_eq!(t.theme(), Theme::Dark);
    let dmg: Vec<_> = t.damage().collect();
    assert!(dmg.is_empty(), "same theme should not produce damage");
}

#[test]
fn ris_resets_to_current_theme() {
    let mut t = Term::new(4, 10, 0, Theme::Light, VoidListener);
    // Verify light palette is active.
    assert_eq!(
        t.palette().background(),
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff
        }
    );

    // RIS (ESC c) should reset to the stored theme (Light), not Dark.
    feed(&mut t, b"\x1bc");

    assert_eq!(t.theme(), Theme::Light);
    assert_eq!(
        t.palette().background(),
        Rgb {
            r: 0xff,
            g: 0xff,
            b: 0xff
        }
    );
    assert_eq!(
        t.palette().foreground(),
        Rgb {
            r: 0x2e,
            g: 0x34,
            b: 0x36
        }
    );
}

// Selection dirty flag tests.

#[test]
fn selection_dirty_initially_false() {
    let term = make_term();
    assert!(!term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_character_input() {
    let mut term = make_term();
    feed(&mut term, b"A");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_cleared_by_clear_selection_dirty() {
    let mut term = make_term();
    feed(&mut term, b"A");
    assert!(term.is_selection_dirty());
    term.clear_selection_dirty();
    assert!(!term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_erase_display() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // ED 2 — erase entire display.
    feed(&mut term, b"\x1b[2J");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_erase_line() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // EL 0 — erase from cursor to end of line.
    feed(&mut term, b"\x1b[K");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_insert_blank() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // ICH — insert 1 blank character.
    feed(&mut term, b"\x1b[@");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_delete_chars() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // DCH — delete 1 character.
    feed(&mut term, b"\x1b[P");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_scroll_up() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // SU — scroll up 1 line.
    feed(&mut term, b"\x1b[S");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_scroll_down() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // SD — scroll down 1 line.
    feed(&mut term, b"\x1b[T");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_insert_lines() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // IL — insert 1 line.
    feed(&mut term, b"\x1b[L");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_delete_lines() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // DL — delete 1 line.
    feed(&mut term, b"\x1b[M");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_erase_chars() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // ECH — erase 1 character.
    feed(&mut term, b"\x1b[X");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_linefeed() {
    let mut term = make_term();
    term.clear_selection_dirty();
    feed(&mut term, b"\n");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_newline() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // NEL — next line.
    feed(&mut term, b"\x1bE");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_reverse_index() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // RI — reverse index.
    feed(&mut term, b"\x1bM");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_reset() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // RIS — full reset.
    feed(&mut term, b"\x1bc");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_swap_alt() {
    let mut term = make_term();
    term.clear_selection_dirty();
    term.swap_alt();
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_not_set_by_cursor_movement() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // CUF — move cursor forward 1.
    feed(&mut term, b"\x1b[C");
    assert!(!term.is_selection_dirty());
}

#[test]
fn selection_dirty_not_set_by_sgr() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // SGR bold.
    feed(&mut term, b"\x1b[1m");
    assert!(!term.is_selection_dirty());
}

#[test]
fn selection_dirty_not_set_by_backspace() {
    let mut term = make_term();
    term.clear_selection_dirty();
    feed(&mut term, b"\x08");
    assert!(!term.is_selection_dirty());
}

#[test]
fn selection_dirty_not_set_by_carriage_return() {
    let mut term = make_term();
    term.clear_selection_dirty();
    feed(&mut term, b"\r");
    assert!(!term.is_selection_dirty());
}

#[test]
fn selection_dirty_not_set_by_cup_goto() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // CUP — move cursor to row 5, col 10.
    feed(&mut term, b"\x1b[5;10H");
    assert!(!term.is_selection_dirty());
}

#[test]
fn selection_dirty_not_set_by_cursor_up() {
    let mut term = make_term();
    // Move cursor down first so up has room.
    feed(&mut term, b"\x1b[10B");
    term.clear_selection_dirty();
    // CUU — move cursor up.
    feed(&mut term, b"\x1b[A");
    assert!(!term.is_selection_dirty());
}

#[test]
fn selection_dirty_not_set_by_cursor_down() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // CUD — move cursor down.
    feed(&mut term, b"\x1b[B");
    assert!(!term.is_selection_dirty());
}

#[test]
fn selection_dirty_not_set_by_cursor_backward() {
    let mut term = make_term();
    // Move cursor right first so backward has room.
    feed(&mut term, b"\x1b[10C");
    term.clear_selection_dirty();
    // CUB — move cursor backward.
    feed(&mut term, b"\x1b[D");
    assert!(!term.is_selection_dirty());
}

#[test]
fn selection_dirty_not_set_by_save_restore_cursor() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // DECSC + DECRC — save then restore cursor.
    feed(&mut term, b"\x1b7\x1b8");
    assert!(!term.is_selection_dirty());
}

#[test]
fn selection_dirty_not_set_by_mode_set() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // DECSET 25 — show cursor.
    feed(&mut term, b"\x1b[?25h");
    assert!(!term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_erase_display_below() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // ED 0 — erase below cursor.
    feed(&mut term, b"\x1b[0J");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_erase_display_above() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // ED 1 — erase above cursor.
    feed(&mut term, b"\x1b[1J");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_erase_scrollback() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // ED 3 — erase scrollback.
    feed(&mut term, b"\x1b[3J");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_cleared_then_resets_on_new_output() {
    let mut term = make_term();
    feed(&mut term, b"A");
    assert!(term.is_selection_dirty());
    term.clear_selection_dirty();
    assert!(!term.is_selection_dirty());
    // New output sets the flag again.
    feed(&mut term, b"B");
    assert!(term.is_selection_dirty());
}

#[test]
fn selection_dirty_set_by_alt_screen_via_decset() {
    let mut term = make_term();
    term.clear_selection_dirty();
    // DECSET 1049 — switch to alt screen.
    feed(&mut term, b"\x1b[?1049h");
    assert!(term.is_selection_dirty());
}

// ── Term::resize integration ────────────────────────────────────────

#[test]
fn term_resize_changes_both_grids() {
    let mut term = make_term();
    assert_eq!(term.grid().lines(), 24);
    assert_eq!(term.grid().cols(), 80);

    term.resize(10, 40);

    // Primary grid (active) resized.
    assert_eq!(term.grid().lines(), 10);
    assert_eq!(term.grid().cols(), 40);

    // Switch to alt and verify it too.
    term.swap_alt();
    assert_eq!(term.grid().lines(), 10);
    assert_eq!(term.grid().cols(), 40);
}

#[test]
fn term_resize_preserves_content() {
    let mut term = make_term();
    // Write "hello" at (0,0) via VTE.
    feed(&mut term, b"hello");

    term.resize(10, 40);

    assert_eq!(term.grid()[Line(0)][Column(0)].ch, 'h');
    assert_eq!(term.grid()[Line(0)][Column(1)].ch, 'e');
    assert_eq!(term.grid()[Line(0)][Column(4)].ch, 'o');
}

#[test]
fn term_resize_marks_selection_dirty() {
    let mut term = make_term();
    term.clear_selection_dirty();

    term.resize(10, 40);

    assert!(term.is_selection_dirty());
}

#[test]
fn term_resize_marks_all_dirty() {
    let mut term = make_term();
    term.grid_mut().dirty_mut().drain().for_each(drop);

    term.resize(10, 40);

    assert!(term.grid().dirty().is_all_dirty());
}

#[test]
fn term_resize_zero_is_noop() {
    let mut term = make_term();
    term.resize(0, 40);
    assert_eq!(term.grid().lines(), 24);
    assert_eq!(term.grid().cols(), 80);

    term.resize(10, 0);
    assert_eq!(term.grid().lines(), 24);
    assert_eq!(term.grid().cols(), 80);
}

#[test]
fn term_resize_with_vte_wrapped_content() {
    let mut term = Term::new(5, 10, 100, Theme::default(), VoidListener);
    // Write 20 chars — fills 10-col row and wraps to next line via VTE handler.
    feed(&mut term, b"abcdefghijklmnopqrst");
    // Cursor should be on line 1 after wrap.
    assert_eq!(term.grid().cursor().line(), 1);

    // Grow to 20 cols — wrapped line should unwrap.
    term.resize(5, 20);

    assert_eq!(term.grid().cols(), 20);
    // Content should be on one line now.
    assert_eq!(term.grid()[Line(0)][Column(0)].ch, 'a');
    assert_eq!(term.grid()[Line(0)][Column(9)].ch, 'j');
    assert_eq!(term.grid()[Line(0)][Column(10)].ch, 'k');
    assert_eq!(term.grid()[Line(0)][Column(19)].ch, 't');
}

// --- Prompt marker tests ---

#[test]
fn mark_prompt_row_creates_marker_with_prompt_row_only() {
    let mut term = make_term();
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();

    let markers = term.prompt_markers();
    assert_eq!(markers.len(), 1);
    assert!(markers[0].command.is_none());
    assert!(markers[0].output.is_none());
}

#[test]
fn mark_command_start_fills_last_marker() {
    let mut term = make_term();
    // Mark prompt first.
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();
    // Mark command start.
    term.set_command_start_mark_pending(true);
    term.mark_command_start_row();

    let markers = term.prompt_markers();
    assert_eq!(markers.len(), 1);
    assert!(markers[0].command.is_some());
    assert!(markers[0].output.is_none());
}

#[test]
fn mark_output_start_fills_last_marker() {
    let mut term = make_term();
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();
    term.set_output_start_mark_pending(true);
    term.mark_output_start_row();

    let markers = term.prompt_markers();
    assert_eq!(markers.len(), 1);
    assert!(markers[0].output.is_some());
}

#[test]
fn mark_prompt_row_avoids_duplicates() {
    let mut term = make_term();
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();
    // Same row, should not create a duplicate.
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();

    assert_eq!(term.prompt_markers().len(), 1);
}

#[test]
fn prune_prompt_markers_removes_evicted() {
    let mut term = make_term();
    // Push 3 lines to scrollback so cursor moves.
    feed(&mut term, b"\r\n\r\n\r\n");
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();

    let row_before = term.prompt_markers()[0].prompt;
    assert!(row_before > 0, "prompt row should be > 0 after newlines");

    // Simulate evicting 1 row — all row indices shift down by 1.
    term.prune_prompt_markers(1);

    let markers = term.prompt_markers();
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].prompt, row_before - 1);
}

#[test]
fn prune_prompt_markers_removes_fully_evicted() {
    let mut term = make_term();
    // Marker at the very first row (row 0 in scrollback space).
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();

    // Evict beyond that row.
    term.prune_prompt_markers(100);

    assert!(term.prompt_markers().is_empty());
}

#[test]
fn prune_prompt_markers_adjusts_all_fields() {
    let mut term = make_term();
    // Push a few lines.
    feed(&mut term, b"\r\n\r\n\r\n\r\n\r\n");
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();
    // Move cursor down then mark B.
    feed(&mut term, b"\r\n");
    term.set_command_start_mark_pending(true);
    term.mark_command_start_row();
    // Move cursor down then mark C.
    feed(&mut term, b"\r\n");
    term.set_output_start_mark_pending(true);
    term.mark_output_start_row();

    let ps = term.prompt_markers()[0].prompt;
    let cs = term.prompt_markers()[0].command.unwrap();
    let os = term.prompt_markers()[0].output.unwrap();

    // Evict 2 rows.
    term.prune_prompt_markers(2);

    let m = &term.prompt_markers()[0];
    assert_eq!(m.prompt, ps - 2);
    assert_eq!(m.command, Some(cs - 2));
    assert_eq!(m.output, Some(os - 2));
}

#[test]
fn command_output_range_returns_correct_bounds() {
    let mut term = make_term();
    // Simulate a prompt lifecycle: A at row 0, B at row 0, C at row 1.
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();
    term.set_command_start_mark_pending(true);
    term.mark_command_start_row();
    feed(&mut term, b"\r\n");
    term.set_output_start_mark_pending(true);
    term.mark_output_start_row();

    let output_start = term.prompt_markers()[0].output.unwrap();
    let range = term.command_output_range(0);
    assert!(range.is_some());
    let (start, end) = range.unwrap();
    assert_eq!(start, output_start);
    // Last marker: end should be the cursor row.
    let cursor_row = term.grid().scrollback().len() + term.grid().cursor().line();
    assert_eq!(end, cursor_row);
}

#[test]
fn command_output_range_bounded_by_next_prompt() {
    let mut term = make_term();
    // First prompt lifecycle.
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();
    term.set_command_start_mark_pending(true);
    term.mark_command_start_row();
    feed(&mut term, b"\r\n");
    term.set_output_start_mark_pending(true);
    term.mark_output_start_row();

    // Push more lines and create a second prompt.
    feed(&mut term, b"\r\n\r\n\r\n");
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();

    let second_prompt_start = term.prompt_markers()[1].prompt;
    let range = term.command_output_range(0).unwrap();
    assert_eq!(range.1, second_prompt_start - 1);
}

#[test]
fn command_input_range_returns_correct_bounds() {
    let mut term = make_term();
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();
    term.set_command_start_mark_pending(true);
    term.mark_command_start_row();
    feed(&mut term, b"\r\n");
    term.set_output_start_mark_pending(true);
    term.mark_output_start_row();

    let cmd_start = term.prompt_markers()[0].command.unwrap();
    let output_start = term.prompt_markers()[0].output.unwrap();
    let range = term.command_input_range(0);
    assert!(range.is_some());
    let (start, end) = range.unwrap();
    assert_eq!(start, cmd_start);
    assert_eq!(end, output_start - 1);
}

#[test]
fn range_returns_none_when_no_markers() {
    let term = make_term();
    assert!(term.command_output_range(0).is_none());
    assert!(term.command_input_range(0).is_none());
}

#[test]
fn range_returns_none_when_output_start_missing() {
    let mut term = make_term();
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();
    // No C marker.
    assert!(term.command_output_range(0).is_none());
}

#[test]
fn range_returns_none_when_command_start_missing() {
    let mut term = make_term();
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();
    // No B marker.
    assert!(term.command_input_range(0).is_none());
}

#[test]
fn scroll_to_previous_prompt_scrolls_viewport() {
    let mut term = Term::new(10, 80, 1000, Theme::default(), VoidListener);
    // Fill scrollback: 30 lines.
    for _ in 0..30 {
        feed(&mut term, b"\r\n");
    }
    // Mark a prompt partway through.
    // We need to go back and mark at a known position.
    // Instead, mark at current position (row 30).
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();

    // Push more lines.
    for _ in 0..20 {
        feed(&mut term, b"\r\n");
    }

    // Viewport is at bottom. scroll_to_previous_prompt should scroll.
    let scrolled = term.scroll_to_previous_prompt();
    assert!(scrolled);
    assert!(term.grid().display_offset() > 0);
}

#[test]
fn scroll_to_next_prompt_scrolls_viewport() {
    let mut term = Term::new(10, 80, 1000, Theme::default(), VoidListener);
    // Fill scrollback: 30 lines.
    for _ in 0..30 {
        feed(&mut term, b"\r\n");
    }
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();

    // Scroll all the way back.
    term.grid_mut().scroll_display(isize::MAX);

    // Now scroll_to_next_prompt should find the marker.
    let scrolled = term.scroll_to_next_prompt();
    assert!(scrolled);
}

// --- RIS clears shell integration state ---

#[test]
fn ris_clears_prompt_state() {
    let mut term = make_term();
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();
    term.set_command_start_mark_pending(true);
    term.mark_command_start_row();

    assert_eq!(term.prompt_markers().len(), 1);
    assert_eq!(term.prompt_state(), super::PromptState::None);

    // Manually set prompt state to simulate mid-cycle.
    *term.prompt_state_mut() = super::PromptState::CommandStart;

    // RIS should clear everything.
    feed(&mut term, b"\x1bc");

    assert_eq!(term.prompt_state(), super::PromptState::None);
    assert!(term.prompt_markers().is_empty());
    assert!(!term.prompt_mark_pending());
    assert!(!term.command_start_mark_pending());
    assert!(!term.output_start_mark_pending());
}

#[test]
fn ris_clears_cwd_and_title_state() {
    let mut term = make_term();
    *term.cwd_mut() = Some("/home/user".to_string());
    term.set_has_explicit_title(true);
    term.mark_title_dirty();

    feed(&mut term, b"\x1bc");

    assert!(term.cwd().is_none());
    assert!(!term.has_explicit_title());
    assert_eq!(term.effective_title(), "");
}

#[test]
fn ris_clears_command_timing() {
    let mut term = make_term();
    term.set_command_start(std::time::Instant::now());
    let _ = term.finish_command();

    // Verify we had a duration.
    assert!(term.last_command_duration().is_some());

    feed(&mut term, b"\x1bc");

    assert!(term.last_command_duration().is_none());
}

#[test]
fn ris_clears_pending_notifications() {
    let mut term = make_term();
    term.push_notification(super::Notification {
        title: "Build".to_string(),
        body: "Done".to_string(),
    });

    assert_eq!(term.drain_notifications().len(), 1);

    // Push another.
    term.push_notification(super::Notification {
        title: "Test".to_string(),
        body: "Pass".to_string(),
    });

    feed(&mut term, b"\x1bc");

    assert!(term.drain_notifications().is_empty());
}

// --- Drain notifications idempotent ---

#[test]
fn drain_notifications_returns_empty_on_second_call() {
    let mut term = make_term();
    term.push_notification(super::Notification {
        title: String::new(),
        body: "hello".to_string(),
    });

    let first = term.drain_notifications();
    assert_eq!(first.len(), 1);

    let second = term.drain_notifications();
    assert!(second.is_empty(), "second drain should return empty");
}

// --- Multiple sequential OSC 133;A ---

#[test]
fn multiple_prompt_starts_without_completion_create_separate_markers() {
    let mut term = make_term();

    // First prompt at row 0.
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();

    // Move cursor down.
    feed(&mut term, b"\r\n\r\n");

    // Second prompt at a different row (simulates Ctrl-C re-prompt).
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();

    // Should have two separate markers.
    assert_eq!(term.prompt_markers().len(), 2);
    assert!(term.prompt_markers()[0].prompt < term.prompt_markers()[1].prompt);
    // First marker has no command/output (incomplete).
    assert!(term.prompt_markers()[0].command.is_none());
    assert!(term.prompt_markers()[0].output.is_none());
}

// --- Prompt marker at scrollback boundary ---

#[test]
fn prune_prompt_markers_zero_eviction_is_noop() {
    let mut term = make_term();
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();

    let before = term.prompt_markers().len();
    term.prune_prompt_markers(0);
    assert_eq!(term.prompt_markers().len(), before);
}

#[test]
fn prune_prompt_markers_exact_boundary() {
    let mut term = make_term();
    // Marker at row 0.
    term.set_prompt_mark_pending(true);
    term.mark_prompt_row();

    // Evict exactly 1 row — marker at row 0 is below threshold (0 < 1).
    term.prune_prompt_markers(1);
    assert!(
        term.prompt_markers().is_empty(),
        "marker at row 0 should be evicted when evicted=1"
    );
}
