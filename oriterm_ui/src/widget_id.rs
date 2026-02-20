//! Unique widget identifier for event routing and hit testing.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for a widget instance.
///
/// Generated via `WidgetId::next()` using a global atomic counter.
/// Two IDs are equal only if they refer to the same widget.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId(u64);

/// Global counter for generating unique widget IDs.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

impl WidgetId {
    /// Creates a new unique widget ID.
    pub fn next() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Returns the raw numeric value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WidgetId({})", self.0)
    }
}

impl fmt::Display for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
