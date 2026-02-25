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
pub mod paste;
pub mod search;
pub mod selection;
pub mod sync;
pub mod term;
pub mod theme;

pub use cell::{Cell, CellExtra, CellFlags, Hyperlink};
pub use color::{Palette, Rgb, SelectionColors};
pub use event::{ClipboardType, Event, EventListener, VoidListener};
pub use grid::{
    Cursor, CursorShape, DisplayEraseMode, Grid, LineEraseMode, Row, StableRowIndex, TabClearMode,
};
pub use index::{Boundary, Column, Direction, Line, Point, Side};
pub use search::text::extract_row_text;
pub use search::{MatchType, SearchMatch, SearchState};
pub use selection::{
    ClickDetector, DEFAULT_WORD_DELIMITERS, Selection, SelectionBounds, SelectionMode,
    SelectionPoint, logical_line_end, logical_line_start,
};
pub use sync::{FairMutex, FairMutexGuard};
pub use term::{
    DamageLine, RenderableCell, RenderableContent, RenderableCursor, Term, TermDamage, TermMode,
};
pub use theme::Theme;
