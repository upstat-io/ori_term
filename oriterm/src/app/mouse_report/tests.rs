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
fn normal_coords_clamped_at_222() {
    let mut buf = [0u8; 32];
    let len = encode_normal(&mut buf, 0, 500, 500);
    assert_eq!(len, 6);
    // 32 + 1 + 222 = 255 (max u8 value).
    assert_eq!(buf[4], 255);
    assert_eq!(buf[5], 255);
}

#[test]
fn normal_release_code_is_3() {
    let mut buf = [0u8; 32];
    // Release in Normal mode uses code 3 (not the button code).
    let len = encode_normal(&mut buf, 3, 5, 5);
    assert_eq!(len, 6);
    assert_eq!(buf[3], 35); // 32 + 3
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
