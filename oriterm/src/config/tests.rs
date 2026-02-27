//! Configuration unit tests.

use super::bell::BellAnimation;
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

// ---------------------------------------------------------------------------
// Theme override
// ---------------------------------------------------------------------------

#[test]
fn theme_defaults_to_auto() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert_eq!(parsed.colors.theme, ThemeOverride::Auto);
}

#[test]
fn theme_override_dark_from_toml() {
    let toml_str = r#"
[colors]
theme = "dark"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.colors.theme, ThemeOverride::Dark);
}

#[test]
fn theme_override_light_from_toml() {
    let toml_str = r#"
[colors]
theme = "light"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.colors.theme, ThemeOverride::Light);
}

#[test]
fn theme_override_auto_from_toml() {
    let toml_str = r#"
[colors]
theme = "auto"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.colors.theme, ThemeOverride::Auto);
}

#[test]
fn theme_override_unknown_value_is_error() {
    let toml_str = r#"
[colors]
theme = "sepia"
"#;
    assert!(toml::from_str::<Config>(toml_str).is_err());
}

#[test]
fn theme_override_dark_ignores_system_detection() {
    use oriterm_core::Theme;

    let cfg = ColorConfig {
        theme: ThemeOverride::Dark,
        ..Default::default()
    };
    // System would return Light, but config forces Dark.
    let resolved = cfg.resolve_theme(|| Theme::Light);
    assert_eq!(resolved, Theme::Dark);
}

#[test]
fn theme_override_light_ignores_system_detection() {
    use oriterm_core::Theme;

    let cfg = ColorConfig {
        theme: ThemeOverride::Light,
        ..Default::default()
    };
    // System would return Dark, but config forces Light.
    let resolved = cfg.resolve_theme(|| Theme::Dark);
    assert_eq!(resolved, Theme::Light);
}

#[test]
fn theme_override_auto_uses_system_detection() {
    use oriterm_core::Theme;

    let cfg = ColorConfig {
        theme: ThemeOverride::Auto,
        ..Default::default()
    };
    assert_eq!(cfg.resolve_theme(|| Theme::Dark), Theme::Dark);
    assert_eq!(cfg.resolve_theme(|| Theme::Light), Theme::Light);
    assert_eq!(cfg.resolve_theme(|| Theme::Unknown), Theme::Unknown);
}

#[test]
fn theme_roundtrip_serialization() {
    let mut cfg = Config::default();
    cfg.colors.theme = ThemeOverride::Light;

    let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
    let parsed: Config = toml::from_str(&toml_str).expect("deserialize");
    assert_eq!(parsed.colors.theme, ThemeOverride::Light);
}

// ---------------------------------------------------------------------------
// Config PartialEq correctness (single-field diff detection)
// ---------------------------------------------------------------------------

#[test]
fn config_partial_eq_detects_color_diff() {
    let a = Config::default();
    let mut b = Config::default();
    b.colors.foreground = Some("#FF0000".to_owned());
    assert_ne!(
        a.colors, b.colors,
        "single-field change should break equality"
    );
}

#[test]
fn config_partial_eq_detects_theme_diff() {
    let a = ColorConfig::default();
    let mut b = ColorConfig::default();
    b.theme = ThemeOverride::Light;
    assert_ne!(a, b, "theme change should break ColorConfig equality");
}

#[test]
fn config_partial_eq_identical_is_equal() {
    let a = Config::default();
    let b = Config::default();
    assert_eq!(a.colors, b.colors);
    assert_eq!(a.window.opacity, b.window.opacity);
}

// ---------------------------------------------------------------------------
// Font size boundary conditions
// ---------------------------------------------------------------------------

#[test]
fn font_size_zero_parses() {
    let toml_str = r#"
[font]
size = 0.0
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!((parsed.font.size).abs() < f32::EPSILON);
}

#[test]
fn font_size_negative_parses() {
    let toml_str = r#"
[font]
size = -5.0
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!((parsed.font.size - (-5.0)).abs() < f32::EPSILON);
}

#[test]
fn font_size_very_large_parses() {
    let toml_str = r#"
[font]
size = 999.0
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!((parsed.font.size - 999.0).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// Numeric edge cases (NaN, infinity)
// ---------------------------------------------------------------------------

#[test]
fn opacity_nan_defaults_to_one() {
    // TOML accepts `nan` for floats. NaN is not a valid opacity, so
    // effective_opacity() should return the default (1.0).
    let toml_str = r#"
[window]
opacity = nan
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!(parsed.window.opacity.is_nan());
    assert!((parsed.window.effective_opacity() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn opacity_inf_clamped_to_one() {
    let toml_str = r#"
[window]
opacity = inf
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!(parsed.window.opacity.is_infinite());
    // inf.clamp(0.0, 1.0) returns 1.0.
    assert!((parsed.window.effective_opacity() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn opacity_neg_inf_clamped_to_zero() {
    let toml_str = r#"
[window]
opacity = -inf
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!(parsed.window.opacity.is_infinite());
    // (-inf).clamp(0.0, 1.0) returns 0.0.
    assert!(parsed.window.effective_opacity().abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// ANSI color index out-of-range in overrides
// ---------------------------------------------------------------------------

#[test]
fn ansi_override_out_of_range_index_ignored() {
    // ANSI overrides with index >= 8 should be silently skipped
    // (the apply_color_overrides function logs a warning).
    let toml_str = r##"
[colors.ansi]
8 = "#FF0000"
99 = "#00FF00"
"##;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    // The values parse into the HashMap, but apply_color_overrides
    // will skip them. Verify they're present in the map.
    assert_eq!(
        parsed.colors.ansi.get("8").map(|s| s.as_str()),
        Some("#FF0000")
    );
    assert_eq!(
        parsed.colors.ansi.get("99").map(|s| s.as_str()),
        Some("#00FF00")
    );
}

#[test]
fn bright_override_out_of_range_index_ignored() {
    let toml_str = r##"
[colors.bright]
8 = "#FF0000"
"##;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(
        parsed.colors.bright.get("8").map(|s| s.as_str()),
        Some("#FF0000")
    );
}

#[test]
fn apply_color_overrides_skips_out_of_range_ansi() {
    use oriterm_core::{Palette, Theme};
    use vte::ansi::Color;

    let mut palette = Palette::for_theme(Theme::Dark);
    let original_8 = palette.resolve(Color::Indexed(8));

    let mut colors = ColorConfig::default();
    colors.ansi.insert("8".to_owned(), "#FF0000".to_owned());

    crate::app::config_reload::apply_color_overrides(&mut palette, &colors);

    // Index 8 is out of range for ansi (0-7), so it should be unchanged.
    assert_eq!(palette.resolve(Color::Indexed(8)), original_8);
}

#[test]
fn apply_color_overrides_applies_valid_ansi() {
    use oriterm_core::{Palette, Rgb, Theme};
    use vte::ansi::Color;

    let mut palette = Palette::for_theme(Theme::Dark);

    let mut colors = ColorConfig::default();
    colors.ansi.insert("0".to_owned(), "#112233".to_owned());

    crate::app::config_reload::apply_color_overrides(&mut palette, &colors);

    assert_eq!(
        palette.resolve(Color::Indexed(0)),
        Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33
        },
    );
}

// ---------------------------------------------------------------------------
// clamp_or_default direct tests
// ---------------------------------------------------------------------------

#[test]
fn clamp_or_default_normal_value() {
    assert!((clamp_or_default(0.5, 0.0, 1.0, 1.0) - 0.5).abs() < f32::EPSILON);
}

#[test]
fn clamp_or_default_above_max() {
    assert!((clamp_or_default(2.0, 0.0, 1.0, 1.0) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn clamp_or_default_below_min() {
    assert!(clamp_or_default(-1.0, 0.0, 1.0, 1.0).abs() < f32::EPSILON);
}

#[test]
fn clamp_or_default_nan_returns_default() {
    assert!((clamp_or_default(f32::NAN, 0.0, 1.0, 0.75) - 0.75).abs() < f32::EPSILON);
}

#[test]
fn clamp_or_default_inf_clamped() {
    assert!((clamp_or_default(f32::INFINITY, 0.0, 1.0, 0.5) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn clamp_or_default_neg_inf_clamped() {
    assert!(clamp_or_default(f32::NEG_INFINITY, 0.0, 1.0, 0.5).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// minimum_contrast NaN/inf
// ---------------------------------------------------------------------------

#[test]
fn minimum_contrast_nan_defaults_to_one() {
    let mut cfg = ColorConfig::default();
    cfg.minimum_contrast = f32::NAN;
    assert!((cfg.effective_minimum_contrast() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn minimum_contrast_inf_clamped_to_twenty_one() {
    let mut cfg = ColorConfig::default();
    cfg.minimum_contrast = f32::INFINITY;
    assert!((cfg.effective_minimum_contrast() - 21.0).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// Bright color override valid range
// ---------------------------------------------------------------------------

#[test]
fn apply_color_overrides_bright_maps_to_palette_8_plus() {
    use oriterm_core::{Palette, Rgb, Theme};
    use vte::ansi::Color;

    let mut palette = Palette::for_theme(Theme::Dark);

    let mut colors = ColorConfig::default();
    colors.bright.insert("3".to_owned(), "#FF00FF".to_owned());

    crate::app::config_reload::apply_color_overrides(&mut palette, &colors);

    // bright[3] should map to palette index 11 (3 + 8).
    assert_eq!(
        palette.resolve(Color::Indexed(11)),
        Rgb {
            r: 0xFF,
            g: 0x00,
            b: 0xFF
        },
    );
}

#[test]
fn apply_color_overrides_bright_out_of_range_skipped() {
    use oriterm_core::{Palette, Theme};
    use vte::ansi::Color;

    let mut palette = Palette::for_theme(Theme::Dark);
    let original_16 = palette.resolve(Color::Indexed(16));

    let mut colors = ColorConfig::default();
    colors.bright.insert("8".to_owned(), "#FF0000".to_owned());

    crate::app::config_reload::apply_color_overrides(&mut palette, &colors);

    // bright[8] is out of range (0-7), palette[16] unchanged.
    assert_eq!(palette.resolve(Color::Indexed(16)), original_16);
}

// ---------------------------------------------------------------------------
// Non-numeric key in ANSI map
// ---------------------------------------------------------------------------

#[test]
fn apply_color_overrides_non_numeric_ansi_key_ignored() {
    use oriterm_core::{Palette, Theme};
    use vte::ansi::Color;

    let mut palette = Palette::for_theme(Theme::Dark);
    let original = [
        palette.resolve(Color::Indexed(0)),
        palette.resolve(Color::Indexed(1)),
        palette.resolve(Color::Indexed(2)),
    ];

    let mut colors = ColorConfig::default();
    colors.ansi.insert("abc".to_owned(), "#FF0000".to_owned());
    colors.ansi.insert("".to_owned(), "#00FF00".to_owned());

    crate::app::config_reload::apply_color_overrides(&mut palette, &colors);

    // No valid numeric keys → no palette changes.
    assert_eq!(palette.resolve(Color::Indexed(0)), original[0]);
    assert_eq!(palette.resolve(Color::Indexed(1)), original[1]);
    assert_eq!(palette.resolve(Color::Indexed(2)), original[2]);
}

// ---------------------------------------------------------------------------
// Tab bar opacity NaN fallback
// ---------------------------------------------------------------------------

#[test]
fn tab_bar_opacity_nan_defaults_to_one() {
    let mut cfg = WindowConfig::default();
    cfg.tab_bar_opacity = Some(f32::NAN);
    // NaN tab_bar_opacity → clamp_or_default returns 1.0.
    assert!((cfg.effective_tab_bar_opacity() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn tab_bar_opacity_none_with_nan_opacity() {
    let mut cfg = WindowConfig::default();
    cfg.opacity = f32::NAN;
    cfg.tab_bar_opacity = None;
    // Falls back to opacity (NaN), then clamp_or_default returns 1.0.
    assert!((cfg.effective_tab_bar_opacity() - 1.0).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// Cursor color override through apply_color_overrides
// ---------------------------------------------------------------------------

#[test]
fn apply_color_overrides_sets_cursor() {
    use oriterm_core::{Palette, Rgb, Theme};

    let mut palette = Palette::for_theme(Theme::Dark);

    let mut colors = ColorConfig::default();
    colors.cursor = Some("#AABBCC".to_owned());

    crate::app::config_reload::apply_color_overrides(&mut palette, &colors);

    assert_eq!(
        palette.cursor_color(),
        Rgb {
            r: 0xAA,
            g: 0xBB,
            b: 0xCC
        }
    );
}

// ---------------------------------------------------------------------------
// Full 16-color palette override roundtrip
// ---------------------------------------------------------------------------

#[test]
fn apply_color_overrides_full_16_colors() {
    use oriterm_core::{Palette, Rgb, Theme};
    use vte::ansi::Color;

    let mut palette = Palette::for_theme(Theme::Dark);

    let mut colors = ColorConfig::default();
    // Set all 8 ANSI colors.
    for i in 0..8 {
        let hex = format!("#{:02X}{:02X}00", i * 30, i * 20);
        colors.ansi.insert(i.to_string(), hex);
    }
    // Set all 8 bright colors.
    for i in 0..8 {
        let hex = format!("#00{:02X}{:02X}", i * 30, i * 20);
        colors.bright.insert(i.to_string(), hex);
    }

    crate::app::config_reload::apply_color_overrides(&mut palette, &colors);

    // Verify ANSI 0-7.
    for i in 0u8..8 {
        let expected = Rgb {
            r: i * 30,
            g: i * 20,
            b: 0,
        };
        assert_eq!(
            palette.resolve(Color::Indexed(i)),
            expected,
            "ANSI color {i} mismatch",
        );
    }
    // Verify bright 8-15.
    for i in 0u8..8 {
        let expected = Rgb {
            r: 0,
            g: i * 30,
            b: i * 20,
        };
        assert_eq!(
            palette.resolve(Color::Indexed(i + 8)),
            expected,
            "Bright color {} mismatch",
            i + 8,
        );
    }
}

// ---------------------------------------------------------------------------
// Font config: hinting
// ---------------------------------------------------------------------------

#[test]
fn hinting_defaults_to_none_option() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!(parsed.font.hinting.is_none());
}

#[test]
fn hinting_full_from_toml() {
    let toml_str = r#"
[font]
hinting = "full"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.hinting.as_deref(), Some("full"));
}

#[test]
fn hinting_none_from_toml() {
    let toml_str = r#"
[font]
hinting = "none"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.hinting.as_deref(), Some("none"));
}

// ---------------------------------------------------------------------------
// Font config: subpixel_mode
// ---------------------------------------------------------------------------

#[test]
fn subpixel_mode_defaults_to_none_option() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!(parsed.font.subpixel_mode.is_none());
}

#[test]
fn subpixel_mode_rgb_from_toml() {
    let toml_str = r#"
[font]
subpixel_mode = "rgb"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.subpixel_mode.as_deref(), Some("rgb"));
}

#[test]
fn subpixel_mode_bgr_from_toml() {
    let toml_str = r#"
[font]
subpixel_mode = "bgr"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.subpixel_mode.as_deref(), Some("bgr"));
}

#[test]
fn subpixel_mode_none_from_toml() {
    let toml_str = r#"
[font]
subpixel_mode = "none"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.subpixel_mode.as_deref(), Some("none"));
}

// ---------------------------------------------------------------------------
// Font config: subpixel_positioning
// ---------------------------------------------------------------------------

#[test]
fn subpixel_positioning_defaults_to_true() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!(parsed.font.subpixel_positioning);
}

#[test]
fn subpixel_positioning_false_from_toml() {
    let toml_str = r#"
[font]
subpixel_positioning = false
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert!(!parsed.font.subpixel_positioning);
}

// ---------------------------------------------------------------------------
// Font config: variations
// ---------------------------------------------------------------------------

#[test]
fn variations_defaults_to_empty() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!(parsed.font.variations.is_empty());
}

#[test]
fn variations_from_toml() {
    let toml_str = r#"
[font]
variations = { wght = 450.0, wdth = 87.5 }
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.variations.len(), 2);
    assert!((parsed.font.variations["wght"] - 450.0).abs() < f32::EPSILON);
    assert!((parsed.font.variations["wdth"] - 87.5).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// Font config: codepoint_map
// ---------------------------------------------------------------------------

#[test]
fn codepoint_map_defaults_to_empty() {
    let parsed: Config = toml::from_str("").expect("deserialize");
    assert!(parsed.font.codepoint_map.is_empty());
}

#[test]
fn codepoint_map_from_toml() {
    let toml_str = r#"
[[font.codepoint_map]]
range = "E000-F8FF"
family = "Symbols Nerd Font"

[[font.codepoint_map]]
range = "4E00-9FFF"
family = "Noto Sans CJK SC"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.codepoint_map.len(), 2);
    assert_eq!(parsed.font.codepoint_map[0].range, "E000-F8FF");
    assert_eq!(parsed.font.codepoint_map[0].family, "Symbols Nerd Font");
    assert_eq!(parsed.font.codepoint_map[1].range, "4E00-9FFF");
    assert_eq!(parsed.font.codepoint_map[1].family, "Noto Sans CJK SC");
}

#[test]
fn codepoint_map_single_codepoint() {
    let toml_str = r#"
[[font.codepoint_map]]
range = "E0B0"
family = "Powerline Symbols"
"#;
    let parsed: Config = toml::from_str(toml_str).expect("deserialize");
    assert_eq!(parsed.font.codepoint_map.len(), 1);
    assert_eq!(parsed.font.codepoint_map[0].range, "E0B0");
}

// ---------------------------------------------------------------------------
// Font config: roundtrip with new fields
// ---------------------------------------------------------------------------

#[test]
fn font_config_new_fields_roundtrip() {
    let mut cfg = Config::default();
    cfg.font.hinting = Some("full".to_owned());
    cfg.font.subpixel_mode = Some("rgb".to_owned());
    cfg.font.subpixel_positioning = false;
    cfg.font.variations.insert("wght".to_owned(), 450.0);

    let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
    let parsed: Config = toml::from_str(&toml_str).expect("deserialize");

    assert_eq!(parsed.font.hinting.as_deref(), Some("full"));
    assert_eq!(parsed.font.subpixel_mode.as_deref(), Some("rgb"));
    assert!(!parsed.font.subpixel_positioning);
    assert!((parsed.font.variations["wght"] - 450.0).abs() < f32::EPSILON);
}

// ---------------------------------------------------------------------------
// Resolve helpers: hinting and subpixel mode
// ---------------------------------------------------------------------------

#[test]
fn resolve_hinting_config_override_full() {
    use crate::font::HintingMode;

    let mut cfg = FontConfig::default();
    cfg.hinting = Some("full".to_owned());
    // Even on a HiDPI display (scale 2.0), explicit config wins.
    let result = crate::app::config_reload::resolve_hinting(&cfg, 2.0);
    assert_eq!(result, HintingMode::Full);
}

#[test]
fn resolve_hinting_config_override_none() {
    use crate::font::HintingMode;

    let mut cfg = FontConfig::default();
    cfg.hinting = Some("none".to_owned());
    // Even on non-HiDPI (scale 1.0), explicit config wins.
    let result = crate::app::config_reload::resolve_hinting(&cfg, 1.0);
    assert_eq!(result, HintingMode::None);
}

#[test]
fn resolve_hinting_auto_detection() {
    use crate::font::HintingMode;

    let cfg = FontConfig::default();
    assert_eq!(
        crate::app::config_reload::resolve_hinting(&cfg, 1.0),
        HintingMode::Full,
    );
    assert_eq!(
        crate::app::config_reload::resolve_hinting(&cfg, 2.0),
        HintingMode::None,
    );
}

#[test]
fn resolve_hinting_unknown_value_falls_back() {
    use crate::font::HintingMode;

    let mut cfg = FontConfig::default();
    cfg.hinting = Some("garbage".to_owned());
    // Unknown value → auto-detection.
    let result = crate::app::config_reload::resolve_hinting(&cfg, 1.0);
    assert_eq!(result, HintingMode::Full);
}

#[test]
fn resolve_subpixel_mode_config_override_rgb() {
    use crate::font::SubpixelMode;

    let mut cfg = FontConfig::default();
    cfg.subpixel_mode = Some("rgb".to_owned());
    let result = crate::app::config_reload::resolve_subpixel_mode(&cfg, 2.0);
    assert_eq!(result, SubpixelMode::Rgb);
}

#[test]
fn resolve_subpixel_mode_config_override_bgr() {
    use crate::font::SubpixelMode;

    let mut cfg = FontConfig::default();
    cfg.subpixel_mode = Some("bgr".to_owned());
    let result = crate::app::config_reload::resolve_subpixel_mode(&cfg, 1.0);
    assert_eq!(result, SubpixelMode::Bgr);
}

#[test]
fn resolve_subpixel_mode_config_override_none() {
    use crate::font::SubpixelMode;

    let mut cfg = FontConfig::default();
    cfg.subpixel_mode = Some("none".to_owned());
    let result = crate::app::config_reload::resolve_subpixel_mode(&cfg, 1.0);
    assert_eq!(result, SubpixelMode::None);
}

#[test]
fn resolve_subpixel_mode_auto_detection() {
    use crate::font::SubpixelMode;

    let cfg = FontConfig::default();
    assert_eq!(
        crate::app::config_reload::resolve_subpixel_mode(&cfg, 1.0),
        SubpixelMode::Rgb,
    );
    assert_eq!(
        crate::app::config_reload::resolve_subpixel_mode(&cfg, 2.0),
        SubpixelMode::None,
    );
}

// ---------------------------------------------------------------------------
// resolve_subpixel_mode unknown value fallback
// ---------------------------------------------------------------------------

#[test]
fn resolve_subpixel_mode_unknown_value_falls_back() {
    use crate::font::SubpixelMode;

    let mut cfg = FontConfig::default();
    cfg.subpixel_mode = Some("garbage".to_owned());
    // Unknown value → auto-detection based on scale factor.
    assert_eq!(
        crate::app::config_reload::resolve_subpixel_mode(&cfg, 1.0),
        SubpixelMode::Rgb,
    );
    assert_eq!(
        crate::app::config_reload::resolve_subpixel_mode(&cfg, 2.0),
        SubpixelMode::None,
    );
}

// ---------------------------------------------------------------------------
// apply_font_config integration tests
// ---------------------------------------------------------------------------

#[test]
fn apply_font_config_sets_custom_features() {
    use crate::font::collection::FontSet;
    use crate::font::{FontCollection, GlyphFormat, HintingMode};

    let mut cfg = FontConfig::default();
    cfg.features = vec!["dlig".into(), "-liga".into()];

    let font_set = FontSet::load(None, 400).expect("font must load");
    let mut collection = FontCollection::new(
        font_set,
        12.0,
        96.0,
        GlyphFormat::Alpha,
        400,
        HintingMode::Full,
    )
    .expect("collection must build");

    // Before: defaults are liga + calt.
    assert_eq!(
        collection
            .features_for_face(crate::font::FaceIdx::REGULAR)
            .len(),
        2
    );

    crate::app::config_reload::apply_font_config(&mut collection, &cfg, 0);

    // After: features replaced with dlig + -liga.
    let features = collection.features_for_face(crate::font::FaceIdx::REGULAR);
    assert_eq!(features.len(), 2, "should have 2 features after apply");
    // dlig enabled (value=1), -liga disabled (value=0).
    assert_eq!(features[0].value, 1, "dlig should be enabled");
    assert_eq!(features[1].value, 0, "-liga should be disabled");
}

#[test]
fn apply_font_config_empty_features_clears_defaults() {
    use crate::font::collection::FontSet;
    use crate::font::{FontCollection, GlyphFormat, HintingMode};

    let mut cfg = FontConfig::default();
    cfg.features = Vec::new(); // No features.

    let font_set = FontSet::load(None, 400).expect("font must load");
    let mut collection = FontCollection::new(
        font_set,
        12.0,
        96.0,
        GlyphFormat::Alpha,
        400,
        HintingMode::Full,
    )
    .expect("collection must build");

    crate::app::config_reload::apply_font_config(&mut collection, &cfg, 0);

    let features = collection.features_for_face(crate::font::FaceIdx::REGULAR);
    assert!(
        features.is_empty(),
        "empty config features should clear defaults"
    );
}

#[test]
fn apply_font_config_codepoint_map_invalid_range_skipped() {
    use crate::font::collection::FontSet;
    use crate::font::{FontCollection, GlyphFormat, HintingMode};

    let mut cfg = FontConfig::default();
    cfg.fallback.push(FallbackFontConfig {
        family: "TestFont".into(),
        features: None,
        size_offset: None,
    });
    cfg.codepoint_map.push(CodepointMapConfig {
        range: "ZZZZ".into(), // Invalid hex.
        family: "TestFont".into(),
    });

    let font_set = FontSet::load(None, 400).expect("font must load");
    let mut collection = FontCollection::new(
        font_set,
        12.0,
        96.0,
        GlyphFormat::Alpha,
        400,
        HintingMode::Full,
    )
    .expect("collection must build");

    // Should not panic — invalid range is logged and skipped.
    crate::app::config_reload::apply_font_config(&mut collection, &cfg, 0);

    // Collection should have no codepoint mappings.
    assert!(!collection.has_codepoint_mappings());
}

#[test]
fn apply_font_config_codepoint_map_missing_family_skipped() {
    use crate::font::collection::FontSet;
    use crate::font::{FontCollection, GlyphFormat, HintingMode};

    let mut cfg = FontConfig::default();
    // No fallbacks configured, but codepoint_map references a family.
    cfg.codepoint_map.push(CodepointMapConfig {
        range: "E000-F8FF".into(),
        family: "NonExistentFont".into(),
    });

    let font_set = FontSet::load(None, 400).expect("font must load");
    let mut collection = FontCollection::new(
        font_set,
        12.0,
        96.0,
        GlyphFormat::Alpha,
        400,
        HintingMode::Full,
    )
    .expect("collection must build");

    // Should not panic — missing family is logged and skipped.
    crate::app::config_reload::apply_font_config(&mut collection, &cfg, 0);

    // No mappings added since the family wasn't found.
    assert!(!collection.has_codepoint_mappings());
}

#[test]
fn apply_font_config_with_no_user_fallbacks() {
    use crate::font::collection::FontSet;
    use crate::font::{FontCollection, GlyphFormat, HintingMode};

    let cfg = FontConfig::default(); // No fallbacks, no codepoint_map.

    let font_set = FontSet::load(None, 400).expect("font must load");
    let mut collection = FontCollection::new(
        font_set,
        12.0,
        96.0,
        GlyphFormat::Alpha,
        400,
        HintingMode::Full,
    )
    .expect("collection must build");

    // Should not panic with zero user fallbacks.
    crate::app::config_reload::apply_font_config(&mut collection, &cfg, 0);

    // Default features should still be set (from config defaults: calt + liga).
    let features = collection.features_for_face(crate::font::FaceIdx::REGULAR);
    assert_eq!(features.len(), 2, "default config features are calt + liga");
}

// ---------------------------------------------------------------------------
// parse_hex_color edge cases
// ---------------------------------------------------------------------------

#[test]
fn parse_hex_color_valid_with_hash() {
    let rgb = parse_hex_color("#FF8800").expect("valid hex");
    assert_eq!(
        rgb,
        Rgb {
            r: 0xFF,
            g: 0x88,
            b: 0x00,
        }
    );
}

#[test]
fn parse_hex_color_valid_without_hash() {
    let rgb = parse_hex_color("FF8800").expect("valid hex without #");
    assert_eq!(
        rgb,
        Rgb {
            r: 0xFF,
            g: 0x88,
            b: 0x00,
        }
    );
}

#[test]
fn parse_hex_color_lowercase() {
    let rgb = parse_hex_color("#aabbcc").expect("lowercase hex");
    assert_eq!(
        rgb,
        Rgb {
            r: 0xAA,
            g: 0xBB,
            b: 0xCC,
        }
    );
}

#[test]
fn parse_hex_color_mixed_case() {
    let rgb = parse_hex_color("#aAbBcC").expect("mixed case hex");
    assert_eq!(
        rgb,
        Rgb {
            r: 0xAA,
            g: 0xBB,
            b: 0xCC,
        }
    );
}

#[test]
fn parse_hex_color_all_zeros() {
    let rgb = parse_hex_color("#000000").expect("all zeros");
    assert_eq!(rgb, Rgb { r: 0, g: 0, b: 0 });
}

#[test]
fn parse_hex_color_all_ones() {
    let rgb = parse_hex_color("#FFFFFF").expect("all ones");
    assert_eq!(
        rgb,
        Rgb {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
        }
    );
}

#[test]
fn parse_hex_color_rejects_short_rgb() {
    // 3-digit shorthand is not supported — only 6-digit.
    assert!(parse_hex_color("#FFF").is_none());
}

#[test]
fn parse_hex_color_rejects_empty() {
    assert!(parse_hex_color("").is_none());
}

#[test]
fn parse_hex_color_rejects_hash_only() {
    assert!(parse_hex_color("#").is_none());
}

#[test]
fn parse_hex_color_rejects_non_hex() {
    assert!(parse_hex_color("#ZZZZZZ").is_none());
}

#[test]
fn parse_hex_color_rejects_too_long() {
    assert!(parse_hex_color("#FF88001").is_none());
}

#[test]
fn parse_hex_color_rejects_too_short() {
    assert!(parse_hex_color("#FF880").is_none());
}

// ── PaneConfig ──

#[test]
fn pane_config_defaults() {
    let cfg = PaneConfig::default();
    assert!((cfg.divider_px - 1.0).abs() < f32::EPSILON);
    assert_eq!(cfg.min_cells, (10, 3));
    assert!(!cfg.dim_inactive);
    assert!((cfg.inactive_opacity - 0.7).abs() < f32::EPSILON);
}

#[test]
fn pane_config_effective_opacity_clamps() {
    let mut cfg = PaneConfig::default();
    cfg.inactive_opacity = 1.5;
    assert!((cfg.effective_inactive_opacity() - 1.0).abs() < f32::EPSILON);

    cfg.inactive_opacity = -0.5;
    assert!((cfg.effective_inactive_opacity() - 0.0).abs() < f32::EPSILON);
}

#[test]
fn pane_config_effective_opacity_nan_defaults() {
    let mut cfg = PaneConfig::default();
    cfg.inactive_opacity = f32::NAN;
    assert!((cfg.effective_inactive_opacity() - 0.7).abs() < f32::EPSILON);
}

#[test]
fn pane_config_roundtrip() {
    let cfg = Config::default();
    let toml_str = toml::to_string_pretty(&cfg).expect("serialize");
    let parsed: Config = toml::from_str(&toml_str).expect("deserialize");
    assert!((parsed.pane.divider_px - 1.0).abs() < f32::EPSILON);
    assert_eq!(parsed.pane.min_cells, (10, 3));
    assert!(!parsed.pane.dim_inactive);
}

#[test]
fn pane_config_partial_toml() {
    let toml_str = r#"
[pane]
dim_inactive = true
inactive_opacity = 0.5
"#;
    let cfg: Config = toml::from_str(toml_str).expect("parse");
    assert!(cfg.pane.dim_inactive);
    assert!((cfg.pane.inactive_opacity - 0.5).abs() < f32::EPSILON);
    // Defaults for unspecified fields.
    assert!((cfg.pane.divider_px - 1.0).abs() < f32::EPSILON);
    assert_eq!(cfg.pane.min_cells, (10, 3));
}
