//! GUI session model: windows, tabs, and pane layouts.
//!
//! This module owns all presentation state — how panes are grouped into
//! tabs, how tabs are grouped into windows, how panes are arranged
//! within a tab. The mux layer knows nothing about this; it just
//! provides panes.

// Some session methods/fields are defined but not yet consumed — they are used
// when section 02.4 switches all mutations to local-first (ID allocators,
// undo/redo, tab reorder, etc.).
#![allow(
    dead_code,
    reason = "session API consumed incrementally across sections 02.2–02.4"
)]

pub mod id;
mod registry;
mod tab;
mod window;

// Layout submodules (copied from oriterm_mux in Section 04).
pub mod compute;
pub mod floating;
pub mod nav;
pub mod rect;
pub mod split_tree;

pub use id::{TabId, WindowId};
pub use registry::SessionRegistry;
pub use tab::Tab;
pub use window::Window;

// Layout re-exports for convenient access from consuming modules.
pub use compute::{DividerLayout, LayoutDescriptor, PaneLayout, compute_all};
pub use floating::{FloatingPane, snap_to_edge};
pub use rect::Rect;
pub use split_tree::SplitDirection;
