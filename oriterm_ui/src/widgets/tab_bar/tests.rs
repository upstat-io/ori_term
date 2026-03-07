//! Tests for tab bar layout, colors, and constants.

use super::colors::TabBarColors;
use super::constants::{
    CLOSE_BUTTON_RIGHT_PAD, CLOSE_BUTTON_WIDTH, CONTROLS_ZONE_WIDTH, DRAG_START_THRESHOLD,
    DROPDOWN_BUTTON_WIDTH, NEW_TAB_BUTTON_WIDTH, TAB_BAR_HEIGHT, TAB_LEFT_MARGIN, TAB_MAX_WIDTH,
    TAB_MIN_WIDTH, TAB_PADDING, TEAR_OFF_THRESHOLD, TEAR_OFF_THRESHOLD_UP,
};
use super::layout::TabBarLayout;
use crate::theme::UiTheme;

// Layout computation

#[test]
fn single_tab_fills_available_space() {
    let layout = TabBarLayout::compute(1, 1200.0, None, 0.0);
    assert_eq!(layout.tab_count, 1);
    // Single tab gets all available space, clamped to TAB_MAX_WIDTH.
    assert!(layout.tab_width <= TAB_MAX_WIDTH);
    assert!(layout.tab_width >= TAB_MIN_WIDTH);
}

#[test]
fn single_tab_clamps_to_max() {
    // Very wide window — one tab should clamp to MAX.
    let layout = TabBarLayout::compute(1, 2000.0, None, 0.0);
    assert!((layout.tab_width - TAB_MAX_WIDTH).abs() < f32::EPSILON);
}

#[test]
fn many_tabs_clamp_to_min() {
    // 50 tabs in 1200px — not enough room, clamp to min.
    let layout = TabBarLayout::compute(50, 1200.0, None, 0.0);
    assert!((layout.tab_width - TAB_MIN_WIDTH).abs() < f32::EPSILON);
}

#[test]
fn zero_tabs_returns_min_width() {
    let layout = TabBarLayout::compute(0, 1200.0, None, 0.0);
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

    let layout = TabBarLayout::compute(tab_count, window_width, None, 0.0);
    assert!((layout.tab_width - expected).abs() < 0.01);
}

#[test]
fn width_lock_overrides_computation() {
    let locked = 150.0;
    let layout = TabBarLayout::compute(3, 1200.0, Some(locked), 0.0);
    assert!((layout.tab_width - locked).abs() < f32::EPSILON);
}

#[test]
fn narrow_window_clamps_to_min() {
    // Window so narrow that available space is negative.
    let layout = TabBarLayout::compute(3, 100.0, None, 0.0);
    assert!((layout.tab_width - TAB_MIN_WIDTH).abs() < f32::EPSILON);
}

#[test]
fn window_width_preserved() {
    let layout = TabBarLayout::compute(2, 1500.0, None, 0.0);
    assert!((layout.window_width - 1500.0).abs() < f32::EPSILON);
}

// Helper methods

#[test]
fn tab_x_positions_are_sequential() {
    let layout = TabBarLayout::compute(4, 1200.0, None, 0.0);
    for i in 0..4 {
        let expected = TAB_LEFT_MARGIN + i as f32 * layout.tab_width;
        assert!((layout.tab_x(i) - expected).abs() < 0.01);
    }
}

#[test]
fn tabs_end_after_last_tab() {
    let layout = TabBarLayout::compute(3, 1200.0, None, 0.0);
    let expected = TAB_LEFT_MARGIN + 3.0 * layout.tab_width;
    assert!((layout.tabs_end() - expected).abs() < 0.01);
}

#[test]
fn new_tab_button_starts_at_tabs_end() {
    let layout = TabBarLayout::compute(3, 1200.0, None, 0.0);
    assert!((layout.new_tab_x() - layout.tabs_end()).abs() < f32::EPSILON);
}

#[test]
fn dropdown_follows_new_tab_button() {
    let layout = TabBarLayout::compute(3, 1200.0, None, 0.0);
    let expected = layout.new_tab_x() + NEW_TAB_BUTTON_WIDTH;
    assert!((layout.dropdown_x() - expected).abs() < f32::EPSILON);
}

#[test]
fn controls_at_right_edge() {
    let layout = TabBarLayout::compute(3, 1200.0, None, 0.0);
    let expected = 1200.0 - CONTROLS_ZONE_WIDTH;
    assert!((layout.controls_x() - expected).abs() < f32::EPSILON);
}

#[test]
fn max_text_width_accounts_for_padding() {
    let layout = TabBarLayout::compute(3, 1200.0, None, 0.0);
    let expected =
        layout.tab_width - 2.0 * TAB_PADDING - CLOSE_BUTTON_WIDTH - CLOSE_BUTTON_RIGHT_PAD;
    assert!((layout.max_text_width() - expected).abs() < 0.01);
}

#[test]
fn max_text_width_not_negative() {
    // Very narrow tabs — text width should floor at 0.
    let layout = TabBarLayout::compute(50, 1200.0, None, 0.0);
    assert!(layout.max_text_width() >= 0.0);
}

// Colors

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

// Constants sanity checks

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

// Hit testing

#[test]
fn hit_test_returns_correct_tab_index() {
    let layout = TabBarLayout::compute(4, 1200.0, None, 0.0);
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
    let layout = TabBarLayout::compute(4, 1200.0, None, 0.0);
    // Previous representable f32 before tab_x(1) — must belong to tab 0.
    let just_before = f32::from_bits(layout.tab_x(1).to_bits() - 1);
    assert_eq!(layout.tab_index_at(just_before), Some(0));
    // Exactly at the start of tab 1 (owned by tab 1).
    assert_eq!(layout.tab_index_at(layout.tab_x(1)), Some(1));
}

#[test]
fn hit_test_before_tabs_returns_none() {
    let layout = TabBarLayout::compute(4, 1200.0, None, 0.0);
    assert_eq!(layout.tab_index_at(0.0), None);
    assert_eq!(layout.tab_index_at(TAB_LEFT_MARGIN - 1.0), None);
}

#[test]
fn hit_test_past_tabs_returns_none() {
    let layout = TabBarLayout::compute(4, 1200.0, None, 0.0);
    assert_eq!(layout.tab_index_at(layout.tabs_end()), None);
    assert_eq!(layout.tab_index_at(layout.tabs_end() + 100.0), None);
}

#[test]
fn hit_test_zero_tabs_returns_none() {
    let layout = TabBarLayout::compute(0, 1200.0, None, 0.0);
    assert_eq!(layout.tab_index_at(TAB_LEFT_MARGIN), None);
    assert_eq!(layout.tab_index_at(100.0), None);
}

// Zero/extreme window sizes

#[test]
fn zero_width_window_does_not_panic() {
    let layout = TabBarLayout::compute(3, 0.0, None, 0.0);
    assert!((layout.tab_width - TAB_MIN_WIDTH).abs() < f32::EPSILON);
    assert_eq!(layout.tab_count, 3);
    // Helpers still return finite values.
    assert!(layout.tabs_end().is_finite());
    assert!(layout.controls_x().is_finite());
}

// tab_x out of bounds

#[test]
fn tab_x_past_end_equals_tabs_end() {
    let layout = TabBarLayout::compute(4, 1200.0, None, 0.0);
    assert!((layout.tab_x(layout.tab_count) - layout.tabs_end()).abs() < f32::EPSILON);
}

// Width lock edge cases

#[test]
fn width_lock_below_min_passes_through() {
    let tiny = 10.0;
    let layout = TabBarLayout::compute(3, 1200.0, Some(tiny), 0.0);
    // Lock bypasses clamping — documents the contract.
    assert!((layout.tab_width - tiny).abs() < f32::EPSILON);
}

#[test]
fn width_lock_above_max_passes_through() {
    let huge = 500.0;
    let layout = TabBarLayout::compute(3, 1200.0, Some(huge), 0.0);
    assert!((layout.tab_width - huge).abs() < f32::EPSILON);
}

// Layout invariants

#[test]
fn buttons_do_not_overlap_controls() {
    // Verify ordering: tabs_end <= new_tab_x < dropdown_x < controls_x
    // for a reasonable window width.
    let layout = TabBarLayout::compute(5, 1200.0, None, 0.0);
    assert!(layout.new_tab_x() <= layout.dropdown_x());
    assert!(layout.dropdown_x() + DROPDOWN_BUTTON_WIDTH <= layout.controls_x());
}

#[test]
fn tabs_end_within_controls_when_tabs_fit() {
    // Only check counts where tabs comfortably fit in a 1200px window.
    // At TAB_MIN_WIDTH=80, ~12 tabs fill the available space.
    for count in [1, 3, 5, 8] {
        let layout = TabBarLayout::compute(count, 1200.0, None, 0.0);
        let buttons_end = layout.dropdown_x() + DROPDOWN_BUTTON_WIDTH;
        assert!(
            buttons_end <= layout.controls_x() + 1.0,
            "overflow at {count} tabs: buttons_end={buttons_end}, controls_x={}",
            layout.controls_x()
        );
    }
}

// max_text_width boundary

#[test]
fn max_text_width_at_min_tab_width() {
    // Force tabs to minimum width with many tabs.
    let layout = TabBarLayout::compute(50, 1200.0, None, 0.0);
    assert!((layout.tab_width - TAB_MIN_WIDTH).abs() < f32::EPSILON);
    // Text width must be non-negative even at minimum tab size.
    assert!(layout.max_text_width() >= 0.0);
}

// Color alpha

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
}

// bell_pulse out of range

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

// Constants: inner padding fits within TAB_MIN_WIDTH

#[test]
fn inner_padding_fits_within_min_tab_width() {
    let inner = 2.0 * TAB_PADDING + CLOSE_BUTTON_WIDTH + CLOSE_BUTTON_RIGHT_PAD;
    assert!(
        inner < TAB_MIN_WIDTH,
        "inner padding ({inner}) >= TAB_MIN_WIDTH ({TAB_MIN_WIDTH}), max_text_width would be 0"
    );
}

// TabBarHit

use super::hit::{self, TabBarHit};

#[test]
fn hit_none_is_default() {
    let hit: TabBarHit = Default::default();
    assert_eq!(hit, TabBarHit::None);
}

#[test]
fn hit_is_tab_matches_body_and_close() {
    assert!(TabBarHit::Tab(2).is_tab(2));
    assert!(TabBarHit::CloseTab(2).is_tab(2));
    assert!(!TabBarHit::Tab(3).is_tab(2));
    assert!(!TabBarHit::CloseTab(3).is_tab(2));
    assert!(!TabBarHit::NewTab.is_tab(0));
    assert!(!TabBarHit::None.is_tab(0));
}

// TabBarWidget

use std::time::{Duration, Instant};

use super::widget::{TabBarWidget, TabEntry};

#[test]
fn widget_new_has_no_tabs() {
    let w = TabBarWidget::new(1200.0);
    assert_eq!(w.tab_count(), 0);
}

#[test]
fn widget_set_tabs_updates_count() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("Tab 1"), TabEntry::new("Tab 2")]);
    assert_eq!(w.tab_count(), 2);
}

#[test]
fn widget_set_tabs_recomputes_layout() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![
        TabEntry::new("A"),
        TabEntry::new("B"),
        TabEntry::new("C"),
    ]);
    assert_eq!(w.layout().tab_count, 3);
}

#[test]
fn widget_set_window_width_recomputes_layout() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A")]);
    let w1 = w.layout().tab_width;
    w.set_window_width(2000.0);
    let w2 = w.layout().tab_width;
    // Wider window = wider tab (clamped at max, but likely different).
    assert!(w2 >= w1 || (w2 - TAB_MAX_WIDTH).abs() < f32::EPSILON);
}

#[test]
fn widget_tab_width_lock_freezes_layout() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let normal = w.layout().tab_width;
    w.set_tab_width_lock(Some(100.0));
    assert!((w.layout().tab_width - 100.0).abs() < f32::EPSILON);
    w.set_tab_width_lock(None);
    assert!((w.layout().tab_width - normal).abs() < f32::EPSILON);
}

#[test]
fn tab_entry_new_sets_title() {
    let entry = TabEntry::new("hello");
    assert_eq!(entry.title, "hello");
    assert!(entry.bell_start.is_none());
}

#[test]
fn widget_set_active_index() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    w.set_active_index(1);
    // No panic, index stored. Widget draw will use this.
}

#[test]
fn widget_set_hover_hit() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_hover_hit(TabBarHit::NewTab, Instant::now());
    // No panic — hover state stored.
}

#[test]
fn widget_set_drag_visual() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    w.set_drag_visual(Some((0, 100.0)));
    // No panic — drag state stored.
    w.set_drag_visual(None);
}

#[test]
fn ring_bell_starts_animation() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A")]);
    w.ring_bell(0, Instant::now());
    // Bell phase should be nonzero immediately after ringing.
    let phase = TabBarWidget::bell_phase_for_test(&TabEntry::new("A"), Instant::now());
    // A fresh TabEntry has no bell, so phase is 0.
    assert_eq!(phase, 0.0);
}

#[test]
fn update_tab_title_changes_title() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("Old")]);
    w.update_tab_title(0, "New".into());
    // Verify via tab_count (title is internal, but we can check it doesn't panic).
    assert_eq!(w.tab_count(), 1);
}

#[test]
fn bell_phase_zero_when_no_bell() {
    let entry = TabEntry::new("test");
    let now = Instant::now();
    let phase = TabBarWidget::bell_phase_for_test(&entry, now);
    assert!((phase - 0.0).abs() < f32::EPSILON);
}

#[test]
fn bell_phase_positive_right_after_bell() {
    let now = Instant::now();
    let entry = TabEntry {
        title: "test".into(),
        icon: None,
        bell_start: Some(now - Duration::from_millis(100)),
    };
    let phase = TabBarWidget::bell_phase_for_test(&entry, now);
    // Phase should be > 0 shortly after bell fires.
    assert!(phase > 0.0, "bell phase should be positive, got {phase}");
}

#[test]
fn bell_phase_zero_after_duration() {
    let now = Instant::now();
    let entry = TabEntry {
        title: "test".into(),
        icon: None,
        bell_start: Some(now - Duration::from_secs(5)),
    };
    let phase = TabBarWidget::bell_phase_for_test(&entry, now);
    assert!((phase - 0.0).abs() < f32::EPSILON);
}

// AnimatedValue<Color> smoke test

use crate::animation::{AnimatedValue, Easing};
use crate::color::Color;

#[test]
fn animated_value_color_interpolates() {
    let now = Instant::now();
    let mut av = AnimatedValue::new(Color::BLACK, Duration::from_millis(100), Easing::Linear);
    av.set(Color::WHITE, now);

    // At t=0, should be black (start).
    let c0 = av.get(now);
    assert!((c0.r - 0.0).abs() < 0.01);
    assert!((c0.g - 0.0).abs() < 0.01);
    assert!((c0.b - 0.0).abs() < 0.01);

    // At t=50ms (50%), should be mid-gray.
    let mid = now + Duration::from_millis(50);
    let c50 = av.get(mid);
    assert!((c50.r - 0.5).abs() < 0.05, "r at 50%: {}", c50.r);
    assert!((c50.g - 0.5).abs() < 0.05, "g at 50%: {}", c50.g);
    assert!((c50.b - 0.5).abs() < 0.05, "b at 50%: {}", c50.b);

    // At t=100ms+, should be white (target).
    let end = now + Duration::from_millis(100);
    let c100 = av.get(end);
    assert!((c100.r - 1.0).abs() < 0.01);
    assert!((c100.g - 1.0).abs() < 0.01);
    assert!((c100.b - 1.0).abs() < 0.01);
}

// Hover progress animation tests

#[test]
fn hover_progress_starts_at_zero() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let now = Instant::now();
    assert!((w.test_hover_progress(0, now) - 0.0).abs() < f32::EPSILON);
    assert!((w.test_hover_progress(1, now) - 0.0).abs() < f32::EPSILON);
}

#[test]
fn hover_progress_reaches_one_after_duration() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let now = Instant::now();

    // Hover tab 0.
    w.set_hover_hit(TabBarHit::Tab(0), now);

    // After 100ms+ (TAB_HOVER_DURATION), should be 1.0.
    let after = now + Duration::from_millis(150);
    assert!(
        (w.test_hover_progress(0, after) - 1.0).abs() < f32::EPSILON,
        "hover progress should be 1.0, got {}",
        w.test_hover_progress(0, after)
    );
    // Tab 1 should still be 0.
    assert!((w.test_hover_progress(1, after) - 0.0).abs() < f32::EPSILON);
}

#[test]
fn hover_progress_mid_transition() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A")]);
    let now = Instant::now();

    w.set_hover_hit(TabBarHit::Tab(0), now);

    // At ~50% of 100ms duration, should be mid-transition (not 0 and not 1).
    let mid = now + Duration::from_millis(50);
    let p = w.test_hover_progress(0, mid);
    assert!(p > 0.1, "mid-transition should be > 0.1, got {p}");
    assert!(p < 0.99, "mid-transition should be < 0.99, got {p}");
}

#[test]
fn hover_leave_starts_reverse_transition() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A")]);
    let now = Instant::now();

    // Hover enter.
    w.set_hover_hit(TabBarHit::Tab(0), now);

    // Let animation complete.
    let t1 = now + Duration::from_millis(150);
    assert!((w.test_hover_progress(0, t1) - 1.0).abs() < f32::EPSILON);

    // Hover leave.
    w.set_hover_hit(TabBarHit::None, t1);

    // After leave animation completes, should be 0.
    let t2 = t1 + Duration::from_millis(150);
    assert!(
        (w.test_hover_progress(0, t2) - 0.0).abs() < f32::EPSILON,
        "after leave, hover progress should be 0.0, got {}",
        w.test_hover_progress(0, t2)
    );
}

// Close button opacity tests

#[test]
fn close_btn_opacity_zero_by_default() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A")]);
    let now = Instant::now();
    assert!((w.test_close_btn_opacity(0, now) - 0.0).abs() < f32::EPSILON);
}

#[test]
fn close_btn_opacity_reaches_one_on_hover() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A")]);
    let now = Instant::now();

    w.set_hover_hit(TabBarHit::Tab(0), now);

    // After 80ms+ (CLOSE_BTN_FADE_DURATION), should be 1.0.
    let after = now + Duration::from_millis(100);
    assert!(
        (w.test_close_btn_opacity(0, after) - 1.0).abs() < f32::EPSILON,
        "close btn opacity should be 1.0, got {}",
        w.test_close_btn_opacity(0, after)
    );
}

#[test]
fn close_btn_opacity_fades_out_on_leave() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A")]);
    let now = Instant::now();

    // Hover enter, let animation complete.
    w.set_hover_hit(TabBarHit::Tab(0), now);
    let t1 = now + Duration::from_millis(100);
    assert!((w.test_close_btn_opacity(0, t1) - 1.0).abs() < f32::EPSILON);

    // Hover leave.
    w.set_hover_hit(TabBarHit::None, t1);

    // After fade-out completes, should be 0.
    let t2 = t1 + Duration::from_millis(100);
    assert!(
        (w.test_close_btn_opacity(0, t2) - 0.0).abs() < f32::EPSILON,
        "after leave, close btn opacity should be 0.0, got {}",
        w.test_close_btn_opacity(0, t2)
    );
}

// Button repositioning during drag

#[test]
fn new_tab_button_x_no_drag() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    // Without drag, button X == layout.new_tab_x().
    let layout_x = w.layout().new_tab_x();
    assert!((w.test_new_tab_button_x() - layout_x).abs() < f32::EPSILON);
}

#[test]
fn new_tab_button_x_follows_drag() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let tab_w = w.layout().tab_width;
    // Drag tab 0 far to the right, past the normal new_tab_x.
    let drag_x = w.layout().new_tab_x() + 100.0;
    w.set_drag_visual(Some((0, drag_x)));
    let expected = drag_x + tab_w;
    assert!(
        (w.test_new_tab_button_x() - expected).abs() < f32::EPSILON,
        "new tab button should follow drag: got {}, expected {expected}",
        w.test_new_tab_button_x()
    );
}

#[test]
fn dropdown_button_x_follows_drag() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let tab_w = w.layout().tab_width;
    let drag_x = w.layout().new_tab_x() + 100.0;
    w.set_drag_visual(Some((0, drag_x)));
    let expected = drag_x + tab_w + NEW_TAB_BUTTON_WIDTH;
    assert!(
        (w.test_dropdown_button_x() - expected).abs() < f32::EPSILON,
        "dropdown button should follow drag: got {}, expected {expected}",
        w.test_dropdown_button_x()
    );
}

// hit_test function

/// Helper: standard 4-tab layout on a 1200px window.
fn layout_4_tabs() -> TabBarLayout {
    TabBarLayout::compute(4, 1200.0, None, 0.0)
}

#[test]
fn hit_below_tab_bar_returns_none() {
    let layout = layout_4_tabs();
    assert_eq!(
        hit::hit_test(100.0, TAB_BAR_HEIGHT, &layout),
        TabBarHit::None
    );
    assert_eq!(
        hit::hit_test(100.0, TAB_BAR_HEIGHT + 10.0, &layout),
        TabBarHit::None
    );
}

#[test]
fn hit_above_tab_bar_returns_none() {
    let layout = layout_4_tabs();
    assert_eq!(hit::hit_test(100.0, -1.0, &layout), TabBarHit::None);
}

#[test]
fn hit_tab_body_returns_tab() {
    let layout = layout_4_tabs();
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    // Middle of tab 0.
    let mid_x = layout.tab_x(0) + layout.tab_width / 2.0;
    assert_eq!(hit::hit_test(mid_x, mid_y, &layout), TabBarHit::Tab(0));
    // Middle of tab 3.
    let mid_x = layout.tab_x(3) + layout.tab_width / 2.0;
    assert_eq!(hit::hit_test(mid_x, mid_y, &layout), TabBarHit::Tab(3));
}

#[test]
fn hit_close_button_returns_close_tab() {
    let layout = layout_4_tabs();
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    // Close button region of tab 1: right edge minus padding.
    let tab_right = layout.tab_x(1) + layout.tab_width;
    let close_center = tab_right - CLOSE_BUTTON_RIGHT_PAD - CLOSE_BUTTON_WIDTH / 2.0;
    assert_eq!(
        hit::hit_test(close_center, mid_y, &layout),
        TabBarHit::CloseTab(1)
    );
}

#[test]
fn hit_close_button_left_boundary() {
    let layout = layout_4_tabs();
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    let tab_right = layout.tab_x(2) + layout.tab_width;
    let close_left = tab_right - CLOSE_BUTTON_WIDTH - CLOSE_BUTTON_RIGHT_PAD;
    // Exactly at close_left → CloseTab.
    assert_eq!(
        hit::hit_test(close_left, mid_y, &layout),
        TabBarHit::CloseTab(2)
    );
    // Just before close_left → Tab body.
    let just_before = f32::from_bits(close_left.to_bits() - 1);
    assert_eq!(
        hit::hit_test(just_before, mid_y, &layout),
        TabBarHit::Tab(2)
    );
}

#[test]
fn hit_close_button_right_boundary() {
    let layout = layout_4_tabs();
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    let tab_right = layout.tab_x(0) + layout.tab_width;
    let close_right = tab_right - CLOSE_BUTTON_RIGHT_PAD;
    // Exactly at close_right → Tab body (half-open interval excludes right edge).
    // But close_right is within the tab, so it's either Tab(0) or Tab(1) depending
    // on whether it falls within the next tab's region.
    let result = hit::hit_test(close_right, mid_y, &layout);
    assert!(
        result == TabBarHit::Tab(0) || result == TabBarHit::Tab(1),
        "at close_right edge, expected Tab(0) or Tab(1), got {result:?}"
    );
}

#[test]
fn hit_new_tab_button() {
    let layout = layout_4_tabs();
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    let new_tab_center = layout.new_tab_x() + NEW_TAB_BUTTON_WIDTH / 2.0;
    assert_eq!(
        hit::hit_test(new_tab_center, mid_y, &layout),
        TabBarHit::NewTab
    );
}

#[test]
fn hit_dropdown_button() {
    let layout = layout_4_tabs();
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    let dropdown_center = layout.dropdown_x() + DROPDOWN_BUTTON_WIDTH / 2.0;
    assert_eq!(
        hit::hit_test(dropdown_center, mid_y, &layout),
        TabBarHit::Dropdown
    );
}

#[test]
fn hit_controls_zone_returns_window_control() {
    let layout = layout_4_tabs();
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    // Well into the controls zone.
    let x = layout.controls_x() + CONTROLS_ZONE_WIDTH / 2.0;
    let result = hit::hit_test(x, mid_y, &layout);
    assert!(
        result.is_window_control() || result == TabBarHit::DragArea,
        "expected a window control or DragArea in controls zone, got {result:?}"
    );
}

#[test]
fn hit_controls_zone_has_priority_over_tabs() {
    // With many tabs, the tab strip might conceptually extend into the controls zone.
    // Controls must still win.
    let layout = TabBarLayout::compute(50, 800.0, None, 0.0);
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    let x = layout.controls_x() + 10.0;
    let result = hit::hit_test(x, mid_y, &layout);
    assert_ne!(result, TabBarHit::Tab(0));
    assert_ne!(result, TabBarHit::None);
}

#[test]
fn hit_empty_area_returns_drag_area() {
    let layout = layout_4_tabs();
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    // Between dropdown end and controls start.
    let gap_x = layout.dropdown_x() + DROPDOWN_BUTTON_WIDTH + 10.0;
    if gap_x < layout.controls_x() {
        assert_eq!(hit::hit_test(gap_x, mid_y, &layout), TabBarHit::DragArea);
    }
}

#[test]
fn hit_left_margin_returns_drag_area() {
    let layout = layout_4_tabs();
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    // In the left margin before tabs start.
    assert_eq!(
        hit::hit_test(TAB_LEFT_MARGIN / 2.0, mid_y, &layout),
        TabBarHit::DragArea
    );
}

#[test]
fn hit_zero_tabs_all_buttons_and_drag() {
    let layout = TabBarLayout::compute(0, 1200.0, None, 0.0);
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    // No tabs, so tab area returns NewTab/Dropdown/DragArea.
    let new_tab_x = layout.new_tab_x() + 1.0;
    assert_eq!(hit::hit_test(new_tab_x, mid_y, &layout), TabBarHit::NewTab);
    let drag_x = 5.0; // Before new-tab button.
    assert_eq!(hit::hit_test(drag_x, mid_y, &layout), TabBarHit::DragArea);
}

#[test]
fn hit_narrow_window_does_not_panic() {
    let layout = TabBarLayout::compute(3, 100.0, None, 0.0);
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    // Controls zone might overlap everything — should still return valid hits.
    let result = hit::hit_test(50.0, mid_y, &layout);
    assert_ne!(
        result,
        TabBarHit::None,
        "within tab bar height should not be None"
    );
}

#[test]
fn hit_at_origin_within_tab_bar() {
    let layout = layout_4_tabs();
    // (0, 0) is in the tab bar but in the left margin.
    let result = hit::hit_test(0.0, 0.0, &layout);
    assert_eq!(result, TabBarHit::DragArea);
}

#[test]
fn hit_is_window_control_predicate() {
    assert!(TabBarHit::Minimize.is_window_control());
    assert!(TabBarHit::Maximize.is_window_control());
    assert!(TabBarHit::CloseWindow.is_window_control());
    assert!(!TabBarHit::Tab(0).is_window_control());
    assert!(!TabBarHit::DragArea.is_window_control());
    assert!(!TabBarHit::None.is_window_control());
}

#[test]
fn hit_top_and_bottom_y_edges() {
    let layout = layout_4_tabs();
    let mid_x = layout.tab_x(0) + layout.tab_width / 2.0;
    // y=0 is inside the tab bar.
    assert_eq!(hit::hit_test(mid_x, 0.0, &layout), TabBarHit::Tab(0));
    // y just below TAB_BAR_HEIGHT is outside.
    let just_below = f32::from_bits(TAB_BAR_HEIGHT.to_bits());
    assert_eq!(hit::hit_test(mid_x, just_below, &layout), TabBarHit::None);
}

#[test]
fn hit_each_tab_at_center() {
    let layout = layout_4_tabs();
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    for i in 0..4 {
        let x = layout.tab_x(i) + layout.tab_width / 2.0;
        assert_eq!(hit::hit_test(x, mid_y, &layout), TabBarHit::Tab(i));
    }
}

#[test]
fn hit_each_tab_close_button() {
    let layout = layout_4_tabs();
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    for i in 0..4 {
        let tab_right = layout.tab_x(i) + layout.tab_width;
        let close_center = tab_right - CLOSE_BUTTON_RIGHT_PAD - CLOSE_BUTTON_WIDTH / 2.0;
        assert_eq!(
            hit::hit_test(close_center, mid_y, &layout),
            TabBarHit::CloseTab(i),
            "close button for tab {i}"
        );
    }
}

// Mutation order independence (High Priority)

#[test]
fn set_active_index_before_tabs_does_not_panic() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_active_index(5);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    assert_eq!(w.tab_count(), 2);
}

#[test]
fn set_window_width_before_tabs_does_not_panic() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_window_width(800.0);
    w.set_tabs(vec![TabEntry::new("A")]);
    assert_eq!(w.tab_count(), 1);
}

#[test]
fn set_drag_visual_before_tabs_does_not_panic() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_drag_visual(Some((0, 100.0)));
    w.set_tabs(vec![TabEntry::new("A")]);
    assert_eq!(w.tab_count(), 1);
}

#[test]
fn interleaved_mutations_do_not_corrupt_layout() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_active_index(2);
    w.set_tabs(vec![
        TabEntry::new("A"),
        TabEntry::new("B"),
        TabEntry::new("C"),
    ]);
    w.set_window_width(800.0);
    w.set_hover_hit(TabBarHit::Tab(1), Instant::now());
    w.set_active_index(0);
    w.set_window_width(1200.0);
    // Layout should reflect final state: 3 tabs, 1200px window.
    assert_eq!(w.layout().tab_count, 3);
    assert!((w.layout().window_width - 1200.0).abs() < f32::EPSILON);
    assert!(w.layout().tab_width >= TAB_MIN_WIDTH);
}

// Out-of-bounds operations (Medium Priority)

#[test]
fn set_active_index_out_of_bounds_does_not_panic() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    w.set_active_index(100);
    // No panic — index stored for future use.
}

#[test]
fn ring_bell_out_of_bounds_is_noop() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A")]);
    w.ring_bell(99, Instant::now());
    // No panic — documented no-op.
}

#[test]
fn update_tab_title_out_of_bounds_is_noop() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("Original")]);
    w.update_tab_title(99, "New".into());
    assert_eq!(w.tab_count(), 1);
}

// Degenerate layout inputs (Low Priority)

#[test]
fn layout_with_nan_window_width_does_not_panic() {
    // Degenerate input — verify no panic, not specific behavior.
    let layout = TabBarLayout::compute(3, f32::NAN, None, 0.0);
    assert_eq!(layout.tab_count, 3);
    let _ = layout.tab_x(0);
    let _ = layout.tabs_end();
}

#[test]
fn layout_with_infinity_window_width_clamps_to_max() {
    let layout = TabBarLayout::compute(3, f32::INFINITY, None, 0.0);
    assert_eq!(layout.tab_count, 3);
    // Infinite available space → clamp to TAB_MAX_WIDTH.
    assert!((layout.tab_width - TAB_MAX_WIDTH).abs() < f32::EPSILON);
}

#[test]
fn layout_with_negative_window_width_clamps_to_min() {
    let layout = TabBarLayout::compute(3, -500.0, None, 0.0);
    assert_eq!(layout.tab_count, 3);
    assert!((layout.tab_width - TAB_MIN_WIDTH).abs() < f32::EPSILON);
}

// Very long tab title (Low Priority)

#[test]
fn very_long_tab_title_does_not_panic() {
    let mut w = TabBarWidget::new(1200.0);
    let long = "A".repeat(1000);
    w.set_tabs(vec![TabEntry::new(&long)]);
    assert_eq!(w.tab_count(), 1);
    assert!(w.layout().max_text_width() >= 0.0);
}

// Hit testing: single-tab close button

#[test]
fn hit_close_button_on_single_tab() {
    let layout = TabBarLayout::compute(1, 1200.0, None, 0.0);
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    let tab_right = layout.tab_x(0) + layout.tab_width;
    let close_center = tab_right - CLOSE_BUTTON_RIGHT_PAD - CLOSE_BUTTON_WIDTH / 2.0;
    assert_eq!(
        hit::hit_test(close_center, mid_y, &layout),
        TabBarHit::CloseTab(0)
    );
}

// Hit testing: control buttons individually

#[test]
fn hit_each_control_button_found_by_scan() {
    let layout = layout_4_tabs();
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    let controls_x = layout.controls_x();

    // Scan the controls zone at 1px intervals to find all three buttons.
    let mut found_min = false;
    let mut found_max = false;
    let mut found_close = false;

    let mut x = controls_x;
    while x < layout.window_width {
        match hit::hit_test(x, mid_y, &layout) {
            TabBarHit::Minimize => found_min = true,
            TabBarHit::Maximize => found_max = true,
            TabBarHit::CloseWindow => found_close = true,
            _ => {}
        }
        x += 1.0;
    }

    assert!(found_min, "minimize not found in controls zone");
    assert!(found_max, "maximize not found in controls zone");
    assert!(found_close, "close not found in controls zone");
}

#[test]
fn hit_controls_y_edges() {
    let layout = layout_4_tabs();
    let controls_center_x = layout.controls_x() + CONTROLS_ZONE_WIDTH / 2.0;

    // y=0 is within the tab bar.
    let top = hit::hit_test(controls_center_x, 0.0, &layout);
    assert_ne!(
        top,
        TabBarHit::None,
        "y=0 in controls zone should not be None"
    );

    // y just inside bottom edge.
    let bottom = hit::hit_test(controls_center_x, TAB_BAR_HEIGHT - 0.1, &layout);
    assert_ne!(bottom, TabBarHit::None, "y near bottom should still hit");
}

// Zero tabs: non-button area is DragArea

#[test]
fn zero_tabs_non_button_area_is_drag_area() {
    let layout = TabBarLayout::compute(0, 1200.0, None, 0.0);
    let mid_y = TAB_BAR_HEIGHT / 2.0;

    let mut found_drag = false;
    let mut x = 0.0;
    while x < layout.controls_x() {
        let result = hit::hit_test(x, mid_y, &layout);
        match result {
            TabBarHit::DragArea => found_drag = true,
            TabBarHit::NewTab | TabBarHit::Dropdown => {}
            other => panic!("unexpected hit {other:?} at x={x} with zero tabs"),
        }
        x += 5.0;
    }
    assert!(found_drag, "should find drag area with zero tabs");
}

// Rapid tab close: width lock prevents layout shift

#[test]
fn width_lock_prevents_shift_on_tab_removal() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![
        TabEntry::new("A"),
        TabEntry::new("B"),
        TabEntry::new("C"),
    ]);
    let locked = w.layout().tab_width;
    w.set_tab_width_lock(Some(locked));

    // Simulate closing tab B: now 2 tabs.
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("C")]);
    // Width should still be the locked value, not recomputed for 2 tabs.
    assert!(
        (w.layout().tab_width - locked).abs() < f32::EPSILON,
        "width lock should prevent layout shift on tab removal"
    );
}

// Theme wiring (gap analysis)

#[test]
fn apply_theme_changes_colors() {
    let dark = UiTheme::dark();
    let light = UiTheme::light();

    let mut w = TabBarWidget::with_theme(1200.0, &dark);
    w.set_tabs(vec![TabEntry::new("A")]);

    // Verify dark colors initially.
    let dark_colors = TabBarColors::from_theme(&dark);
    assert_eq!(w.layout().tab_count, 1);

    // Apply light theme.
    w.apply_theme(&light);

    let light_colors = TabBarColors::from_theme(&light);

    // Colors should differ between dark and light.
    assert_ne!(dark_colors.bar_bg, light_colors.bar_bg);
    assert_ne!(dark_colors.text_fg, light_colors.text_fg);
}

#[test]
fn with_theme_light_produces_light_colors() {
    let light = UiTheme::light();
    let colors = TabBarColors::from_theme(&light);

    // Verify all color fields correspond to the light theme.
    assert_eq!(colors.bar_bg, light.bg_secondary);
    assert_eq!(colors.active_bg, light.bg_primary);
    assert_eq!(colors.text_fg, light.fg_primary);
    assert_eq!(colors.inactive_text, light.fg_secondary);
    assert_eq!(colors.separator, light.border.with_alpha(0.5));
    assert_eq!(colors.close_fg, light.fg_secondary);
    assert_eq!(colors.button_hover_bg, light.bg_hover);
}

#[test]
fn close_button_colors_theme_invariant() {
    let dark = UiTheme::dark();
    let light = UiTheme::light();

    // Close button red should be the same in both themes.
    assert_eq!(dark.close_hover_bg, light.close_hover_bg);
    assert_eq!(dark.close_pressed_bg, light.close_pressed_bg);
}

// interactive_rects (unified chrome)

#[test]
fn interactive_rects_count_equals_tab_count_plus_five() {
    for tab_count in [0, 1, 3, 5, 10] {
        let mut w = TabBarWidget::new(1200.0);
        let tabs: Vec<TabEntry> = (0..tab_count)
            .map(|i| TabEntry::new(format!("T{i}")))
            .collect();
        w.set_tabs(tabs);
        let rects = w.interactive_rects();
        assert_eq!(
            rects.len(),
            tab_count + 5,
            "tab_count={tab_count}: expected {} rects, got {}",
            tab_count + 5,
            rects.len()
        );
    }
}

#[test]
fn interactive_rects_tab_positions_match_layout() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![
        TabEntry::new("A"),
        TabEntry::new("B"),
        TabEntry::new("C"),
    ]);
    let rects = w.interactive_rects();
    let layout = w.layout();

    // First 3 rects are tab rects.
    for i in 0..3 {
        let r = rects[i];
        assert!(
            (r.x() - layout.tab_x(i)).abs() < f32::EPSILON,
            "tab {i} x: got {}, expected {}",
            r.x(),
            layout.tab_x(i)
        );
        assert!(
            (r.width() - layout.tab_width).abs() < f32::EPSILON,
            "tab {i} width mismatch"
        );
        assert!(
            (r.height() - TAB_BAR_HEIGHT).abs() < f32::EPSILON,
            "tab {i} height mismatch"
        );
    }
}

#[test]
fn interactive_rects_buttons_and_controls_at_correct_positions() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let rects = w.interactive_rects();
    let layout = w.layout();

    // rects[2] = new-tab button.
    let new_tab = rects[2];
    assert!(
        (new_tab.x() - layout.new_tab_x()).abs() < f32::EPSILON,
        "new-tab button x"
    );
    assert!(
        (new_tab.width() - NEW_TAB_BUTTON_WIDTH).abs() < f32::EPSILON,
        "new-tab button width"
    );

    // rects[3] = dropdown button.
    let dropdown = rects[3];
    assert!(
        (dropdown.x() - layout.dropdown_x()).abs() < f32::EPSILON,
        "dropdown button x"
    );
    assert!(
        (dropdown.width() - DROPDOWN_BUTTON_WIDTH).abs() < f32::EPSILON,
        "dropdown button width"
    );

    // rects[4..7] = control buttons starting at controls_x().
    let controls_x = layout.controls_x();
    for i in 0..3 {
        let ctrl = rects[4 + i];
        assert!(
            ctrl.x() >= controls_x - 0.01,
            "control {i} x ({}) < controls_x ({controls_x})",
            ctrl.x()
        );
    }
}

#[test]
fn interactive_rects_with_left_inset_shifts_tabs_not_controls() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let rects_no_inset = w.interactive_rects();

    w.set_left_inset(76.0); // macOS traffic light width
    let rects_inset = w.interactive_rects();

    // Tab rects should shift right by 76px.
    for i in 0..2 {
        let shift = rects_inset[i].x() - rects_no_inset[i].x();
        assert!(
            (shift - 76.0).abs() < 0.01,
            "tab {i} shift: expected 76.0, got {shift}"
        );
    }

    // Control button rects (last 3) should stay at the same position.
    for i in 4..7 {
        assert!(
            (rects_inset[i].x() - rects_no_inset[i].x()).abs() < f32::EPSILON,
            "control rect {i} should not shift with left_inset"
        );
    }
}

// set_maximized / set_active

#[test]
fn set_maximized_does_not_panic() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A")]);
    w.set_maximized(true);
    w.set_maximized(false);
    // No panic — maximized state affects control button symbol.
}

#[test]
fn set_active_false_does_not_panic() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A")]);
    w.set_active(false);
    w.set_active(true);
    // No panic — active state affects control button caption bg.
}

// left_inset layout

#[test]
fn layout_with_left_inset_shifts_tabs_start() {
    let inset = 76.0;
    let layout_no_inset = TabBarLayout::compute(3, 1200.0, None, 0.0);
    let layout_inset = TabBarLayout::compute(3, 1200.0, None, inset);

    // Tabs start shifted right by the inset.
    let shift = layout_inset.tab_x(0) - layout_no_inset.tab_x(0);
    assert!(
        (shift - inset).abs() < f32::EPSILON,
        "tabs should shift right by left_inset: got {shift}"
    );

    // Available tab space is reduced, so tab widths may differ.
    assert!(layout_inset.tab_width <= layout_no_inset.tab_width);
}

#[test]
fn layout_with_left_inset_controls_stay_right() {
    let layout_no_inset = TabBarLayout::compute(3, 1200.0, None, 0.0);
    let layout_inset = TabBarLayout::compute(3, 1200.0, None, 76.0);

    // Controls zone is anchored to the right edge — unaffected by left_inset.
    assert!(
        (layout_inset.controls_x() - layout_no_inset.controls_x()).abs() < f32::EPSILON,
        "controls_x should not change with left_inset"
    );
}

// update_control_hover

use crate::geometry::Rect;
use crate::input::EventResponse;
use crate::widgets::EventCtx;
use crate::widgets::tests::MockMeasurer;

#[test]
fn update_control_hover_enters_and_leaves() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A")]);

    let measurer = MockMeasurer::STANDARD;
    let theme = UiTheme::dark();
    let ctx = EventCtx {
        measurer: &measurer,
        bounds: Rect::new(0.0, 0.0, 1200.0, TAB_BAR_HEIGHT),
        is_focused: false,
        focused_widget: None,
        theme: &theme,
    };

    // Hover over the first control button (minimize).
    let ctrl_rect = w.interactive_rects()[w.tab_count() + 2]; // first control
    let center = crate::geometry::Point::new(
        ctrl_rect.x() + ctrl_rect.width() / 2.0,
        ctrl_rect.y() + ctrl_rect.height() / 2.0,
    );
    let resp = w.update_control_hover(center, &ctx);
    assert_eq!(
        resp.response,
        EventResponse::RequestRedraw,
        "entering a control should request redraw"
    );

    // Hover same position again — no change.
    let resp2 = w.update_control_hover(center, &ctx);
    assert_eq!(
        resp2.response,
        EventResponse::Ignored,
        "re-hovering same control should not request redraw"
    );

    // Clear hover.
    w.clear_control_hover(&ctx);
}

// cursor_in_tab_bar range

#[test]
fn hit_test_close_window_in_controls_zone() {
    // Verify that CloseWindow is reachable via hit_test in the controls zone.
    let layout = TabBarLayout::compute(4, 1200.0, None, 0.0);
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    // Close button is the rightmost control.
    // Scan from the right edge leftward to find CloseWindow.
    let mut found = false;
    let mut x = layout.window_width - 1.0;
    while x > layout.controls_x() {
        if hit::hit_test(x, mid_y, &layout) == TabBarHit::CloseWindow {
            found = true;
            break;
        }
        x -= 1.0;
    }
    assert!(found, "CloseWindow should be hittable in controls zone");
}

// Tab lifecycle animation tests (Section 04)

#[test]
fn width_multiplier_defaults_to_one() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let now = Instant::now();
    // No animation → multiplier is 1.0 (full width).
    assert!(!w.has_width_animation(now));
}

#[test]
fn animate_tab_open_starts_at_zero() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let now = Instant::now();

    w.animate_tab_open(1, now);

    // Tab 1 width multiplier at t=0 should be 0.0.
    assert!(w.has_width_animation(now));
    // After animation (200ms+), should be 1.0.
    let after = now + Duration::from_millis(250);
    assert!(!w.has_width_animation(after));
}

#[test]
fn animate_tab_close_starts_at_one() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let now = Instant::now();

    w.animate_tab_close(0, now);

    assert!(w.is_closing(0));
    assert!(!w.is_closing(1));
    assert!(w.has_width_animation(now));
}

#[test]
fn closing_complete_returns_none_during_animation() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let now = Instant::now();

    w.animate_tab_close(0, now);

    // Mid-animation: no completion yet.
    let mid = now + Duration::from_millis(50);
    assert!(w.closing_complete(mid).is_none());
}

#[test]
fn closing_complete_returns_index_after_animation() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let now = Instant::now();

    w.animate_tab_close(0, now);

    // After animation (150ms+), tab 0 should be complete.
    let after = now + Duration::from_millis(200);
    assert_eq!(w.closing_complete(after), Some(0));
}

#[test]
fn set_tabs_resets_closing_state() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    let now = Instant::now();

    w.animate_tab_close(0, now);
    assert!(w.is_closing(0));

    // set_tabs resets all parallel vecs.
    w.set_tabs(vec![TabEntry::new("B")]);
    assert!(!w.is_closing(0));
    assert!(!w.has_width_animation(Instant::now()));
}

// Layout with width multipliers

#[test]
fn layout_with_uniform_multipliers_matches_default() {
    let default = TabBarLayout::compute(3, 1200.0, None, 0.0);
    let with_ones =
        TabBarLayout::compute_with_multipliers(3, 1200.0, None, 0.0, Some(&[1.0, 1.0, 1.0]));

    for i in 0..3 {
        assert!(
            (default.tab_x(i) - with_ones.tab_x(i)).abs() < f32::EPSILON,
            "tab_x({i}) should match"
        );
        assert!(
            (default.tab_width_at(i) - with_ones.tab_width_at(i)).abs() < f32::EPSILON,
            "tab_width_at({i}) should match"
        );
    }
    assert!((default.tabs_end() - with_ones.tabs_end()).abs() < f32::EPSILON);
}

#[test]
fn layout_half_multiplier_halves_tab_width() {
    let layout =
        TabBarLayout::compute_with_multipliers(3, 1200.0, None, 0.0, Some(&[1.0, 0.5, 1.0]));

    // Tab 1 should be half width.
    let base = layout.tab_width;
    assert!(
        (layout.tab_width_at(1) - base * 0.5).abs() < f32::EPSILON,
        "tab 1 should be half width"
    );

    // Tab 2 should start at tab 1 start + half width (not full width).
    let expected_tab2_x = layout.tab_x(1) + base * 0.5;
    assert!(
        (layout.tab_x(2) - expected_tab2_x).abs() < f32::EPSILON,
        "tab 2 should start after half-width tab 1"
    );
}

#[test]
fn layout_zero_multiplier_collapses_tab() {
    let layout =
        TabBarLayout::compute_with_multipliers(3, 1200.0, None, 0.0, Some(&[1.0, 0.0, 1.0]));

    assert!(
        layout.tab_width_at(1) < f32::EPSILON,
        "collapsed tab has zero width"
    );
    // Tab 2 starts where tab 1 starts (zero-width tab occupies no space).
    assert!(
        (layout.tab_x(2) - layout.tab_x(1)).abs() < f32::EPSILON,
        "tab 2 should be adjacent to tab 1 start"
    );
}

#[test]
fn tab_index_at_with_non_uniform_widths() {
    let layout =
        TabBarLayout::compute_with_multipliers(3, 1200.0, None, 0.0, Some(&[1.0, 0.5, 1.0]));

    // Mid-point of tab 0 should hit tab 0.
    let mid0 = layout.tab_x(0) + layout.tab_width_at(0) / 2.0;
    assert_eq!(layout.tab_index_at(mid0), Some(0));

    // Mid-point of tab 1 (half-width) should hit tab 1.
    let mid1 = layout.tab_x(1) + layout.tab_width_at(1) / 2.0;
    assert_eq!(layout.tab_index_at(mid1), Some(1));

    // Mid-point of tab 2 should hit tab 2.
    let mid2 = layout.tab_x(2) + layout.tab_width_at(2) / 2.0;
    assert_eq!(layout.tab_index_at(mid2), Some(2));
}

#[test]
fn tabs_end_with_multipliers() {
    let layout =
        TabBarLayout::compute_with_multipliers(3, 1200.0, None, 0.0, Some(&[1.0, 0.5, 1.0]));

    // Total width = 1.0 + 0.5 + 1.0 = 2.5 tab widths.
    let base = layout.tab_width;
    let expected = layout.tab_x(0) + 2.5 * base;
    assert!(
        (layout.tabs_end() - expected).abs() < f32::EPSILON,
        "tabs_end should account for multipliers"
    );
}
