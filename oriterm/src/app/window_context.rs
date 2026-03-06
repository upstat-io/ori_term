//! Per-window state container.
//!
//! Groups all state that is specific to a single OS window: widgets, render
//! caches, interaction state, and the dirty flag. Extracted from [`App`] to
//! enable multi-window support (Section 32.3).

use std::time::Instant;

use oriterm_mux::id::PaneId;

use crate::session::DividerLayout;

use oriterm_ui::compositor::layer_animator::LayerAnimator;
use oriterm_ui::compositor::layer_tree::LayerTree;
use oriterm_ui::draw::DrawList;
use oriterm_ui::geometry::Rect;
use oriterm_ui::overlay::OverlayManager;
use oriterm_ui::widgets::tab_bar::{TabBarWidget, TabSlideState};
use oriterm_ui::widgets::window_chrome::WindowChromeWidget;

use super::context_menu::ContextMenuState;
use super::divider_drag::DividerDragState;
use super::floating_drag::FloatingDragState;
use super::tab_drag::TabDragState;
use crate::gpu::{FrameInput, PaneRenderCache, WindowRenderer};
use crate::url_detect::{DetectedUrl, UrlDetectCache};
use crate::widgets::terminal_grid::TerminalGridWidget;
use crate::window::TermWindow;

/// Per-window state: widgets, caches, interaction state, and dirty flag.
///
/// Each OS window gets its own `WindowContext`. The [`App`](super::App) stores
/// these in a `HashMap<WindowId, WindowContext>` keyed by winit window ID.
pub(crate) struct WindowContext {
    // Core window handle.
    pub(super) window: TermWindow,

    // Per-window GPU renderer (owns fonts, atlases, shaping, instance buffers).
    pub(super) renderer: Option<WindowRenderer>,

    // Widget layer.
    pub(super) chrome: WindowChromeWidget,
    pub(super) tab_bar: TabBarWidget,
    pub(super) terminal_grid: TerminalGridWidget,

    // Render caches.
    pub(super) pane_cache: PaneRenderCache,
    pub(super) frame: Option<FrameInput>,
    pub(super) chrome_draw_list: DrawList,
    /// Pane rendered in the previous single-pane frame. Used to detect
    /// tab switches / tear-off so `renderable_cache` contamination from
    /// `swap_renderable_content` is flushed with a forced refresh.
    pub(super) last_rendered_pane: Option<PaneId>,

    // Layout caches.
    pub(super) cached_dividers: Option<Vec<DividerLayout>>,

    // Compositor state.
    pub(super) layer_tree: LayerTree,
    pub(super) layer_animator: LayerAnimator,
    pub(super) tab_slide: TabSlideState,

    // Interaction state.
    pub(super) hovering_divider: Option<DividerLayout>,
    pub(super) divider_drag: Option<DividerDragState>,
    pub(super) floating_drag: Option<FloatingDragState>,
    pub(super) tab_drag: Option<TabDragState>,
    pub(super) overlays: OverlayManager,
    pub(super) context_menu: Option<ContextMenuState>,
    pub(super) hovered_url: Option<DetectedUrl>,
    pub(super) url_cache: UrlDetectCache,
    pub(super) pending_paste: Option<String>,
    pub(super) last_drag_area_press: Option<Instant>,

    // Reusable buffers.
    pub(super) search_bar_buf: String,

    // Redraw coalescing.
    pub(super) dirty: bool,
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
        renderer: Option<WindowRenderer>,
    ) -> Self {
        Self {
            window,
            renderer,
            chrome,
            tab_bar,
            terminal_grid,
            pane_cache: PaneRenderCache::new(),
            frame: None,
            chrome_draw_list: DrawList::new(),
            last_rendered_pane: None,
            layer_tree: LayerTree::new(Rect::default()),
            layer_animator: LayerAnimator::new(),
            tab_slide: TabSlideState::new(),
            cached_dividers: None,
            hovering_divider: None,
            divider_drag: None,
            floating_drag: None,
            tab_drag: None,
            overlays: OverlayManager::new(Rect::default()),
            context_menu: None,
            hovered_url: None,
            url_cache: UrlDetectCache::default(),
            pending_paste: None,
            last_drag_area_press: None,
            search_bar_buf: String::new(),
            dirty: true,
        }
    }
}
