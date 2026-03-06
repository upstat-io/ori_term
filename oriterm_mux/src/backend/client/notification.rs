//! Convert daemon push PDUs to [`MuxNotification`]s.
//!
//! The reader thread calls [`pdu_to_notification`] for PDUs that are not
//! handled directly in the reader loop. `NotifyPaneOutput` and
//! `NotifyPaneSnapshot` are intercepted in the reader loop (stored in
//! the `pushed_snapshots` shared map) and never reach this function.

use crate::mux_event::MuxNotification;
use crate::protocol::MuxPdu;

/// Convert a daemon push PDU into a [`MuxNotification`].
///
/// Returns `None` for PDUs that have no direct notification equivalent
/// (logged at debug level).
///
/// Note: `NotifyPaneOutput` and `NotifyPaneSnapshot` are handled directly
/// in the reader loop and should never reach this function.
pub(super) fn pdu_to_notification(pdu: MuxPdu) -> Option<MuxNotification> {
    match pdu {
        MuxPdu::NotifyPaneExited { pane_id } => Some(MuxNotification::PaneClosed(pane_id)),
        MuxPdu::NotifyPaneTitleChanged { pane_id, .. } => {
            Some(MuxNotification::PaneTitleChanged(pane_id))
        }
        MuxPdu::NotifyPaneBell { pane_id } => Some(MuxNotification::PaneBell(pane_id)),
        other => {
            log::debug!("unexpected notification PDU: {other:?}");
            None
        }
    }
}
