//! Core widget types and traits for the UI framework.
//!
//! Provides the [`Widget`] trait, action/response types, and context structs
//! that widgets use during layout, drawing, and event handling. Each widget
//! is a concrete struct implementing `Widget`; no trait objects are needed
//! in the widget tree.

pub mod text_measurer;

pub mod button;
pub mod checkbox;
pub mod dropdown;
pub mod flex;
pub mod label;
pub mod panel;
pub mod scroll;
pub mod separator;
pub mod slider;
pub mod spacer;
pub mod stack;
pub mod text_input;
pub mod toggle;

use std::cell::Cell;
use std::time::Instant;

use crate::color::Color;
use crate::draw::DrawList;
use crate::geometry::Rect;
use crate::input::{EventResponse, HoverEvent, KeyEvent, MouseEvent};
use crate::layout::LayoutBox;
use crate::widget_id::WidgetId;

pub use text_measurer::TextMeasurer;

/// How a widget responded to an event, including an optional semantic action.
///
/// Widgets return this from event handlers. The `response` field tells the
/// framework how to handle propagation; the `action` field carries semantic
/// meaning for the application layer.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetResponse {
    /// How the framework should handle this event.
    pub response: EventResponse,
    /// Optional semantic action for the application layer to interpret.
    pub action: Option<WidgetAction>,
}

impl WidgetResponse {
    /// Event handled, no action emitted.
    pub fn handled() -> Self {
        Self {
            response: EventResponse::Handled,
            action: None,
        }
    }

    /// Event ignored — propagate to parent.
    pub fn ignored() -> Self {
        Self {
            response: EventResponse::Ignored,
            action: None,
        }
    }

    /// Event handled and a redraw is needed.
    pub fn redraw() -> Self {
        Self {
            response: EventResponse::RequestRedraw,
            action: None,
        }
    }

    /// Event handled, focus requested, no action.
    pub fn focus() -> Self {
        Self {
            response: EventResponse::RequestFocus,
            action: None,
        }
    }

    /// Attaches an action to this response.
    #[must_use]
    pub fn with_action(mut self, action: WidgetAction) -> Self {
        self.action = Some(action);
        self
    }
}

/// A semantic action emitted by a widget for the application layer.
///
/// No closures — the app layer matches on variants and interprets them.
/// This keeps widgets stateless with respect to application logic.
#[derive(Debug, Clone, PartialEq)]
pub enum WidgetAction {
    /// A button or clickable widget was activated.
    Clicked(WidgetId),
    /// A boolean value was toggled (checkbox, toggle switch).
    Toggled { id: WidgetId, value: bool },
    /// A numeric value changed (slider).
    ValueChanged { id: WidgetId, value: f32 },
    /// Text content changed (text input).
    TextChanged { id: WidgetId, text: String },
    /// An item was selected by index (dropdown).
    Selected { id: WidgetId, index: usize },
    /// An overlay content widget requests its own dismissal.
    DismissOverlay(WidgetId),
}

/// Context passed to [`Widget::layout`].
pub struct LayoutCtx<'a> {
    /// Text measurement provider.
    pub measurer: &'a dyn TextMeasurer,
}

/// Context passed to [`Widget::draw`].
pub struct DrawCtx<'a> {
    /// Text shaping provider.
    pub measurer: &'a dyn TextMeasurer,
    /// The draw command list to append to.
    pub draw_list: &'a mut DrawList,
    /// The widget's computed bounds (from layout).
    pub bounds: Rect,
    /// The currently focused widget, if any.
    pub focused_widget: Option<WidgetId>,
    /// Current frame timestamp for animation interpolation.
    pub now: Instant,
    /// Set to `true` by widgets with running animations to request redraw.
    pub animations_running: &'a Cell<bool>,
}

/// Context passed to mouse and keyboard event handlers.
pub struct EventCtx<'a> {
    /// Text measurement provider.
    pub measurer: &'a dyn TextMeasurer,
    /// The widget's computed bounds (from layout).
    pub bounds: Rect,
    /// Whether this widget currently has keyboard focus.
    pub is_focused: bool,
}

/// The core widget trait.
///
/// Each widget is a concrete struct that implements this trait. Widgets
/// own their visual state (hovered, pressed) and app state (checked, value),
/// plus a style struct with `Default` dark-theme defaults.
pub trait Widget {
    /// Returns this widget's unique identifier.
    fn id(&self) -> WidgetId;

    /// Whether this widget can receive keyboard focus.
    fn is_focusable(&self) -> bool;

    /// Builds a layout descriptor for the layout solver.
    fn layout(&self, ctx: &LayoutCtx<'_>) -> LayoutBox;

    /// Draws the widget into the draw list.
    fn draw(&self, ctx: &mut DrawCtx<'_>);

    /// Handles a mouse event. Returns a response with optional action.
    fn handle_mouse(&mut self, event: &MouseEvent, ctx: &EventCtx<'_>) -> WidgetResponse;

    /// Handles a synthetic hover event (enter/leave).
    fn handle_hover(&mut self, event: HoverEvent, ctx: &EventCtx<'_>) -> WidgetResponse;

    /// Handles a keyboard event. Returns a response with optional action.
    fn handle_key(&mut self, event: KeyEvent, ctx: &EventCtx<'_>) -> WidgetResponse;
}

// Default dark-theme colors shared across widget styles.

/// Default widget background color (dark surface).
pub const DEFAULT_BG: Color = Color::from_rgb_u8(0x2D, 0x2D, 0x2D);

/// Default widget background when hovered.
pub const DEFAULT_HOVER_BG: Color = Color::from_rgb_u8(0x3D, 0x3D, 0x3D);

/// Default widget background when pressed.
pub const DEFAULT_PRESSED_BG: Color = Color::from_rgb_u8(0x1D, 0x1D, 0x1D);

/// Default widget foreground / text color.
pub const DEFAULT_FG: Color = Color::from_rgb_u8(0xE0, 0xE0, 0xE0);

/// Default widget border color.
pub const DEFAULT_BORDER: Color = Color::from_rgb_u8(0x55, 0x55, 0x55);

/// Default accent color (for toggles, sliders, focused elements).
pub const DEFAULT_ACCENT: Color = Color::from_rgb_u8(0x4A, 0x9E, 0xFF);

/// Default disabled text/foreground color.
pub const DEFAULT_DISABLED_FG: Color = Color::from_rgb_u8(0x80, 0x80, 0x80);

/// Default disabled background color.
pub const DEFAULT_DISABLED_BG: Color = Color::from_rgb_u8(0x25, 0x25, 0x25);

/// Default focus ring color.
pub const DEFAULT_FOCUS_RING: Color = Color::from_rgb_u8(0x4A, 0x9E, 0xFF);

#[cfg(test)]
pub(crate) mod tests;
