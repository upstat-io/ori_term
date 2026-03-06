//! GUI-local identity types for tabs and windows.
//!
//! These are distinct from `oriterm_mux::PaneId` — the mux owns pane identity,
//! but the GUI owns tab and window identity. Different clients connected to the
//! same mux daemon may use different tab/window IDs for the same panes.

use std::fmt;
use std::marker::PhantomData;

/// GUI-local tab identifier.
///
/// A tab is a layout container that holds one or more panes arranged in a
/// split tree, plus an optional floating pane layer. This ID is allocated
/// and owned by the GUI, not the mux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TabId(u64);

/// GUI-local window identifier.
///
/// Distinct from `winit::window::WindowId` (platform window). The GUI
/// maintains a mapping between this logical ID and the platform window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct WindowId(u64);

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

/// Sealed trait for GUI session ID newtypes, enabling type-safe allocation.
pub(crate) trait SessionId: sealed::Sealed + Copy {
    /// Construct this ID type from a raw counter value.
    fn from_raw(raw: u64) -> Self;

    /// Return the underlying raw value.
    #[allow(dead_code, reason = "used in tests; part of the SessionId contract")]
    fn raw(self) -> u64;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::TabId {}
    impl Sealed for super::WindowId {}
}

impl SessionId for TabId {
    fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    fn raw(self) -> u64 {
        self.0
    }
}

impl SessionId for WindowId {
    fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    fn raw(self) -> u64 {
        self.0
    }
}

impl TabId {
    /// Create a `TabId` from a raw value.
    ///
    /// Prefer `IdAllocator::<TabId>::alloc()` for runtime allocation. This
    /// constructor is for deserialization and test setup.
    #[allow(dead_code, reason = "used in tests and deserialization")]
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the underlying raw value.
    #[allow(dead_code, reason = "used in tests and serialization")]
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl WindowId {
    /// Create a `WindowId` from a raw value.
    ///
    /// Prefer `IdAllocator::<WindowId>::alloc()` for runtime allocation. This
    /// constructor is for deserialization and test setup.
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the underlying raw value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// Type-safe monotonic ID allocator for GUI session IDs.
///
/// Each ID domain (tabs, windows) gets its own allocator parameterized by
/// the ID type. IDs start at 1; 0 is reserved as "no ID" sentinel.
#[derive(Debug)]
pub(crate) struct IdAllocator<T: SessionId> {
    counter: u64,
    _phantom: PhantomData<T>,
}

impl<T: SessionId> IdAllocator<T> {
    /// Create a new allocator. First allocated ID will be 1.
    pub fn new() -> Self {
        Self {
            counter: 1,
            _phantom: PhantomData,
        }
    }

    /// Allocate the next ID, incrementing the counter.
    pub fn alloc(&mut self) -> T {
        let id = self.counter;
        self.counter += 1;
        T::from_raw(id)
    }
}

impl<T: SessionId> Default for IdAllocator<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
