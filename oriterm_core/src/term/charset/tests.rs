//! Tests for character set translation.

use super::{CharsetIndex, CharsetState, StandardCharset};

#[test]
fn default_all_ascii_no_translation() {
    let mut state = CharsetState::default();
    assert_eq!(state.translate('A'), 'A');
    assert_eq!(state.translate('q'), 'q');
    assert_eq!(state.translate('~'), '~');
}

#[test]
fn dec_special_graphics_q_maps_to_horizontal_line() {
    let mut state = CharsetState::default();
    state.set_charset(
        CharsetIndex::G0,
        StandardCharset::SpecialCharacterAndLineDrawing,
    );
    assert_eq!(state.translate('q'), '─'); // U+2500
}

#[test]
fn dec_special_graphics_box_drawing_chars() {
    let mut state = CharsetState::default();
    state.set_charset(
        CharsetIndex::G0,
        StandardCharset::SpecialCharacterAndLineDrawing,
    );

    assert_eq!(state.translate('l'), '┌'); // top-left corner
    assert_eq!(state.translate('k'), '┐'); // top-right corner
    assert_eq!(state.translate('m'), '└'); // bottom-left corner
    assert_eq!(state.translate('j'), '┘'); // bottom-right corner
    assert_eq!(state.translate('x'), '│'); // vertical line
    assert_eq!(state.translate('n'), '┼'); // cross
}

#[test]
fn single_shift_applies_for_one_char_then_reverts() {
    let mut state = CharsetState::default();
    // G0 = ASCII (default), G2 = DEC special graphics.
    state.set_charset(
        CharsetIndex::G2,
        StandardCharset::SpecialCharacterAndLineDrawing,
    );
    state.set_single_shift(CharsetIndex::G2);

    // First char uses G2 (DEC special graphics).
    assert_eq!(state.translate('q'), '─');
    // Second char reverts to G0 (ASCII).
    assert_eq!(state.translate('q'), 'q');
}

#[test]
fn g0_g1_switching() {
    let mut state = CharsetState::default();
    state.set_charset(
        CharsetIndex::G1,
        StandardCharset::SpecialCharacterAndLineDrawing,
    );

    // Active is G0 (ASCII) by default.
    assert_eq!(state.translate('q'), 'q');

    // Switch to G1 (DEC special graphics).
    state.set_active(CharsetIndex::G1);
    assert_eq!(state.translate('q'), '─');

    // Switch back to G0 (ASCII).
    state.set_active(CharsetIndex::G0);
    assert_eq!(state.translate('q'), 'q');
}

#[test]
fn chars_outside_mapping_range_pass_through() {
    let mut state = CharsetState::default();
    state.set_charset(
        CharsetIndex::G0,
        StandardCharset::SpecialCharacterAndLineDrawing,
    );

    // Characters below 0x5F pass through unchanged.
    assert_eq!(state.translate('A'), 'A');
    assert_eq!(state.translate('0'), '0');
    assert_eq!(state.translate(' '), ' ');
}

#[test]
fn single_shift_overrides_active_charset() {
    let mut state = CharsetState::default();
    // G0 = DEC special graphics, G1 = ASCII.
    state.set_charset(
        CharsetIndex::G0,
        StandardCharset::SpecialCharacterAndLineDrawing,
    );
    state.set_charset(CharsetIndex::G1, StandardCharset::Ascii);
    state.set_active(CharsetIndex::G0);

    // Single shift to G1 (ASCII) — should pass through.
    state.set_single_shift(CharsetIndex::G1);
    assert_eq!(state.translate('q'), 'q');

    // Next char uses G0 (DEC special graphics) again.
    assert_eq!(state.translate('q'), '─');
}
