//! Overlay and modal system for floating UI layers.
//!
//! Provides [`OverlayManager`] for managing a stack of overlays above the
//! main widget tree. Used by context menus, dropdown popups, command palette,
//! tooltips, and modal dialogs.

mod manager;
mod overlay_id;
mod placement;

pub use manager::{OverlayEventResult, OverlayManager};
pub use overlay_id::OverlayId;
pub use placement::Placement;

#[cfg(test)]
mod tests;
