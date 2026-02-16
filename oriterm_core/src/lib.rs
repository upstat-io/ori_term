//! Core terminal emulation data structures and logic.
//!
//! This crate provides the foundational types for terminal emulation:
//! cell representation, grid storage, cursor management, scrollback,
//! and all grid editing/navigation operations. It contains no GUI,
//! rendering, or platform-specific code.

#![deny(unsafe_code)]

pub mod cell;
pub mod color;
pub mod event;
pub mod grid;
pub mod index;
pub mod sync;
pub mod term;
pub mod theme;

pub use cell::{Cell, CellExtra, CellFlags, Hyperlink};
pub use color::{Palette, Rgb};
pub use event::{ClipboardType, Event, EventListener, VoidListener};
pub use grid::{Cursor, CursorShape, DisplayEraseMode, Grid, LineEraseMode, Row, TabClearMode};
pub use index::{Boundary, Column, Direction, Line, Point, Side};
pub use sync::{FairMutex, FairMutexGuard, FairMutexLease};
pub use term::{
    DamageLine, RenderableCell, RenderableContent, RenderableCursor, Term, TermDamage, TermMode,
};
pub use theme::Theme;
