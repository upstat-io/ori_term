//! Kitty keyboard protocol encoding (CSI u format).
//!
//! Progressive enhancement keyboard protocol for modern terminal applications.
//! Encodes keys in `ESC [ codepoint ; modifiers [: event_type] u` format.
//! Mode flags control which information is reported, from basic disambiguation
//! through full key release/repeat reporting.

use winit::keyboard::{Key, NamedKey};

use super::{KeyEventType, KeyInput, Modifiers};
use oriterm_core::TermMode;

/// Kitty-defined codepoints for functional keys.
///
/// Character keys use their Unicode codepoint directly. Named/functional
/// keys use the codepoints defined by the Kitty keyboard protocol spec.
fn kitty_codepoint(key: NamedKey) -> Option<u32> {
    Some(match key {
        NamedKey::Escape => 27,
        NamedKey::Enter => 13,
        NamedKey::Tab => 9,
        NamedKey::Backspace => 127,
        NamedKey::Insert => 57348,
        NamedKey::Delete => 57349,
        NamedKey::ArrowLeft => 57350,
        NamedKey::ArrowRight => 57351,
        NamedKey::ArrowUp => 57352,
        NamedKey::ArrowDown => 57353,
        NamedKey::PageUp => 57354,
        NamedKey::PageDown => 57355,
        NamedKey::Home => 57356,
        NamedKey::End => 57357,
        NamedKey::CapsLock => 57358,
        NamedKey::ScrollLock => 57359,
        NamedKey::NumLock => 57360,
        NamedKey::PrintScreen => 57361,
        NamedKey::Pause => 57362,
        NamedKey::ContextMenu => 57363,
        NamedKey::F1 => 57364,
        NamedKey::F2 => 57365,
        NamedKey::F3 => 57366,
        NamedKey::F4 => 57367,
        NamedKey::F5 => 57368,
        NamedKey::F6 => 57369,
        NamedKey::F7 => 57370,
        NamedKey::F8 => 57371,
        NamedKey::F9 => 57372,
        NamedKey::F10 => 57373,
        NamedKey::F11 => 57374,
        NamedKey::F12 => 57375,
        NamedKey::F13 => 57376,
        NamedKey::F14 => 57377,
        NamedKey::F15 => 57378,
        NamedKey::F16 => 57379,
        NamedKey::F17 => 57380,
        NamedKey::F18 => 57381,
        NamedKey::F19 => 57382,
        NamedKey::F20 => 57383,
        NamedKey::F21 => 57384,
        NamedKey::F22 => 57385,
        NamedKey::F23 => 57386,
        NamedKey::F24 => 57387,
        NamedKey::F25 => 57388,
        NamedKey::F26 => 57389,
        NamedKey::F27 => 57390,
        NamedKey::F28 => 57391,
        NamedKey::F29 => 57392,
        NamedKey::F30 => 57393,
        NamedKey::F31 => 57394,
        NamedKey::F32 => 57395,
        NamedKey::F33 => 57396,
        NamedKey::F34 => 57397,
        NamedKey::F35 => 57398,
        NamedKey::Space => 32,
        _ => return None,
    })
}

/// Encode a key event using the Kitty keyboard protocol (CSI u format).
///
/// Format: `ESC [ codepoint ; modifiers [: event_type] u`
///
/// Returns an empty `Vec` for unhandled keys or suppressed release events.
pub(super) fn encode_kitty(input: &KeyInput<'_>) -> Vec<u8> {
    let report_all = input.mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC);
    let report_events = input.mode.contains(TermMode::REPORT_EVENT_TYPES);

    // Determine the codepoint.
    let codepoint = match input.key {
        Key::Named(named) => match kitty_codepoint(*named) {
            Some(cp) => cp,
            None => return Vec::new(),
        },
        Key::Character(ch) => match resolve_char_codepoint(ch.as_str()) {
            Some(cp) => {
                // Printable char, no mods, normal press → send as plain text.
                if should_send_as_text(cp, input.mods, report_all, report_events, input.event_type)
                {
                    return input.text.map_or_else(Vec::new, |t| t.as_bytes().to_vec());
                }
                cp
            }
            None => return input.text.map_or_else(Vec::new, |t| t.as_bytes().to_vec()),
        },
        _ => return Vec::new(),
    };

    // Build event type suffix (only when REPORT_EVENT_TYPES active).
    let event_suffix = match resolve_event_suffix(report_events, input.event_type) {
        Some(s) => s,
        None => return Vec::new(), // Release without REPORT_EVENT_TYPES → suppress.
    };

    // Build CSI u sequence.
    build_csi_u(codepoint, input.mods, event_suffix)
}

/// Extract the Unicode codepoint from a single-character string.
///
/// Returns `None` for multi-character strings (send as text instead).
fn resolve_char_codepoint(s: &str) -> Option<u32> {
    let mut chars = s.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None; // Multi-char — not encodable as a single codepoint.
    }
    Some(c as u32)
}

/// Whether a character key should bypass CSI u and send plain text.
///
/// True when: printable (cp >= 32, not DEL), no modifiers, normal press,
/// and neither `REPORT_ALL_KEYS` nor non-press event type requires encoding.
fn should_send_as_text(
    cp: u32,
    mods: Modifiers,
    report_all: bool,
    report_events: bool,
    event_type: KeyEventType,
) -> bool {
    let needs_event_type = report_events && event_type != KeyEventType::Press;
    !report_all && !needs_event_type && mods.is_empty() && cp >= 32 && cp != 127
}

/// Compute the event type suffix for the CSI u sequence.
///
/// Returns `None` if the event should be suppressed (release without
/// `REPORT_EVENT_TYPES`). Returns `Some("")` for normal press events.
fn resolve_event_suffix(report_events: bool, event_type: KeyEventType) -> Option<&'static str> {
    if report_events {
        Some(match event_type {
            KeyEventType::Press => "",
            KeyEventType::Repeat => ":2",
            KeyEventType::Release => ":3",
        })
    } else {
        // Without REPORT_EVENT_TYPES, release events should not be sent.
        if event_type == KeyEventType::Release {
            None
        } else {
            Some("")
        }
    }
}

/// Build the final `ESC [ codepoint ; modifier [: event_type] u` sequence.
fn build_csi_u(codepoint: u32, mods: Modifiers, event_suffix: &str) -> Vec<u8> {
    let mod_param = mods.xterm_param();
    if mod_param > 0 || !event_suffix.is_empty() {
        let m = if mod_param > 0 { mod_param } else { 1 };
        format!("\x1b[{codepoint};{m}{event_suffix}u").into_bytes()
    } else {
        format!("\x1b[{codepoint}u").into_bytes()
    }
}
