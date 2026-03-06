//! Types shared across the dispatch submodule.

use std::collections::HashMap;
use std::sync::Arc;

use crate::pane::Pane;
use crate::{InProcessMux, MuxPdu, PaneId};

use super::super::snapshot::SnapshotCache;

/// Side effects returned from [`super::dispatch_request`].
///
/// Moves PDU-internal routing decisions out of the caller and into the
/// dispatch function. The caller reads named fields instead of inspecting
/// the raw PDU.
pub(in crate::server) struct DispatchResult {
    /// Response PDU to send back to the client.
    pub response: Option<MuxPdu>,
    /// Whether the request changed subscription state (Subscribe/Unsubscribe).
    pub sub_changed: bool,
    /// Pane that was unsubscribed (for `pending_push` cleanup).
    pub unsubscribed_pane: Option<PaneId>,
}

/// Shared context for request dispatch.
///
/// Groups the server-owned state that `dispatch_request` needs. Avoids
/// threading 6+ scratch buffers as individual parameters.
pub(in crate::server) struct DispatchContext<'a> {
    pub mux: &'a mut InProcessMux,
    pub panes: &'a mut HashMap<PaneId, Pane>,
    pub wakeup: &'a Arc<dyn Fn() + Send + Sync>,
    pub closed_panes: &'a mut Vec<PaneId>,
    pub snapshot_cache: &'a mut SnapshotCache,
    pub immediate_push: &'a mut Vec<PaneId>,
}
