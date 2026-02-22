//! Tests for key event encoding (legacy xterm + Kitty protocol dispatcher).

use winit::keyboard::{Key, KeyLocation, NamedKey};

use oriterm_core::TermMode;

use super::{KeyEventType, KeyInput, Modifiers, encode_key};

fn no_mode() -> TermMode {
    TermMode::default()
}

fn app_cursor_mode() -> TermMode {
    TermMode::default() | TermMode::APP_CURSOR
}

fn app_keypad_mode() -> TermMode {
    TermMode::default() | TermMode::APP_KEYPAD
}

/// Encode a key press at standard location with no text.
fn enc(key: Key, mods: Modifiers, mode: TermMode) -> Vec<u8> {
    encode_key(&KeyInput {
        key: &key,
        mods,
        mode,
        text: None,
        location: KeyLocation::Standard,
        event_type: KeyEventType::Press,
    })
}

/// Encode a key press at standard location with text.
fn enc_text(key: Key, mods: Modifiers, mode: TermMode, text: &str) -> Vec<u8> {
    encode_key(&KeyInput {
        key: &key,
        mods,
        mode,
        text: Some(text),
        location: KeyLocation::Standard,
        event_type: KeyEventType::Press,
    })
}

/// Encode a key press at numpad location.
fn enc_numpad(key: Key, mods: Modifiers, mode: TermMode) -> Vec<u8> {
    encode_key(&KeyInput {
        key: &key,
        mods,
        mode,
        text: None,
        location: KeyLocation::Numpad,
        event_type: KeyEventType::Press,
    })
}

/// Encode a key release at standard location.
fn enc_release(key: Key, mods: Modifiers, mode: TermMode) -> Vec<u8> {
    encode_key(&KeyInput {
        key: &key,
        mods,
        mode,
        text: None,
        location: KeyLocation::Standard,
        event_type: KeyEventType::Release,
    })
}

// --- Ctrl+letter C0 codes ---

#[test]
fn ctrl_a() {
    let r = enc(Key::Character("a".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x01]);
}

#[test]
fn ctrl_c() {
    let r = enc(Key::Character("c".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x03]);
}

#[test]
fn ctrl_d() {
    let r = enc(Key::Character("d".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x04]);
}

#[test]
fn ctrl_z() {
    let r = enc(Key::Character("z".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x1a]);
}

#[test]
fn ctrl_a_uppercase() {
    let r = enc(Key::Character("A".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x01]);
}

#[test]
fn ctrl_space() {
    let r = enc(Key::Named(NamedKey::Space), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x00]);
}

#[test]
fn ctrl_bracket_esc() {
    let r = enc(Key::Character("[".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x1b]);
}

#[test]
fn ctrl_backslash() {
    let r = enc(Key::Character("\\".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x1c]);
}

#[test]
fn ctrl_close_bracket() {
    let r = enc(Key::Character("]".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x1d]);
}

// --- Alt prefix ---

#[test]
fn alt_a() {
    let r = enc_text(Key::Character("a".into()), Modifiers::ALT, no_mode(), "a");
    assert_eq!(r, vec![0x1b, b'a']);
}

#[test]
fn alt_ctrl_a() {
    let r = enc(
        Key::Character("a".into()),
        Modifiers::ALT | Modifiers::CONTROL,
        no_mode(),
    );
    assert_eq!(r, vec![0x1b, 0x01]);
}

#[test]
fn alt_space() {
    let r = enc(Key::Named(NamedKey::Space), Modifiers::ALT, no_mode());
    assert_eq!(r, vec![0x1b, b' ']);
}

#[test]
fn alt_ctrl_space() {
    let r = enc(
        Key::Named(NamedKey::Space),
        Modifiers::ALT | Modifiers::CONTROL,
        no_mode(),
    );
    assert_eq!(r, vec![0x1b, 0x00]);
}

// --- Modifier-encoded named keys ---

#[test]
fn ctrl_up() {
    let r = enc(Key::Named(NamedKey::ArrowUp), Modifiers::CONTROL, no_mode());
    assert_eq!(r, b"\x1b[1;5A");
}

#[test]
fn shift_right() {
    let r = enc(
        Key::Named(NamedKey::ArrowRight),
        Modifiers::SHIFT,
        no_mode(),
    );
    assert_eq!(r, b"\x1b[1;2C");
}

#[test]
fn ctrl_shift_left() {
    let r = enc(
        Key::Named(NamedKey::ArrowLeft),
        Modifiers::CONTROL | Modifiers::SHIFT,
        no_mode(),
    );
    assert_eq!(r, b"\x1b[1;6D");
}

#[test]
fn ctrl_f5() {
    let r = enc(Key::Named(NamedKey::F5), Modifiers::CONTROL, no_mode());
    assert_eq!(r, b"\x1b[15;5~");
}

#[test]
fn shift_f1() {
    let r = enc(Key::Named(NamedKey::F1), Modifiers::SHIFT, no_mode());
    assert_eq!(r, b"\x1b[1;2P");
}

#[test]
fn ctrl_delete() {
    let r = enc(Key::Named(NamedKey::Delete), Modifiers::CONTROL, no_mode());
    assert_eq!(r, b"\x1b[3;5~");
}

#[test]
fn ctrl_page_up() {
    let r = enc(Key::Named(NamedKey::PageUp), Modifiers::CONTROL, no_mode());
    assert_eq!(r, b"\x1b[5;5~");
}

#[test]
fn shift_f5() {
    let r = enc(Key::Named(NamedKey::F5), Modifiers::SHIFT, no_mode());
    assert_eq!(r, b"\x1b[15;2~");
}

// --- Application cursor mode ---

#[test]
fn app_cursor_up_no_mods() {
    let r = enc(
        Key::Named(NamedKey::ArrowUp),
        Modifiers::empty(),
        app_cursor_mode(),
    );
    assert_eq!(r, b"\x1bOA");
}

#[test]
fn app_cursor_down_no_mods() {
    let r = enc(
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
        app_cursor_mode(),
    );
    assert_eq!(r, b"\x1bOB");
}

#[test]
fn app_cursor_home_no_mods() {
    let r = enc(
        Key::Named(NamedKey::Home),
        Modifiers::empty(),
        app_cursor_mode(),
    );
    assert_eq!(r, b"\x1bOH");
}

#[test]
fn app_cursor_end_no_mods() {
    let r = enc(
        Key::Named(NamedKey::End),
        Modifiers::empty(),
        app_cursor_mode(),
    );
    assert_eq!(r, b"\x1bOF");
}

#[test]
fn app_cursor_up_with_ctrl() {
    // Modifiers override SS3 — use CSI format.
    let r = enc(
        Key::Named(NamedKey::ArrowUp),
        Modifiers::CONTROL,
        app_cursor_mode(),
    );
    assert_eq!(r, b"\x1b[1;5A");
}

// --- Unmodified basic keys ---

#[test]
fn enter() {
    assert_eq!(
        enc(Key::Named(NamedKey::Enter), Modifiers::empty(), no_mode()),
        b"\r"
    );
}

#[test]
fn backspace() {
    assert_eq!(
        enc(
            Key::Named(NamedKey::Backspace),
            Modifiers::empty(),
            no_mode()
        ),
        vec![0x7f]
    );
}

#[test]
fn tab() {
    assert_eq!(
        enc(Key::Named(NamedKey::Tab), Modifiers::empty(), no_mode()),
        b"\t"
    );
}

#[test]
fn shift_tab() {
    assert_eq!(
        enc(Key::Named(NamedKey::Tab), Modifiers::SHIFT, no_mode()),
        b"\x1b[Z"
    );
}

#[test]
fn escape() {
    assert_eq!(
        enc(Key::Named(NamedKey::Escape), Modifiers::empty(), no_mode()),
        vec![0x1b]
    );
}

#[test]
fn alt_backspace() {
    assert_eq!(
        enc(Key::Named(NamedKey::Backspace), Modifiers::ALT, no_mode()),
        vec![0x1b, 0x7f]
    );
}

#[test]
fn space() {
    assert_eq!(
        enc(Key::Named(NamedKey::Space), Modifiers::empty(), no_mode()),
        vec![b' ']
    );
}

// --- Unmodified named keys ---

#[test]
fn arrow_up_normal() {
    assert_eq!(
        enc(Key::Named(NamedKey::ArrowUp), Modifiers::empty(), no_mode()),
        b"\x1b[A"
    );
}

#[test]
fn arrow_down_normal() {
    assert_eq!(
        enc(
            Key::Named(NamedKey::ArrowDown),
            Modifiers::empty(),
            no_mode()
        ),
        b"\x1b[B"
    );
}

#[test]
fn home_normal() {
    assert_eq!(
        enc(Key::Named(NamedKey::Home), Modifiers::empty(), no_mode()),
        b"\x1b[H"
    );
}

#[test]
fn end_normal() {
    assert_eq!(
        enc(Key::Named(NamedKey::End), Modifiers::empty(), no_mode()),
        b"\x1b[F"
    );
}

#[test]
fn insert() {
    assert_eq!(
        enc(Key::Named(NamedKey::Insert), Modifiers::empty(), no_mode()),
        b"\x1b[2~"
    );
}

#[test]
fn delete() {
    assert_eq!(
        enc(Key::Named(NamedKey::Delete), Modifiers::empty(), no_mode()),
        b"\x1b[3~"
    );
}

#[test]
fn page_up() {
    assert_eq!(
        enc(Key::Named(NamedKey::PageUp), Modifiers::empty(), no_mode()),
        b"\x1b[5~"
    );
}

#[test]
fn page_down() {
    assert_eq!(
        enc(
            Key::Named(NamedKey::PageDown),
            Modifiers::empty(),
            no_mode()
        ),
        b"\x1b[6~"
    );
}

#[test]
fn f1() {
    assert_eq!(
        enc(Key::Named(NamedKey::F1), Modifiers::empty(), no_mode()),
        b"\x1bOP"
    );
}

#[test]
fn f5() {
    assert_eq!(
        enc(Key::Named(NamedKey::F5), Modifiers::empty(), no_mode()),
        b"\x1b[15~"
    );
}

#[test]
fn f12() {
    assert_eq!(
        enc(Key::Named(NamedKey::F12), Modifiers::empty(), no_mode()),
        b"\x1b[24~"
    );
}

// --- Plain text fallback ---

#[test]
fn plain_text() {
    let r = enc_text(
        Key::Character("x".into()),
        Modifiers::empty(),
        no_mode(),
        "x",
    );
    assert_eq!(r, b"x");
}

#[test]
fn plain_utf8_text() {
    let r = enc_text(
        Key::Character("好".into()),
        Modifiers::empty(),
        no_mode(),
        "好",
    );
    assert_eq!(r, "好".as_bytes());
}

// --- `APP_KEYPAD` numpad ---

#[test]
fn numpad_5_app_keypad() {
    let r = enc_numpad(
        Key::Character("5".into()),
        Modifiers::empty(),
        app_keypad_mode(),
    );
    assert_eq!(r, b"\x1bOu");
}

#[test]
fn numpad_0_app_keypad() {
    let r = enc_numpad(
        Key::Character("0".into()),
        Modifiers::empty(),
        app_keypad_mode(),
    );
    assert_eq!(r, b"\x1bOp");
}

#[test]
fn numpad_5_no_app_keypad() {
    let r = enc_numpad(Key::Character("5".into()), Modifiers::empty(), no_mode());
    // Without `APP_KEYPAD`, numpad falls through to legacy text. No text → empty.
    assert!(r.is_empty());
}

#[test]
fn numpad_enter_app_keypad() {
    let r = enc_numpad(
        Key::Named(NamedKey::Enter),
        Modifiers::empty(),
        app_keypad_mode(),
    );
    assert_eq!(r, b"\x1bOM");
}

#[test]
fn numpad_plus_app_keypad() {
    let r = enc_numpad(
        Key::Character("+".into()),
        Modifiers::empty(),
        app_keypad_mode(),
    );
    assert_eq!(r, b"\x1bOk");
}

#[test]
fn numpad_minus_app_keypad() {
    let r = enc_numpad(
        Key::Character("-".into()),
        Modifiers::empty(),
        app_keypad_mode(),
    );
    assert_eq!(r, b"\x1bOm");
}

#[test]
fn numpad_star_app_keypad() {
    let r = enc_numpad(
        Key::Character("*".into()),
        Modifiers::empty(),
        app_keypad_mode(),
    );
    assert_eq!(r, b"\x1bOj");
}

#[test]
fn numpad_dot_app_keypad() {
    let r = enc_numpad(
        Key::Character(".".into()),
        Modifiers::empty(),
        app_keypad_mode(),
    );
    assert_eq!(r, b"\x1bOn");
}

#[test]
fn non_numpad_5_app_keypad() {
    // Standard location — `APP_KEYPAD` should not affect it.
    let r = enc_text(
        Key::Character("5".into()),
        Modifiers::empty(),
        app_keypad_mode(),
        "5",
    );
    assert_eq!(r, b"5");
}

// --- Legacy release produces nothing ---

#[test]
fn legacy_release_empty() {
    let r = enc_release(Key::Named(NamedKey::ArrowUp), Modifiers::empty(), no_mode());
    assert!(r.is_empty());
}

#[test]
fn legacy_release_char_empty() {
    let r = enc_release(Key::Character("a".into()), Modifiers::empty(), no_mode());
    assert!(r.is_empty());
}

// --- Modifier parameter encoding ---

#[test]
fn modifier_param_shift() {
    assert_eq!(Modifiers::SHIFT.xterm_param(), 2);
}

#[test]
fn modifier_param_alt() {
    assert_eq!(Modifiers::ALT.xterm_param(), 3);
}

#[test]
fn modifier_param_ctrl() {
    assert_eq!(Modifiers::CONTROL.xterm_param(), 5);
}

#[test]
fn modifier_param_ctrl_shift() {
    assert_eq!((Modifiers::CONTROL | Modifiers::SHIFT).xterm_param(), 6);
}

#[test]
fn modifier_param_ctrl_alt() {
    assert_eq!((Modifiers::CONTROL | Modifiers::ALT).xterm_param(), 7);
}

#[test]
fn modifier_param_ctrl_alt_shift() {
    assert_eq!(
        (Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT).xterm_param(),
        8
    );
}

#[test]
fn modifier_param_none() {
    assert_eq!(Modifiers::empty().xterm_param(), 0);
}

// --- F1-F4 use SS3, F5+ use tilde ---

#[test]
fn f1_ss3() {
    assert_eq!(
        enc(Key::Named(NamedKey::F1), Modifiers::empty(), no_mode()),
        b"\x1bOP"
    );
}

#[test]
fn f2_ss3() {
    assert_eq!(
        enc(Key::Named(NamedKey::F2), Modifiers::empty(), no_mode()),
        b"\x1bOQ"
    );
}

#[test]
fn f3_ss3() {
    assert_eq!(
        enc(Key::Named(NamedKey::F3), Modifiers::empty(), no_mode()),
        b"\x1bOR"
    );
}

#[test]
fn f4_ss3() {
    assert_eq!(
        enc(Key::Named(NamedKey::F4), Modifiers::empty(), no_mode()),
        b"\x1bOS"
    );
}

#[test]
fn f6_tilde() {
    assert_eq!(
        enc(Key::Named(NamedKey::F6), Modifiers::empty(), no_mode()),
        b"\x1b[17~"
    );
}

#[test]
fn f11_tilde() {
    assert_eq!(
        enc(Key::Named(NamedKey::F11), Modifiers::empty(), no_mode()),
        b"\x1b[23~"
    );
}

// --- F1-F4 with modifiers use CSI, not SS3 ---

#[test]
fn f1_with_ctrl() {
    assert_eq!(
        enc(Key::Named(NamedKey::F1), Modifiers::CONTROL, no_mode()),
        b"\x1b[1;5P"
    );
}

#[test]
fn f4_with_shift() {
    assert_eq!(
        enc(Key::Named(NamedKey::F4), Modifiers::SHIFT, no_mode()),
        b"\x1b[1;2S"
    );
}

// ==================== Kitty keyboard protocol ====================

fn kitty_disambiguate() -> TermMode {
    TermMode::default() | TermMode::DISAMBIGUATE_ESC_CODES
}

fn kitty_report_events() -> TermMode {
    TermMode::default() | TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_EVENT_TYPES
}

fn kitty_report_all() -> TermMode {
    TermMode::default() | TermMode::REPORT_ALL_KEYS_AS_ESC
}

/// Encode with custom event type.
fn enc_event(
    key: Key,
    mods: Modifiers,
    mode: TermMode,
    text: Option<&str>,
    event_type: KeyEventType,
) -> Vec<u8> {
    encode_key(&KeyInput {
        key: &key,
        mods,
        mode,
        text,
        location: KeyLocation::Standard,
        event_type,
    })
}

// --- Kitty: basic CSI u encoding ---

#[test]
fn kitty_escape() {
    let r = enc(
        Key::Named(NamedKey::Escape),
        Modifiers::empty(),
        kitty_disambiguate(),
    );
    assert_eq!(r, b"\x1b[27u");
}

#[test]
fn kitty_enter() {
    let r = enc(
        Key::Named(NamedKey::Enter),
        Modifiers::empty(),
        kitty_disambiguate(),
    );
    assert_eq!(r, b"\x1b[13u");
}

#[test]
fn kitty_tab() {
    let r = enc(
        Key::Named(NamedKey::Tab),
        Modifiers::empty(),
        kitty_disambiguate(),
    );
    assert_eq!(r, b"\x1b[9u");
}

#[test]
fn kitty_backspace() {
    let r = enc(
        Key::Named(NamedKey::Backspace),
        Modifiers::empty(),
        kitty_disambiguate(),
    );
    assert_eq!(r, b"\x1b[127u");
}

#[test]
fn kitty_f1() {
    let r = enc(
        Key::Named(NamedKey::F1),
        Modifiers::empty(),
        kitty_disambiguate(),
    );
    assert_eq!(r, b"\x1b[57364u");
}

#[test]
fn kitty_arrow_up() {
    let r = enc(
        Key::Named(NamedKey::ArrowUp),
        Modifiers::empty(),
        kitty_disambiguate(),
    );
    assert_eq!(r, b"\x1b[57352u");
}

// --- Kitty: modifiers ---

#[test]
fn kitty_ctrl_a() {
    let r = enc(
        Key::Character("a".into()),
        Modifiers::CONTROL,
        kitty_disambiguate(),
    );
    assert_eq!(r, b"\x1b[97;5u");
}

#[test]
fn kitty_shift_tab() {
    let r = enc(
        Key::Named(NamedKey::Tab),
        Modifiers::SHIFT,
        kitty_disambiguate(),
    );
    assert_eq!(r, b"\x1b[9;2u");
}

#[test]
fn kitty_shift_a() {
    let r = enc(
        Key::Character("A".into()),
        Modifiers::SHIFT,
        kitty_disambiguate(),
    );
    // 'A' is codepoint 65, Shift modifier param = 2.
    assert_eq!(r, b"\x1b[65;2u");
}

// --- Kitty: plain text passthrough ---

#[test]
fn kitty_plain_text() {
    // Printable char with no mods — should send as plain text, not CSI u.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_disambiguate(),
        "a",
    );
    assert_eq!(r, b"a");
}

#[test]
fn kitty_plain_text_no_text_field() {
    // No text field and no mods — empty (no encoding needed).
    let r = enc(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_disambiguate(),
    );
    assert!(r.is_empty());
}

// --- Kitty: REPORT_ALL_KEYS forces CSI u ---

#[test]
fn kitty_report_all_plain_char() {
    // REPORT_ALL_KEYS forces even plain text through CSI u.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_all(),
        "a",
    );
    assert_eq!(r, b"\x1b[97u");
}

// --- Kitty: event types ---

#[test]
fn kitty_release_without_report_events() {
    // DISAMBIGUATE only — release should produce nothing.
    let r = enc_release(
        Key::Named(NamedKey::Escape),
        Modifiers::empty(),
        kitty_disambiguate(),
    );
    assert!(r.is_empty());
}

#[test]
fn kitty_release_with_report_events() {
    let r = enc_event(
        Key::Named(NamedKey::Escape),
        Modifiers::empty(),
        kitty_report_events(),
        None,
        KeyEventType::Release,
    );
    assert_eq!(r, b"\x1b[27;1:3u");
}

#[test]
fn kitty_repeat_with_report_events() {
    let r = enc_event(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_events(),
        Some("a"),
        KeyEventType::Repeat,
    );
    assert_eq!(r, b"\x1b[97;1:2u");
}

#[test]
fn kitty_press_with_report_events() {
    // Press is the default — event type suffix omitted.
    let r = enc_event(
        Key::Named(NamedKey::Escape),
        Modifiers::empty(),
        kitty_report_events(),
        None,
        KeyEventType::Press,
    );
    assert_eq!(r, b"\x1b[27u");
}

// --- Kitty: char release with REPORT_EVENT_TYPES ---

#[test]
fn kitty_char_release_with_report_events() {
    let r = enc_event(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_events(),
        Some("a"),
        KeyEventType::Release,
    );
    assert_eq!(r, b"\x1b[97;1:3u");
}

// --- Kitty: modifier + event type combined ---

#[test]
fn kitty_ctrl_a_release() {
    let r = enc_event(
        Key::Character("a".into()),
        Modifiers::CONTROL,
        kitty_report_events(),
        None,
        KeyEventType::Release,
    );
    assert_eq!(r, b"\x1b[97;5:3u");
}

// --- Legacy release still suppressed ---

#[test]
fn legacy_release_still_empty() {
    let r = enc_release(Key::Named(NamedKey::ArrowUp), Modifiers::empty(), no_mode());
    assert!(r.is_empty());
}

// --- Dispatch priority: Kitty overrides legacy ---

#[test]
fn kitty_overrides_legacy_for_arrow_up() {
    // Legacy would produce ESC[A; Kitty produces ESC[57352u.
    let legacy = enc(Key::Named(NamedKey::ArrowUp), Modifiers::empty(), no_mode());
    let kitty = enc(
        Key::Named(NamedKey::ArrowUp),
        Modifiers::empty(),
        kitty_disambiguate(),
    );
    assert_eq!(legacy, b"\x1b[A");
    assert_eq!(kitty, b"\x1b[57352u");
}

#[test]
fn kitty_overrides_legacy_for_enter() {
    // Legacy would produce \r; Kitty disambiguate produces ESC[13u.
    let legacy = enc(Key::Named(NamedKey::Enter), Modifiers::empty(), no_mode());
    let kitty = enc(
        Key::Named(NamedKey::Enter),
        Modifiers::empty(),
        kitty_disambiguate(),
    );
    assert_eq!(legacy, b"\r");
    assert_eq!(kitty, b"\x1b[13u");
}

// ==================== Enter + LINE_FEED_NEW_LINE mode ====================

fn linefeed_mode() -> TermMode {
    TermMode::default() | TermMode::LINE_FEED_NEW_LINE
}

#[test]
fn enter_linefeed_mode() {
    let r = enc(
        Key::Named(NamedKey::Enter),
        Modifiers::empty(),
        linefeed_mode(),
    );
    assert_eq!(r, b"\r\n");
}

#[test]
fn enter_normal_mode() {
    let r = enc(Key::Named(NamedKey::Enter), Modifiers::empty(), no_mode());
    assert_eq!(r, b"\r");
}

#[test]
fn alt_enter_normal() {
    let r = enc(Key::Named(NamedKey::Enter), Modifiers::ALT, no_mode());
    assert_eq!(r, b"\x1b\r");
}

#[test]
fn alt_enter_linefeed_mode() {
    let r = enc(Key::Named(NamedKey::Enter), Modifiers::ALT, linefeed_mode());
    assert_eq!(r, b"\x1b\r\n");
}

// ==================== Ctrl+Backspace ====================

#[test]
fn ctrl_backspace() {
    // Ctrl+Backspace sends 0x08 (BS), not 0x7f (DEL).
    let r = enc(
        Key::Named(NamedKey::Backspace),
        Modifiers::CONTROL,
        no_mode(),
    );
    assert_eq!(r, vec![0x08]);
}

#[test]
fn alt_ctrl_backspace() {
    let r = enc(
        Key::Named(NamedKey::Backspace),
        Modifiers::ALT | Modifiers::CONTROL,
        no_mode(),
    );
    assert_eq!(r, vec![0x1b, 0x08]);
}

// ==================== Bare modifier keys produce nothing ====================

#[test]
fn bare_shift_produces_nothing() {
    let r = enc(Key::Named(NamedKey::Shift), Modifiers::SHIFT, no_mode());
    assert!(r.is_empty());
}

#[test]
fn bare_control_produces_nothing() {
    let r = enc(Key::Named(NamedKey::Control), Modifiers::CONTROL, no_mode());
    assert!(r.is_empty());
}

#[test]
fn bare_alt_produces_nothing() {
    let r = enc(Key::Named(NamedKey::Alt), Modifiers::ALT, no_mode());
    assert!(r.is_empty());
}

#[test]
fn bare_super_produces_nothing() {
    let r = enc(Key::Named(NamedKey::Super), Modifiers::SUPER, no_mode());
    assert!(r.is_empty());
}

// ==================== Numpad divide in APP_KEYPAD ====================

#[test]
fn numpad_divide_app_keypad() {
    let r = enc_numpad(
        Key::Character("/".into()),
        Modifiers::empty(),
        app_keypad_mode(),
    );
    assert_eq!(r, b"\x1bOo");
}

// ==================== Kitty: Shift+Enter, Shift+Backspace ====================

#[test]
fn kitty_shift_enter() {
    let r = enc(
        Key::Named(NamedKey::Enter),
        Modifiers::SHIFT,
        kitty_disambiguate(),
    );
    assert_eq!(r, b"\x1b[13;2u");
}

#[test]
fn kitty_shift_backspace() {
    let r = enc(
        Key::Named(NamedKey::Backspace),
        Modifiers::SHIFT,
        kitty_disambiguate(),
    );
    assert_eq!(r, b"\x1b[127;2u");
}

// ==================== Kitty: space key ====================

#[test]
fn kitty_space() {
    let r = enc(
        Key::Named(NamedKey::Space),
        Modifiers::empty(),
        kitty_disambiguate(),
    );
    assert_eq!(r, b"\x1b[32u");
}

#[test]
fn kitty_ctrl_space() {
    let r = enc(
        Key::Named(NamedKey::Space),
        Modifiers::CONTROL,
        kitty_disambiguate(),
    );
    assert_eq!(r, b"\x1b[32;5u");
}

// ==================== Kitty: multi-modifier named keys ====================

#[test]
fn kitty_ctrl_shift_arrow_up() {
    let r = enc(
        Key::Named(NamedKey::ArrowUp),
        Modifiers::CONTROL | Modifiers::SHIFT,
        kitty_disambiguate(),
    );
    // Ctrl=4, Shift=1, param = 1 + 4 + 1 = 6.
    assert_eq!(r, b"\x1b[57352;6u");
}

#[test]
fn kitty_alt_ctrl_a() {
    let r = enc(
        Key::Character("a".into()),
        Modifiers::ALT | Modifiers::CONTROL,
        kitty_disambiguate(),
    );
    // Alt=2, Ctrl=4, param = 1 + 2 + 4 = 7.
    assert_eq!(r, b"\x1b[97;7u");
}

// ==================== Multi-char text (dead key compositions) ====================

#[test]
fn multi_char_text_passthrough() {
    // Dead key compositions can produce multi-char strings.
    // These should pass through as text, not be encoded as CSI u.
    let r = enc_text(
        Key::Character("ö".into()),
        Modifiers::empty(),
        no_mode(),
        "ö",
    );
    assert_eq!(r, "ö".as_bytes());
}

#[test]
fn kitty_multi_char_text_passthrough() {
    // Kitty: multi-char Character key → send as text (can't encode as single codepoint).
    let r = enc_text(
        Key::Character("ñ".into()),
        Modifiers::empty(),
        kitty_disambiguate(),
        "ñ",
    );
    // Single codepoint ñ (U+00F1) → plain text passthrough in disambiguate mode.
    assert_eq!(r, "ñ".as_bytes());
}

#[test]
fn kitty_true_multi_char_sends_text() {
    // Two-char string that can't be a single codepoint → text passthrough.
    let r = enc_text(
        Key::Character("ae".into()),
        Modifiers::empty(),
        kitty_disambiguate(),
        "ae",
    );
    assert_eq!(r, b"ae");
}

// ==================== Kitty: associated text (REPORT_ASSOCIATED_TEXT) ====================

fn kitty_report_text() -> TermMode {
    TermMode::default() | TermMode::REPORT_ALL_KEYS_AS_ESC | TermMode::REPORT_ASSOCIATED_TEXT
}

fn kitty_report_text_events() -> TermMode {
    kitty_report_text() | TermMode::REPORT_EVENT_TYPES
}

#[test]
fn kitty_text_plain_char() {
    // 'a' with associated text → CSI u with text codepoint.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_text(),
        "a",
    );
    assert_eq!(r, b"\x1b[97;1;97u");
}

#[test]
fn kitty_text_shift_a() {
    // Shift+a produces 'A' (codepoint 65).
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::SHIFT,
        kitty_report_text(),
        "A",
    );
    assert_eq!(r, b"\x1b[97;2;65u");
}

#[test]
fn kitty_text_ctrl_a_no_text() {
    // Ctrl+a produces control code → text filtered out, no text suffix.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::CONTROL,
        kitty_report_text(),
        "\x01",
    );
    assert_eq!(r, b"\x1b[97;5u");
}

#[test]
fn kitty_text_named_key_no_text() {
    // Named keys (Enter) have no associated text.
    let r = enc(
        Key::Named(NamedKey::Enter),
        Modifiers::empty(),
        kitty_report_text(),
    );
    assert_eq!(r, b"\x1b[13u");
}

#[test]
fn kitty_text_release_no_text() {
    // Release events never include associated text.
    let r = enc_event(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_text_events(),
        Some("a"),
        KeyEventType::Release,
    );
    assert_eq!(r, b"\x1b[97;1:3u");
}

#[test]
fn kitty_text_repeat_includes_text() {
    // Repeat events include associated text.
    let r = enc_event(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_text_events(),
        Some("a"),
        KeyEventType::Repeat,
    );
    assert_eq!(r, b"\x1b[97;1:2;97u");
}

#[test]
fn kitty_text_multi_codepoint() {
    // Multi-codepoint text uses colon separators.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_text(),
        "ab",
    );
    assert_eq!(r, b"\x1b[97;1;97:98u");
}

#[test]
fn kitty_text_filters_control_chars() {
    // Control characters in text are filtered; remaining chars kept.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_text(),
        "a\nb",
    );
    // \n (0x0A) filtered out, leaves 'a' (97) and 'b' (98).
    assert_eq!(r, b"\x1b[97;1;97:98u");
}

#[test]
fn kitty_text_all_control_no_text_suffix() {
    // If all text chars are control codes, no text suffix emitted.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_text(),
        "\x01\x02",
    );
    assert_eq!(r, b"\x1b[97u");
}

#[test]
fn kitty_text_non_ascii() {
    // Non-ASCII codepoint (e.g. U+00E5 = 229 = 'å').
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_text(),
        "\u{00E5}",
    );
    assert_eq!(r, b"\x1b[97;1;229u");
}

#[test]
fn kitty_text_without_flag_no_text() {
    // Without REPORT_ASSOCIATED_TEXT, text is not included even if present.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_all(),
        "a",
    );
    assert_eq!(r, b"\x1b[97u");
}

#[test]
fn kitty_text_bypasses_plain_text_passthrough() {
    // With REPORT_ASSOCIATED_TEXT, plain printable chars still get CSI u encoding.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        TermMode::default() | TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_ASSOCIATED_TEXT,
        "a",
    );
    assert_eq!(r, b"\x1b[97;1;97u");
}

// ==================== Kitty: release gating by mode flags ====================

#[test]
fn kitty_named_key_release_with_report_events_only() {
    // REPORT_EVENT_TYPES without REPORT_ALL — named key release should still be sent.
    let r = enc_event(
        Key::Named(NamedKey::Enter),
        Modifiers::empty(),
        kitty_report_events(),
        None,
        KeyEventType::Release,
    );
    assert_eq!(r, b"\x1b[13;1:3u");
}

#[test]
fn kitty_named_key_release_disambiguate_only() {
    // DISAMBIGUATE alone — release should be suppressed (no REPORT_EVENT_TYPES).
    let r = enc_event(
        Key::Named(NamedKey::Enter),
        Modifiers::empty(),
        kitty_disambiguate(),
        None,
        KeyEventType::Release,
    );
    assert!(r.is_empty());
}

// ==================== Kitty: bare modifiers with REPORT_ALL ====================

#[test]
fn kitty_bare_shift_report_all_produces_nothing() {
    // Bare modifier keys are not in our kitty_codepoint table, so they produce nothing.
    let r = enc(
        Key::Named(NamedKey::Shift),
        Modifiers::SHIFT,
        kitty_report_all(),
    );
    assert!(r.is_empty());
}

#[test]
fn kitty_bare_control_report_all_produces_nothing() {
    let r = enc(
        Key::Named(NamedKey::Control),
        Modifiers::CONTROL,
        kitty_report_all(),
    );
    assert!(r.is_empty());
}

// ==================== Kitty: all flags combined ====================

fn kitty_all_flags() -> TermMode {
    TermMode::default()
        | TermMode::DISAMBIGUATE_ESC_CODES
        | TermMode::REPORT_EVENT_TYPES
        | TermMode::REPORT_ALL_KEYS_AS_ESC
        | TermMode::REPORT_ASSOCIATED_TEXT
}

#[test]
fn kitty_enter_all_flags_press() {
    // All 5 mode bits active — Enter press with no text.
    let r = enc(
        Key::Named(NamedKey::Enter),
        Modifiers::empty(),
        kitty_all_flags(),
    );
    assert_eq!(r, b"\x1b[13u");
}

#[test]
fn kitty_enter_all_flags_release() {
    // All flags — release event includes event type suffix.
    let r = enc_event(
        Key::Named(NamedKey::Enter),
        Modifiers::empty(),
        kitty_all_flags(),
        None,
        KeyEventType::Release,
    );
    assert_eq!(r, b"\x1b[13;1:3u");
}

#[test]
fn kitty_char_all_flags_press_with_text() {
    // All flags — 'a' press includes associated text.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_all_flags(),
        "a",
    );
    assert_eq!(r, b"\x1b[97;1;97u");
}

// ==================== Kitty: associated text edge cases ====================

#[test]
fn kitty_text_del_filtered() {
    // DEL (0x7F) is filtered from associated text.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_text(),
        "a\x7Fb",
    );
    assert_eq!(r, b"\x1b[97;1;97:98u");
}

#[test]
fn kitty_text_c1_control_filtered() {
    // C1 control characters (0x80-0x9F) are filtered from associated text.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_text(),
        "a\u{0085}b",
    );
    // U+0085 (NEL) is in C1 range, filtered out.
    assert_eq!(r, b"\x1b[97;1;97:98u");
}

#[test]
fn kitty_text_space_key_with_text() {
    // Space (codepoint 32) with REPORT_ASSOCIATED_TEXT includes text suffix.
    let r = enc_event(
        Key::Named(NamedKey::Space),
        Modifiers::empty(),
        kitty_report_text(),
        Some(" "),
        KeyEventType::Press,
    );
    assert_eq!(r, b"\x1b[32;1;32u");
}

#[test]
fn kitty_text_ctrl_shift_letter() {
    // Ctrl+Shift+A: key codepoint 97, modifier 6 (Ctrl=4 + Shift=1 + 1), text 'A' (65).
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::CONTROL | Modifiers::SHIFT,
        kitty_report_text(),
        "A",
    );
    assert_eq!(r, b"\x1b[97;6;65u");
}

#[test]
fn kitty_text_emoji_codepoint() {
    // High codepoint emoji in text field.
    let r = enc_text(
        Key::Character("a".into()),
        Modifiers::empty(),
        kitty_report_text(),
        "\u{1F600}",
    );
    // U+1F600 = 128512
    assert_eq!(r, b"\x1b[97;1;128512u");
}

// ==================== Kitty: repeat event for named keys ====================

#[test]
fn kitty_named_key_repeat() {
    // F1 repeat with REPORT_EVENT_TYPES — event type suffix :2.
    let r = enc_event(
        Key::Named(NamedKey::F1),
        Modifiers::empty(),
        kitty_report_events(),
        None,
        KeyEventType::Repeat,
    );
    assert_eq!(r, b"\x1b[57364;1:2u");
}

#[test]
fn kitty_arrow_repeat() {
    // Arrow key repeat.
    let r = enc_event(
        Key::Named(NamedKey::ArrowUp),
        Modifiers::empty(),
        kitty_report_events(),
        None,
        KeyEventType::Repeat,
    );
    assert_eq!(r, b"\x1b[57352;1:2u");
}

// ==================== Ctrl+/ and Ctrl+@ edge cases ====================

#[test]
fn ctrl_slash() {
    // Ctrl+/ traditionally maps to 0x1f (US) via Ctrl+_ alias.
    // Our implementation handles this through the '_' → 0x1f mapping.
    // On most keyboards, Ctrl+/ sends Key::Character("_") or is handled
    // by the OS. If it arrives as "/", it won't produce a control code
    // (correct — "/" is not in the C0 mapping table).
    let r = enc(Key::Character("_".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x1f]);
}

#[test]
fn ctrl_at() {
    // Ctrl+@ = NUL (0x00), via the backtick/2 alias.
    let r = enc(Key::Character("`".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x00]);
}

#[test]
fn ctrl_2() {
    // Ctrl+2 = NUL (0x00), xterm-compatible alias.
    let r = enc(Key::Character("2".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x00]);
}

#[test]
fn ctrl_6() {
    // Ctrl+6 = RS (0x1e), xterm-compatible alias for Ctrl+^.
    let r = enc(Key::Character("6".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x1e]);
}

#[test]
fn ctrl_8() {
    // Ctrl+8 = DEL (0x7f), xterm-compatible.
    let r = enc(Key::Character("8".into()), Modifiers::CONTROL, no_mode());
    assert_eq!(r, vec![0x7f]);
}

// ==================== From<ModifiersState> for Modifiers ====================

use winit::keyboard::ModifiersState;

#[test]
fn from_modifiers_state_empty() {
    let m: Modifiers = ModifiersState::empty().into();
    assert_eq!(m, Modifiers::empty());
}

#[test]
fn from_modifiers_state_shift() {
    let m: Modifiers = ModifiersState::SHIFT.into();
    assert_eq!(m, Modifiers::SHIFT);
}

#[test]
fn from_modifiers_state_alt() {
    let m: Modifiers = ModifiersState::ALT.into();
    assert_eq!(m, Modifiers::ALT);
}

#[test]
fn from_modifiers_state_control() {
    let m: Modifiers = ModifiersState::CONTROL.into();
    assert_eq!(m, Modifiers::CONTROL);
}

#[test]
fn from_modifiers_state_super() {
    let m: Modifiers = ModifiersState::SUPER.into();
    assert_eq!(m, Modifiers::SUPER);
}

#[test]
fn from_modifiers_state_ctrl_shift() {
    let m: Modifiers = (ModifiersState::CONTROL | ModifiersState::SHIFT).into();
    assert_eq!(m, Modifiers::CONTROL | Modifiers::SHIFT);
}

#[test]
fn from_modifiers_state_all() {
    let winit_all = ModifiersState::SHIFT
        | ModifiersState::ALT
        | ModifiersState::CONTROL
        | ModifiersState::SUPER;
    let m: Modifiers = winit_all.into();
    assert_eq!(
        m,
        Modifiers::SHIFT | Modifiers::ALT | Modifiers::CONTROL | Modifiers::SUPER,
    );
}
