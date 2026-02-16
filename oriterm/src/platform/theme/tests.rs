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

    use super::super::{
        classify_desktop, parse_dbus_color_scheme, parse_gsettings_color_scheme,
        parse_kdeglobals, theme_from_gtk_name, DesktopEnvironment,
    };

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
        // Value 0 = no preference — returns None to defer to fallback chain.
        let output = "   variant    variant       uint32 0\n";
        assert_eq!(parse_dbus_color_scheme(output), None);
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

    // --- Desktop environment classification ---

    #[test]
    fn classify_gnome() {
        assert!(matches!(
            classify_desktop("GNOME"),
            Some(DesktopEnvironment::Gnome)
        ));
    }

    #[test]
    fn classify_gnome_lowercase() {
        assert!(matches!(
            classify_desktop("gnome"),
            Some(DesktopEnvironment::Gnome)
        ));
    }

    #[test]
    fn classify_gnome_xorg() {
        assert!(matches!(
            classify_desktop("gnome-xorg"),
            Some(DesktopEnvironment::Gnome)
        ));
    }

    #[test]
    fn classify_unity() {
        assert!(matches!(
            classify_desktop("Unity"),
            Some(DesktopEnvironment::Gnome)
        ));
    }

    #[test]
    fn classify_budgie() {
        assert!(matches!(
            classify_desktop("Budgie"),
            Some(DesktopEnvironment::Gnome)
        ));
    }

    #[test]
    fn classify_pantheon() {
        assert!(matches!(
            classify_desktop("Pantheon"),
            Some(DesktopEnvironment::Gnome)
        ));
    }

    #[test]
    fn classify_kde() {
        assert!(matches!(
            classify_desktop("KDE"),
            Some(DesktopEnvironment::Kde)
        ));
    }

    #[test]
    fn classify_kde_plasma() {
        assert!(matches!(
            classify_desktop("kde-plasma"),
            Some(DesktopEnvironment::Kde)
        ));
    }

    #[test]
    fn classify_cinnamon() {
        assert!(matches!(
            classify_desktop("X-Cinnamon"),
            Some(DesktopEnvironment::Cinnamon)
        ));
    }

    #[test]
    fn classify_cinnamon_lowercase() {
        assert!(matches!(
            classify_desktop("cinnamon"),
            Some(DesktopEnvironment::Cinnamon)
        ));
    }

    #[test]
    fn classify_mate() {
        assert!(matches!(
            classify_desktop("MATE"),
            Some(DesktopEnvironment::Mate)
        ));
    }

    #[test]
    fn classify_xfce() {
        assert!(matches!(
            classify_desktop("XFCE"),
            Some(DesktopEnvironment::Xfce)
        ));
    }

    #[test]
    fn classify_unknown_returns_none() {
        assert!(classify_desktop("SomeUnknownDE").is_none());
    }

    #[test]
    fn classify_empty_returns_none() {
        assert!(classify_desktop("").is_none());
    }

    // --- gsettings color-scheme parsing ---

    #[test]
    fn gsettings_color_scheme_prefer_dark() {
        assert_eq!(
            parse_gsettings_color_scheme("'prefer-dark'\n"),
            Some(Theme::Dark),
        );
    }

    #[test]
    fn gsettings_color_scheme_prefer_light() {
        assert_eq!(
            parse_gsettings_color_scheme("'prefer-light'\n"),
            Some(Theme::Light),
        );
    }

    #[test]
    fn gsettings_color_scheme_default_defers() {
        // 'default' means no preference — returns None to defer.
        assert_eq!(parse_gsettings_color_scheme("'default'\n"), None);
    }

    #[test]
    fn gsettings_color_scheme_empty() {
        assert_eq!(parse_gsettings_color_scheme(""), None);
    }

    #[test]
    fn gsettings_color_scheme_no_quotes() {
        // Some gsettings versions may omit quotes.
        assert_eq!(
            parse_gsettings_color_scheme("prefer-dark\n"),
            Some(Theme::Dark),
        );
    }

    // --- KDE kdeglobals parsing ---

    #[test]
    fn kdeglobals_breeze_dark() {
        let content = "[General]\nColorScheme=BreezeDark\n";
        assert_eq!(parse_kdeglobals(content), Some(Theme::Dark));
    }

    #[test]
    fn kdeglobals_breeze_light() {
        let content = "[General]\nColorScheme=BreezeLight\n";
        assert_eq!(parse_kdeglobals(content), Some(Theme::Light));
    }

    #[test]
    fn kdeglobals_no_general_section() {
        let content = "[Colors:Window]\nBackgroundNormal=255,255,255\n";
        assert_eq!(parse_kdeglobals(content), None);
    }

    #[test]
    fn kdeglobals_no_color_scheme_key() {
        let content = "[General]\nName=Default\n";
        assert_eq!(parse_kdeglobals(content), None);
    }

    #[test]
    fn kdeglobals_general_after_other_sections() {
        let content =
            "[Colors:Window]\nBg=0,0,0\n\n[General]\nColorScheme=BreezeDark\n";
        assert_eq!(parse_kdeglobals(content), Some(Theme::Dark));
    }

    #[test]
    fn kdeglobals_color_scheme_in_wrong_section() {
        // ColorScheme in a non-[General] section should be ignored.
        let content = "[KDE]\nColorScheme=BreezeDark\n[General]\nName=User\n";
        assert_eq!(parse_kdeglobals(content), None);
    }

    #[test]
    fn kdeglobals_empty() {
        assert_eq!(parse_kdeglobals(""), None);
    }
}
