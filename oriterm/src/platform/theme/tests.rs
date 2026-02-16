//! Tests for system theme detection.

use oriterm_core::Theme;

use super::system_theme;

#[test]
fn system_theme_returns_valid_variant() {
    let theme = system_theme();
    // Must be one of the three variants — no panics.
    assert!(
        matches!(theme, Theme::Dark | Theme::Light | Theme::Unknown),
        "unexpected theme variant: {theme:?}",
    );
}

#[test]
fn system_theme_is_deterministic() {
    // Repeated calls within the same process should return the same result.
    let a = system_theme();
    let b = system_theme();
    assert_eq!(a, b);
}

// --- Linux-specific tests ---

#[cfg(target_os = "linux")]
mod linux {
    use oriterm_core::Theme;

    use super::super::{parse_dbus_color_scheme, theme_from_gtk_name};

    #[test]
    fn parse_dbus_dark() {
        // Simulated dbus-send output for dark mode (value 1).
        let output = "   variant    variant       uint32 1\n";
        assert_eq!(parse_dbus_color_scheme(output), Some(Theme::Dark));
    }

    #[test]
    fn parse_dbus_light() {
        // Simulated dbus-send output for light mode (value 2).
        let output = "   variant    variant       uint32 2\n";
        assert_eq!(parse_dbus_color_scheme(output), Some(Theme::Light));
    }

    #[test]
    fn parse_dbus_no_preference() {
        // Value 0 = no preference.
        let output = "   variant    variant       uint32 0\n";
        assert_eq!(parse_dbus_color_scheme(output), Some(Theme::Unknown));
    }

    #[test]
    fn parse_dbus_empty_output() {
        assert_eq!(parse_dbus_color_scheme(""), None);
    }

    #[test]
    fn parse_dbus_garbage() {
        assert_eq!(parse_dbus_color_scheme("no numbers here"), None);
    }

    #[test]
    fn gtk_dark_adwaita() {
        assert_eq!(theme_from_gtk_name(Some("Adwaita:dark")), Theme::Dark);
    }

    #[test]
    fn gtk_light_adwaita() {
        assert_eq!(theme_from_gtk_name(Some("Adwaita")), Theme::Light);
    }

    #[test]
    fn gtk_unset() {
        assert_eq!(theme_from_gtk_name(None), Theme::Unknown);
    }

    #[test]
    fn gtk_case_insensitive() {
        assert_eq!(theme_from_gtk_name(Some("Breeze-DARK")), Theme::Dark);
    }

    #[test]
    fn gtk_adwaita_dark_variant() {
        assert_eq!(theme_from_gtk_name(Some("Adwaita-dark")), Theme::Dark);
    }

    #[test]
    fn gtk_breeze_light() {
        assert_eq!(theme_from_gtk_name(Some("Breeze")), Theme::Light);
    }
}
