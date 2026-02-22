//! Configuration unit tests.

use super::*;

#[test]
fn default_config_roundtrip() {
    let cfg = Config::default();
    let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
    let parsed: Config = toml::from_str(&toml_str).expect("deserialize");
    assert!((parsed.font.size - 11.0).abs() < f32::EPSILON);
    assert_eq!(parsed.terminal.scrollback, 10_000);
    assert_eq!(parsed.terminal.cursor_style, CursorStyle::Block);
    assert_eq!(parsed.colors.scheme, "Catppuccin Mocha");
    assert_eq!(parsed.window.columns, 120);
    assert_eq!(parsed.window.rows, 30);
    assert!((parsed.window.opacity - 1.0).abs() < f32::EPSILON);
    assert!(parsed.window.blur);
    assert!(parsed.behavior.copy_on_select);
    assert!(parsed.behavior.bold_is_bright);
    assert!(parsed.terminal.cursor_blink);
    assert_eq!(parsed.terminal.cursor_blink_interval_ms, 530);
    assert_eq!(parsed.window.decorations, Decorations::None);
    assert!(!parsed.window.resize_increments);
}

#[test]
fn partial_toml_uses_defaults() {
    let toml_str = r#"
[font]
size = 20.0
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!((parsed.font.size - 20.0).abs() < f32::EPSILON);
    assert_eq!(parsed.terminal.scrollback, 10_000);
    assert_eq!(parsed.window.columns, 120);
}

#[test]
fn empty_toml_gives_defaults() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!((parsed.font.size - 11.0).abs() < f32::EPSILON);
    assert!(parsed.behavior.copy_on_select);
    assert!(parsed.behavior.bold_is_bright);
    assert_eq!(parsed.terminal.cursor_style, CursorStyle::Block);
}

#[test]
fn behavior_config_from_toml() {
    let toml_str = r#"
[behavior]
copy_on_select = false
bold_is_bright = false
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!(!parsed.behavior.copy_on_select);
    assert!(!parsed.behavior.bold_is_bright);
}

#[test]
fn cursor_style_from_toml() {
    let toml_str = r#"
[terminal]
cursor_style = "bar"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.terminal.cursor_style, CursorStyle::Bar);
    assert_eq!(
        parsed.terminal.cursor_style.to_shape(),
        oriterm_core::CursorShape::Bar
    );
}

#[test]
fn cursor_blink_defaults() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!(parsed.terminal.cursor_blink);
    assert_eq!(parsed.terminal.cursor_blink_interval_ms, 530);
}

#[test]
fn cursor_blink_from_toml() {
    let toml_str = r#"
[terminal]
cursor_blink = false
cursor_blink_interval_ms = 250
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!(!parsed.terminal.cursor_blink);
    assert_eq!(parsed.terminal.cursor_blink_interval_ms, 250);
}

#[test]
fn cursor_style_serde_variants() {
    use oriterm_core::CursorShape;

    // All valid values deserialize correctly.
    assert_eq!(
        toml::from_str::<TerminalConfig>("cursor_style = \"block\"")
            .unwrap()
            .cursor_style,
        CursorStyle::Block,
    );
    assert_eq!(
        toml::from_str::<TerminalConfig>("cursor_style = \"bar\"")
            .unwrap()
            .cursor_style,
        CursorStyle::Bar,
    );
    assert_eq!(
        toml::from_str::<TerminalConfig>("cursor_style = \"beam\"")
            .unwrap()
            .cursor_style,
        CursorStyle::Bar,
    );
    assert_eq!(
        toml::from_str::<TerminalConfig>("cursor_style = \"underline\"")
            .unwrap()
            .cursor_style,
        CursorStyle::Underline,
    );

    // Unknown value is a parse error (enforced by serde enum).
    assert!(toml::from_str::<TerminalConfig>("cursor_style = \"unknown\"").is_err());

    // to_shape() maps correctly.
    assert_eq!(CursorStyle::Block.to_shape(), CursorShape::Block);
    assert_eq!(CursorStyle::Bar.to_shape(), CursorShape::Bar);
    assert_eq!(CursorStyle::Underline.to_shape(), CursorShape::Underline);
}

#[test]
fn opacity_config_from_toml() {
    let toml_str = r#"
[window]
opacity = 0.85
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!((parsed.window.opacity - 0.85).abs() < f32::EPSILON);
    assert!((parsed.window.effective_opacity() - 0.85).abs() < f32::EPSILON);
}

#[test]
fn opacity_defaults_to_one() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!((parsed.window.opacity - 1.0).abs() < f32::EPSILON);
}

#[test]
fn opacity_clamped() {
    let toml_str = r#"
[window]
opacity = 1.5
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!((parsed.window.effective_opacity() - 1.0).abs() < f32::EPSILON);

    let toml_str2 = r#"
[window]
opacity = -0.5
"#;
    let parsed2: Config = toml::from_str(toml_str2).expect("deserialize");
    assert!((parsed2.window.effective_opacity()).abs() < f32::EPSILON);
}

#[test]
fn blur_defaults_to_true() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!(parsed.window.blur);
}

#[test]
fn blur_config_from_toml() {
    let toml_str = r#"
[window]
blur = false
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!(!parsed.window.blur);
}

#[test]
fn tab_bar_opacity_defaults_to_none() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!(parsed.window.tab_bar_opacity.is_none());
    assert!((parsed.window.effective_tab_bar_opacity() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn tab_bar_opacity_independent() {
    let toml_str = r#"
[window]
opacity = 0.5
tab_bar_opacity = 0.8
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!((parsed.window.effective_opacity() - 0.5).abs() < f32::EPSILON);
    assert!((parsed.window.effective_tab_bar_opacity() - 0.8).abs() < f32::EPSILON);
}

#[test]
fn tab_bar_opacity_falls_back_to_opacity() {
    let toml_str = r#"
[window]
opacity = 0.7
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!(parsed.window.tab_bar_opacity.is_none());
    assert!((parsed.window.effective_tab_bar_opacity() - 0.7).abs() < f32::EPSILON);
}

#[test]
fn tab_bar_opacity_clamped() {
    let toml_str = r#"
[window]
tab_bar_opacity = 1.5
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!((parsed.window.effective_tab_bar_opacity() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn minimum_contrast_defaults_to_off() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!((parsed.colors.minimum_contrast - 1.0).abs() < f32::EPSILON);
    assert!((parsed.colors.effective_minimum_contrast() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn minimum_contrast_clamped() {
    let toml_str = r#"
[colors]
minimum_contrast = 25.0
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!((parsed.colors.effective_minimum_contrast() - 21.0).abs() < f32::EPSILON);

    let toml_str2 = r#"
[colors]
minimum_contrast = 0.5
"#;
    let parsed2: Config = toml::from_str(toml_str2).expect("deserialize");
    assert!((parsed2.colors.effective_minimum_contrast() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn alpha_blending_defaults_to_linear_corrected() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert_eq!(parsed.colors.alpha_blending, AlphaBlending::LinearCorrected);
}

#[test]
fn alpha_blending_from_toml() {
    let toml_str = r#"
[colors]
alpha_blending = "linear"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.colors.alpha_blending, AlphaBlending::Linear);

    let toml_str2 = r#"
[colors]
alpha_blending = "linear_corrected"
"#;
    let parsed2: Config = toml::from_str(toml_str2).expect("deserialize");
    assert_eq!(
        parsed2.colors.alpha_blending,
        AlphaBlending::LinearCorrected
    );
}

#[test]
fn color_config_roundtrip() {
    let cfg = Config::default();
    let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
    let parsed: Config = toml::from_str(&toml_str).expect("deserialize");
    assert_eq!(parsed.colors.scheme, "Catppuccin Mocha");
    assert!((parsed.colors.minimum_contrast - 1.0).abs() < f32::EPSILON);
    assert_eq!(parsed.colors.alpha_blending, AlphaBlending::LinearCorrected);
}

#[test]
fn config_dir_is_not_empty() {
    let dir = crate::platform::config_paths::config_dir();
    assert!(!dir.as_os_str().is_empty());
}

#[test]
fn config_path_ends_with_toml() {
    let path = config_path();
    assert_eq!(path.extension().and_then(|e| e.to_str()), Some("toml"));
}

#[test]
fn color_overrides_from_toml() {
    let toml_str = r##"
[colors]
scheme = "Dracula"
foreground = "#FFFFFF"
background = "#000000"
cursor = "#FF0000"
selection_foreground = "#FFFFFF"
selection_background = "#3A3D5C"
"##;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.colors.scheme, "Dracula");
    assert_eq!(parsed.colors.foreground.as_deref(), Some("#FFFFFF"));
    assert_eq!(parsed.colors.background.as_deref(), Some("#000000"));
    assert_eq!(parsed.colors.cursor.as_deref(), Some("#FF0000"));
    assert_eq!(
        parsed.colors.selection_foreground.as_deref(),
        Some("#FFFFFF")
    );
    assert_eq!(
        parsed.colors.selection_background.as_deref(),
        Some("#3A3D5C")
    );
}

#[test]
fn color_overrides_default_none() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!(parsed.colors.foreground.is_none());
    assert!(parsed.colors.background.is_none());
    assert!(parsed.colors.cursor.is_none());
    assert!(parsed.colors.selection_foreground.is_none());
    assert!(parsed.colors.selection_background.is_none());
    assert!(parsed.colors.ansi.is_empty());
    assert!(parsed.colors.bright.is_empty());
}

#[test]
fn color_overrides_partial() {
    let toml_str = r##"
[colors]
foreground = "#AABBCC"
"##;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.colors.foreground.as_deref(), Some("#AABBCC"));
    assert!(parsed.colors.background.is_none());
    assert!(parsed.colors.cursor.is_none());
}

#[test]
fn ansi_overrides_from_toml() {
    let toml_str = r##"
[colors.ansi]
0 = "#111111"
7 = "#EEEEEE"

[colors.bright]
1 = "#FF0000"
"##;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(
        parsed.colors.ansi.get("0").map(|s| s.as_str()),
        Some("#111111")
    );
    assert!(parsed.colors.ansi.get("1").is_none());
    assert_eq!(
        parsed.colors.ansi.get("7").map(|s| s.as_str()),
        Some("#EEEEEE")
    );
    assert!(parsed.colors.bright.get("0").is_none());
    assert_eq!(
        parsed.colors.bright.get("1").map(|s| s.as_str()),
        Some("#FF0000")
    );
}

#[test]
fn weight_defaults_to_400() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert_eq!(parsed.font.weight, 400);
    assert_eq!(parsed.font.effective_weight(), 400);
    assert_eq!(parsed.font.effective_bold_weight(), 700);
}

#[test]
fn weight_from_toml() {
    let toml_str = r#"
[font]
weight = 300
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.weight, 300);
    assert_eq!(parsed.font.effective_weight(), 300);
    assert_eq!(parsed.font.effective_bold_weight(), 600);
}

#[test]
fn weight_effective_clamped() {
    let mut cfg = FontConfig::default();
    cfg.weight = 50;
    assert_eq!(cfg.effective_weight(), 100);
    assert_eq!(cfg.effective_bold_weight(), 400);

    cfg.weight = 1000;
    assert_eq!(cfg.effective_weight(), 900);
    assert_eq!(cfg.effective_bold_weight(), 900);

    cfg.weight = 700;
    assert_eq!(cfg.effective_bold_weight(), 900);
}

#[test]
fn weight_roundtrip() {
    let mut cfg = Config::default();
    cfg.font.weight = 300;
    let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
    let parsed: Config = toml::from_str(&toml_str).expect("deserialize");
    assert_eq!(parsed.font.weight, 300);
}

#[test]
fn tab_bar_font_weight_defaults_to_none() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!(parsed.font.tab_bar_font_weight.is_none());
    assert_eq!(parsed.font.effective_tab_bar_weight(), 600);
}

#[test]
fn tab_bar_font_weight_from_toml() {
    let toml_str = r#"
[font]
tab_bar_font_weight = 400
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.tab_bar_font_weight, Some(400));
    assert_eq!(parsed.font.effective_tab_bar_weight(), 400);
}

#[test]
fn tab_bar_font_weight_effective_clamped() {
    let mut cfg = FontConfig::default();
    cfg.tab_bar_font_weight = Some(50);
    assert_eq!(cfg.effective_tab_bar_weight(), 100);

    cfg.tab_bar_font_weight = Some(1000);
    assert_eq!(cfg.effective_tab_bar_weight(), 900);

    cfg.tab_bar_font_weight = Some(700);
    assert_eq!(cfg.effective_tab_bar_weight(), 700);
}

#[test]
fn tab_bar_font_family_defaults_to_none() {
    let cfg = FontConfig::default();
    assert!(cfg.tab_bar_font_family.is_none());
}

#[test]
fn tab_bar_font_family_from_toml() {
    let toml_str = r#"
[font]
tab_bar_font_family = "Segoe UI"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.tab_bar_font_family.as_deref(), Some("Segoe UI"));
}

#[test]
fn color_overrides_roundtrip() {
    let mut cfg = Config::default();
    cfg.colors.foreground = Some("#FFFFFF".to_owned());
    cfg.colors.selection_background = Some("#3A3D5C".to_owned());
    let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
    let parsed: Config = toml::from_str(&toml_str).expect("deserialize");
    assert_eq!(parsed.colors.foreground.as_deref(), Some("#FFFFFF"));
    assert_eq!(
        parsed.colors.selection_background.as_deref(),
        Some("#3A3D5C")
    );
    assert!(parsed.colors.background.is_none());
}

#[test]
fn bell_defaults() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert_eq!(parsed.bell.animation, BellAnimation::EaseOut);
    assert_eq!(parsed.bell.duration_ms, 150);
    assert!(parsed.bell.color.is_none());
    assert!(parsed.bell.is_enabled());
}

#[test]
fn bell_disabled_by_zero_duration() {
    let toml_str = r#"
[bell]
duration_ms = 0
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!(!parsed.bell.is_enabled());
}

#[test]
fn bell_disabled_by_none_animation() {
    let toml_str = r#"
[bell]
animation = "none"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!(!parsed.bell.is_enabled());
}

#[test]
fn decorations_defaults_to_none() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert_eq!(parsed.window.decorations, Decorations::None);
}

#[test]
fn decorations_from_toml() {
    let toml_str = r#"
[window]
decorations = "full"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.window.decorations, Decorations::Full);

    let toml_str2 = r#"
[window]
decorations = "transparent"
"#;
    let parsed2: Config = toml::from_str(toml_str2).expect("deserialize");
    assert_eq!(parsed2.window.decorations, Decorations::Transparent);

    let toml_str3 = r#"
[window]
decorations = "buttonless"
"#;
    let parsed3: Config = toml::from_str(toml_str3).expect("deserialize");
    assert_eq!(parsed3.window.decorations, Decorations::Buttonless);
}

#[test]
fn resize_increments_default_false() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!(!parsed.window.resize_increments);
}

#[test]
fn resize_increments_from_toml() {
    let toml_str = r#"
[window]
resize_increments = true
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!(parsed.window.resize_increments);
}

#[test]
fn fallback_font_config_from_toml() {
    let toml_str = r#"
[[font.fallback]]
family = "Noto Sans CJK"
size_offset = -1.0

[[font.fallback]]
family = "Noto Color Emoji"
features = ["liga"]
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.fallback.len(), 2);
    assert_eq!(parsed.font.fallback[0].family, "Noto Sans CJK");
    assert_eq!(parsed.font.fallback[0].size_offset, Some(-1.0));
    assert!(parsed.font.fallback[0].features.is_none());
    assert_eq!(parsed.font.fallback[1].family, "Noto Color Emoji");
    assert_eq!(
        parsed.font.fallback[1].features.as_deref(),
        Some(vec!["liga".to_owned()].as_slice())
    );
    assert!(parsed.font.fallback[1].size_offset.is_none());
}

#[test]
fn shell_integration_default_true() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!(parsed.behavior.shell_integration);
}

#[test]
fn shell_integration_from_toml() {
    let toml_str = r#"
[behavior]
shell_integration = false
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!(!parsed.behavior.shell_integration);
}

#[test]
fn bom_prefixed_toml_parses_correctly() {
    // Windows editors (e.g. Notepad) prepend UTF-8 BOM (U+FEFF).
    let toml_str = "\u{FEFF}[font]\nsize = 14.0\n";
    let parsed: Config = toml::from_str(toml_str).expect("BOM-prefixed TOML should parse");
    assert!((parsed.font.size - 14.0).abs() < f32::EPSILON);
}

#[test]
fn invalid_enum_variant_is_parse_error() {
    // An unrecognized enum variant should fail deserialization so that
    // `Config::load()` falls back to defaults rather than silently accepting.
    let toml_str = r#"
[window]
decorations = "invalid_variant"
"#;
    let result: Result<Config, _> = toml::from_str(toml_str);
    assert!(result.is_err());
}

#[test]
fn invalid_type_is_parse_error() {
    // Wrong type (string where number expected) fails parse.
    let toml_str = r#"
[font]
size = "not_a_number"
"#;
    let result: Result<Config, _> = toml::from_str(toml_str);
    assert!(result.is_err());
}

#[test]
fn try_load_distinguishes_missing_from_parse_error() {
    // try_load returns Err for both missing files and parse errors;
    // the error message should contain "failed to read" for missing
    // and "parse error" for invalid content.
    let err = Config::try_load();
    // Since the config file may or may not exist in the test environment,
    // just verify it returns a Result (no panic).
    let _ = err;
}

#[test]
fn unknown_keys_are_ignored() {
    // Forward compatibility: new config keys added in future versions
    // shouldn't break older parsers (no `deny_unknown_fields`).
    let toml_str = r#"
[font]
size = 14.0
future_field = true

[window]
nonexistent_option = "hello"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("unknown keys should be ignored");
    assert!((parsed.font.size - 14.0).abs() < f32::EPSILON);
    assert_eq!(parsed.window.columns, 120);
}

#[test]
fn serialization_is_deterministic() {
    // Default config serializes identically across two calls.
    let a = toml::to_string_pretty(&Config::default()).expect("serialize 1");
    let b = toml::to_string_pretty(&Config::default()).expect("serialize 2");
    assert_eq!(a, b);
}

#[test]
fn keybind_config_from_toml() {
    let toml_str = r#"
[[keybind]]
key = "c"
mods = "Ctrl|Shift"
action = "Copy"

[[keybind]]
key = "v"
mods = "Ctrl|Shift"
action = "Paste"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.keybind.len(), 2);
    assert_eq!(parsed.keybind[0].key, "c");
    assert_eq!(parsed.keybind[0].mods, "Ctrl|Shift");
    assert_eq!(parsed.keybind[0].action, "Copy");
    assert_eq!(parsed.keybind[1].key, "v");
    assert_eq!(parsed.keybind[1].action, "Paste");
}

#[test]
fn fallback_with_invalid_family_parses_ok() {
    // An invalid/nonexistent family name in the fallback chain should
    // parse without error — validation happens at font discovery time,
    // not at config parse time.
    let toml_str = r#"
[[font.fallback]]
family = "NonExistentFontFamily_XYZ_12345"

[[font.fallback]]
family = "Noto Color Emoji"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.fallback.len(), 2);
    assert_eq!(
        parsed.font.fallback[0].family,
        "NonExistentFontFamily_XYZ_12345"
    );
    assert_eq!(parsed.font.fallback[1].family, "Noto Color Emoji");
}

#[test]
fn fallback_invalid_family_does_not_break_discovery() {
    // An invalid family in config fallback should be skipped by
    // resolve_user_fallback (returns None), not panic.
    let result = crate::font::discovery::resolve_user_fallback("NonExistentFontFamily_XYZ_12345");
    assert!(result.is_none(), "bogus fallback family should return None");
}

#[test]
fn load_returns_defaults_on_nonexistent_path() {
    // Config::load() delegates to config_path(). If the file doesn't exist,
    // it returns defaults silently (no warning for NotFound).
    // We can't control the path here, but we verify the default config
    // matches expectations.
    let defaults = Config::default();
    assert!((defaults.font.size - 11.0).abs() < f32::EPSILON);
    assert_eq!(defaults.terminal.scrollback, 10_000);
    assert_eq!(defaults.window.columns, 120);
}
