//! Multiplexing data structures for oriterm.
//!
//! This crate provides the core data model for the terminal multiplexer:
//! identity types, immutable split trees, floating pane layers, layout
//! computation, and spatial navigation. It is a pure data-structures crate
//! with no I/O, no GUI, and no PTY dependencies — fully testable in
//! isolation.
//!
//! # Architecture
//!
//! `oriterm_mux` sits between `oriterm_core` (terminal emulation) and
//! `oriterm` (GUI binary). It owns all multiplexing state: which panes
//! exist, how they're arranged, and how to navigate between them.

#![deny(unsafe_code)]

pub mod id;
pub mod layout;
pub mod nav;

pub use id::{IdAllocator, PaneId, SessionId, TabId, WindowId};
pub use layout::{SplitDirection, SplitTree};
pub use nav::Direction;
