//! Tab bar rendering widget.
//!
//! [`TabBarWidget`] draws the tab strip: tab backgrounds with titles, close
//! buttons, separators, new-tab (+) button, and dropdown button. All
//! coordinates are in logical pixels; the caller applies scale at the
//! rendering boundary.
//!
//! The widget implements [`Widget`] for draw integration. Event handling
//! stubs are provided here; full hit-test dispatch is Section 16.3.

mod control_state;
mod controls_draw;
mod drag_draw;
mod draw;

use std::time::{Duration, Instant};

use crate::animation::{AnimatedValue, Easing};
use crate::color::Color;
use crate::theme::UiTheme;
use crate::widget_id::WidgetId;
use crate::widgets::window_chrome::controls::{ControlButtonColors, WindowControlButton};
use crate::widgets::window_chrome::layout::ControlKind;

use super::colors::TabBarColors;
use super::hit::TabBarHit;
use super::layout::TabBarLayout;

/// Duration for tab hover background animation.
const TAB_HOVER_DURATION: Duration = Duration::from_millis(100);

/// Duration for close button fade in/out animation.
const CLOSE_BTN_FADE_DURATION: Duration = Duration::from_millis(80);

/// Duration for tab open width animation.
const TAB_OPEN_DURATION: Duration = Duration::from_millis(200);

/// Duration for tab close width animation.
const TAB_CLOSE_DURATION: Duration = Duration::from_millis(150);

/// Icon type for tab entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabIcon {
    /// Single emoji grapheme cluster.
    Emoji(String),
}

/// Per-tab visual state provided by the application layer.
#[derive(Debug, Clone)]
pub struct TabEntry {
    /// Tab title (empty string shows "Terminal" as fallback).
    pub title: String,
    /// Optional icon to show before the title.
    pub icon: Option<TabIcon>,
    /// When the bell last fired (for pulse animation). `None` if no bell.
    pub bell_start: Option<Instant>,
}

impl TabEntry {
    /// Creates a tab entry with the given title, no icon, and no bell.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            icon: None,
            bell_start: None,
        }
    }

    /// Sets the tab icon.
    #[must_use]
    pub fn with_icon(mut self, icon: Option<TabIcon>) -> Self {
        self.icon = icon;
        self
    }
}

/// Tab bar rendering widget.
///
/// Holds all visual state needed to draw the tab strip. The application
/// layer updates state through setter methods; the widget's [`draw`]
/// implementation emits [`DrawCommand`](crate::draw::DrawCommand)s into
/// the draw list.
pub struct TabBarWidget {
    id: WidgetId,

    // Tab data.
    tabs: Vec<TabEntry>,
    active_index: usize,

    // Computed layout.
    layout: TabBarLayout,
    colors: TabBarColors,
    window_width: f32,
    tab_width_lock: Option<f32>,

    // Interaction state.
    hover_hit: TabBarHit,

    // Drag state: (tab index, visual X position in logical pixels).
    drag_visual: Option<(usize, f32)>,

    // Per-tab animation offsets for smooth transitions (pixels).
    anim_offsets: Vec<f32>,

    // Per-tab hover animation progress (0.0 = inactive, 1.0 = hovered).
    hover_progress: Vec<AnimatedValue<f32>>,

    // Per-tab close button fade (0.0 = hidden, 1.0 = visible).
    close_btn_opacity: Vec<AnimatedValue<f32>>,

    // Per-tab width multiplier for open/close animation (0.0 = collapsed, 1.0 = full).
    width_multipliers: Vec<AnimatedValue<f32>>,

    // Per-tab closing flag (true = tab is animating closed, skip interaction).
    closing_tabs: Vec<bool>,

    // Window control buttons: [minimize, maximize/restore, close].
    controls: [WindowControlButton; 3],
    /// Index of the currently hovered control button (`None` if not hovering).
    hovered_control: Option<usize>,

    /// Extra left margin for platform chrome (macOS traffic lights).
    left_inset: f32,
}

impl TabBarWidget {
    /// Creates a new tab bar widget with default dark theme colors.
    pub fn new(window_width: f32) -> Self {
        Self::with_theme(window_width, &UiTheme::dark())
    }

    /// Creates a new tab bar widget with colors from the given theme.
    pub fn with_theme(window_width: f32, theme: &UiTheme) -> Self {
        let layout = TabBarLayout::compute(0, window_width, None, 0.0);
        let caption_bg = theme.bg_secondary;
        let ctrl_colors = control_colors_from_theme(theme);
        let controls = create_controls(ctrl_colors, caption_bg);

        Self {
            id: WidgetId::next(),
            tabs: Vec::new(),
            active_index: 0,
            layout,
            colors: TabBarColors::from_theme(theme),
            window_width,
            tab_width_lock: None,
            hover_hit: TabBarHit::None,
            drag_visual: None,
            anim_offsets: Vec::new(),
            hover_progress: Vec::new(),
            close_btn_opacity: Vec::new(),
            width_multipliers: Vec::new(),
            closing_tabs: Vec::new(),
            controls,
            hovered_control: None,
            left_inset: 0.0,
        }
    }

    // Theme

    /// Updates all theme-derived colors from a new [`UiTheme`].
    pub fn apply_theme(&mut self, theme: &UiTheme) {
        self.colors = TabBarColors::from_theme(theme);
        let ctrl_colors = control_colors_from_theme(theme);
        let caption_bg = theme.bg_secondary;
        for ctrl in &mut self.controls {
            ctrl.set_colors(ctrl_colors);
            ctrl.set_caption_bg(caption_bg);
        }
    }

    // State setters

    /// Updates the tab list and recomputes layout.
    ///
    /// Resets per-tab animation state (hover progress, close button opacity)
    /// since tab indices may have changed due to add/remove/reorder.
    pub fn set_tabs(&mut self, tabs: Vec<TabEntry>) {
        let n = tabs.len();
        self.tabs = tabs;
        self.hover_progress.clear();
        self.hover_progress.resize_with(n, || {
            AnimatedValue::new(0.0, TAB_HOVER_DURATION, Easing::EaseOut)
        });
        self.close_btn_opacity.clear();
        self.close_btn_opacity.resize_with(n, || {
            AnimatedValue::new(0.0, CLOSE_BTN_FADE_DURATION, Easing::EaseOut)
        });
        self.width_multipliers.clear();
        self.width_multipliers.resize_with(n, || {
            AnimatedValue::new(1.0, TAB_OPEN_DURATION, Easing::EaseOut)
        });
        self.closing_tabs.clear();
        self.closing_tabs.resize(n, false);
        self.recompute_layout();
    }

    /// Sets the active tab index.
    pub fn set_active_index(&mut self, index: usize) {
        self.active_index = index;
    }

    /// Updates the window width and recomputes layout.
    pub fn set_window_width(&mut self, width: f32) {
        self.window_width = width;
        self.recompute_layout();
    }

    /// Sets the tab width lock (freezes widths during hover).
    pub fn set_tab_width_lock(&mut self, lock: Option<f32>) {
        self.tab_width_lock = lock;
        self.recompute_layout();
    }

    /// Sets the left inset for platform chrome (macOS traffic lights).
    ///
    /// On macOS: `MACOS_TRAFFIC_LIGHT_WIDTH` (76px). On Windows/Linux: 0.
    pub fn set_left_inset(&mut self, inset: f32) {
        self.left_inset = inset;
        self.recompute_layout();
    }

    /// Updates which element the cursor is hovering, driving hover animations.
    ///
    /// Starts animated transitions for hover background and close button
    /// visibility on the affected tabs.
    pub fn set_hover_hit(&mut self, hit: TabBarHit, now: Instant) {
        let old_tab = self.hover_hit.tab_index();
        let new_tab = hit.tab_index();
        self.hover_hit = hit;

        // Animate hover leave on old tab.
        if let Some(i) = old_tab {
            if Some(i) != new_tab {
                if let Some(p) = self.hover_progress.get_mut(i) {
                    p.set(0.0, now);
                }
                if let Some(o) = self.close_btn_opacity.get_mut(i) {
                    o.set(0.0, now);
                }
            }
        }
        // Animate hover enter on new tab.
        if let Some(i) = new_tab {
            if Some(i) != old_tab {
                if let Some(p) = self.hover_progress.get_mut(i) {
                    p.set(1.0, now);
                }
                if let Some(o) = self.close_btn_opacity.get_mut(i) {
                    o.set(1.0, now);
                }
            }
        }
    }

    /// Sets the dragged tab visual state.
    ///
    /// `Some((index, x))` means tab `index` is being dragged and its visual
    /// position is at `x` logical pixels. `None` means no drag in progress.
    pub fn set_drag_visual(&mut self, drag: Option<(usize, f32)>) {
        self.drag_visual = drag;
    }

    // Tab lifecycle animations

    /// Starts a tab open animation, expanding from zero to full width.
    ///
    /// Call after `set_tabs()` which initializes the entry at 1.0.
    /// This overrides to start from 0.0 and animate to 1.0 over 200ms.
    pub fn animate_tab_open(&mut self, index: usize, now: Instant) {
        if let Some(m) = self.width_multipliers.get_mut(index) {
            m.set_immediate(0.0);
            m.set(1.0, now);
        }
    }

    /// Starts a tab close animation, shrinking from full to zero width.
    ///
    /// Marks the tab as closing (skipped for hover/click interaction).
    /// When the animation completes, call [`closing_complete`] to find
    /// which tab to remove.
    pub fn animate_tab_close(&mut self, index: usize, now: Instant) {
        if let Some(m) = self.width_multipliers.get_mut(index) {
            *m = AnimatedValue::new(1.0, TAB_CLOSE_DURATION, Easing::EaseOut);
            m.set(0.0, now);
        }
        if let Some(c) = self.closing_tabs.get_mut(index) {
            *c = true;
        }
    }

    /// Returns the index of a tab whose close animation has finished.
    ///
    /// The app layer polls this during redraw and removes the finished
    /// tab via `set_tabs()`.
    pub fn closing_complete(&self, now: Instant) -> Option<usize> {
        self.closing_tabs
            .iter()
            .enumerate()
            .find(|&(i, &closing)| {
                closing
                    && self
                        .width_multipliers
                        .get(i)
                        .is_none_or(|m| m.get(now) < 0.01)
            })
            .map(|(i, _)| i)
    }

    /// Whether the tab at `index` is in closing state.
    pub fn is_closing(&self, index: usize) -> bool {
        self.closing_tabs.get(index).copied().unwrap_or(false)
    }

    /// Whether any width animation is currently running.
    pub fn has_width_animation(&self, now: Instant) -> bool {
        self.width_multipliers.iter().any(|m| m.is_animating(now))
    }

    /// Updates layout with current animated width multipliers.
    ///
    /// Call once per frame before draw when width animations are active.
    /// No-op when no width animations are running.
    pub fn update_animated_layout(&mut self, now: Instant) {
        if self.has_width_animation(now) {
            self.recompute_layout_animated(now);
        }
    }

    // Accessors

    /// Current computed layout.
    pub fn layout(&self) -> &TabBarLayout {
        &self.layout
    }

    /// Number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Current hover hit state.
    pub fn hover_hit(&self) -> TabBarHit {
        self.hover_hit
    }

    /// Current tab width lock value, if active.
    pub fn tab_width_lock(&self) -> Option<f32> {
        self.tab_width_lock
    }

    /// Update the title of the tab at `index`.
    ///
    /// No-op if `index` is out of bounds.
    pub fn update_tab_title(&mut self, index: usize, title: String) {
        if let Some(entry) = self.tabs.get_mut(index) {
            entry.title = title;
        }
    }

    /// Start a bell animation on the tab at `index`.
    ///
    /// Records `now` as the bell start time. No-op if `index` is out of
    /// bounds.
    pub fn ring_bell(&mut self, index: usize, now: Instant) {
        if let Some(entry) = self.tabs.get_mut(index) {
            entry.bell_start = Some(now);
        }
    }

    // Private helpers

    /// Recomputes layout from current state.
    ///
    /// When width multipliers are active (during open/close animations),
    /// passes current multiplier values to the layout computation.
    fn recompute_layout(&mut self) {
        self.layout = TabBarLayout::compute(
            self.tabs.len(),
            self.window_width,
            self.tab_width_lock,
            self.left_inset,
        );
    }

    /// Recomputes layout with current animated width multipliers.
    ///
    /// Called during draw when width animations are running. Samples
    /// each `AnimatedValue` at `now` and passes the snapshot to layout.
    fn recompute_layout_animated(&mut self, now: Instant) {
        let multipliers: Vec<f32> = self.width_multipliers.iter().map(|m| m.get(now)).collect();
        self.layout = TabBarLayout::compute_with_multipliers(
            self.tabs.len(),
            self.window_width,
            self.tab_width_lock,
            self.left_inset,
            Some(&multipliers),
        );
    }

    /// Returns the animation offset for a tab, or 0.0 if none.
    fn anim_offset(&self, index: usize) -> f32 {
        self.anim_offsets.get(index).copied().unwrap_or(0.0)
    }

    /// Whether the given tab index is the one being dragged.
    fn is_dragged(&self, index: usize) -> bool {
        self.drag_visual.is_some_and(|(i, _)| i == index)
    }

    /// Swaps the internal animation offset buffer with an external one.
    ///
    /// Used by [`TabSlideState`](super::slide::TabSlideState) to populate
    /// per-tab offsets from compositor transforms without allocating. The
    /// caller fills `buf` with compositor-driven offsets, swaps in, and
    /// gets the old buffer back for reuse next frame.
    pub(crate) fn swap_anim_offsets(&mut self, buf: &mut Vec<f32>) {
        std::mem::swap(&mut self.anim_offsets, buf);
    }
}

// Free functions

/// Builds [`ControlButtonColors`] from a [`UiTheme`].
fn control_colors_from_theme(theme: &UiTheme) -> ControlButtonColors {
    ControlButtonColors {
        fg: theme.fg_primary,
        bg: Color::TRANSPARENT,
        hover_bg: theme.bg_hover,
        close_hover_bg: theme.close_hover_bg,
        close_pressed_bg: theme.close_pressed_bg,
    }
}

/// Creates the three control buttons with initial colors and caption bg.
fn create_controls(colors: ControlButtonColors, caption_bg: Color) -> [WindowControlButton; 3] {
    let mut min_btn = WindowControlButton::new(ControlKind::Minimize, colors);
    min_btn.set_caption_bg(caption_bg);
    let mut max_btn = WindowControlButton::new(ControlKind::MaximizeRestore, colors);
    max_btn.set_caption_bg(caption_bg);
    let mut close_btn = WindowControlButton::new(ControlKind::Close, colors);
    close_btn.set_caption_bg(caption_bg);
    [min_btn, max_btn, close_btn]
}

// Test helpers

#[cfg(test)]
impl TabBarWidget {
    /// Test-only access to bell phase computation.
    pub fn bell_phase_for_test(tab: &TabEntry, now: Instant) -> f32 {
        draw::bell_phase(tab, now)
    }

    /// Test-only access to drag-adjusted new-tab button X.
    pub fn test_new_tab_button_x(&self) -> f32 {
        draw::new_tab_button_x(self)
    }

    /// Test-only access to drag-adjusted dropdown button X.
    pub fn test_dropdown_button_x(&self) -> f32 {
        draw::dropdown_button_x(self)
    }

    /// Test-only access to hover progress for a tab.
    pub fn test_hover_progress(&self, index: usize, now: Instant) -> f32 {
        self.hover_progress.get(index).map_or(0.0, |p| p.get(now))
    }

    /// Test-only access to close button opacity for a tab.
    pub fn test_close_btn_opacity(&self, index: usize, now: Instant) -> f32 {
        self.close_btn_opacity
            .get(index)
            .map_or(0.0, |o| o.get(now))
    }
}
