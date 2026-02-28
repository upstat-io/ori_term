//! Per-window state container.
//!
//! Groups all state that is specific to a single OS window: widgets, render
//! caches, interaction state, and the dirty flag. Extracted from [`App`] to
//! enable multi-window support (Section 32.3).

use std::time::Instant;

use oriterm_mux::layout::DividerLayout;

use oriterm_ui::compositor::layer_animator::LayerAnimator;
use oriterm_ui::compositor::layer_tree::LayerTree;
use oriterm_ui::draw::DrawList;
use oriterm_ui::geometry::Rect;
use oriterm_ui::overlay::OverlayManager;
use oriterm_ui::widgets::tab_bar::TabBarWidget;
use oriterm_ui::widgets::window_chrome::WindowChromeWidget;

use super::divider_drag::DividerDragState;
use super::floating_drag::FloatingDragState;
use crate::gpu::{FrameInput, PaneRenderCache};
use crate::url_detect::{DetectedUrl, UrlDetectCache};
use crate::widgets::terminal_grid::TerminalGridWidget;
use crate::window::TermWindow;

/// Per-window state: widgets, caches, interaction state, and dirty flag.
///
/// Each OS window gets its own `WindowContext`. The [`App`](super::App) stores
/// these in a `HashMap<WindowId, WindowContext>` keyed by winit window ID.
pub(crate) struct WindowContext {
    // Core window handle.
    pub window: TermWindow,

    // Widget layer.
    pub chrome: WindowChromeWidget,
    pub tab_bar: TabBarWidget,
    pub terminal_grid: TerminalGridWidget,

    // Render caches.
    pub pane_cache: PaneRenderCache,
    pub frame: Option<FrameInput>,
    pub chrome_draw_list: DrawList,

    // Layout caches.
    pub cached_dividers: Option<Vec<DividerLayout>>,

    // Compositor state.
    pub layer_tree: LayerTree,
    pub layer_animator: LayerAnimator,

    // Interaction state.
    pub hovering_divider: Option<DividerLayout>,
    pub divider_drag: Option<DividerDragState>,
    pub floating_drag: Option<FloatingDragState>,
    pub overlays: OverlayManager,
    pub hovered_url: Option<DetectedUrl>,
    pub url_cache: UrlDetectCache,
    pub pending_paste: Option<String>,
    pub last_drag_area_press: Option<Instant>,

    // Reusable buffers.
    pub search_bar_buf: String,

    // Redraw coalescing.
    pub dirty: bool,
}

impl WindowContext {
    /// Create a new window context from its constituent parts.
    ///
    /// The `window`, `chrome`, `tab_bar`, and `terminal_grid` are created
    /// during init; all other fields start at their defaults.
    pub fn new(
        window: TermWindow,
        chrome: WindowChromeWidget,
        tab_bar: TabBarWidget,
        terminal_grid: TerminalGridWidget,
    ) -> Self {
        Self {
            window,
            chrome,
            tab_bar,
            terminal_grid,
            pane_cache: PaneRenderCache::new(),
            frame: None,
            chrome_draw_list: DrawList::new(),
            layer_tree: LayerTree::new(Rect::default()),
            layer_animator: LayerAnimator::new(),
            cached_dividers: None,
            hovering_divider: None,
            divider_drag: None,
            floating_drag: None,
            overlays: OverlayManager::new(Rect::default()),
            hovered_url: None,
            url_cache: UrlDetectCache::default(),
            pending_paste: None,
            last_drag_area_press: None,
            search_bar_buf: String::new(),
            dirty: true,
        }
    }
}
