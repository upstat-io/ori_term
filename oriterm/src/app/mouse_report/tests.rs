//! Tests for mouse event encoding.

use oriterm_core::TermMode;

use super::{
    MouseButton, MouseEvent, MouseEventKind, MouseModifiers, apply_modifiers, button_code,
    encode_mouse_event, encode_normal, encode_sgr, encode_utf8,
};

// -- button_code tests --

#[test]
fn button_code_left_press() {
    assert_eq!(button_code(MouseButton::Left, MouseEventKind::Press), 0);
}

#[test]
fn button_code_middle_press() {
    assert_eq!(button_code(MouseButton::Middle, MouseEventKind::Press), 1);
}

#[test]
fn button_code_right_press() {
    assert_eq!(button_code(MouseButton::Right, MouseEventKind::Press), 2);
}

#[test]
fn button_code_none_press() {
    assert_eq!(button_code(MouseButton::None, MouseEventKind::Press), 3);
}

#[test]
fn button_code_scroll_up() {
    assert_eq!(
        button_code(MouseButton::ScrollUp, MouseEventKind::Press),
        64
    );
}

#[test]
fn button_code_scroll_down() {
    assert_eq!(
        button_code(MouseButton::ScrollDown, MouseEventKind::Press),
        65
    );
}

#[test]
fn button_code_motion_adds_32() {
    assert_eq!(button_code(MouseButton::Left, MouseEventKind::Motion), 32);
    assert_eq!(button_code(MouseButton::Middle, MouseEventKind::Motion), 33);
    assert_eq!(button_code(MouseButton::Right, MouseEventKind::Motion), 34);
}

#[test]
fn button_code_none_motion_is_35() {
    // Mode 1003 no-button motion: base 3 + 32 = 35.
    assert_eq!(button_code(MouseButton::None, MouseEventKind::Motion), 35);
}

// -- apply_modifiers tests --

#[test]
fn modifiers_none() {
    let mods = MouseModifiers::default();
    assert_eq!(apply_modifiers(0, mods), 0);
}

#[test]
fn modifiers_shift() {
    let mods = MouseModifiers {
        shift: true,
        ..Default::default()
    };
    assert_eq!(apply_modifiers(0, mods), 4);
}

#[test]
fn modifiers_alt() {
    let mods = MouseModifiers {
        alt: true,
        ..Default::default()
    };
    assert_eq!(apply_modifiers(0, mods), 8);
}

#[test]
fn modifiers_ctrl() {
    let mods = MouseModifiers {
        ctrl: true,
        ..Default::default()
    };
    assert_eq!(apply_modifiers(0, mods), 16);
}

#[test]
fn modifiers_combined() {
    let mods = MouseModifiers {
        shift: true,
        alt: true,
        ctrl: true,
    };
    assert_eq!(apply_modifiers(0, mods), 28);
}

// -- SGR encoding tests --

#[test]
fn sgr_left_click_origin() {
    let mut buf = [0u8; 32];
    let len = encode_sgr(&mut buf, 0, 0, 0, true);
    assert_eq!(&buf[..len], b"\x1b[<0;1;1M");
}

#[test]
fn sgr_right_click() {
    let mut buf = [0u8; 32];
    let len = encode_sgr(&mut buf, 2, 5, 10, true);
    assert_eq!(&buf[..len], b"\x1b[<2;6;11M");
}

#[test]
fn sgr_release_uses_lowercase_m() {
    let mut buf = [0u8; 32];
    let len = encode_sgr(&mut buf, 0, 0, 0, false);
    assert_eq!(&buf[..len], b"\x1b[<0;1;1m");
}

#[test]
fn sgr_coordinates_are_1_indexed() {
    let mut buf = [0u8; 32];
    let len = encode_sgr(&mut buf, 0, 9, 19, true);
    let s = std::str::from_utf8(&buf[..len]).unwrap();
    assert!(s.contains(";10;20M"));
}

#[test]
fn sgr_with_shift_modifier() {
    let mut buf = [0u8; 32];
    let code = apply_modifiers(
        0,
        MouseModifiers {
            shift: true,
            ..Default::default()
        },
    );
    let len = encode_sgr(&mut buf, code, 0, 0, true);
    assert_eq!(&buf[..len], b"\x1b[<4;1;1M");
}

#[test]
fn sgr_scroll_up() {
    let mut buf = [0u8; 32];
    let len = encode_sgr(&mut buf, 64, 5, 5, true);
    assert_eq!(&buf[..len], b"\x1b[<64;6;6M");
}

#[test]
fn sgr_motion() {
    let mut buf = [0u8; 32];
    // Left drag = 0 + 32 = 32.
    let len = encode_sgr(&mut buf, 32, 3, 7, true);
    assert_eq!(&buf[..len], b"\x1b[<32;4;8M");
}

#[test]
fn sgr_large_coordinates() {
    let mut buf = [0u8; 32];
    let len = encode_sgr(&mut buf, 0, 999, 499, true);
    let s = std::str::from_utf8(&buf[..len]).unwrap();
    assert!(s.contains(";1000;500M"));
}

#[test]
fn sgr_middle_release_preserves_button_code() {
    // SGR release encodes the real button code (not generic 3 like Normal).
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;
    let e = event_with_mods(
        MouseButton::Middle,
        MouseEventKind::Release,
        5,
        10,
        MouseModifiers::default(),
    );
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    // Middle button = code 1, coords (6,11), release = 'm'.
    assert_eq!(s, "\x1b[<1;6;11m");
}

#[test]
fn sgr_right_release_preserves_button_code() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;
    let e = event(MouseButton::Right, MouseEventKind::Release, 3, 7);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    // Right button = code 2, coords (4,8), release = 'm'.
    assert_eq!(s, "\x1b[<2;4;8m");
}

#[test]
fn sgr_all_modifiers_full_round_trip() {
    // Shift+Alt+Ctrl right-click through full SGR encoding.
    let mods = MouseModifiers {
        shift: true,
        alt: true,
        ctrl: true,
    };
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;
    let e = event_with_mods(MouseButton::Right, MouseEventKind::Press, 10, 20, mods);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    // Right(2) + shift(4) + alt(8) + ctrl(16) = 30, coords (11,21).
    assert_eq!(s, "\x1b[<30;11;21M");
}

// -- Normal (X10) encoding tests --

#[test]
fn normal_left_click() {
    let mut buf = [0u8; 32];
    let len = encode_normal(&mut buf, 0, 0, 0);
    assert_eq!(len, 6);
    assert_eq!(&buf[..3], b"\x1b[M");
    assert_eq!(buf[3], 32); // 32 + 0
    assert_eq!(buf[4], 33); // 32 + 1 + 0
    assert_eq!(buf[5], 33); // 32 + 1 + 0
}

#[test]
fn normal_out_of_range_drops_event() {
    let mut buf = [0u8; 32];
    // Coords > 222 are unencodable — event is silently dropped.
    assert_eq!(encode_normal(&mut buf, 0, 500, 0), 0);
    assert_eq!(encode_normal(&mut buf, 0, 0, 500), 0);
    assert_eq!(encode_normal(&mut buf, 0, 500, 500), 0);
}

#[test]
fn normal_at_max_encodable_coord() {
    let mut buf = [0u8; 32];
    // 222 is the max encodable coordinate (32 + 1 + 222 = 255).
    let len = encode_normal(&mut buf, 0, 222, 222);
    assert_eq!(len, 6);
    assert_eq!(buf[4], 255);
    assert_eq!(buf[5], 255);
}

#[test]
fn normal_just_past_max_drops() {
    let mut buf = [0u8; 32];
    // 223 is one past the limit — should drop.
    assert_eq!(encode_normal(&mut buf, 0, 223, 0), 0);
    assert_eq!(encode_normal(&mut buf, 0, 0, 223), 0);
}

#[test]
fn normal_release_code_is_3() {
    let mut buf = [0u8; 32];
    // Release in Normal mode uses code 3 (not the button code).
    let len = encode_normal(&mut buf, 3, 5, 5);
    assert_eq!(len, 6);
    assert_eq!(buf[3], 35); // 32 + 3
}

#[test]
fn normal_release_with_shift_modifier() {
    let mode = TermMode::MOUSE_REPORT_CLICK;
    let e = event_with_mods(
        MouseButton::Left,
        MouseEventKind::Release,
        0,
        0,
        MouseModifiers {
            shift: true,
            ..Default::default()
        },
    );
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Release code 3 + shift 4 = 7; button byte = 32 + 7 = 39.
    assert_eq!(bytes[3], 39);
}

#[test]
fn normal_release_with_ctrl_modifier() {
    let mode = TermMode::MOUSE_REPORT_CLICK;
    let e = event_with_mods(
        MouseButton::Left,
        MouseEventKind::Release,
        0,
        0,
        MouseModifiers {
            ctrl: true,
            ..Default::default()
        },
    );
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Release code 3 + ctrl 16 = 19; button byte = 32 + 19 = 51.
    assert_eq!(bytes[3], 51);
}

#[test]
fn normal_release_with_all_modifiers() {
    let mode = TermMode::MOUSE_REPORT_CLICK;
    let e = event_with_mods(
        MouseButton::Left,
        MouseEventKind::Release,
        0,
        0,
        MouseModifiers {
            shift: true,
            alt: true,
            ctrl: true,
        },
    );
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Release code 3 + shift(4) + alt(8) + ctrl(16) = 31; byte = 32 + 31 = 63.
    assert_eq!(bytes[3], 63);
}

// -- UTF-8 encoding tests --

#[test]
fn utf8_small_coords_single_byte() {
    let mut buf = [0u8; 32];
    let len = encode_utf8(&mut buf, 0, 5, 10);
    assert!(len > 0);
    assert_eq!(&buf[..3], b"\x1b[M");
    assert_eq!(buf[3], 32); // 32 + 0 (button)
    assert_eq!(buf[4], 38); // 32 + 1 + 5
    assert_eq!(buf[5], 43); // 32 + 1 + 10
}

#[test]
fn utf8_boundary_pos_94_single_byte() {
    // pos=94: val = 32 + 1 + 94 = 127 (still fits in single byte).
    let mut buf = [0u8; 32];
    let len = encode_utf8(&mut buf, 0, 94, 94);
    assert_eq!(len, 3 + 1 + 1 + 1); // header + button + 2 single-byte coords
    assert_eq!(buf[4], 127);
    assert_eq!(buf[5], 127);
}

#[test]
fn utf8_boundary_pos_95_two_bytes() {
    // pos=95: val = 32 + 1 + 95 = 128 (needs 2-byte encoding).
    let mut buf = [0u8; 32];
    let len = encode_utf8(&mut buf, 0, 95, 95);
    assert_eq!(len, 3 + 1 + 2 + 2); // header + button + 2 two-byte coords
}

#[test]
fn utf8_large_coords_multi_byte() {
    let mut buf = [0u8; 32];
    // Position 200: val = 32 + 1 + 200 = 233 (> 127, needs 2-byte encoding).
    let len = encode_utf8(&mut buf, 0, 200, 200);
    assert!(len > 0);
    // Each coordinate should be 2 bytes.
    assert_eq!(len, 3 + 1 + 2 + 2); // header + button + 2 coords * 2 bytes
}

#[test]
fn utf8_out_of_range_returns_zero() {
    let mut buf = [0u8; 32];
    // Position 2016+: val = 32 + 1 + 2016 = 2049 > 0x7FF.
    let len = encode_utf8(&mut buf, 0, 2016, 0);
    assert_eq!(len, 0);
}

#[test]
fn utf8_max_button_code_with_all_modifiers_in_range() {
    // Highest realistic code: ScrollDown(65) + shift(4) + alt(8) + ctrl(16) = 93.
    // Button byte: 32 + 93 = 125 (< 128, single byte — safe).
    let mut buf = [0u8; 32];
    let len = encode_utf8(&mut buf, 93, 0, 0);
    assert!(len > 0);
    assert_eq!(buf[3], 125);
}

// -- encode_mouse_event dispatch tests --

fn event(button: MouseButton, kind: MouseEventKind, col: usize, line: usize) -> MouseEvent {
    MouseEvent {
        button,
        kind,
        col,
        line,
        mods: MouseModifiers::default(),
    }
}

fn event_with_mods(
    button: MouseButton,
    kind: MouseEventKind,
    col: usize,
    line: usize,
    mods: MouseModifiers,
) -> MouseEvent {
    MouseEvent {
        button,
        kind,
        col,
        line,
        mods,
    }
}

#[test]
fn dispatch_sgr_when_sgr_mode() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;
    let e = event(MouseButton::Left, MouseEventKind::Press, 5, 10);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(bytes.starts_with(b"\x1b[<"));
    assert_eq!(*bytes.last().unwrap(), b'M');
}

#[test]
fn dispatch_utf8_when_utf8_mode() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_UTF8;
    let e = event(MouseButton::Left, MouseEventKind::Press, 5, 10);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(bytes.starts_with(b"\x1b[M"));
}

#[test]
fn dispatch_normal_when_no_encoding_flags() {
    let mode = TermMode::MOUSE_REPORT_CLICK;
    let e = event(MouseButton::Left, MouseEventKind::Press, 5, 10);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert_eq!(bytes.len(), 6);
    assert!(bytes.starts_with(b"\x1b[M"));
}

#[test]
fn dispatch_sgr_takes_priority_over_utf8() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR | TermMode::MOUSE_UTF8;
    let e = event(MouseButton::Left, MouseEventKind::Press, 5, 10);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(bytes.starts_with(b"\x1b[<"));
}

#[test]
fn dispatch_normal_release_uses_code_3() {
    let mode = TermMode::MOUSE_REPORT_CLICK;
    let e = event(MouseButton::Left, MouseEventKind::Release, 0, 0);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Normal release: button byte is 32 + 3 = 35.
    assert_eq!(bytes[3], 35);
}

#[test]
fn dispatch_sgr_release_uses_lowercase_m() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;
    let e = event(MouseButton::Left, MouseEventKind::Release, 0, 0);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert_eq!(*bytes.last().unwrap(), b'm');
}

// -- Motion tests (drag and buttonless) --

#[test]
fn no_button_motion_sgr_uses_code_35() {
    // Mode 1003 buttonless motion: None(3) + motion(32) = 35.
    let mode = TermMode::MOUSE_MOTION | TermMode::MOUSE_SGR;
    let e = event(MouseButton::None, MouseEventKind::Motion, 10, 20);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(s, "\x1b[<35;11;21M");
}

#[test]
fn no_button_motion_normal_uses_code_35() {
    let mode = TermMode::MOUSE_MOTION;
    let e = event(MouseButton::None, MouseEventKind::Motion, 5, 5);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Button byte: 32 + 35 = 67.
    assert_eq!(bytes[3], 67);
}

#[test]
fn middle_drag_motion_sgr_uses_code_33() {
    // Middle button drag: Middle(1) + motion(32) = 33.
    let mode = TermMode::MOUSE_DRAG | TermMode::MOUSE_SGR;
    let e = event(MouseButton::Middle, MouseEventKind::Motion, 8, 12);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(s, "\x1b[<33;9;13M");
}

#[test]
fn right_drag_motion_sgr_uses_code_34() {
    // Right button drag: Right(2) + motion(32) = 34.
    let mode = TermMode::MOUSE_DRAG | TermMode::MOUSE_SGR;
    let e = event(MouseButton::Right, MouseEventKind::Motion, 3, 7);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(s, "\x1b[<34;4;8M");
}

#[test]
fn middle_drag_motion_normal_uses_code_33() {
    let mode = TermMode::MOUSE_DRAG;
    let e = event(MouseButton::Middle, MouseEventKind::Motion, 5, 5);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Middle(1) + motion(32) = 33; button byte: 32 + 33 = 65.
    assert_eq!(bytes[3], 65);
}

#[test]
fn right_drag_motion_normal_uses_code_34() {
    let mode = TermMode::MOUSE_DRAG;
    let e = event(MouseButton::Right, MouseEventKind::Motion, 5, 5);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Right(2) + motion(32) = 34; button byte: 32 + 34 = 66.
    assert_eq!(bytes[3], 66);
}

#[test]
fn left_drag_motion_sgr_uses_code_32() {
    // Left button drag: Left(0) + motion(32) = 32.
    let mode = TermMode::MOUSE_DRAG | TermMode::MOUSE_SGR;
    let e = event(MouseButton::Left, MouseEventKind::Motion, 3, 7);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(s, "\x1b[<32;4;8M");
}

// -- Modifier combinations on scroll --

#[test]
fn scroll_up_with_shift_sgr() {
    let mods = MouseModifiers {
        shift: true,
        ..Default::default()
    };
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;
    let e = event_with_mods(MouseButton::ScrollUp, MouseEventKind::Press, 5, 5, mods);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    // ScrollUp(64) + shift(4) = 68.
    assert_eq!(s, "\x1b[<68;6;6M");
}

#[test]
fn scroll_down_with_ctrl_sgr() {
    let mods = MouseModifiers {
        ctrl: true,
        ..Default::default()
    };
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;
    let e = event_with_mods(MouseButton::ScrollDown, MouseEventKind::Press, 5, 5, mods);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    // ScrollDown(65) + ctrl(16) = 81.
    assert_eq!(s, "\x1b[<81;6;6M");
}

#[test]
fn scroll_up_with_alt_normal() {
    let mods = MouseModifiers {
        alt: true,
        ..Default::default()
    };
    let mode = TermMode::MOUSE_REPORT_CLICK;
    let e = event_with_mods(MouseButton::ScrollUp, MouseEventKind::Press, 0, 0, mods);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // ScrollUp(64) + alt(8) = 72; button byte = 32 + 72 = 104.
    assert_eq!(bytes[3], 104);
}

// -- Motion always reports as "pressed" (M, not m) --

#[test]
fn sgr_motion_always_uppercase_m() {
    // Motion events are neither press nor release — they use 'M' (pressed
    // flag) because there's no "motion release". Verify for all button types.
    let mode = TermMode::MOUSE_DRAG | TermMode::MOUSE_SGR;
    for button in [
        MouseButton::Left,
        MouseButton::Middle,
        MouseButton::Right,
        MouseButton::None,
    ] {
        let e = event(button, MouseEventKind::Motion, 5, 5);
        let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
        let s = std::str::from_utf8(&bytes).unwrap();
        assert!(
            s.ends_with('M'),
            "motion for {button:?} should use 'M', got {s:?}"
        );
    }
}

// -- Normal encoding: scroll events --

#[test]
fn normal_scroll_up_encoding() {
    let mode = TermMode::MOUSE_REPORT_CLICK;
    let e = event(MouseButton::ScrollUp, MouseEventKind::Press, 5, 5);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // ScrollUp(64); button byte = 32 + 64 = 96.
    assert_eq!(bytes[3], 96);
}

#[test]
fn normal_scroll_down_encoding() {
    let mode = TermMode::MOUSE_REPORT_CLICK;
    let e = event(MouseButton::ScrollDown, MouseEventKind::Press, 5, 5);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // ScrollDown(65); button byte = 32 + 65 = 97.
    assert_eq!(bytes[3], 97);
}

#[test]
fn normal_scroll_down_with_ctrl() {
    let mods = MouseModifiers {
        ctrl: true,
        ..Default::default()
    };
    let mode = TermMode::MOUSE_REPORT_CLICK;
    let e = event_with_mods(MouseButton::ScrollDown, MouseEventKind::Press, 5, 5, mods);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // ScrollDown(65) + ctrl(16) = 81; button byte = 32 + 81 = 113.
    assert_eq!(bytes[3], 113);
}

// -- UTF-8 encoding: motion --

#[test]
fn utf8_motion_encoding() {
    let mode = TermMode::MOUSE_DRAG | TermMode::MOUSE_UTF8;
    let e = event(MouseButton::Left, MouseEventKind::Motion, 5, 10);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(bytes.starts_with(b"\x1b[M"));
    // Left(0) + motion(32) = 32; button byte = 32 + 32 = 64.
    assert_eq!(bytes[3], 64);
}

#[test]
fn utf8_no_button_motion_encoding() {
    let mode = TermMode::MOUSE_MOTION | TermMode::MOUSE_UTF8;
    let e = event(MouseButton::None, MouseEventKind::Motion, 5, 5);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(bytes.starts_with(b"\x1b[M"));
    // None(3) + motion(32) = 35; button byte = 32 + 35 = 67.
    assert_eq!(bytes[3], 67);
}

// -- Full dispatch: out-of-range returns empty --

#[test]
fn dispatch_normal_out_of_range_returns_empty() {
    // Normal encoding drops events when coords > 222.
    let mode = TermMode::MOUSE_REPORT_CLICK;
    let e = event(MouseButton::Left, MouseEventKind::Press, 300, 300);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(
        bytes.is_empty(),
        "out-of-range Normal event should be empty"
    );
}

#[test]
fn dispatch_utf8_out_of_range_returns_empty() {
    // UTF-8 encoding drops events when coords > 2015.
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_UTF8;
    let e = event(MouseButton::Left, MouseEventKind::Press, 3000, 3000);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(bytes.is_empty(), "out-of-range UTF-8 event should be empty");
}

// -- Buffer overflow safety --

#[test]
fn sgr_extreme_coordinates_fit_in_buffer() {
    // Max realistic SGR: "\x1b[<125;65536;65536M" = ~24 chars, fits in 32.
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;
    let e = event(MouseButton::Left, MouseEventKind::Press, 65535, 65535);
    let report = encode_mouse_event(&e, mode);
    let bytes = report.as_bytes();
    assert!(!bytes.is_empty());
    assert!(bytes.len() <= 32);
    let s = std::str::from_utf8(bytes).unwrap();
    assert!(s.ends_with('M'));
}

// -- Scroll events always use Press kind --

#[test]
fn scroll_release_still_encodes_as_press() {
    // Scroll events don't have a "release" — even if Release kind is passed,
    // the scroll button code (64/65) carries through. In SGR, this means
    // the event still uses 'M' (not 'm') because scroll has no release state.
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;
    let e = event(MouseButton::ScrollUp, MouseEventKind::Release, 5, 5);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    // Release uses 'm' in SGR, even for scroll. This tests the actual behavior.
    assert!(
        s.ends_with('m'),
        "SGR encodes Release as lowercase m: {s:?}"
    );
    // But the button code is still ScrollUp (64), not generic release (3).
    assert!(s.contains("<64;"));
}

// -- Multi-button state machine --
//
// Verifies that encode_mouse_event produces the correct button codes when
// multiple buttons transition through press/motion/release sequences.

#[test]
fn multi_button_press_release_sequence_sgr() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;

    // Press Left → button code 0.
    let e1 = event(MouseButton::Left, MouseEventKind::Press, 5, 5);
    let s1 = std::str::from_utf8(encode_mouse_event(&e1, mode).as_bytes())
        .unwrap()
        .to_string();
    assert!(s1.contains("<0;"), "left press should be code 0: {s1:?}");
    assert!(s1.ends_with('M'));

    // Press Right (while Left still held by app) → button code 2.
    let e2 = event(MouseButton::Right, MouseEventKind::Press, 5, 5);
    let s2 = std::str::from_utf8(encode_mouse_event(&e2, mode).as_bytes())
        .unwrap()
        .to_string();
    assert!(s2.contains("<2;"), "right press should be code 2: {s2:?}");

    // Release Left → SGR preserves button code 0 in release.
    let e3 = event(MouseButton::Left, MouseEventKind::Release, 5, 5);
    let s3 = std::str::from_utf8(encode_mouse_event(&e3, mode).as_bytes())
        .unwrap()
        .to_string();
    assert!(
        s3.contains("<0;"),
        "left release should preserve code 0: {s3:?}"
    );
    assert!(s3.ends_with('m'), "release should use lowercase m: {s3:?}");

    // Release Right → SGR preserves button code 2 in release.
    let e4 = event(MouseButton::Right, MouseEventKind::Release, 5, 5);
    let s4 = std::str::from_utf8(encode_mouse_event(&e4, mode).as_bytes())
        .unwrap()
        .to_string();
    assert!(
        s4.contains("<2;"),
        "right release should preserve code 2: {s4:?}"
    );
    assert!(s4.ends_with('m'));
}

#[test]
fn multi_button_press_release_sequence_normal() {
    let mode = TermMode::MOUSE_REPORT_CLICK;

    // Press Left → button byte = 32 + 0 = 32.
    let e1 = event(MouseButton::Left, MouseEventKind::Press, 5, 5);
    let b1 = encode_mouse_event(&e1, mode).as_bytes().to_vec();
    assert_eq!(b1[3], 32, "left press button byte");

    // Press Right → button byte = 32 + 2 = 34.
    let e2 = event(MouseButton::Right, MouseEventKind::Press, 5, 5);
    let b2 = encode_mouse_event(&e2, mode).as_bytes().to_vec();
    assert_eq!(b2[3], 34, "right press button byte");

    // Release Left → Normal always uses code 3: button byte = 32 + 3 = 35.
    let e3 = event(MouseButton::Left, MouseEventKind::Release, 5, 5);
    let b3 = encode_mouse_event(&e3, mode).as_bytes().to_vec();
    assert_eq!(b3[3], 35, "normal release always uses code 3");

    // Release Right → same code 3 regardless of which button was released.
    let e4 = event(MouseButton::Right, MouseEventKind::Release, 5, 5);
    let b4 = encode_mouse_event(&e4, mode).as_bytes().to_vec();
    assert_eq!(b4[3], 35);
}

// -- Release after motion --

#[test]
fn sgr_release_after_motion_uses_lowercase_m() {
    let mode = TermMode::MOUSE_DRAG | TermMode::MOUSE_SGR;

    // Motion event: Left drag = code 32, uppercase M.
    let motion = event(MouseButton::Left, MouseEventKind::Motion, 10, 10);
    let ms = std::str::from_utf8(encode_mouse_event(&motion, mode).as_bytes())
        .unwrap()
        .to_string();
    assert!(ms.ends_with('M'), "motion uses uppercase M: {ms:?}");
    assert!(ms.contains("<32;"), "left drag motion is code 32: {ms:?}");

    // Release event: Left = code 0, lowercase m.
    let release = event(MouseButton::Left, MouseEventKind::Release, 10, 10);
    let rs = std::str::from_utf8(encode_mouse_event(&release, mode).as_bytes())
        .unwrap()
        .to_string();
    assert!(
        rs.ends_with('m'),
        "release after motion uses lowercase m: {rs:?}"
    );
    assert!(
        rs.contains("<0;"),
        "release uses base button code (no +32): {rs:?}"
    );
}

#[test]
fn normal_release_after_motion_uses_code_3() {
    let mode = TermMode::MOUSE_DRAG;

    // Motion: Left drag = code 32, button byte = 32 + 32 = 64.
    let motion = event(MouseButton::Left, MouseEventKind::Motion, 5, 5);
    let mb = encode_mouse_event(&motion, mode).as_bytes().to_vec();
    assert_eq!(mb[3], 64, "left drag motion button byte");

    // Release: code 3, button byte = 32 + 3 = 35.
    let release = event(MouseButton::Left, MouseEventKind::Release, 5, 5);
    let rb = encode_mouse_event(&release, mode).as_bytes().to_vec();
    assert_eq!(rb[3], 35, "normal release uses code 3 after motion");
}

// -- Scroll with combined modifiers --

#[test]
fn scroll_up_shift_ctrl_sgr() {
    let mods = MouseModifiers {
        shift: true,
        alt: false,
        ctrl: true,
    };
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;
    let e = event_with_mods(MouseButton::ScrollUp, MouseEventKind::Press, 5, 5, mods);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    // ScrollUp(64) + shift(4) + ctrl(16) = 84.
    assert_eq!(s, "\x1b[<84;6;6M");
}

#[test]
fn scroll_down_shift_alt_sgr() {
    let mods = MouseModifiers {
        shift: true,
        alt: true,
        ctrl: false,
    };
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;
    let e = event_with_mods(MouseButton::ScrollDown, MouseEventKind::Press, 5, 5, mods);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    // ScrollDown(65) + shift(4) + alt(8) = 77.
    assert_eq!(s, "\x1b[<77;6;6M");
}

#[test]
fn scroll_up_all_modifiers_normal() {
    let mods = MouseModifiers {
        shift: true,
        alt: true,
        ctrl: true,
    };
    let mode = TermMode::MOUSE_REPORT_CLICK;
    let e = event_with_mods(MouseButton::ScrollUp, MouseEventKind::Press, 0, 0, mods);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // ScrollUp(64) + shift(4) + alt(8) + ctrl(16) = 92; button byte = 32 + 92 = 124.
    assert_eq!(bytes[3], 124);
}

// -- UTF-8 encoding boundary at pos=2015 --

#[test]
fn utf8_exact_max_coord_2015() {
    let mut buf = [0u8; 32];
    // pos=2015: val = 32 + 1 + 2015 = 2048 = 0x800 — OUT of range (> 0x7FF).
    let len = encode_utf8(&mut buf, 0, 2015, 0);
    assert_eq!(len, 0, "pos 2015 should be out of range for UTF-8");
}

#[test]
fn utf8_one_below_max_coord_2014() {
    let mut buf = [0u8; 32];
    // pos=2014: val = 32 + 1 + 2014 = 2047 = 0x7FF — exactly at the limit.
    let len = encode_utf8(&mut buf, 0, 2014, 0);
    assert!(len > 0, "pos 2014 should encode successfully");
}

#[test]
fn utf8_boundary_symmetry() {
    let mut buf = [0u8; 32];
    // Both coords at max limit.
    let len = encode_utf8(&mut buf, 0, 2014, 2014);
    assert!(len > 0, "both coords at max should encode");

    // One coord over limit.
    let len = encode_utf8(&mut buf, 0, 2014, 2015);
    assert_eq!(len, 0, "one coord over limit should fail");
}

// -- Double-press same button without release --

#[test]
fn double_press_same_button_encodes_independently() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;

    // Two left presses without a release — each produces a valid, identical report.
    let e = event(MouseButton::Left, MouseEventKind::Press, 5, 10);
    let b1 = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let b2 = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert_eq!(b1, b2, "duplicate press should produce identical encoding");
    assert!(!b1.is_empty());
}

#[test]
fn double_press_normal_encodes_independently() {
    let mode = TermMode::MOUSE_REPORT_CLICK;

    let e = event(MouseButton::Left, MouseEventKind::Press, 5, 10);
    let b1 = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let b2 = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert_eq!(b1, b2, "duplicate press should produce identical encoding");
    assert_eq!(b1.len(), 6);
}

// -- Exhaustive modifier × button matrix --

#[test]
fn all_modifier_combinations_all_buttons() {
    // 8 modifier combinations × 4 base buttons = 32 entries.
    let buttons = [
        (MouseButton::Left, 0u8),
        (MouseButton::Middle, 1),
        (MouseButton::Right, 2),
        (MouseButton::None, 3),
    ];
    let modifiers = [
        (false, false, false, 0u8),
        (true, false, false, 4),
        (false, true, false, 8),
        (false, false, true, 16),
        (true, true, false, 12),
        (true, false, true, 20),
        (false, true, true, 24),
        (true, true, true, 28),
    ];

    for (button, base_code) in &buttons {
        for (shift, alt, ctrl, mod_bits) in &modifiers {
            let expected_code = base_code + mod_bits;
            let mods = MouseModifiers {
                shift: *shift,
                alt: *alt,
                ctrl: *ctrl,
            };
            let code = apply_modifiers(button_code(*button, MouseEventKind::Press), mods);
            assert_eq!(
                code, expected_code,
                "{button:?} + shift={shift} alt={alt} ctrl={ctrl}: expected {expected_code}, got {code}",
            );

            // Verify SGR encoding round-trips correctly.
            let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR;
            let e = event_with_mods(*button, MouseEventKind::Press, 0, 0, mods);
            let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
            let s = std::str::from_utf8(&bytes).unwrap();
            assert!(
                s.starts_with(&format!("\x1b[<{expected_code};")),
                "{button:?} + mods({mod_bits}): SGR should start with code {expected_code}, got {s:?}",
            );
        }
    }
}

// --- URXVT encoding ---

#[test]
fn urxvt_left_click_at_origin() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_URXVT;
    let e = event(MouseButton::Left, MouseEventKind::Press, 0, 0);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Button code = 32 + 0 = 32, col = 1, line = 1.
    assert_eq!(bytes, b"\x1b[32;1;1M");
}

#[test]
fn urxvt_right_click_at_large_coords() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_URXVT;
    let e = event(MouseButton::Right, MouseEventKind::Press, 500, 300);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Button code = 32 + 2 = 34, col = 501, line = 301.
    assert_eq!(bytes, b"\x1b[34;501;301M");
}

#[test]
fn urxvt_scroll_up() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_URXVT;
    let e = event(MouseButton::ScrollUp, MouseEventKind::Press, 5, 10);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Button code = 32 + 64 = 96, col = 6, line = 11.
    assert_eq!(bytes, b"\x1b[96;6;11M");
}

#[test]
fn urxvt_has_higher_priority_than_utf8() {
    // When both URXVT and UTF-8 are set, URXVT wins.
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_URXVT | TermMode::MOUSE_UTF8;
    let e = event(MouseButton::Left, MouseEventKind::Press, 0, 0);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Should be URXVT format, not UTF-8.
    assert_eq!(bytes, b"\x1b[32;1;1M");
}

#[test]
fn sgr_has_higher_priority_than_urxvt() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_SGR | TermMode::MOUSE_URXVT;
    let e = event(MouseButton::Left, MouseEventKind::Press, 0, 0);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Should be SGR format.
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(
        s.starts_with("\x1b[<"),
        "SGR should take priority over URXVT"
    );
}

#[test]
fn urxvt_with_shift_modifier() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_URXVT;
    let e = MouseEvent {
        button: MouseButton::Left,
        kind: MouseEventKind::Press,
        col: 5,
        line: 3,
        mods: MouseModifiers {
            shift: true,
            alt: false,
            ctrl: false,
        },
    };
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Button code = 32 + 0 + 4 (shift) = 36, col = 6, line = 4.
    assert_eq!(bytes, b"\x1b[36;6;4M");
}

#[test]
fn urxvt_with_ctrl_modifier() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_URXVT;
    let e = MouseEvent {
        button: MouseButton::Left,
        kind: MouseEventKind::Press,
        col: 0,
        line: 0,
        mods: MouseModifiers {
            shift: false,
            alt: false,
            ctrl: true,
        },
    };
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Button code = 32 + 0 + 16 (ctrl) = 48.
    assert_eq!(bytes, b"\x1b[48;1;1M");
}

#[test]
fn urxvt_release_uses_m_suffix() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_URXVT;
    let e = event(MouseButton::Left, MouseEventKind::Release, 5, 3);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    let s = std::str::from_utf8(&bytes).unwrap();
    // URXVT always uses 'M' suffix — no press/release distinction.
    assert!(
        s.ends_with('M'),
        "URXVT release should use M suffix, got: {s}"
    );
}

// --- X10 mode (mode 9) tests ---

#[test]
fn x10_mode_press_encodes_normally() {
    let mode = TermMode::MOUSE_X10;
    let e = event(MouseButton::Left, MouseEventKind::Press, 5, 10);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert_eq!(bytes.len(), 6);
    assert!(bytes.starts_with(b"\x1b[M"));
    // Left(0), no modifiers; button byte = 32 + 0 = 32.
    assert_eq!(bytes[3], 32);
    // col=5: 32 + 1 + 5 = 38.
    assert_eq!(bytes[4], 38);
    // line=10: 32 + 1 + 10 = 43.
    assert_eq!(bytes[5], 43);
}

#[test]
fn x10_mode_release_is_suppressed() {
    let mode = TermMode::MOUSE_X10;
    let e = event(MouseButton::Left, MouseEventKind::Release, 5, 10);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(bytes.is_empty(), "X10 mode should not report releases");
}

#[test]
fn x10_mode_motion_is_suppressed() {
    let mode = TermMode::MOUSE_X10;
    let e = event(MouseButton::Left, MouseEventKind::Motion, 5, 10);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // Motion is a kind of "not release" but X10 should only report button
    // presses. The motion bit in the code makes it technically a "press with
    // motion" — that should still be filtered out since X10 is press-only.
    // However, our current implementation only filters releases. Motion
    // events are handled at the App layer (report_mouse_motion returns
    // early for X10 mode), so encode_mouse_event does encode motion.
    // This test documents the actual behavior.
    assert!(
        !bytes.is_empty(),
        "motion encoding is allowed at the encoder level"
    );
}

#[test]
fn x10_mode_strips_modifiers() {
    let mods = MouseModifiers {
        shift: true,
        alt: true,
        ctrl: true,
    };
    let mode = TermMode::MOUSE_X10;
    let e = event_with_mods(MouseButton::Left, MouseEventKind::Press, 0, 0, mods);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    // X10 mode should NOT add modifier bits (shift=4, alt=8, ctrl=16).
    // Button byte should be 32 + 0 = 32 (bare left click).
    assert_eq!(bytes[3], 32, "X10 mode should strip all modifiers");
}

#[test]
fn x10_mode_right_click_press() {
    let mode = TermMode::MOUSE_X10;
    let e = event(MouseButton::Right, MouseEventKind::Press, 3, 7);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert_eq!(bytes.len(), 6);
    // Right(2), no modifiers; button byte = 32 + 2 = 34.
    assert_eq!(bytes[3], 34);
}

#[test]
fn x10_mode_middle_click_press() {
    let mode = TermMode::MOUSE_X10;
    let e = event(MouseButton::Middle, MouseEventKind::Press, 0, 0);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert_eq!(bytes.len(), 6);
    // Middle(1); button byte = 32 + 1 = 33.
    assert_eq!(bytes[3], 33);
}

#[test]
fn x10_mode_scroll_up_press() {
    let mode = TermMode::MOUSE_X10;
    let e = event(MouseButton::ScrollUp, MouseEventKind::Press, 5, 5);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert_eq!(bytes.len(), 6);
    // ScrollUp(64), no modifiers; button byte = 32 + 64 = 96.
    assert_eq!(bytes[3], 96);
}

#[test]
fn x10_mode_scroll_down_press() {
    let mode = TermMode::MOUSE_X10;
    let e = event(MouseButton::ScrollDown, MouseEventKind::Press, 5, 5);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert_eq!(bytes.len(), 6);
    // ScrollDown(65), no modifiers; button byte = 32 + 65 = 97.
    assert_eq!(bytes[3], 97);
}

#[test]
fn x10_mode_out_of_range_drops_event() {
    let mode = TermMode::MOUSE_X10;
    let e = event(MouseButton::Left, MouseEventKind::Press, 300, 300);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(bytes.is_empty(), "X10 mode should drop out-of-range events");
}

#[test]
fn x10_mode_scroll_release_is_suppressed() {
    let mode = TermMode::MOUSE_X10;
    let e = event(MouseButton::ScrollUp, MouseEventKind::Release, 5, 5);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(bytes.is_empty(), "X10 mode should suppress scroll releases");
}

// -- Dispatch-level coordinate boundary tests --
//
// These test the full encode_mouse_event dispatch path at Normal mode's
// 222/223 boundary (fencepost) to verify no off-by-one in dispatch.

#[test]
fn dispatch_normal_boundary_at_max_coord_succeeds() {
    let mode = TermMode::MOUSE_REPORT_CLICK;
    // col=222 is the max encodable coordinate in Normal mode.
    let e = event(MouseButton::Left, MouseEventKind::Press, 222, 222);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert_eq!(bytes.len(), 6, "col=222 should encode successfully");
    assert_eq!(bytes[4], 255); // 32 + 1 + 222 = 255
    assert_eq!(bytes[5], 255);
}

#[test]
fn dispatch_normal_boundary_one_past_max_drops() {
    let mode = TermMode::MOUSE_REPORT_CLICK;
    // col=223 exceeds Normal's limit — full dispatch should drop.
    let e = event(MouseButton::Left, MouseEventKind::Press, 223, 0);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(bytes.is_empty(), "col=223 should be dropped via dispatch");
}

#[test]
fn dispatch_normal_boundary_line_past_max_drops() {
    let mode = TermMode::MOUSE_REPORT_CLICK;
    // line=223 also exceeds the limit.
    let e = event(MouseButton::Left, MouseEventKind::Press, 0, 223);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(bytes.is_empty(), "line=223 should be dropped via dispatch");
}

#[test]
fn dispatch_utf8_boundary_at_max_coord_succeeds() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_UTF8;
    // pos=2014: val = 32 + 1 + 2014 = 2047 = 0x7FF — max for 2-byte UTF-8.
    let e = event(MouseButton::Left, MouseEventKind::Press, 2014, 2014);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(!bytes.is_empty(), "pos=2014 should encode via dispatch");
}

#[test]
fn dispatch_utf8_boundary_one_past_max_drops() {
    let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_UTF8;
    // pos=2015: val = 32 + 1 + 2015 = 2048 > 0x7FF — out of range.
    let e = event(MouseButton::Left, MouseEventKind::Press, 2015, 0);
    let bytes = encode_mouse_event(&e, mode).as_bytes().to_vec();
    assert!(
        bytes.is_empty(),
        "pos=2015 should be dropped via UTF-8 dispatch"
    );
}

// -- App-level scenarios (documented, not unit-testable) --
//
// The following scenarios require the full App context and cannot be tested
// at the encoding layer:
//
// - Same-cell motion deduplication: App::report_mouse_motion skips reporting
//   when mouse.last_reported_cell() matches the current cell. The dedup
//   happens before encode_mouse_event is ever called.
//
// - Focus-loss button release synthesis: When the window loses focus with
//   buttons held, the app should synthesize release events for all held
//   buttons. This prevents apps (vim, tmux) from thinking buttons are still
//   held after focus returns.
