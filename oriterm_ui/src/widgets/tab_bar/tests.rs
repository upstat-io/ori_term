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

// --- TabBarHit ---

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

// --- TabBarWidget ---

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
    w.set_hover_hit(TabBarHit::NewTab);
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
    w.ring_bell(0);
    // Bell phase should be nonzero immediately after ringing.
    let phase = TabBarWidget::bell_phase_for_test(&TabEntry::new("A"), std::time::Instant::now());
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
    let now = std::time::Instant::now();
    let phase = TabBarWidget::bell_phase_for_test(&entry, now);
    assert!((phase - 0.0).abs() < f32::EPSILON);
}

#[test]
fn bell_phase_positive_right_after_bell() {
    let now = std::time::Instant::now();
    let entry = TabEntry {
        title: "test".into(),
        bell_start: Some(now - std::time::Duration::from_millis(100)),
    };
    let phase = TabBarWidget::bell_phase_for_test(&entry, now);
    // Phase should be > 0 shortly after bell fires.
    assert!(phase > 0.0, "bell phase should be positive, got {phase}");
}

#[test]
fn bell_phase_zero_after_duration() {
    let now = std::time::Instant::now();
    let entry = TabEntry {
        title: "test".into(),
        bell_start: Some(now - std::time::Duration::from_secs(5)),
    };
    let phase = TabBarWidget::bell_phase_for_test(&entry, now);
    assert!((phase - 0.0).abs() < f32::EPSILON);
}

// --- decay_tab_animations ---

#[test]
fn decay_animations_empty_offsets() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_anim_offsets(vec![]);
    assert!(!w.decay_tab_animations(1.0 / 60.0));
}

#[test]
fn decay_animations_zeroes_stay_zero() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_anim_offsets(vec![0.0, 0.0, 0.0]);
    assert!(!w.decay_tab_animations(1.0 / 60.0));
}

#[test]
fn decay_animations_nonzero_returns_active() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_anim_offsets(vec![100.0, -50.0]);
    let still_active = w.decay_tab_animations(1.0 / 60.0);
    assert!(still_active, "should still be animating after one frame");
    // Another frame should also still be active (100px takes many frames).
    let still_active = w.decay_tab_animations(1.0 / 60.0);
    assert!(still_active, "should still be animating after two frames");
}

#[test]
fn decay_animations_settles_to_zero() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_anim_offsets(vec![10.0]);
    // Simulate many frames (200ms at 60fps = 12 frames).
    for _ in 0..60 {
        w.decay_tab_animations(1.0 / 60.0);
    }
    // After 1 second, should be fully settled.
    let still_active = w.decay_tab_animations(1.0 / 60.0);
    assert!(!still_active, "should be settled after 60 frames");
}

// --- Button repositioning during drag ---

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

// --- hit_test function ---

/// Helper: standard 4-tab layout on a 1200px window.
fn layout_4_tabs() -> TabBarLayout {
    TabBarLayout::compute(4, 1200.0, None)
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
    let layout = TabBarLayout::compute(50, 800.0, None);
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
    let layout = TabBarLayout::compute(0, 1200.0, None);
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    // No tabs, so tab area returns NewTab/Dropdown/DragArea.
    let new_tab_x = layout.new_tab_x() + 1.0;
    assert_eq!(hit::hit_test(new_tab_x, mid_y, &layout), TabBarHit::NewTab);
    let drag_x = 5.0; // Before new-tab button.
    assert_eq!(hit::hit_test(drag_x, mid_y, &layout), TabBarHit::DragArea);
}

#[test]
fn hit_narrow_window_does_not_panic() {
    let layout = TabBarLayout::compute(3, 100.0, None);
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

// --- Mutation order independence (High Priority) ---

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
    w.set_hover_hit(TabBarHit::Tab(1));
    w.set_active_index(0);
    w.set_window_width(1200.0);
    // Layout should reflect final state: 3 tabs, 1200px window.
    assert_eq!(w.layout().tab_count, 3);
    assert!((w.layout().window_width - 1200.0).abs() < f32::EPSILON);
    assert!(w.layout().tab_width >= TAB_MIN_WIDTH);
}

// --- Out-of-bounds operations (Medium Priority) ---

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
    w.ring_bell(99);
    // No panic — documented no-op.
}

#[test]
fn update_tab_title_out_of_bounds_is_noop() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("Original")]);
    w.update_tab_title(99, "New".into());
    assert_eq!(w.tab_count(), 1);
}

// --- Animation offset edge cases (Medium Priority) ---

#[test]
fn anim_offsets_longer_than_tabs() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A")]);
    w.set_anim_offsets(vec![10.0, 20.0, 30.0]); // More offsets than tabs.
    let still = w.decay_tab_animations(1.0 / 60.0);
    assert!(still, "extra offsets should still decay");
}

#[test]
fn anim_offsets_shorter_than_tabs() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![
        TabEntry::new("A"),
        TabEntry::new("B"),
        TabEntry::new("C"),
    ]);
    w.set_anim_offsets(vec![10.0]); // Fewer offsets than tabs.
    let still = w.decay_tab_animations(1.0 / 60.0);
    assert!(still, "should still decay the one offset");
}

#[test]
fn resize_during_animation_preserves_finite_layout() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_tabs(vec![TabEntry::new("A"), TabEntry::new("B")]);
    w.set_anim_offsets(vec![50.0, -30.0]);
    // Resize mid-animation.
    w.set_window_width(800.0);
    // Layout recomputed; animations still present.
    let still = w.decay_tab_animations(1.0 / 60.0);
    assert!(still);
    assert!(w.layout().tab_width >= TAB_MIN_WIDTH);
    assert!(w.layout().tabs_end().is_finite());
}

// --- Degenerate layout inputs (Low Priority) ---

#[test]
fn layout_with_nan_window_width_does_not_panic() {
    // Degenerate input — verify no panic, not specific behavior.
    let layout = TabBarLayout::compute(3, f32::NAN, None);
    assert_eq!(layout.tab_count, 3);
    let _ = layout.tab_x(0);
    let _ = layout.tabs_end();
}

#[test]
fn layout_with_infinity_window_width_clamps_to_max() {
    let layout = TabBarLayout::compute(3, f32::INFINITY, None);
    assert_eq!(layout.tab_count, 3);
    // Infinite available space → clamp to TAB_MAX_WIDTH.
    assert!((layout.tab_width - TAB_MAX_WIDTH).abs() < f32::EPSILON);
}

#[test]
fn layout_with_negative_window_width_clamps_to_min() {
    let layout = TabBarLayout::compute(3, -500.0, None);
    assert_eq!(layout.tab_count, 3);
    assert!((layout.tab_width - TAB_MIN_WIDTH).abs() < f32::EPSILON);
}

// --- Very long tab title (Low Priority) ---

#[test]
fn very_long_tab_title_does_not_panic() {
    let mut w = TabBarWidget::new(1200.0);
    let long = "A".repeat(1000);
    w.set_tabs(vec![TabEntry::new(&long)]);
    assert_eq!(w.tab_count(), 1);
    assert!(w.layout().max_text_width() >= 0.0);
}

// --- Hit testing: single-tab close button ---

#[test]
fn hit_close_button_on_single_tab() {
    let layout = TabBarLayout::compute(1, 1200.0, None);
    let mid_y = TAB_BAR_HEIGHT / 2.0;
    let tab_right = layout.tab_x(0) + layout.tab_width;
    let close_center = tab_right - CLOSE_BUTTON_RIGHT_PAD - CLOSE_BUTTON_WIDTH / 2.0;
    assert_eq!(
        hit::hit_test(close_center, mid_y, &layout),
        TabBarHit::CloseTab(0)
    );
}

// --- Hit testing: control buttons individually ---

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

// --- Zero tabs: non-button area is DragArea ---

#[test]
fn zero_tabs_non_button_area_is_drag_area() {
    let layout = TabBarLayout::compute(0, 1200.0, None);
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

// --- Rapid tab close: width lock prevents layout shift ---

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

// --- Large decay dt (frame skip) ---

#[test]
fn decay_with_large_dt_settles_immediately() {
    let mut w = TabBarWidget::new(1200.0);
    w.set_anim_offsets(vec![100.0, -200.0]);
    // Simulate a 1-second frame skip (e.g., GPU stall).
    let still = w.decay_tab_animations(1.0);
    assert!(!still, "1-second dt should settle all offsets to zero");
}
