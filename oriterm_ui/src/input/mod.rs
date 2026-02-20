//! Widget-level input handling: event types, hit testing, and routing.
//!
//! Distinct from `hit_test` (window chrome hit testing). This module handles
//! widget-tree traversal, mouse/keyboard event dispatch, hover tracking,
//! and mouse capture.

mod event;
mod hit_test;
mod routing;

pub use event::{
    EventResponse, HoverEvent, Key, KeyEvent, Modifiers, MouseButton, MouseEvent, MouseEventKind,
    ScrollDelta,
};
pub use hit_test::{layout_hit_test, layout_hit_test_clipped};
pub use routing::{InputState, RouteAction};

#[cfg(test)]
mod tests;
