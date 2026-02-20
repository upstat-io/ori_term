//! Unique overlay identifier for the overlay/modal system.
//!
//! Separate ID space from [`WidgetId`](crate::widget_id::WidgetId) — overlays
//! contain widgets but are not widgets themselves.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for an overlay instance.
///
/// Generated via `OverlayId::next()` using a global atomic counter.
/// Distinct from `WidgetId` — overlays are not part of the widget tree.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlayId(u64);

/// Global counter for generating unique overlay IDs.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

impl OverlayId {
    /// Creates a new unique overlay ID.
    pub fn next() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Returns the raw numeric value.
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for OverlayId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OverlayId({})", self.0)
    }
}

impl fmt::Display for OverlayId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
