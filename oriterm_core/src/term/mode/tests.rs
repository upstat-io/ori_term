//! Tests for terminal mode flags.

use vte::ansi::KeyboardModes;

use super::TermMode;

#[test]
fn default_has_show_cursor_line_wrap_and_alternate_scroll() {
    let mode = TermMode::default();
    assert!(mode.contains(TermMode::SHOW_CURSOR));
    assert!(mode.contains(TermMode::LINE_WRAP));
    assert!(mode.contains(TermMode::ALTERNATE_SCROLL));
}

#[test]
fn default_does_not_have_other_modes() {
    let mode = TermMode::default();
    assert!(!mode.contains(TermMode::APP_CURSOR));
    assert!(!mode.contains(TermMode::ALT_SCREEN));
    assert!(!mode.contains(TermMode::BRACKETED_PASTE));
    assert!(!mode.contains(TermMode::MOUSE_REPORT_CLICK));
}

#[test]
fn set_and_clear_individual_modes() {
    let mut mode = TermMode::default();

    mode.insert(TermMode::BRACKETED_PASTE);
    assert!(mode.contains(TermMode::BRACKETED_PASTE));

    mode.remove(TermMode::BRACKETED_PASTE);
    assert!(!mode.contains(TermMode::BRACKETED_PASTE));

    // Original defaults still intact.
    assert!(mode.contains(TermMode::SHOW_CURSOR));
    assert!(mode.contains(TermMode::LINE_WRAP));
}

#[test]
fn any_mouse_is_union_of_mouse_modes() {
    let expected = TermMode::MOUSE_REPORT_CLICK
        | TermMode::MOUSE_DRAG
        | TermMode::MOUSE_MOTION
        | TermMode::MOUSE_X10;
    assert_eq!(TermMode::ANY_MOUSE, expected);
}

#[test]
fn any_mouse_detects_any_single_mouse_mode() {
    let click_only = TermMode::MOUSE_REPORT_CLICK;
    assert!(click_only.intersects(TermMode::ANY_MOUSE));

    let drag_only = TermMode::MOUSE_DRAG;
    assert!(drag_only.intersects(TermMode::ANY_MOUSE));

    let motion_only = TermMode::MOUSE_MOTION;
    assert!(motion_only.intersects(TermMode::ANY_MOUSE));

    let x10_only = TermMode::MOUSE_X10;
    assert!(x10_only.intersects(TermMode::ANY_MOUSE));
}

#[test]
fn empty_mode_has_no_mouse() {
    let mode = TermMode::empty();
    assert!(!mode.intersects(TermMode::ANY_MOUSE));
}

#[test]
fn all_flags_are_distinct() {
    let flags = [
        TermMode::SHOW_CURSOR,
        TermMode::APP_CURSOR,
        TermMode::APP_KEYPAD,
        TermMode::MOUSE_REPORT_CLICK,
        TermMode::MOUSE_DRAG,
        TermMode::MOUSE_MOTION,
        TermMode::MOUSE_SGR,
        TermMode::MOUSE_UTF8,
        TermMode::ALT_SCREEN,
        TermMode::LINE_WRAP,
        TermMode::ORIGIN,
        TermMode::INSERT,
        TermMode::FOCUS_IN_OUT,
        TermMode::BRACKETED_PASTE,
        TermMode::SYNC_UPDATE,
        TermMode::URGENCY_HINTS,
        TermMode::CURSOR_BLINKING,
        TermMode::LINE_FEED_NEW_LINE,
        TermMode::DISAMBIGUATE_ESC_CODES,
        TermMode::REPORT_EVENT_TYPES,
        TermMode::REPORT_ALTERNATE_KEYS,
        TermMode::REPORT_ALL_KEYS_AS_ESC,
        TermMode::REPORT_ASSOCIATED_TEXT,
        TermMode::ALTERNATE_SCROLL,
        TermMode::MOUSE_URXVT,
        TermMode::MOUSE_X10,
    ];

    // Each individual flag has exactly one bit set (excluding composite ANY_MOUSE).
    for flag in &flags {
        assert!(
            flag.bits().is_power_of_two(),
            "{flag:?} is not a single bit"
        );
    }
}

#[test]
fn kitty_keyboard_protocol_is_union_of_all_kitty_flags() {
    let expected = TermMode::DISAMBIGUATE_ESC_CODES
        | TermMode::REPORT_EVENT_TYPES
        | TermMode::REPORT_ALTERNATE_KEYS
        | TermMode::REPORT_ALL_KEYS_AS_ESC
        | TermMode::REPORT_ASSOCIATED_TEXT;
    assert_eq!(TermMode::KITTY_KEYBOARD_PROTOCOL, expected);
}

#[test]
fn keyboard_modes_to_term_mode_conversion() {
    let modes = KeyboardModes::DISAMBIGUATE_ESC_CODES | KeyboardModes::REPORT_EVENT_TYPES;
    let term_mode = TermMode::from(modes);

    assert!(term_mode.contains(TermMode::DISAMBIGUATE_ESC_CODES));
    assert!(term_mode.contains(TermMode::REPORT_EVENT_TYPES));
    assert!(!term_mode.contains(TermMode::REPORT_ALTERNATE_KEYS));
    assert!(!term_mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC));
    assert!(!term_mode.contains(TermMode::REPORT_ASSOCIATED_TEXT));
}

#[test]
fn keyboard_modes_no_mode_converts_to_empty() {
    let term_mode = TermMode::from(KeyboardModes::NO_MODE);
    assert!(!term_mode.intersects(TermMode::KITTY_KEYBOARD_PROTOCOL));
}

#[test]
fn any_mouse_encoding_is_union_of_encoding_modes() {
    let expected = TermMode::MOUSE_SGR | TermMode::MOUSE_UTF8 | TermMode::MOUSE_URXVT;
    assert_eq!(TermMode::ANY_MOUSE_ENCODING, expected);
}

#[test]
fn new_flags_are_distinct() {
    assert!(TermMode::REVERSE_WRAP.bits().is_power_of_two());
    assert!(TermMode::MOUSE_URXVT.bits().is_power_of_two());
    assert!(TermMode::MOUSE_X10.bits().is_power_of_two());
    assert_ne!(TermMode::REVERSE_WRAP, TermMode::MOUSE_URXVT);
    assert_ne!(TermMode::MOUSE_X10, TermMode::MOUSE_URXVT);
    assert_ne!(TermMode::MOUSE_X10, TermMode::REVERSE_WRAP);
}

#[test]
fn default_does_not_have_new_modes() {
    let mode = TermMode::default();
    assert!(!mode.contains(TermMode::REVERSE_WRAP));
    assert!(!mode.contains(TermMode::MOUSE_URXVT));
    assert!(!mode.contains(TermMode::MOUSE_X10));
}
