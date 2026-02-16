//! Tests for the Theme type.

use super::Theme;

#[test]
fn default_is_dark() {
    assert_eq!(Theme::default(), Theme::Dark);
}

#[test]
fn dark_is_dark() {
    assert!(Theme::Dark.is_dark());
}

#[test]
fn light_is_not_dark() {
    assert!(!Theme::Light.is_dark());
}

#[test]
fn unknown_is_dark() {
    // Unknown falls back to dark — the conventional terminal default.
    assert!(Theme::Unknown.is_dark());
}
