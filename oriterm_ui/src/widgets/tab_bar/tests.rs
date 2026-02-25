//! Tests for tab bar layout, colors, and constants.

use super::colors::TabBarColors;
use super::constants::*;
use super::layout::TabBarLayout;
use crate::theme::UiTheme;

// --- Layout computation ---

#[test]
fn single_tab_fills_available_space() {
    let layout = TabBarLayout::compute(1, 1200.0, None);
    assert_eq!(layout.tab_count, 1);
    // Single tab gets all available space, clamped to TAB_MAX_WIDTH.
    assert!(layout.tab_width <= TAB_MAX_WIDTH);
    assert!(layout.tab_width >= TAB_MIN_WIDTH);
}

#[test]
fn single_tab_clamps_to_max() {
    // Very wide window — one tab should clamp to MAX.
    let layout = TabBarLayout::compute(1, 2000.0, None);
    assert!((layout.tab_width - TAB_MAX_WIDTH).abs() < f32::EPSILON);
}

#[test]
fn many_tabs_clamp_to_min() {
    // 50 tabs in 1200px — not enough room, clamp to min.
    let layout = TabBarLayout::compute(50, 1200.0, None);
    assert!((layout.tab_width - TAB_MIN_WIDTH).abs() < f32::EPSILON);
}

#[test]
fn zero_tabs_returns_min_width() {
    let layout = TabBarLayout::compute(0, 1200.0, None);
    assert_eq!(layout.tab_count, 0);
    assert!((layout.tab_width - TAB_MIN_WIDTH).abs() < f32::EPSILON);
}

#[test]
fn tabs_split_available_space_evenly() {
    let window_width = 1200.0;
    let available = window_width
        - TAB_LEFT_MARGIN
        - NEW_TAB_BUTTON_WIDTH
        - DROPDOWN_BUTTON_WIDTH
        - CONTROLS_ZONE_WIDTH;
    let tab_count = 5;
    let expected = (available / tab_count as f32).clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH);

    let layout = TabBarLayout::compute(tab_count, window_width, None);
    assert!((layout.tab_width - expected).abs() < 0.01);
}

#[test]
fn width_lock_overrides_computation() {
    let locked = 150.0;
    let layout = TabBarLayout::compute(3, 1200.0, Some(locked));
    assert!((layout.tab_width - locked).abs() < f32::EPSILON);
}

#[test]
fn narrow_window_clamps_to_min() {
    // Window so narrow that available space is negative.
    let layout = TabBarLayout::compute(3, 100.0, None);
    assert!((layout.tab_width - TAB_MIN_WIDTH).abs() < f32::EPSILON);
}

#[test]
fn window_width_preserved() {
    let layout = TabBarLayout::compute(2, 1500.0, None);
    assert!((layout.window_width - 1500.0).abs() < f32::EPSILON);
}

// --- Helper methods ---

#[test]
fn tab_x_positions_are_sequential() {
    let layout = TabBarLayout::compute(4, 1200.0, None);
    for i in 0..4 {
        let expected = TAB_LEFT_MARGIN + i as f32 * layout.tab_width;
        assert!((layout.tab_x(i) - expected).abs() < 0.01);
    }
}

#[test]
fn tabs_end_after_last_tab() {
    let layout = TabBarLayout::compute(3, 1200.0, None);
    let expected = TAB_LEFT_MARGIN + 3.0 * layout.tab_width;
    assert!((layout.tabs_end() - expected).abs() < 0.01);
}

#[test]
fn new_tab_button_starts_at_tabs_end() {
    let layout = TabBarLayout::compute(3, 1200.0, None);
    assert!((layout.new_tab_x() - layout.tabs_end()).abs() < f32::EPSILON);
}

#[test]
fn dropdown_follows_new_tab_button() {
    let layout = TabBarLayout::compute(3, 1200.0, None);
    let expected = layout.new_tab_x() + NEW_TAB_BUTTON_WIDTH;
    assert!((layout.dropdown_x() - expected).abs() < f32::EPSILON);
}

#[test]
fn controls_at_right_edge() {
    let layout = TabBarLayout::compute(3, 1200.0, None);
    let expected = 1200.0 - CONTROLS_ZONE_WIDTH;
    assert!((layout.controls_x() - expected).abs() < f32::EPSILON);
}

#[test]
fn max_text_width_accounts_for_padding() {
    let layout = TabBarLayout::compute(3, 1200.0, None);
    let expected =
        layout.tab_width - 2.0 * TAB_PADDING - CLOSE_BUTTON_WIDTH - CLOSE_BUTTON_RIGHT_PAD;
    assert!((layout.max_text_width() - expected).abs() < 0.01);
}

#[test]
fn max_text_width_not_negative() {
    // Very narrow tabs — text width should floor at 0.
    let layout = TabBarLayout::compute(50, 1200.0, None);
    assert!(layout.max_text_width() >= 0.0);
}

// --- Colors ---

#[test]
fn colors_from_dark_theme() {
    let theme = UiTheme::dark();
    let colors = TabBarColors::from_theme(&theme);
    assert_eq!(colors.bar_bg, theme.bg_secondary);
    assert_eq!(colors.active_bg, theme.bg_primary);
    assert_eq!(colors.text_fg, theme.fg_primary);
    assert_eq!(colors.inactive_text, theme.fg_secondary);
}

#[test]
fn colors_from_light_theme() {
    let theme = UiTheme::light();
    let colors = TabBarColors::from_theme(&theme);
    assert_eq!(colors.bar_bg, theme.bg_secondary);
    assert_eq!(colors.active_bg, theme.bg_primary);
}

#[test]
fn bell_pulse_endpoints() {
    let theme = UiTheme::dark();
    let colors = TabBarColors::from_theme(&theme);

    // Phase 0 → inactive_bg.
    let c0 = colors.bell_pulse(0.0);
    assert!((c0.r - colors.inactive_bg.r).abs() < 0.001);
    assert!((c0.g - colors.inactive_bg.g).abs() < 0.001);
    assert!((c0.b - colors.inactive_bg.b).abs() < 0.001);

    // Phase 1 → tab_hover_bg.
    let c1 = colors.bell_pulse(1.0);
    assert!((c1.r - colors.tab_hover_bg.r).abs() < 0.001);
    assert!((c1.g - colors.tab_hover_bg.g).abs() < 0.001);
    assert!((c1.b - colors.tab_hover_bg.b).abs() < 0.001);
}

#[test]
fn bell_pulse_midpoint() {
    let theme = UiTheme::dark();
    let colors = TabBarColors::from_theme(&theme);

    let mid = colors.bell_pulse(0.5);
    let expected_r = (colors.inactive_bg.r + colors.tab_hover_bg.r) / 2.0;
    assert!((mid.r - expected_r).abs() < 0.01);
}

// --- Constants sanity checks ---

#[test]
fn constants_are_positive() {
    assert!(TAB_BAR_HEIGHT > 0.0);
    assert!(TAB_MIN_WIDTH > 0.0);
    assert!(TAB_MAX_WIDTH > TAB_MIN_WIDTH);
    assert!(TAB_LEFT_MARGIN >= 0.0);
    assert!(TAB_PADDING > 0.0);
    assert!(CLOSE_BUTTON_WIDTH > 0.0);
    assert!(NEW_TAB_BUTTON_WIDTH > 0.0);
    assert!(DROPDOWN_BUTTON_WIDTH > 0.0);
    assert!(CONTROLS_ZONE_WIDTH > 0.0);
}

#[test]
fn drag_thresholds_ordered() {
    assert!(DRAG_START_THRESHOLD > 0.0);
    assert!(TEAR_OFF_THRESHOLD > DRAG_START_THRESHOLD);
    assert!(TEAR_OFF_THRESHOLD_UP > 0.0);
    assert!(TEAR_OFF_THRESHOLD_UP < TEAR_OFF_THRESHOLD);
}

// --- Hit testing ---

#[test]
fn hit_test_returns_correct_tab_index() {
    let layout = TabBarLayout::compute(4, 1200.0, None);
    // Middle of first tab.
    let mid0 = layout.tab_x(0) + layout.tab_width / 2.0;
    assert_eq!(layout.tab_index_at(mid0), Some(0));
    // Middle of third tab.
    let mid2 = layout.tab_x(2) + layout.tab_width / 2.0;
    assert_eq!(layout.tab_index_at(mid2), Some(2));
    // Middle of last tab.
    let mid3 = layout.tab_x(3) + layout.tab_width / 2.0;
    assert_eq!(layout.tab_index_at(mid3), Some(3));
}

#[test]
fn hit_test_boundary_between_tabs() {
    let layout = TabBarLayout::compute(4, 1200.0, None);
    // Previous representable f32 before tab_x(1) — must belong to tab 0.
    let just_before = f32::from_bits(layout.tab_x(1).to_bits() - 1);
    assert_eq!(layout.tab_index_at(just_before), Some(0));
    // Exactly at the start of tab 1 (owned by tab 1).
    assert_eq!(layout.tab_index_at(layout.tab_x(1)), Some(1));
}

#[test]
fn hit_test_before_tabs_returns_none() {
    let layout = TabBarLayout::compute(4, 1200.0, None);
    assert_eq!(layout.tab_index_at(0.0), None);
    assert_eq!(layout.tab_index_at(TAB_LEFT_MARGIN - 1.0), None);
}

#[test]
fn hit_test_past_tabs_returns_none() {
    let layout = TabBarLayout::compute(4, 1200.0, None);
    assert_eq!(layout.tab_index_at(layout.tabs_end()), None);
    assert_eq!(layout.tab_index_at(layout.tabs_end() + 100.0), None);
}

#[test]
fn hit_test_zero_tabs_returns_none() {
    let layout = TabBarLayout::compute(0, 1200.0, None);
    assert_eq!(layout.tab_index_at(TAB_LEFT_MARGIN), None);
    assert_eq!(layout.tab_index_at(100.0), None);
}

// --- Zero/extreme window sizes ---

#[test]
fn zero_width_window_does_not_panic() {
    let layout = TabBarLayout::compute(3, 0.0, None);
    assert!((layout.tab_width - TAB_MIN_WIDTH).abs() < f32::EPSILON);
    assert_eq!(layout.tab_count, 3);
    // Helpers still return finite values.
    assert!(layout.tabs_end().is_finite());
    assert!(layout.controls_x().is_finite());
}

// --- tab_x out of bounds ---

#[test]
fn tab_x_past_end_equals_tabs_end() {
    let layout = TabBarLayout::compute(4, 1200.0, None);
    assert!((layout.tab_x(layout.tab_count) - layout.tabs_end()).abs() < f32::EPSILON);
}

// --- Width lock edge cases ---

#[test]
fn width_lock_below_min_passes_through() {
    let tiny = 10.0;
    let layout = TabBarLayout::compute(3, 1200.0, Some(tiny));
    // Lock bypasses clamping — documents the contract.
    assert!((layout.tab_width - tiny).abs() < f32::EPSILON);
}

#[test]
fn width_lock_above_max_passes_through() {
    let huge = 500.0;
    let layout = TabBarLayout::compute(3, 1200.0, Some(huge));
    assert!((layout.tab_width - huge).abs() < f32::EPSILON);
}

// --- Layout invariants ---

#[test]
fn buttons_do_not_overlap_controls() {
    // Verify ordering: tabs_end <= new_tab_x < dropdown_x < controls_x
    // for a reasonable window width.
    let layout = TabBarLayout::compute(5, 1200.0, None);
    assert!(layout.new_tab_x() <= layout.dropdown_x());
    assert!(layout.dropdown_x() + DROPDOWN_BUTTON_WIDTH <= layout.controls_x());
}

#[test]
fn tabs_end_within_controls_when_tabs_fit() {
    // Only check counts where tabs comfortably fit in a 1200px window.
    // At TAB_MIN_WIDTH=80, ~12 tabs fill the available space.
    for count in [1, 3, 5, 8] {
        let layout = TabBarLayout::compute(count, 1200.0, None);
        let buttons_end = layout.dropdown_x() + DROPDOWN_BUTTON_WIDTH;
        assert!(
            buttons_end <= layout.controls_x() + 1.0,
            "overflow at {count} tabs: buttons_end={buttons_end}, controls_x={}",
            layout.controls_x()
        );
    }
}

// --- max_text_width boundary ---

#[test]
fn max_text_width_at_min_tab_width() {
    // Force tabs to minimum width with many tabs.
    let layout = TabBarLayout::compute(50, 1200.0, None);
    assert!((layout.tab_width - TAB_MIN_WIDTH).abs() < f32::EPSILON);
    // Text width must be non-negative even at minimum tab size.
    assert!(layout.max_text_width() >= 0.0);
}

// --- Color alpha ---

#[test]
fn all_theme_colors_have_nonzero_alpha() {
    let theme = UiTheme::dark();
    let colors = TabBarColors::from_theme(&theme);
    assert!(colors.bar_bg.a > 0.0, "bar_bg alpha is zero");
    assert!(colors.active_bg.a > 0.0, "active_bg alpha is zero");
    assert!(colors.inactive_bg.a > 0.0, "inactive_bg alpha is zero");
    assert!(colors.tab_hover_bg.a > 0.0, "tab_hover_bg alpha is zero");
    assert!(colors.text_fg.a > 0.0, "text_fg alpha is zero");
    assert!(colors.inactive_text.a > 0.0, "inactive_text alpha is zero");
    assert!(colors.separator.a > 0.0, "separator alpha is zero");
    assert!(colors.close_fg.a > 0.0, "close_fg alpha is zero");
    assert!(
        colors.button_hover_bg.a > 0.0,
        "button_hover_bg alpha is zero"
    );
    assert!(
        colors.control_hover_bg.a > 0.0,
        "control_hover_bg alpha is zero"
    );
    assert!(colors.control_fg.a > 0.0, "control_fg alpha is zero");
    assert!(
        colors.control_close_hover_bg.a > 0.0,
        "control_close_hover_bg alpha is zero"
    );
    assert!(
        colors.control_close_hover_fg.a > 0.0,
        "control_close_hover_fg alpha is zero"
    );
}

// --- bell_pulse out of range ---

#[test]
fn bell_pulse_out_of_range_does_not_panic() {
    let theme = UiTheme::dark();
    let colors = TabBarColors::from_theme(&theme);
    // Extrapolates linearly outside [0, 1] — no panic, finite results.
    let below = colors.bell_pulse(-0.1);
    let above = colors.bell_pulse(1.1);
    assert!(below.r.is_finite());
    assert!(above.r.is_finite());
}

// --- Constants: inner padding fits within TAB_MIN_WIDTH ---

#[test]
fn inner_padding_fits_within_min_tab_width() {
    let inner = 2.0 * TAB_PADDING + CLOSE_BUTTON_WIDTH + CLOSE_BUTTON_RIGHT_PAD;
    assert!(
        inner < TAB_MIN_WIDTH,
        "inner padding ({inner}) >= TAB_MIN_WIDTH ({TAB_MIN_WIDTH}), max_text_width would be 0"
    );
}
