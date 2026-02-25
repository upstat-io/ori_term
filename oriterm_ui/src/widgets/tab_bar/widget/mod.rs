//! Tab bar rendering widget.
//!
//! [`TabBarWidget`] draws the tab strip: tab backgrounds with titles, close
//! buttons, separators, new-tab (+) button, and dropdown button. All
//! coordinates are in logical pixels; the caller applies scale at the
//! rendering boundary.
//!
//! The widget implements [`Widget`] for draw integration. Event handling
//! stubs are provided here; full hit-test dispatch is Section 16.3.

mod draw;

use std::time::Instant;

use crate::theme::UiTheme;
use crate::widget_id::WidgetId;

use super::colors::TabBarColors;
use super::hit::TabBarHit;
use super::layout::TabBarLayout;

/// Per-tab visual state provided by the application layer.
#[derive(Debug, Clone)]
pub struct TabEntry {
    /// Tab title (empty string shows "Terminal" as fallback).
    pub title: String,
    /// When the bell last fired (for pulse animation). `None` if no bell.
    pub bell_start: Option<Instant>,
}

impl TabEntry {
    /// Creates a tab entry with the given title and no bell.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            bell_start: None,
        }
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
}

impl TabBarWidget {
    /// Creates a new tab bar widget with default dark theme colors.
    pub fn new(window_width: f32) -> Self {
        Self::with_theme(window_width, &UiTheme::dark())
    }

    /// Creates a new tab bar widget with colors from the given theme.
    pub fn with_theme(window_width: f32, theme: &UiTheme) -> Self {
        let layout = TabBarLayout::compute(0, window_width, None);
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
        }
    }

    // --- State setters ---

    /// Updates the tab list and recomputes layout.
    pub fn set_tabs(&mut self, tabs: Vec<TabEntry>) {
        self.tabs = tabs;
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

    /// Updates which element the cursor is hovering.
    pub fn set_hover_hit(&mut self, hit: TabBarHit) {
        self.hover_hit = hit;
    }

    /// Sets the dragged tab visual state.
    ///
    /// `Some((index, x))` means tab `index` is being dragged and its visual
    /// position is at `x` logical pixels. `None` means no drag in progress.
    pub fn set_drag_visual(&mut self, drag: Option<(usize, f32)>) {
        self.drag_visual = drag;
    }

    /// Sets per-tab animation offsets.
    ///
    /// Each entry is a pixel offset applied to the corresponding tab's X
    /// position during rendering. Used for smooth tab reorder transitions.
    pub fn set_anim_offsets(&mut self, offsets: Vec<f32>) {
        self.anim_offsets = offsets;
    }

    // --- Accessors ---

    /// Current computed layout.
    pub fn layout(&self) -> &TabBarLayout {
        &self.layout
    }

    /// Number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
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
    /// Records the current instant as the bell start time. No-op if
    /// `index` is out of bounds.
    pub fn ring_bell(&mut self, index: usize) {
        if let Some(entry) = self.tabs.get_mut(index) {
            entry.bell_start = Some(Instant::now());
        }
    }

    // --- Private helpers ---

    /// Recomputes layout from current state.
    fn recompute_layout(&mut self) {
        self.layout =
            TabBarLayout::compute(self.tabs.len(), self.window_width, self.tab_width_lock);
    }

    /// Returns the animation offset for a tab, or 0.0 if none.
    fn anim_offset(&self, index: usize) -> f32 {
        self.anim_offsets.get(index).copied().unwrap_or(0.0)
    }

    /// Whether the given tab index is the one being dragged.
    fn is_dragged(&self, index: usize) -> bool {
        self.drag_visual.is_some_and(|(i, _)| i == index)
    }

    /// Decay tab animation offsets toward zero.
    ///
    /// Returns `true` if any animation is still active (needs continued
    /// rendering). Called each frame during the settling phase after a
    /// drag-end or tab reorder.
    ///
    /// Uses exponential decay with a ~50ms half-life, snapping to zero
    /// when offsets fall below 0.5 logical pixels.
    pub fn decay_tab_animations(&mut self, dt: f32) -> bool {
        /// Exponential decay rate (higher = faster settling).
        /// At 15.0: ~50ms half-life, effectively settled by ~200ms.
        const DECAY_RATE: f32 = 15.0;
        /// Snap to zero when offset is smaller than this (logical pixels).
        const SNAP_THRESHOLD: f32 = 0.5;

        let factor = (-DECAY_RATE * dt).exp();
        let mut any_active = false;
        for offset in &mut self.anim_offsets {
            if *offset == 0.0 {
                continue;
            }
            *offset *= factor;
            if offset.abs() < SNAP_THRESHOLD {
                *offset = 0.0;
            } else {
                any_active = true;
            }
        }
        any_active
    }
}

// --- Test helpers ---

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
}
