//! Tests for Term<T> struct.

use vte::ansi::KeyboardModes;

use crate::event::VoidListener;
use crate::grid::CursorShape;

use super::{Term, TermMode};

fn make_term() -> Term<VoidListener> {
    Term::new(24, 80, 1000, VoidListener)
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
    assert_eq!(term.grid()[crate::index::Line(0)][crate::index::Column(0)].ch, 'A');
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
    assert_eq!(term.grid()[crate::index::Line(0)][crate::index::Column(0)].ch, ' ');

    // Write 'B' on alt.
    term.grid_mut().put_char('B');

    // Switch back to primary.
    term.swap_alt();
    assert!(!term.mode().contains(TermMode::ALT_SCREEN));

    // Primary still has 'A'.
    assert_eq!(term.grid()[crate::index::Line(0)][crate::index::Column(0)].ch, 'A');
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
    term.keyboard_mode_stack.push(mode1);
    term.keyboard_mode_stack.push(mode3);

    // After swap, the active stack should be the (empty) inactive stack.
    term.swap_alt();
    assert!(term.keyboard_mode_stack.is_empty());
    assert_eq!(term.inactive_keyboard_mode_stack, vec![mode1, mode3]);

    // Swap back: stacks return.
    term.swap_alt();
    assert_eq!(term.keyboard_mode_stack, vec![mode1, mode3]);
    assert!(term.inactive_keyboard_mode_stack.is_empty());
}
