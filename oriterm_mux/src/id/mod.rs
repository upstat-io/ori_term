//! Identity types for the multiplexing system.
//!
//! Every pane, tab, window, and session in the mux layer is identified by a
//! strongly-typed newtype ID. These types prevent accidental mixing of IDs
//! from different domains (e.g., passing a `TabId` where a `PaneId` is
//! expected) and provide readable `Display` output for logging.

use std::fmt;

/// Globally unique pane identifier.
///
/// Each pane represents one shell process with its own terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaneId(u64);

/// Globally unique tab identifier.
///
/// A tab is a layout container that holds one or more panes arranged in a
/// split tree, plus an optional floating pane layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TabId(u64);

/// Mux-level window identifier.
///
/// This is the mux layer's own window identity, distinct from the platform
/// window ID (e.g., `winit::window::WindowId`). The GUI layer maintains a
/// bidirectional mapping between the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WindowId(u64);

/// Session identifier for persistence and restore.
///
/// A session groups windows, tabs, and panes into a restorable unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SessionId(u64);

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pane({})", self.0)
    }
}

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tab({})", self.0)
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Window({})", self.0)
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Session({})", self.0)
    }
}

/// Monotonic ID allocator.
///
/// Each ID domain (panes, tabs, windows, sessions) gets its own allocator.
/// IDs start at 1; 0 is reserved as "no ID" for sentinel use.
#[derive(Debug)]
pub struct IdAllocator {
    counter: u64,
}

impl IdAllocator {
    /// Create a new allocator. First allocated ID will be 1.
    pub fn new() -> Self {
        Self { counter: 1 }
    }

    /// Allocate the next ID value, incrementing the counter.
    pub fn alloc(&mut self) -> u64 {
        let id = self.counter;
        self.counter += 1;
        id
    }
}

impl Default for IdAllocator {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience constructors for ID types.
///
/// These are intentionally not `From<u64>` to avoid accidental construction.
/// Use `IdAllocator` for normal allocation; these are for deserialization and
/// test setup.
impl PaneId {
    /// Create a `PaneId` from a raw value.
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the underlying raw value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl TabId {
    /// Create a `TabId` from a raw value.
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the underlying raw value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl WindowId {
    /// Create a `WindowId` from a raw value.
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the underlying raw value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl SessionId {
    /// Create a `SessionId` from a raw value.
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the underlying raw value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests;
