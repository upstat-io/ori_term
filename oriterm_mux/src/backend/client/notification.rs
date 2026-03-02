//! Convert daemon push PDUs to [`MuxNotification`]s.
//!
//! The reader thread calls [`pdu_to_notification`] for every PDU with
//! `seq == 0` (push notification). Unrecognized PDUs are logged and dropped.

use crate::mux_event::MuxNotification;
use crate::protocol::MuxPdu;

/// Convert a daemon push PDU into a [`MuxNotification`].
///
/// Returns `None` for PDUs that have no direct notification equivalent
/// (logged at debug level).
pub(super) fn pdu_to_notification(pdu: MuxPdu) -> Option<MuxNotification> {
    match pdu {
        MuxPdu::NotifyPaneOutput { pane_id } => Some(MuxNotification::PaneDirty(pane_id)),
        MuxPdu::NotifyPaneExited { pane_id } => Some(MuxNotification::PaneClosed(pane_id)),
        MuxPdu::NotifyPaneTitleChanged { pane_id, .. } => {
            Some(MuxNotification::PaneTitleChanged(pane_id))
        }
        MuxPdu::NotifyPaneBell { pane_id } => Some(MuxNotification::Alert(pane_id)),
        MuxPdu::NotifyWindowTabsChanged { window_id } => {
            Some(MuxNotification::WindowTabsChanged(window_id))
        }
        MuxPdu::NotifyTabMoved {
            tab_id,
            from_window,
            to_window,
        } => {
            log::debug!(
                "tab {tab_id} moved from {from_window} to {to_window} \
                 (no direct MuxNotification equivalent)"
            );
            None
        }
        other => {
            log::debug!("unexpected notification PDU: {other:?}");
            None
        }
    }
}
