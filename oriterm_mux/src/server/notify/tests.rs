//! Tests for notification → IPC PDU routing.

use std::collections::HashMap;
use std::sync::Arc;

use oriterm_core::ClipboardType;

use crate::mux_event::MuxNotification;
use crate::pane::Pane;
use crate::{MuxPdu, PaneId};

use super::{TargetClients, notification_to_pdu};

fn empty_panes() -> HashMap<PaneId, Pane> {
    HashMap::new()
}

// -- Notifications that produce Some --

/// `PaneOutput` → `NotifyPaneOutput` routed to pane subscribers.
#[test]
fn pane_output_to_notify() {
    let pid = PaneId::from_raw(1);
    let notif = MuxNotification::PaneOutput(pid);
    let (target, pdu) = notification_to_pdu(&notif, &empty_panes()).unwrap();

    assert!(matches!(target, TargetClients::PaneSubscribers(id) if id == pid));
    assert_eq!(pdu, MuxPdu::NotifyPaneOutput { pane_id: pid });
}

/// `PaneClosed` → `NotifyPaneExited` routed to pane subscribers.
#[test]
fn pane_closed_to_notify_exited() {
    let pid = PaneId::from_raw(2);
    let notif = MuxNotification::PaneClosed(pid);
    let (target, pdu) = notification_to_pdu(&notif, &empty_panes()).unwrap();

    assert!(matches!(target, TargetClients::PaneSubscribers(id) if id == pid));
    assert_eq!(pdu, MuxPdu::NotifyPaneExited { pane_id: pid });
}

/// `PaneTitleChanged` with pane not in map → empty title string.
#[test]
fn pane_title_changed_missing_pane() {
    let pid = PaneId::from_raw(3);
    let notif = MuxNotification::PaneTitleChanged(pid);
    let (target, pdu) = notification_to_pdu(&notif, &empty_panes()).unwrap();

    assert!(matches!(target, TargetClients::PaneSubscribers(id) if id == pid));
    assert_eq!(
        pdu,
        MuxPdu::NotifyPaneTitleChanged {
            pane_id: pid,
            title: String::new(),
        }
    );
}

/// `PaneBell` → `NotifyPaneBell` routed to pane subscribers.
#[test]
fn pane_bell_to_notify() {
    let pid = PaneId::from_raw(4);
    let notif = MuxNotification::PaneBell(pid);
    let (target, pdu) = notification_to_pdu(&notif, &empty_panes()).unwrap();

    assert!(matches!(target, TargetClients::PaneSubscribers(id) if id == pid));
    assert_eq!(pdu, MuxPdu::NotifyPaneBell { pane_id: pid });
}

// -- Notifications that return None --

/// `CommandComplete` is not pushed over IPC.
#[test]
fn command_complete_returns_none() {
    let notif = MuxNotification::CommandComplete {
        pane_id: PaneId::from_raw(1),
        duration: std::time::Duration::from_secs(5),
    };
    assert!(notification_to_pdu(&notif, &empty_panes()).is_none());
}

/// `ClipboardStore` is not pushed over IPC.
#[test]
fn clipboard_store_returns_none() {
    let notif = MuxNotification::ClipboardStore {
        pane_id: PaneId::from_raw(1),
        clipboard_type: ClipboardType::Clipboard,
        text: "hello".into(),
    };
    assert!(notification_to_pdu(&notif, &empty_panes()).is_none());
}

/// `ClipboardLoad` is not pushed over IPC.
#[test]
fn clipboard_load_returns_none() {
    let notif = MuxNotification::ClipboardLoad {
        pane_id: PaneId::from_raw(1),
        clipboard_type: ClipboardType::Clipboard,
        formatter: Arc::new(|s| s.to_string()),
    };
    assert!(notification_to_pdu(&notif, &empty_panes()).is_none());
}
