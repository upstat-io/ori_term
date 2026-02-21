//! Legacy xterm-style key encoding and `APP_KEYPAD` numpad sequences.
//!
//! Produces VT/xterm escape sequences for named keys (arrows, function keys),
//! C0 control codes for Ctrl+letter, and ESC-prefixed bytes for Alt+key. This
//! is the baseline encoding used when the Kitty keyboard protocol is not active.

use winit::keyboard::{Key, NamedKey};

use super::Modifiers;
use oriterm_core::TermMode;

/// Named key with a letter terminator (SS3 / CSI variant).
///
/// With modifiers, all letter keys use CSI format: `ESC [ 1 ; {mod} {term}`.
/// Without modifiers, the SS3 behavior depends on the key type:
/// - F1-F4: always SS3 (`ESC O P/Q/R/S`), matching xterm/Ghostty.
/// - Arrows, Home, End: SS3 only in application cursor mode (DECCKM).
struct LetterKey {
    /// Terminator byte (e.g. `b'A'` for Up).
    term: u8,
    /// Whether SS3 requires application cursor mode.
    ///
    /// `true` = arrows, Home, End — SS3 only when DECCKM is active.
    /// `false` = F1-F4 — always SS3 when unmodified.
    needs_app_cursor: bool,
}

/// Named key with a tilde terminator (`ESC [ {num} ~`).
///
/// With modifiers: `ESC [ {num} ; {mod} ~`.
struct TildeKey {
    /// Numeric parameter before the tilde.
    num: u8,
}

/// Look up a letter-terminated named key.
///
/// Returns the terminator byte and SS3/CSI variant flag for arrow keys,
/// Home, End, and F1-F4.
fn letter_key(key: NamedKey) -> Option<LetterKey> {
    Some(match key {
        // Arrows: SS3 only in app cursor mode (DECCKM).
        NamedKey::ArrowUp => LetterKey {
            term: b'A',
            needs_app_cursor: true,
        },
        NamedKey::ArrowDown => LetterKey {
            term: b'B',
            needs_app_cursor: true,
        },
        NamedKey::ArrowRight => LetterKey {
            term: b'C',
            needs_app_cursor: true,
        },
        NamedKey::ArrowLeft => LetterKey {
            term: b'D',
            needs_app_cursor: true,
        },
        // Home/End: SS3 only in app cursor mode (DECCKM).
        NamedKey::Home => LetterKey {
            term: b'H',
            needs_app_cursor: true,
        },
        NamedKey::End => LetterKey {
            term: b'F',
            needs_app_cursor: true,
        },
        // F1-F4: always SS3 when unmodified (xterm behavior).
        NamedKey::F1 => LetterKey {
            term: b'P',
            needs_app_cursor: false,
        },
        NamedKey::F2 => LetterKey {
            term: b'Q',
            needs_app_cursor: false,
        },
        NamedKey::F3 => LetterKey {
            term: b'R',
            needs_app_cursor: false,
        },
        NamedKey::F4 => LetterKey {
            term: b'S',
            needs_app_cursor: false,
        },
        _ => return None,
    })
}

/// Look up a tilde-terminated named key.
///
/// Returns the numeric parameter for Insert, Delete, PageUp/Down, and F5-F12.
fn tilde_key(key: NamedKey) -> Option<TildeKey> {
    Some(match key {
        NamedKey::Insert => TildeKey { num: 2 },
        NamedKey::Delete => TildeKey { num: 3 },
        NamedKey::PageUp => TildeKey { num: 5 },
        NamedKey::PageDown => TildeKey { num: 6 },
        NamedKey::F5 => TildeKey { num: 15 },
        NamedKey::F6 => TildeKey { num: 17 },
        NamedKey::F7 => TildeKey { num: 18 },
        NamedKey::F8 => TildeKey { num: 19 },
        NamedKey::F9 => TildeKey { num: 20 },
        NamedKey::F10 => TildeKey { num: 21 },
        NamedKey::F11 => TildeKey { num: 23 },
        NamedKey::F12 => TildeKey { num: 24 },
        _ => return None,
    })
}

/// Encode a key event using legacy xterm/VT sequences.
///
/// Handles named keys (arrows, function keys, Home/End, etc.), Ctrl+letter
/// C0 control codes, Alt+key ESC prefix, and plain text fallback.
pub(super) fn encode_legacy(
    key: &Key,
    mods: Modifiers,
    mode: TermMode,
    text: Option<&str>,
) -> Vec<u8> {
    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let mod_param = mods.xterm_param();

    // Named keys.
    if let Key::Named(named) = key {
        // Letter-terminated keys (arrows, Home, End, F1-F4).
        if let Some(lk) = letter_key(*named) {
            return if mod_param > 0 {
                // Modifiers always use CSI format: ESC [ 1 ; {mod} {term}.
                format!("\x1b[1;{}{}", mod_param, lk.term as char).into_bytes()
            } else if !lk.needs_app_cursor || app_cursor {
                // SS3 format: F1-F4 always, arrows/Home/End only in DECCKM.
                vec![0x1b, b'O', lk.term]
            } else {
                // CSI format for arrows/Home/End in normal mode.
                vec![0x1b, b'[', lk.term]
            };
        }

        // Tilde-terminated keys (Insert, Delete, PgUp, PgDn, F5-F12).
        if let Some(tk) = tilde_key(*named) {
            return if mod_param > 0 {
                format!("\x1b[{};{}~", tk.num, mod_param).into_bytes()
            } else {
                format!("\x1b[{}~", tk.num).into_bytes()
            };
        }

        // Simple named keys with fixed byte output.
        return encode_simple_named(*named, mods, mode);
    }

    // Character keys.
    if let Key::Character(ch) = key {
        return encode_character(ch.as_str(), mods, text);
    }

    // Unhandled key type.
    Vec::new()
}

/// Encode simple named keys (Enter, Backspace, Tab, Escape, Space).
fn encode_simple_named(named: NamedKey, mods: Modifiers, mode: TermMode) -> Vec<u8> {
    match named {
        NamedKey::Enter => {
            let base = if mode.contains(TermMode::LINE_FEED_NEW_LINE) {
                &b"\r\n"[..]
            } else {
                &b"\r"[..]
            };
            if mods.contains(Modifiers::ALT) {
                let mut v = Vec::with_capacity(1 + base.len());
                v.push(0x1b);
                v.extend_from_slice(base);
                v
            } else {
                base.to_vec()
            }
        }
        NamedKey::Backspace => {
            // Ctrl+Backspace sends 0x08 (BS); plain sends 0x7f (DEL).
            let byte = if mods.contains(Modifiers::CONTROL) {
                0x08
            } else {
                0x7f
            };
            if mods.contains(Modifiers::ALT) {
                vec![0x1b, byte]
            } else {
                vec![byte]
            }
        }
        NamedKey::Tab => {
            if mods.contains(Modifiers::SHIFT) {
                b"\x1b[Z".to_vec()
            } else {
                vec![b'\t']
            }
        }
        NamedKey::Escape => vec![0x1b],
        NamedKey::Space => encode_space(mods),
        _ => Vec::new(),
    }
}

/// Encode Space with modifier combinations.
fn encode_space(mods: Modifiers) -> Vec<u8> {
    if mods.contains(Modifiers::CONTROL) {
        // Ctrl+Space = NUL, optionally with Alt prefix.
        let mut v = Vec::with_capacity(2);
        if mods.contains(Modifiers::ALT) {
            v.push(0x1b);
        }
        v.push(0x00);
        v
    } else if mods.contains(Modifiers::ALT) {
        vec![0x1b, b' ']
    } else {
        vec![b' ']
    }
}

/// Encode a character key (Ctrl+letter → C0, Alt → ESC prefix, or plain text).
fn encode_character(s: &str, mods: Modifiers, text: Option<&str>) -> Vec<u8> {
    // Ctrl+letter → C0 control code.
    if mods.contains(Modifiers::CONTROL) {
        if let Some(c0) = ctrl_key_byte(s) {
            let mut v = Vec::with_capacity(2);
            if mods.contains(Modifiers::ALT) {
                v.push(0x1b);
            }
            v.push(c0);
            return v;
        }
    }

    // Alt prefix for character keys (without Ctrl).
    if mods.contains(Modifiers::ALT) && !mods.contains(Modifiers::CONTROL) {
        if let Some(t) = text {
            let mut v = Vec::with_capacity(1 + t.len());
            v.push(0x1b);
            v.extend_from_slice(t.as_bytes());
            return v;
        }
    }

    // Fallback: send the text as-is.
    text.map_or_else(Vec::new, |t| t.as_bytes().to_vec())
}

/// Map a Ctrl+key combination to its C0 control byte.
///
/// Handles a-z (case-insensitive), bracket/punctuation keys (`[`, `\\`, `]`,
/// `^`, `_`, `` ` ``), and the digit shortcuts 2-8 (xterm-compatible).
fn ctrl_key_byte(s: &str) -> Option<u8> {
    let bytes = s.as_bytes();
    if bytes.len() != 1 {
        return None;
    }
    match bytes[0] {
        // a-z → 0x01-0x1A.
        b'a'..=b'z' => Some(bytes[0] - b'a' + 1),
        b'A'..=b'Z' => Some(bytes[0] - b'A' + 1),
        // Punctuation → control codes.
        b'[' | b'3' => Some(0x1b),  // Ctrl+[ = ESC
        b'\\' | b'4' => Some(0x1c), // Ctrl+\ = FS
        b']' | b'5' => Some(0x1d),  // Ctrl+] = GS
        b'^' | b'6' => Some(0x1e),  // Ctrl+^ = RS
        b'_' | b'7' => Some(0x1f),  // Ctrl+_ = US
        b'`' | b'2' => Some(0x00),  // Ctrl+` = NUL
        b'8' => Some(0x7f),         // Ctrl+8 = DEL
        _ => None,
    }
}

/// Encode numpad keys in `APP_KEYPAD` mode (SS3 sequences).
///
/// Produces `ESC O {code}` for numpad digits 0-9, operators, decimal,
/// and Enter. Returns `None` for non-numpad keys.
pub(super) fn encode_numpad_app(key: &Key) -> Option<Vec<u8>> {
    let code = match key {
        Key::Character(c) => match c.as_str() {
            "0" => b'p',
            "1" => b'q',
            "2" => b'r',
            "3" => b's',
            "4" => b't',
            "5" => b'u',
            "6" => b'v',
            "7" => b'w',
            "8" => b'x',
            "9" => b'y',
            "+" => b'k',
            "-" => b'm',
            "*" => b'j',
            "/" => b'o',
            "." => b'n',
            _ => return None,
        },
        Key::Named(NamedKey::Enter) => b'M',
        _ => return None,
    };
    Some(vec![0x1b, b'O', code])
}
