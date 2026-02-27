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

#[test]
fn merge_duplicate_user_entries_last_wins() {
    // When user specifies the same key+mods twice, last entry wins.
    let user = vec![
        KeybindConfig {
            key: "t".to_owned(),
            mods: "Ctrl".to_owned(),
            action: "CloseTab".to_owned(),
        },
        KeybindConfig {
            key: "t".to_owned(),
            mods: "Ctrl".to_owned(),
            action: "ZoomIn".to_owned(),
        },
    ];
    let bindings = merge_bindings(&user);
    let key = BindingKey::Character("t".to_owned());
    let action = find_binding(&bindings, &key, Modifiers::CONTROL);
    assert_eq!(action, Some(&Action::ZoomIn));
}

#[test]
fn modifier_strict_equality_no_subset_matching() {
    // Ctrl+C must not match Ctrl+Shift+C and vice versa.
    let bindings = default_bindings();
    let key = BindingKey::Character("c".to_owned());

    // Ctrl alone → SmartCopy.
    assert_eq!(
        find_binding(&bindings, &key, Modifiers::CONTROL),
        Some(&Action::SmartCopy)
    );
    // Ctrl+Shift → Copy.
    assert_eq!(
        find_binding(&bindings, &key, Modifiers::CONTROL | Modifiers::SHIFT),
        Some(&Action::Copy)
    );
    // Ctrl+Alt → no match (superset not matched).
    assert_eq!(
        find_binding(&bindings, &key, Modifiers::CONTROL | Modifiers::ALT),
        None
    );
    // Shift alone → no match (subset not matched).
    assert_eq!(find_binding(&bindings, &key, Modifiers::SHIFT), None);
}

#[test]
fn merge_adds_new_binding_not_in_defaults() {
    // A user binding for a key+mod combo that doesn't exist in defaults
    // should be appended.
    let user = vec![KeybindConfig {
        key: "z".to_owned(),
        mods: "Ctrl|Shift".to_owned(),
        action: "DuplicateTab".to_owned(),
    }];
    let bindings = merge_bindings(&user);
    let key = BindingKey::Character("z".to_owned());
    let action = find_binding(&bindings, &key, Modifiers::CONTROL | Modifiers::SHIFT);
    assert_eq!(action, Some(&Action::DuplicateTab));
}

#[test]
fn parse_mods_whitespace_around_pipe() {
    // Spaces around the pipe separator should be trimmed.
    assert_eq!(
        parse_mods("Ctrl | Shift"),
        Modifiers::CONTROL | Modifiers::SHIFT
    );
    assert_eq!(parse_mods(" Alt "), Modifiers::ALT);
    assert_eq!(
        parse_mods("Ctrl | Shift | Alt"),
        Modifiers::CONTROL | Modifiers::SHIFT | Modifiers::ALT
    );
}

#[test]
fn parse_key_multi_byte_char() {
    // Multi-byte characters (CJK, emoji) — `s.len()` can be > 1 byte
    // but still a single Unicode char.
    assert_eq!(
        parse_key("\u{00e9}"),
        Some(BindingKey::Character("\u{00e9}".to_owned()))
    );
    // 3-byte CJK char — `s.len()` is 3, within the <=4 threshold.
    assert_eq!(
        parse_key("\u{4e16}"),
        Some(BindingKey::Character("\u{4e16}".to_owned()))
    );
    // 4-byte emoji — `s.len()` is 4, exactly at the <=4 threshold.
    assert_eq!(
        parse_key("\u{1f600}"),
        Some(BindingKey::Character("\u{1f600}".to_owned()))
    );
}

#[test]
fn unescape_truncated_hex() {
    use super::parse::unescape_send_text;
    // Truncated hex: `\x1` has only one hex digit — pads with '0'.
    assert_eq!(unescape_send_text("\\x1"), "\x10");
    // Bare `\x` with no hex digits at all.
    assert_eq!(unescape_send_text("\\x"), "\0");
}

#[test]
fn merge_skips_invalid_key_preserves_rest() {
    // An entry with an unrecognized key should be skipped silently;
    // the remaining user bindings and all defaults must survive.
    let user = vec![
        KeybindConfig {
            key: "NotAKey!!!".to_owned(),
            mods: "Ctrl".to_owned(),
            action: "Copy".to_owned(),
        },
        KeybindConfig {
            key: "z".to_owned(),
            mods: "Ctrl".to_owned(),
            action: "ZoomIn".to_owned(),
        },
    ];
    let bindings = merge_bindings(&user);

    // The valid binding should be present.
    let key = BindingKey::Character("z".to_owned());
    assert_eq!(
        find_binding(&bindings, &key, Modifiers::CONTROL),
        Some(&Action::ZoomIn)
    );

    // Default bindings should still be intact.
    let key_t = BindingKey::Character("t".to_owned());
    assert_eq!(
        find_binding(&bindings, &key_t, Modifiers::CONTROL),
        Some(&Action::NewTab)
    );
}

#[test]
fn merge_skips_invalid_action_preserves_rest() {
    // An entry with a valid key but unrecognized action should be skipped;
    // existing default for that key+mods must remain.
    let user = vec![KeybindConfig {
        key: "t".to_owned(),
        mods: "Ctrl".to_owned(),
        action: "DoSomethingBogus".to_owned(),
    }];
    let bindings = merge_bindings(&user);

    // The default Ctrl+T -> NewTab should survive because the
    // user entry was skipped (invalid action).
    let key = BindingKey::Character("t".to_owned());
    assert_eq!(
        find_binding(&bindings, &key, Modifiers::CONTROL),
        Some(&Action::NewTab)
    );
}

#[test]
fn parse_mods_unknown_modifier_ignored() {
    // Unknown modifier names like "Hyper" or "Meta" should be
    // silently ignored, not produce random modifier bits.
    assert_eq!(parse_mods("Hyper"), Modifiers::empty());
    assert_eq!(parse_mods("Meta"), Modifiers::empty());
    assert_eq!(
        parse_mods("Ctrl|Hyper|Shift"),
        Modifiers::CONTROL | Modifiers::SHIFT
    );
}

// ---------------------------------------------------------------------------
// Modifier parsing edge cases
// ---------------------------------------------------------------------------

#[test]
fn parse_mods_repeated_modifier_is_idempotent() {
    // Duplicating a modifier in the config string should not produce
    // different bits than writing it once.
    assert_eq!(parse_mods("Ctrl|Ctrl"), Modifiers::CONTROL);
    assert_eq!(parse_mods("Shift|Shift|Shift"), Modifiers::SHIFT,);
    assert_eq!(
        parse_mods("Ctrl|Shift|Ctrl"),
        Modifiers::CONTROL | Modifiers::SHIFT,
    );
}

#[test]
fn parse_mods_case_sensitive() {
    // Modifier parsing is case-sensitive — lowercase "ctrl" is unknown.
    assert_eq!(parse_mods("ctrl"), Modifiers::empty());
    assert_eq!(parse_mods("CTRL"), Modifiers::empty());
    assert_eq!(parse_mods("shift"), Modifiers::empty());
}

#[test]
fn parse_mods_trailing_pipe() {
    // Trailing pipe separator leaves an empty part, which should be
    // ignored (empty string is not a valid modifier).
    assert_eq!(parse_mods("Ctrl|"), Modifiers::CONTROL);
    assert_eq!(parse_mods("|Shift"), Modifiers::SHIFT);
    assert_eq!(parse_mods("|"), Modifiers::empty());
}

// ---------------------------------------------------------------------------
// key_to_binding_key edge cases
// ---------------------------------------------------------------------------

#[test]
fn key_to_binding_key_dead_key_returns_none() {
    let key = Key::Dead(Some('`'));
    assert_eq!(key_to_binding_key(&key), None);
}

#[test]
fn key_to_binding_key_dead_key_none_returns_none() {
    let key = Key::Dead(None);
    assert_eq!(key_to_binding_key(&key), None);
}

#[test]
fn key_to_binding_key_unidentified_returns_none() {
    use winit::keyboard::NativeKey;
    let key = Key::Unidentified(NativeKey::Unidentified);
    assert_eq!(key_to_binding_key(&key), None);
}

#[test]
fn key_to_binding_key_empty_character_returns_none() {
    let key = Key::Character("".into());
    assert_eq!(key_to_binding_key(&key), None);
}

// ---------------------------------------------------------------------------
// parse_action SendText edge cases
// ---------------------------------------------------------------------------

#[test]
fn parse_action_send_text_empty_payload() {
    // SendText with no text after the colon should produce an empty string.
    assert_eq!(
        parse_action("SendText:"),
        Some(Action::SendText(String::new())),
    );
}

#[test]
fn parse_action_send_text_payload_with_colons() {
    // Colon is only significant for the first occurrence (prefix split).
    // Additional colons in the payload should be preserved.
    assert_eq!(
        parse_action("SendText:foo:bar:baz"),
        Some(Action::SendText("foo:bar:baz".to_owned())),
    );
}

// ---------------------------------------------------------------------------
// parse_key extended function keys
// ---------------------------------------------------------------------------

#[test]
fn parse_key_extended_function_keys() {
    assert_eq!(parse_key("F13"), Some(BindingKey::Named(NamedKey::F13)));
    assert_eq!(parse_key("F24"), Some(BindingKey::Named(NamedKey::F24)));
    // F25 and above are not in our table.
    assert_eq!(parse_key("F25"), None);
}

// ---------------------------------------------------------------------------
// Merge preserves user binding order
// ---------------------------------------------------------------------------

#[test]
fn merge_user_bindings_searched_in_order() {
    // User bindings are appended in order. When searching with
    // `find_binding`, the first match wins (since defaults are checked
    // first, then appended user bindings). This test verifies that
    // the last user binding for the same key+mods replaces earlier ones
    // (via retain + push), so order is deterministic.
    let user = vec![
        KeybindConfig {
            key: "x".to_owned(),
            mods: "Alt".to_owned(),
            action: "Copy".to_owned(),
        },
        KeybindConfig {
            key: "y".to_owned(),
            mods: "Alt".to_owned(),
            action: "Paste".to_owned(),
        },
    ];
    let bindings = merge_bindings(&user);

    // Both should be present — order of insertion should match.
    let x = BindingKey::Character("x".to_owned());
    let y = BindingKey::Character("y".to_owned());
    assert_eq!(
        find_binding(&bindings, &x, Modifiers::ALT),
        Some(&Action::Copy)
    );
    assert_eq!(
        find_binding(&bindings, &y, Modifiers::ALT),
        Some(&Action::Paste)
    );
}

// ---------------------------------------------------------------------------
// Action as_str() roundtrip through parse_action
// ---------------------------------------------------------------------------

#[test]
fn action_as_str_roundtrip() {
    // Every action's as_str() should parse back to the same action
    // via parse_action(), except SendText (dynamic payload).
    let actions = [
        Action::Copy,
        Action::Paste,
        Action::SmartCopy,
        Action::SmartPaste,
        Action::NewTab,
        Action::CloseTab,
        Action::NextTab,
        Action::PrevTab,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ZoomReset,
        Action::ScrollPageUp,
        Action::ScrollPageDown,
        Action::ScrollToTop,
        Action::ScrollToBottom,
        Action::OpenSearch,
        Action::ReloadConfig,
        Action::PreviousPrompt,
        Action::NextPrompt,
        Action::DuplicateTab,
        Action::MoveTabToNewWindow,
        Action::ToggleFullscreen,
        Action::EnterMarkMode,
        Action::SplitRight,
        Action::SplitDown,
        Action::FocusPaneUp,
        Action::FocusPaneDown,
        Action::FocusPaneLeft,
        Action::FocusPaneRight,
        Action::NextPane,
        Action::PrevPane,
        Action::ClosePane,
        Action::None,
    ];
    for action in &actions {
        let s = action.as_str();
        let parsed = parse_action(s);
        assert_eq!(parsed.as_ref(), Some(action), "roundtrip failed for {s:?}",);
    }
}

// ---------------------------------------------------------------------------
// Pane default binding tests
// ---------------------------------------------------------------------------

#[test]
fn split_right_default_binding() {
    let bindings = default_bindings();
    let key = BindingKey::Character("o".to_owned());
    let mods = Modifiers::CONTROL | Modifiers::SHIFT;
    assert_eq!(
        find_binding(&bindings, &key, mods),
        Some(&Action::SplitRight),
    );
}

#[test]
fn split_down_default_binding() {
    let bindings = default_bindings();
    let key = BindingKey::Character("e".to_owned());
    let mods = Modifiers::CONTROL | Modifiers::SHIFT;
    assert_eq!(
        find_binding(&bindings, &key, mods),
        Some(&Action::SplitDown),
    );
}

#[test]
fn focus_pane_arrow_defaults() {
    let bindings = default_bindings();
    let mods = Modifiers::CONTROL | Modifiers::ALT;
    assert_eq!(
        find_binding(&bindings, &BindingKey::Named(NamedKey::ArrowUp), mods),
        Some(&Action::FocusPaneUp),
    );
    assert_eq!(
        find_binding(&bindings, &BindingKey::Named(NamedKey::ArrowDown), mods),
        Some(&Action::FocusPaneDown),
    );
    assert_eq!(
        find_binding(&bindings, &BindingKey::Named(NamedKey::ArrowLeft), mods),
        Some(&Action::FocusPaneLeft),
    );
    assert_eq!(
        find_binding(&bindings, &BindingKey::Named(NamedKey::ArrowRight), mods),
        Some(&Action::FocusPaneRight),
    );
}

#[test]
fn cycle_pane_defaults() {
    let bindings = default_bindings();
    let mods = Modifiers::CONTROL | Modifiers::ALT;
    assert_eq!(
        find_binding(&bindings, &BindingKey::Character("[".to_owned()), mods),
        Some(&Action::PrevPane),
    );
    assert_eq!(
        find_binding(&bindings, &BindingKey::Character("]".to_owned()), mods),
        Some(&Action::NextPane),
    );
}

#[test]
fn close_pane_default_binding() {
    let bindings = default_bindings();
    let key = BindingKey::Character("w".to_owned());
    let mods = Modifiers::CONTROL | Modifiers::SHIFT;
    assert_eq!(
        find_binding(&bindings, &key, mods),
        Some(&Action::ClosePane),
    );
}

#[test]
fn resize_pane_arrow_defaults() {
    let bindings = default_bindings();
    let mods = Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT;
    assert_eq!(
        find_binding(&bindings, &BindingKey::Named(NamedKey::ArrowUp), mods),
        Some(&Action::ResizePaneUp),
    );
    assert_eq!(
        find_binding(&bindings, &BindingKey::Named(NamedKey::ArrowDown), mods),
        Some(&Action::ResizePaneDown),
    );
    assert_eq!(
        find_binding(&bindings, &BindingKey::Named(NamedKey::ArrowLeft), mods),
        Some(&Action::ResizePaneLeft),
    );
    assert_eq!(
        find_binding(&bindings, &BindingKey::Named(NamedKey::ArrowRight), mods),
        Some(&Action::ResizePaneRight),
    );
}

#[test]
fn equalize_panes_default_binding() {
    let bindings = default_bindings();
    let key = BindingKey::Character("=".to_owned());
    let mods = Modifiers::CONTROL | Modifiers::SHIFT;
    assert_eq!(
        find_binding(&bindings, &key, mods),
        Some(&Action::EqualizePanes),
    );
}

#[test]
fn resize_actions_roundtrip_through_parse() {
    let resize_actions = [
        Action::ResizePaneUp,
        Action::ResizePaneDown,
        Action::ResizePaneLeft,
        Action::ResizePaneRight,
        Action::EqualizePanes,
    ];
    for action in &resize_actions {
        let s = action.as_str();
        let parsed = parse_action(s);
        assert_eq!(parsed.as_ref(), Some(action), "roundtrip failed for {s:?}",);
    }
}

#[test]
fn resize_bindings_no_collision_with_focus_bindings() {
    // Resize uses Ctrl+Alt+Shift+Arrow, focus uses Ctrl+Alt+Arrow.
    // They must not collide.
    let bindings = default_bindings();
    let focus_mods = Modifiers::CONTROL | Modifiers::ALT;
    let resize_mods = Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT;

    let up_key = BindingKey::Named(NamedKey::ArrowUp);
    assert_eq!(
        find_binding(&bindings, &up_key, focus_mods),
        Some(&Action::FocusPaneUp),
    );
    assert_eq!(
        find_binding(&bindings, &up_key, resize_mods),
        Some(&Action::ResizePaneUp),
    );
}
