//! Per-pane `PreparedFrame` caching for multi-pane rendering.
//!
//! Avoids re-preparing unchanged panes on every frame. Only dirty panes
//! (new PTY output or layout changes) go through the full extract→shape→fill
//! pipeline; clean panes reuse their cached GPU instances.

use std::collections::HashMap;

use oriterm_mux::id::PaneId;

use crate::session::PaneLayout;

use super::prepared_frame::PreparedFrame;

/// Cached GPU-ready instances for a single pane.
struct CachedPaneFrame {
    /// GPU instances from the last prepare pass.
    prepared: PreparedFrame,
    /// Layout at time of preparation (for invalidation on resize/move).
    layout: PaneLayout,
}

/// Per-pane render cache.
///
/// Stores one [`PreparedFrame`] per pane. On each frame, callers check
/// [`get_or_prepare`](Self::get_or_prepare) — if the pane is clean and
/// its layout unchanged, the cached frame is returned without re-preparing.
pub(crate) struct PaneRenderCache {
    entries: HashMap<PaneId, CachedPaneFrame>,
}

impl PaneRenderCache {
    /// Create an empty cache.
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Get a cached frame or prepare a new one.
    ///
    /// Returns the cached `PreparedFrame` if `dirty` is false and the
    /// `layout` matches the cached entry. Otherwise calls `prepare_fn` to
    /// produce a new frame, stores it, and returns a reference.
    ///
    /// `prepare_fn` receives a mutable reference to the cached `PreparedFrame`
    /// (already cleared) so it can fill instances in-place without allocating.
    pub(crate) fn get_or_prepare(
        &mut self,
        pane_id: PaneId,
        layout: &PaneLayout,
        dirty: bool,
        prepare_fn: impl FnOnce(&mut PreparedFrame),
    ) -> &PreparedFrame {
        let entry = self.entries.entry(pane_id);

        match entry {
            std::collections::hash_map::Entry::Occupied(mut occ) => {
                let cached = occ.get_mut();
                if !dirty && cached.layout == *layout {
                    // Cache hit — reuse existing instances.
                    return &occ.into_mut().prepared;
                }
                // Cache miss — re-prepare in place.
                cached.prepared.clear();
                prepare_fn(&mut cached.prepared);
                cached.layout = *layout;
                &occ.into_mut().prepared
            }
            std::collections::hash_map::Entry::Vacant(vac) => {
                let viewport = super::frame_input::ViewportSize::new(
                    layout.pixel_rect.width as u32,
                    layout.pixel_rect.height as u32,
                );
                let bg = oriterm_core::Rgb { r: 0, g: 0, b: 0 };
                let mut prepared = PreparedFrame::new(viewport, bg, 1.0);
                prepare_fn(&mut prepared);
                let cached = vac.insert(CachedPaneFrame {
                    prepared,
                    layout: *layout,
                });
                &cached.prepared
            }
        }
    }

    /// Check whether a valid cache entry exists for this pane at the given layout.
    pub(crate) fn is_cached(&self, pane_id: PaneId, layout: &PaneLayout) -> bool {
        self.entries
            .get(&pane_id)
            .is_some_and(|e| e.layout == *layout)
    }

    /// Read-only access to a cached pane frame.
    ///
    /// Returns `None` if no entry exists. Does not check layout staleness —
    /// call [`is_cached`](Self::is_cached) first if layout validation is needed.
    pub(crate) fn get_cached(&self, pane_id: PaneId) -> Option<&PreparedFrame> {
        self.entries.get(&pane_id).map(|e| &e.prepared)
    }

    /// Force a specific pane to re-prepare on the next frame.
    #[allow(
        dead_code,
        reason = "used for targeted invalidation (e.g. palette change per pane)"
    )]
    pub(crate) fn invalidate(&mut self, pane_id: PaneId) {
        self.entries.remove(&pane_id);
    }

    /// Remove a closed pane's cached frame, freeing memory.
    pub(crate) fn remove(&mut self, pane_id: PaneId) {
        self.entries.remove(&pane_id);
    }

    /// Invalidate all cached panes (e.g. atlas rebuild, font change).
    pub(crate) fn invalidate_all(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests;
