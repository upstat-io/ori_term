//! Identity types for the multiplexing system.
//!
//! Every pane, domain, and client in the mux layer is identified by a
//! strongly-typed newtype ID. These types prevent accidental mixing of IDs
//! from different domains and provide readable `Display` output for logging.

use std::fmt;
use std::marker::PhantomData;

/// Globally unique pane identifier.
///
/// Each pane represents one shell process with its own terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PaneId(u64);

/// Domain identifier for shell-spawning backends.
///
/// Each domain represents a distinct environment where shells can be
/// spawned: local machine, WSL distro, SSH host, serial port, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DomainId(u64);

/// Client connection identifier.
///
/// Each window process that connects to the mux daemon receives a unique
/// `ClientId` for the duration of its connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ClientId(u64);

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pane({})", self.0)
    }
}

impl fmt::Display for DomainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Domain({})", self.0)
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Client({})", self.0)
    }
}

/// Sealed trait for mux ID newtypes, enabling type-safe allocation.
///
/// This trait is sealed — only the ID types in this module implement it.
/// External crates cannot add implementations.
pub trait MuxId: sealed::Sealed + Copy {
    /// Construct this ID type from a raw counter value.
    fn from_raw(raw: u64) -> Self;

    /// Return the underlying raw value.
    fn raw(self) -> u64;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::PaneId {}
    impl Sealed for super::DomainId {}
    impl Sealed for super::ClientId {}
}

impl MuxId for PaneId {
    fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    fn raw(self) -> u64 {
        self.0
    }
}

impl MuxId for DomainId {
    fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    fn raw(self) -> u64 {
        self.0
    }
}

impl MuxId for ClientId {
    fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    fn raw(self) -> u64 {
        self.0
    }
}

/// Convenience constructors for ID types.
///
/// These are intentionally not `From<u64>` to avoid accidental construction.
/// Use `IdAllocator` for normal allocation; `from_raw`/`raw` are for
/// deserialization, test setup, and cross-boundary ID transfer.
///
/// **Warning:** IDs created via `from_raw` bypass the allocator's uniqueness
/// guarantee. If the raw value overlaps with a future allocator-produced ID,
/// you will get collisions. Prefer `IdAllocator::alloc` for all runtime ID
/// creation.
impl PaneId {
    /// Create a `PaneId` from a raw value.
    ///
    /// Prefer `IdAllocator::<PaneId>::alloc()` for runtime allocation. This
    /// constructor is for deserialization and test setup — raw values that
    /// collide with allocator-produced IDs will cause silent bugs.
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the underlying raw value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl DomainId {
    /// Create a `DomainId` from a raw value.
    ///
    /// Prefer `IdAllocator::<DomainId>::alloc()` for runtime allocation. This
    /// constructor is for deserialization and test setup — raw values that
    /// collide with allocator-produced IDs will cause silent bugs.
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the underlying raw value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl ClientId {
    /// Create a `ClientId` from a raw value.
    ///
    /// Prefer `IdAllocator::<ClientId>::alloc()` for runtime allocation. This
    /// constructor is for deserialization and test setup — raw values that
    /// collide with allocator-produced IDs will cause silent bugs.
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the underlying raw value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// Type-safe monotonic ID allocator.
///
/// Each ID domain (panes, domains, clients) gets its own allocator
/// parameterized by the ID type, preventing cross-domain allocation mistakes.
///
/// IDs start at 1; 0 is reserved as "no ID" for sentinel use.
#[derive(Debug)]
pub struct IdAllocator<T: MuxId> {
    counter: u64,
    _phantom: PhantomData<T>,
}

impl<T: MuxId> IdAllocator<T> {
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

impl<T: MuxId> Default for IdAllocator<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
