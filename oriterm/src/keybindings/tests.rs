//! Tests for keybinding parsing, matching, and merging.

use winit::keyboard::{Key, NamedKey};

use crate::config::KeybindConfig;
use crate::key_encoding::Modifiers;

use super::*;

#[test]
fn default_bindings_not_empty() {
    let bindings = default_bindings();
    assert!(!bindings.is_empty());
    assert!(bindings.len() >= 20);
}

#[test]
fn find_binding_ctrl_t() {
    let bindings = default_bindings();
    let key = BindingKey::Character("t".to_owned());
    let action = find_binding(&bindings, &key, Modifiers::CONTROL);
    assert_eq!(action, Some(&Action::NewTab));
}

#[test]
fn find_binding_no_match() {
    let bindings = default_bindings();
    let key = BindingKey::Character("z".to_owned());
    let action = find_binding(&bindings, &key, Modifiers::CONTROL);
    assert_eq!(action, None);
}

#[test]
fn merge_user_override() {
    let user = vec![KeybindConfig {
        key: "t".to_owned(),
        mods: "Ctrl".to_owned(),
        action: "CloseTab".to_owned(),
    }];
    let bindings = merge_bindings(&user);
    let key = BindingKey::Character("t".to_owned());
    let action = find_binding(&bindings, &key, Modifiers::CONTROL);
    assert_eq!(action, Some(&Action::CloseTab));
}

#[test]
fn merge_user_unbind() {
    let user = vec![KeybindConfig {
        key: "t".to_owned(),
        mods: "Ctrl".to_owned(),
        action: "None".to_owned(),
    }];
    let bindings = merge_bindings(&user);
    let key = BindingKey::Character("t".to_owned());
    let action = find_binding(&bindings, &key, Modifiers::CONTROL);
    assert_eq!(action, None);
}

#[test]
fn merge_preserves_unaffected() {
    let user = vec![KeybindConfig {
        key: "t".to_owned(),
        mods: "Ctrl".to_owned(),
        action: "None".to_owned(),
    }];
    let bindings = merge_bindings(&user);
    let key = BindingKey::Character("w".to_owned());
    let action = find_binding(&bindings, &key, Modifiers::CONTROL);
    assert_eq!(action, Some(&Action::CloseTab));
}

#[test]
fn parse_mods_variants() {
    assert_eq!(parse_mods("Ctrl"), Modifiers::CONTROL);
    assert_eq!(
        parse_mods("Ctrl|Shift"),
        Modifiers::CONTROL | Modifiers::SHIFT
    );
    assert_eq!(parse_mods("Alt"), Modifiers::ALT);
    assert_eq!(parse_mods(""), Modifiers::empty());
    assert_eq!(parse_mods("None"), Modifiers::empty());
}

#[test]
fn parse_key_variants() {
    assert_eq!(parse_key("c"), Some(BindingKey::Character("c".to_owned())));
    assert_eq!(
        parse_key("PageUp"),
        Some(BindingKey::Named(NamedKey::PageUp))
    );
    assert_eq!(parse_key("Tab"), Some(BindingKey::Named(NamedKey::Tab)));
    assert_eq!(parse_key("F1"), Some(BindingKey::Named(NamedKey::F1)));
}

#[test]
fn parse_action_variants() {
    assert_eq!(parse_action("Copy"), Some(Action::Copy));
    assert_eq!(parse_action("Paste"), Some(Action::Paste));
    assert_eq!(parse_action("NewTab"), Some(Action::NewTab));
    assert_eq!(
        parse_action("ToggleFullscreen"),
        Some(Action::ToggleFullscreen)
    );
    assert_eq!(parse_action("None"), Some(Action::None));
    assert_eq!(
        parse_action("SendText:\\x1b[A"),
        Some(Action::SendText("\x1b[A".to_owned()))
    );
    assert_eq!(parse_action("UnknownAction"), None);
}

#[test]
fn key_normalization() {
    let key = Key::Character("C".into());
    let bk = key_to_binding_key(&key);
    assert_eq!(bk, Some(BindingKey::Character("c".to_owned())));
}

#[test]
fn smart_copy_distinct_from_copy() {
    let bindings = default_bindings();
    let key = BindingKey::Character("c".to_owned());

    // Ctrl+C -> SmartCopy.
    let action = find_binding(&bindings, &key, Modifiers::CONTROL);
    assert_eq!(action, Some(&Action::SmartCopy));

    // Ctrl+Shift+C -> Copy.
    let action = find_binding(&bindings, &key, Modifiers::CONTROL | Modifiers::SHIFT);
    assert_eq!(action, Some(&Action::Copy));
}

#[test]
fn smart_paste_distinct_from_paste() {
    let bindings = default_bindings();
    let key = BindingKey::Character("v".to_owned());

    let action = find_binding(&bindings, &key, Modifiers::CONTROL);
    assert_eq!(action, Some(&Action::SmartPaste));

    let action = find_binding(&bindings, &key, Modifiers::CONTROL | Modifiers::SHIFT);
    assert_eq!(action, Some(&Action::Paste));
}

#[test]
fn unescape_sequences() {
    use super::parse::unescape_send_text;
    assert_eq!(unescape_send_text("\\x1b[15~"), "\x1b[15~");
    assert_eq!(unescape_send_text("a\\nb"), "a\nb");
    assert_eq!(unescape_send_text("\\r\\t\\\\"), "\r\t\\");
}

#[test]
fn toggle_fullscreen_alt_enter() {
    let bindings = default_bindings();
    let key = BindingKey::Named(NamedKey::Enter);
    let action = find_binding(&bindings, &key, Modifiers::ALT);
    assert_eq!(action, Some(&Action::ToggleFullscreen));
}

#[test]
fn parse_key_unknown_returns_none() {
    assert_eq!(parse_key("NotAKey"), None);
    assert_eq!(parse_key(""), None);
}

#[test]
fn parse_key_single_char_lowercased() {
    assert_eq!(parse_key("A"), Some(BindingKey::Character("a".to_owned())));
    assert_eq!(parse_key("="), Some(BindingKey::Character("=".to_owned())));
}

#[test]
fn merge_send_text_binding() {
    let user = vec![KeybindConfig {
        key: "a".to_owned(),
        mods: "Alt".to_owned(),
        action: "SendText:\\x1b[A".to_owned(),
    }];
    let bindings = merge_bindings(&user);
    let key = BindingKey::Character("a".to_owned());
    let action = find_binding(&bindings, &key, Modifiers::ALT);
    assert_eq!(action, Some(&Action::SendText("\x1b[A".to_owned())));
}
