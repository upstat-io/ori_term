//! Tests for scheme resolution and conditional parsing.

use oriterm_core::Theme;

use super::{builtin_names, find_builtin, parse_conditional, resolve_scheme_name};

#[test]
fn find_builtin_case_insensitive() {
    let scheme = find_builtin("catppuccin mocha").expect("should find");
    assert_eq!(scheme.name, "Catppuccin Mocha");

    let scheme = find_builtin("CATPPUCCIN MOCHA").expect("should find uppercase");
    assert_eq!(scheme.name, "Catppuccin Mocha");
}

#[test]
fn find_builtin_exact_match() {
    let scheme = find_builtin("Nord").expect("should find");
    assert_eq!(scheme.name, "Nord");
}

#[test]
fn find_builtin_missing_returns_none() {
    assert!(find_builtin("Nonexistent Theme").is_none());
}

#[test]
fn builtin_names_not_empty() {
    let names = builtin_names();
    assert!(
        names.len() >= 50,
        "expected 50+ builtins, got {}",
        names.len()
    );
}

#[test]
fn builtin_names_unique() {
    let names = builtin_names();
    let mut seen = std::collections::HashSet::new();
    for name in &names {
        let lower = name.to_ascii_lowercase();
        assert!(seen.insert(lower.clone()), "duplicate scheme name: {name}");
    }
}

#[test]
fn builtin_schemes_have_valid_rgb() {
    // All 16 ANSI + fg/bg/cursor should be in 0x00-0xFF range (inherently true
    // for u8 fields, but verify the schemes loaded correctly by checking they
    // are not all zero — a common copy-paste error).
    for name in builtin_names() {
        let scheme = find_builtin(name).unwrap();
        // At minimum, fg or bg should be non-black for most schemes.
        let non_zero = scheme.fg != oriterm_core::Rgb { r: 0, g: 0, b: 0 }
            || scheme.bg != oriterm_core::Rgb { r: 0, g: 0, b: 0 };
        assert!(non_zero, "scheme {name} has all-zero fg and bg");
    }
}

#[test]
fn parse_conditional_dark_light() {
    let result = parse_conditional("dark:Tokyo Night, light:Tokyo Night Light");
    assert_eq!(result, Some(("Tokyo Night", "Tokyo Night Light")));
}

#[test]
fn parse_conditional_reversed_order() {
    let result = parse_conditional("light:Catppuccin Latte, dark:Catppuccin Mocha");
    assert_eq!(result, Some(("Catppuccin Mocha", "Catppuccin Latte")));
}

#[test]
fn parse_conditional_plain_name() {
    assert!(parse_conditional("Nord").is_none());
}

#[test]
fn parse_conditional_only_dark() {
    assert!(parse_conditional("dark:Tokyo Night").is_none());
}

#[test]
fn parse_conditional_extra_whitespace() {
    let result = parse_conditional("  dark: One Dark ,  light: One Light  ");
    assert_eq!(result, Some(("One Dark", "One Light")));
}

#[test]
fn resolve_scheme_name_plain() {
    let name = resolve_scheme_name("Nord", Theme::Dark);
    assert_eq!(name, "Nord");
}

#[test]
fn resolve_scheme_name_conditional_dark() {
    let name = resolve_scheme_name("dark:Tokyo Night, light:Tokyo Night Light", Theme::Dark);
    assert_eq!(name, "Tokyo Night");
}

#[test]
fn resolve_scheme_name_conditional_light() {
    let name = resolve_scheme_name("dark:Tokyo Night, light:Tokyo Night Light", Theme::Light);
    assert_eq!(name, "Tokyo Night Light");
}
