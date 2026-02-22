//! Tests for CLI subcommands.

use clap_complete::Shell;

use super::{
    format_action, format_binding, format_binding_key, generate_completions, is_valid_hex_color,
};
use crate::config::Config;
use crate::key_encoding::Modifiers;
use crate::keybindings::{Action, BindingKey, KeyBinding};

#[test]
fn validate_config_default_is_valid() {
    // Default config has no file to load, so `try_load` will fail with
    // "file not found". Validate the struct directly instead.
    let config = Config::default();
    let mut errors = Vec::new();
    super::validate_colors(&config, &mut errors);
    super::validate_keybindings(&config, &mut errors);
    assert!(
        errors.is_empty(),
        "default config should be valid: {errors:?}"
    );
}

#[test]
fn validate_colors_rejects_bad_hex() {
    let mut config = Config::default();
    config.colors.foreground = Some("not-a-color".to_owned());

    let mut errors = Vec::new();
    super::validate_colors(&config, &mut errors);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("invalid hex color"), "{}", errors[0]);
}

#[test]
fn validate_colors_accepts_valid_hex() {
    let mut config = Config::default();
    config.colors.foreground = Some("#ff00aa".to_owned());
    config.colors.background = Some("#000000".to_owned());

    let mut errors = Vec::new();
    super::validate_colors(&config, &mut errors);
    assert!(errors.is_empty(), "valid hex should pass: {errors:?}");
}

#[test]
fn validate_keybindings_rejects_bad_key() {
    let mut config = Config::default();
    config.keybind.push(crate::config::KeybindConfig {
        key: "NotAKey!!!".to_owned(),
        mods: String::new(),
        action: "Copy".to_owned(),
    });

    let mut errors = Vec::new();
    super::validate_keybindings(&config, &mut errors);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("unknown key"), "{}", errors[0]);
}

#[test]
fn validate_keybindings_rejects_bad_action() {
    let mut config = Config::default();
    config.keybind.push(crate::config::KeybindConfig {
        key: "c".to_owned(),
        mods: "Ctrl".to_owned(),
        action: "DoSomethingInvalid".to_owned(),
    });

    let mut errors = Vec::new();
    super::validate_keybindings(&config, &mut errors);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("unknown action"), "{}", errors[0]);
}

#[test]
fn show_config_roundtrip() {
    let config = Config::default();
    let toml_str = toml::to_string_pretty(&config).expect("serialize");
    let parsed: Config = toml::from_str(&toml_str).expect("re-parse");

    // Verify key fields survive the roundtrip.
    assert_eq!(parsed.font.size, config.font.size);
    assert_eq!(parsed.terminal.scrollback, config.terminal.scrollback);
    assert_eq!(parsed.colors.scheme, config.colors.scheme);
    assert_eq!(parsed.window.columns, config.window.columns);
}

#[test]
fn ls_fonts_finds_primary() {
    let result = crate::font::discovery::discover_fonts(None, 400);
    // Discovery always succeeds (embedded fallback guarantees a result).
    assert!(
        !result.primary.family_name.is_empty(),
        "primary family name should not be empty"
    );
    assert!(
        result.primary.has_variant[0],
        "regular variant should be present"
    );
}

#[test]
fn is_valid_hex_color_cases() {
    assert!(is_valid_hex_color("#ff00aa"));
    assert!(is_valid_hex_color("#000000"));
    assert!(is_valid_hex_color("#FFFFFF"));
    assert!(is_valid_hex_color("abcdef")); // Without # prefix.
    assert!(!is_valid_hex_color("not-hex"));
    assert!(!is_valid_hex_color("#fff")); // Too short.
    assert!(!is_valid_hex_color("#gggggg")); // Invalid hex chars.
    assert!(!is_valid_hex_color("")); // Empty.
}

#[test]
fn format_binding_shows_mods_and_key() {
    let b = KeyBinding {
        key: BindingKey::Character("c".to_owned()),
        mods: Modifiers::CONTROL | Modifiers::SHIFT,
        action: Action::Copy,
    };
    let s = format_binding(&b);
    assert!(s.contains("Ctrl"), "should contain Ctrl: {s}");
    assert!(s.contains("Shift"), "should contain Shift: {s}");
    assert!(s.contains("C"), "should contain key C: {s}");
    assert!(s.contains("Copy"), "should contain action: {s}");
}

#[test]
fn format_binding_key_named() {
    let key = BindingKey::Named(winit::keyboard::NamedKey::Tab);
    let s = format_binding_key(&key);
    assert_eq!(s, "Tab");
}

#[test]
fn format_binding_key_character() {
    let key = BindingKey::Character("v".to_owned());
    let s = format_binding_key(&key);
    assert_eq!(s, "V");
}

#[test]
fn format_action_variants() {
    assert_eq!(format_action(&Action::Copy), "Copy");
    assert_eq!(format_action(&Action::Paste), "Paste");
    assert_eq!(format_action(&Action::NewTab), "NewTab");
    assert_eq!(format_action(&Action::None), "None");
    assert_eq!(
        format_action(&Action::SendText("hello".to_owned())),
        "SendText:\"hello\""
    );
}

// ── High priority: error accumulation ──

#[test]
fn validate_accumulates_color_and_keybinding_errors() {
    let mut config = Config::default();
    config.colors.foreground = Some("bad1".to_owned());
    config.colors.background = Some("bad2".to_owned());
    config.keybind.push(crate::config::KeybindConfig {
        key: "???".to_owned(),
        mods: String::new(),
        action: "Bogus".to_owned(),
    });

    let mut errors = Vec::new();
    super::validate_colors(&config, &mut errors);
    super::validate_keybindings(&config, &mut errors);
    // Two color errors + one key error + one action error = 4.
    assert_eq!(errors.len(), 4, "should accumulate all errors: {errors:?}");
}

// ── High priority: bright/ansi map validation ──

#[test]
fn validate_colors_rejects_bad_ansi_map_entry() {
    let mut config = Config::default();
    config.colors.ansi.insert("0".to_owned(), "xyz".to_owned());

    let mut errors = Vec::new();
    super::validate_colors(&config, &mut errors);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("colors.ansi.0"), "{}", errors[0]);
}

#[test]
fn validate_colors_rejects_bad_bright_map_entry() {
    let mut config = Config::default();
    config
        .colors
        .bright
        .insert("3".to_owned(), "not-hex".to_owned());

    let mut errors = Vec::new();
    super::validate_colors(&config, &mut errors);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("colors.bright.3"), "{}", errors[0]);
}

// ── High priority: bell color validation ──

#[test]
fn validate_colors_rejects_bad_bell_color() {
    let mut config = Config::default();
    config.bell.color = Some("nope".to_owned());

    let mut errors = Vec::new();
    super::validate_colors(&config, &mut errors);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("bell.color"), "{}", errors[0]);
}

#[test]
fn validate_colors_accepts_valid_bell_color() {
    let mut config = Config::default();
    config.bell.color = Some("#aabbcc".to_owned());

    let mut errors = Vec::new();
    super::validate_colors(&config, &mut errors);
    assert!(
        errors.is_empty(),
        "valid bell color should pass: {errors:?}"
    );
}

// ── High priority: format binding with no modifiers ──

#[test]
fn format_binding_no_modifiers() {
    let b = KeyBinding {
        key: BindingKey::Character("a".to_owned()),
        mods: Modifiers::empty(),
        action: Action::NewTab,
    };
    let s = format_binding(&b);
    // Should be just "A -> NewTab" with no leading "+".
    assert_eq!(s, "A -> NewTab");
    assert!(!s.starts_with('+'), "should not start with +: {s}");
}

// ── High priority: format binding with all four modifiers ──

#[test]
fn format_binding_all_four_modifiers() {
    let b = KeyBinding {
        key: BindingKey::Named(winit::keyboard::NamedKey::F1),
        mods: Modifiers::CONTROL | Modifiers::SHIFT | Modifiers::ALT | Modifiers::SUPER,
        action: Action::ToggleFullscreen,
    };
    let s = format_binding(&b);
    assert!(s.contains("Ctrl"), "{s}");
    assert!(s.contains("Shift"), "{s}");
    assert!(s.contains("Alt"), "{s}");
    assert!(s.contains("Super"), "{s}");
    assert!(s.contains("F1"), "{s}");
    assert!(s.contains("ToggleFullscreen"), "{s}");
    // Verify modifier order: Ctrl before Shift before Alt before Super.
    let ctrl_pos = s.find("Ctrl").unwrap();
    let shift_pos = s.find("Shift").unwrap();
    let alt_pos = s.find("Alt").unwrap();
    let super_pos = s.find("Super").unwrap();
    assert!(ctrl_pos < shift_pos, "Ctrl should precede Shift");
    assert!(shift_pos < alt_pos, "Shift should precede Alt");
    assert!(alt_pos < super_pos, "Alt should precede Super");
}

// ── Medium priority: exhaustive format_action coverage ──

#[test]
fn format_action_all_variants() {
    // Every non-SendText, non-None Action should format to its variant name.
    let cases: &[(Action, &str)] = &[
        (Action::Copy, "Copy"),
        (Action::Paste, "Paste"),
        (Action::SmartCopy, "SmartCopy"),
        (Action::SmartPaste, "SmartPaste"),
        (Action::NewTab, "NewTab"),
        (Action::CloseTab, "CloseTab"),
        (Action::NextTab, "NextTab"),
        (Action::PrevTab, "PrevTab"),
        (Action::ZoomIn, "ZoomIn"),
        (Action::ZoomOut, "ZoomOut"),
        (Action::ZoomReset, "ZoomReset"),
        (Action::ScrollPageUp, "ScrollPageUp"),
        (Action::ScrollPageDown, "ScrollPageDown"),
        (Action::ScrollToTop, "ScrollToTop"),
        (Action::ScrollToBottom, "ScrollToBottom"),
        (Action::OpenSearch, "OpenSearch"),
        (Action::ReloadConfig, "ReloadConfig"),
        (Action::PreviousPrompt, "PreviousPrompt"),
        (Action::NextPrompt, "NextPrompt"),
        (Action::DuplicateTab, "DuplicateTab"),
        (Action::MoveTabToNewWindow, "MoveTabToNewWindow"),
        (Action::ToggleFullscreen, "ToggleFullscreen"),
        (Action::EnterMarkMode, "EnterMarkMode"),
        (Action::None, "None"),
    ];
    for (action, expected) in cases {
        assert_eq!(format_action(action), *expected, "mismatch for {action:?}");
    }
}

// ── Medium priority: show-config roundtrip with non-default values ──

#[test]
fn show_config_roundtrip_with_overrides() {
    let mut config = Config::default();
    config.font.size = 18.0;
    config.font.family = Some("Fira Code".to_owned());
    config.terminal.scrollback = 50_000;
    config.colors.foreground = Some("#aabbcc".to_owned());
    config.window.opacity = 0.85;
    config.keybind.push(crate::config::KeybindConfig {
        key: "q".to_owned(),
        mods: "Ctrl".to_owned(),
        action: "CloseTab".to_owned(),
    });

    let toml_str = toml::to_string_pretty(&config).expect("serialize");
    let parsed: Config = toml::from_str(&toml_str).expect("re-parse");

    assert_eq!(parsed.font.size, 18.0);
    assert_eq!(parsed.font.family.as_deref(), Some("Fira Code"));
    assert_eq!(parsed.terminal.scrollback, 50_000);
    assert_eq!(parsed.colors.foreground.as_deref(), Some("#aabbcc"));
    assert_eq!(parsed.window.opacity, 0.85);
    assert_eq!(parsed.keybind.len(), 1);
    assert_eq!(parsed.keybind[0].key, "q");
    assert_eq!(parsed.keybind[0].action, "CloseTab");
}

// ── Medium priority: all 5 color fields bad at once ──

#[test]
fn validate_colors_reports_all_bad_fields() {
    let mut config = Config::default();
    config.colors.foreground = Some("bad".to_owned());
    config.colors.background = Some("bad".to_owned());
    config.colors.cursor = Some("bad".to_owned());
    config.colors.selection_foreground = Some("bad".to_owned());
    config.colors.selection_background = Some("bad".to_owned());

    let mut errors = Vec::new();
    super::validate_colors(&config, &mut errors);
    assert_eq!(errors.len(), 5, "should report all 5 fields: {errors:?}");

    // Verify each field name appears in exactly one error.
    let fields = [
        "colors.foreground",
        "colors.background",
        "colors.cursor",
        "colors.selection_foreground",
        "colors.selection_background",
    ];
    for field in fields {
        let count = errors.iter().filter(|e| e.contains(field)).count();
        assert_eq!(count, 1, "field {field} should appear exactly once");
    }
}

// ── Medium priority: keybinding with both bad key and bad action ──

#[test]
fn validate_keybindings_reports_bad_key_and_bad_action() {
    let mut config = Config::default();
    config.keybind.push(crate::config::KeybindConfig {
        key: "!!!".to_owned(),
        mods: String::new(),
        action: "Nonexistent".to_owned(),
    });

    let mut errors = Vec::new();
    super::validate_keybindings(&config, &mut errors);
    assert_eq!(
        errors.len(),
        2,
        "should report both key and action: {errors:?}"
    );
    assert!(errors.iter().any(|e| e.contains("unknown key")));
    assert!(errors.iter().any(|e| e.contains("unknown action")));
}

// ── Shell completions ──

#[test]
fn completions_bash_produces_nonempty_output() {
    let output = generate_completions(Shell::Bash);
    assert!(!output.is_empty(), "bash completions should not be empty");
    let text = String::from_utf8(output).expect("valid UTF-8");
    assert!(
        text.contains("ls-fonts"),
        "bash completions should mention ls-fonts subcommand"
    );
    assert!(
        text.contains("show-keys"),
        "bash completions should mention show-keys subcommand"
    );
    assert!(
        text.contains("completions"),
        "bash completions should mention completions subcommand"
    );
}

#[test]
fn completions_zsh_produces_nonempty_output() {
    let output = generate_completions(Shell::Zsh);
    assert!(!output.is_empty(), "zsh completions should not be empty");
    let text = String::from_utf8(output).expect("valid UTF-8");
    assert!(
        text.contains("ls-fonts"),
        "zsh completions should mention ls-fonts subcommand"
    );
    assert!(
        text.contains("show-keys"),
        "zsh completions should mention show-keys subcommand"
    );
    assert!(
        text.contains("completions"),
        "zsh completions should mention completions subcommand"
    );
}

#[test]
fn completions_fish_produces_nonempty_output() {
    let output = generate_completions(Shell::Fish);
    assert!(!output.is_empty(), "fish completions should not be empty");
    let text = String::from_utf8(output).expect("valid UTF-8");
    assert!(
        text.contains("ls-fonts"),
        "fish completions should mention ls-fonts subcommand"
    );
    assert!(
        text.contains("show-keys"),
        "fish completions should mention show-keys subcommand"
    );
    assert!(
        text.contains("completions"),
        "fish completions should mention completions subcommand"
    );
}

#[test]
fn completions_powershell_produces_nonempty_output() {
    let output = generate_completions(Shell::PowerShell);
    assert!(
        !output.is_empty(),
        "PowerShell completions should not be empty"
    );
    let text = String::from_utf8(output).expect("valid UTF-8");
    assert!(
        text.contains("ls-fonts"),
        "PowerShell completions should mention ls-fonts subcommand"
    );
    assert!(
        text.contains("show-keys"),
        "PowerShell completions should mention show-keys subcommand"
    );
    assert!(
        text.contains("completions"),
        "PowerShell completions should mention completions subcommand"
    );
}

#[test]
fn completions_contain_all_subcommands() {
    // Verify all subcommands appear in bash completions (representative shell).
    let output = generate_completions(Shell::Bash);
    let text = String::from_utf8(output).expect("valid UTF-8");

    let expected = [
        "ls-fonts",
        "show-keys",
        "list-themes",
        "validate-config",
        "show-config",
        "completions",
    ];
    for name in expected {
        assert!(
            text.contains(name),
            "completions should contain subcommand {name:?}"
        );
    }
}

// ── format_binding with SendText action ──

#[test]
fn format_binding_with_send_text_action() {
    let b = KeyBinding {
        key: BindingKey::Character("a".to_owned()),
        mods: Modifiers::ALT,
        action: Action::SendText("\x1b[A".to_owned()),
    };
    let s = format_binding(&b);
    assert!(s.contains("Alt"), "should contain Alt modifier: {s}");
    assert!(s.contains("A"), "should contain key: {s}");
    assert!(
        s.contains("SendText:"),
        "should contain SendText prefix: {s}"
    );
    assert!(s.contains("->"), "should contain arrow separator: {s}");
}
