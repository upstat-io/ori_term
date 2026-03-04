//! Extract phase: convert pane snapshots into owned frame data.
//!
//! Both embedded and daemon modes render from [`PaneSnapshot`](oriterm_mux::PaneSnapshot).
//! The snapshot contains pre-resolved RGB cells, so conversion to
//! [`FrameInput`] is a straightforward type mapping — no terminal lock needed.

mod from_snapshot;

pub(crate) use self::from_snapshot::{
    extract_frame_from_snapshot, extract_frame_from_snapshot_into,
};
