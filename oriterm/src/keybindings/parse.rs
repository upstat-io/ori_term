//! Config parsing and merge for keybindings.

use winit::keyboard::NamedKey;

use crate::key_encoding::Modifiers;

use super::defaults::default_bindings;
use super::{Action, BindingKey, KeyBinding};
use crate::config::KeybindConfig;

/// Merge user keybinding overrides with defaults.
///
/// User bindings replace defaults that share the same (key, mods).
/// `Action::None` removes a binding without adding a replacement.
pub(crate) fn merge_bindings(user: &[KeybindConfig]) -> Vec<KeyBinding> {
    let mut bindings = default_bindings();

    for cfg in user {
        let Some(key) = parse_key(&cfg.key) else {
            log::warn!("keybindings: unknown key {:?}", cfg.key);
            continue;
        };
        let mods = parse_mods(&cfg.mods);
        let Some(action) = parse_action(&cfg.action) else {
            log::warn!("keybindings: unknown action {:?}", cfg.action);
            continue;
        };

        // Remove any existing binding with the same key+mods.
        bindings.retain(|b| !(b.key == key && b.mods == mods));

        // Action::None means "unbind" — don't add a replacement.
        if action != Action::None {
            bindings.push(KeyBinding { key, mods, action });
        }
    }

    bindings
}

/// Parse a key string from TOML config.
///
/// Named keys: `Tab`, `PageUp`, `PageDown`, `Home`, `End`, `Insert`,
/// `Delete`, `Escape`, `Enter`, `Backspace`, `Space`, arrow keys, F1-F24.
/// Single characters are lowercased.
pub(crate) fn parse_key(s: &str) -> Option<BindingKey> {
    let named = match s {
        "Tab" => Some(NamedKey::Tab),
        "PageUp" => Some(NamedKey::PageUp),
        "PageDown" => Some(NamedKey::PageDown),
        "Home" => Some(NamedKey::Home),
        "End" => Some(NamedKey::End),
        "Insert" => Some(NamedKey::Insert),
        "Delete" => Some(NamedKey::Delete),
        "Escape" => Some(NamedKey::Escape),
        "Enter" => Some(NamedKey::Enter),
        "Backspace" => Some(NamedKey::Backspace),
        "Space" => Some(NamedKey::Space),
        "ArrowUp" => Some(NamedKey::ArrowUp),
        "ArrowDown" => Some(NamedKey::ArrowDown),
        "ArrowLeft" => Some(NamedKey::ArrowLeft),
        "ArrowRight" => Some(NamedKey::ArrowRight),
        "F1" => Some(NamedKey::F1),
        "F2" => Some(NamedKey::F2),
        "F3" => Some(NamedKey::F3),
        "F4" => Some(NamedKey::F4),
        "F5" => Some(NamedKey::F5),
        "F6" => Some(NamedKey::F6),
        "F7" => Some(NamedKey::F7),
        "F8" => Some(NamedKey::F8),
        "F9" => Some(NamedKey::F9),
        "F10" => Some(NamedKey::F10),
        "F11" => Some(NamedKey::F11),
        "F12" => Some(NamedKey::F12),
        "F13" => Some(NamedKey::F13),
        "F14" => Some(NamedKey::F14),
        "F15" => Some(NamedKey::F15),
        "F16" => Some(NamedKey::F16),
        "F17" => Some(NamedKey::F17),
        "F18" => Some(NamedKey::F18),
        "F19" => Some(NamedKey::F19),
        "F20" => Some(NamedKey::F20),
        "F21" => Some(NamedKey::F21),
        "F22" => Some(NamedKey::F22),
        "F23" => Some(NamedKey::F23),
        "F24" => Some(NamedKey::F24),
        _ => None,
    };

    if let Some(n) = named {
        return Some(BindingKey::Named(n));
    }

    // Single-character key (always lowercase).
    if !s.is_empty() && s.len() <= 4 {
        let mut chars = s.chars();
        if let Some(c) = chars.next() {
            if chars.next().is_none() {
                return Some(BindingKey::Character(c.to_lowercase().to_string()));
            }
        }
    }

    None
}

/// Parse a modifier string like "Ctrl", "Ctrl|Shift", "Alt", "", or "None".
pub(crate) fn parse_mods(s: &str) -> Modifiers {
    let mut mods = Modifiers::empty();
    for part in s.split('|') {
        match part.trim() {
            "Ctrl" | "Control" => mods |= Modifiers::CONTROL,
            "Shift" => mods |= Modifiers::SHIFT,
            "Alt" => mods |= Modifiers::ALT,
            "Super" => mods |= Modifiers::SUPER,
            _ => {} // "None", "", or unknown — no modifier.
        }
    }
    mods
}

/// Parse an action string.
///
/// Supports `SendText:...` for literal text with escape sequences.
pub(crate) fn parse_action(s: &str) -> Option<Action> {
    if let Some(text) = s.strip_prefix("SendText:") {
        return Some(Action::SendText(unescape_send_text(text)));
    }

    Some(match s {
        "Copy" => Action::Copy,
        "Paste" => Action::Paste,
        "SmartCopy" => Action::SmartCopy,
        "SmartPaste" => Action::SmartPaste,
        "NewTab" => Action::NewTab,
        "CloseTab" => Action::CloseTab,
        "NextTab" => Action::NextTab,
        "PrevTab" => Action::PrevTab,
        "ZoomIn" => Action::ZoomIn,
        "ZoomOut" => Action::ZoomOut,
        "ZoomReset" => Action::ZoomReset,
        "ScrollPageUp" => Action::ScrollPageUp,
        "ScrollPageDown" => Action::ScrollPageDown,
        "ScrollToTop" => Action::ScrollToTop,
        "ScrollToBottom" => Action::ScrollToBottom,
        "OpenSearch" => Action::OpenSearch,
        "ReloadConfig" => Action::ReloadConfig,
        "PreviousPrompt" => Action::PreviousPrompt,
        "NextPrompt" => Action::NextPrompt,
        "DuplicateTab" => Action::DuplicateTab,
        "MoveTabToNewWindow" => Action::MoveTabToNewWindow,
        "ToggleFullscreen" => Action::ToggleFullscreen,
        "None" => Action::None,
        _ => return None,
    })
}

/// Process escape sequences in `SendText` values.
///
/// Supports: `\x1b` -> ESC, `\n` -> newline, `\r` -> CR,
/// `\t` -> tab, `\\` -> backslash, `\xHH` -> hex byte.
pub(super) fn unescape_send_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('x') => {
                    let hi = chars.next().unwrap_or('0');
                    let lo = chars.next().unwrap_or('0');
                    let hex: String = [hi, lo].iter().collect();
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        out.push(byte as char);
                    }
                }
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('\\') | None => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}
