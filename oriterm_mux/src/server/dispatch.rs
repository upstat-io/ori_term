//! Request dispatch for the mux server.
//!
//! Matches incoming [`MuxPdu`] request variants and calls the appropriate
//! [`InProcessMux`] methods, returning response PDUs. This module also
//! provides snapshot building utilities that convert internal terminal
//! state into wire-friendly types.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use oriterm_core::Theme;

use crate::domain::SpawnConfig;
use crate::pane::Pane;
use crate::{ClientId, InProcessMux, MuxPdu, PaneId};

use super::connection::ClientConnection;
use super::snapshot;

/// Dispatch a client request PDU to the mux, returning an optional response.
///
/// Returns `None` for fire-and-forget messages (Input, Resize) and for
/// unexpected PDU variants (responses/notifications sent by a client).
#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "exhaustive match dispatch — splitting would scatter the routing table"
)]
pub fn dispatch_request(
    mux: &mut InProcessMux,
    panes: &mut HashMap<PaneId, Pane>,
    conn: &mut ClientConnection,
    pdu: MuxPdu,
    wakeup: &Arc<dyn Fn() + Send + Sync>,
    closed_panes: &mut Vec<PaneId>,
) -> Option<MuxPdu> {
    match pdu {
        MuxPdu::Hello { pid } => {
            conn.set_client_pid(pid);
            log::info!("client {} handshake (pid={pid})", conn.id());
            Some(MuxPdu::HelloAck {
                client_id: conn.id(),
            })
        }

        MuxPdu::CreateWindow => {
            let window_id = mux.create_window();
            log::debug!("created {window_id}");
            Some(MuxPdu::WindowCreated { window_id })
        }

        MuxPdu::CreateTab {
            window_id,
            shell,
            cwd,
            theme,
        } => {
            let config = SpawnConfig {
                shell,
                cwd: cwd.map(PathBuf::from),
                ..SpawnConfig::default()
            };
            let theme = parse_theme(theme.as_deref());
            match mux.create_tab(window_id, &config, theme, wakeup) {
                Ok((tab_id, pane_id, pane)) => {
                    panes.insert(pane_id, pane);
                    let domain_id = mux.default_domain();
                    log::debug!("created {tab_id} with {pane_id} in {window_id}");
                    Some(MuxPdu::TabCreated {
                        tab_id,
                        pane_id,
                        domain_id,
                    })
                }
                Err(e) => Some(MuxPdu::Error {
                    message: format!("create_tab failed: {e}"),
                }),
            }
        }

        MuxPdu::CloseTab { tab_id } => {
            let removed = mux.close_tab(tab_id);
            for &pid in &removed {
                panes.remove(&pid);
            }
            closed_panes.extend_from_slice(&removed);
            log::debug!("closed {tab_id}");
            Some(MuxPdu::TabClosed)
        }

        MuxPdu::ClosePane { pane_id } => {
            mux.close_pane(pane_id);
            panes.remove(&pane_id);
            closed_panes.push(pane_id);
            log::debug!("closed {pane_id}");
            Some(MuxPdu::PaneClosedAck)
        }

        MuxPdu::CloseWindow { window_id } => {
            let pane_ids = mux.close_window(window_id);
            for &pid in &pane_ids {
                panes.remove(&pid);
            }
            closed_panes.extend_from_slice(&pane_ids);
            log::debug!("closed {window_id}, {} panes removed", pane_ids.len());
            Some(MuxPdu::WindowClosed { pane_ids })
        }

        MuxPdu::Input { pane_id, data } => {
            if let Some(pane) = panes.get(&pane_id) {
                pane.write_input(&data);
            }
            None // Fire-and-forget.
        }

        MuxPdu::Resize {
            pane_id,
            cols,
            rows,
        } => {
            if let Some(pane) = panes.get(&pane_id) {
                pane.resize_grid(rows, cols);
                pane.resize_pty(rows, cols);
            }
            None // Fire-and-forget.
        }

        MuxPdu::ClaimWindow { window_id } => {
            conn.set_window_id(window_id);
            log::info!("client {} claimed {window_id}", conn.id());
            Some(MuxPdu::WindowClaimed)
        }

        MuxPdu::Ping => Some(MuxPdu::PingAck),

        MuxPdu::MoveTabToWindow {
            tab_id,
            target_window_id,
        } => {
            let ok = mux.move_tab_to_window(tab_id, target_window_id);
            if ok {
                log::debug!("moved {tab_id} to {target_window_id}");
                Some(MuxPdu::TabMovedAck)
            } else {
                Some(MuxPdu::Error {
                    message: format!("move_tab_to_window failed: {tab_id} -> {target_window_id}"),
                })
            }
        }

        MuxPdu::Subscribe { pane_id } => {
            conn.subscribe(pane_id);
            match panes.get(&pane_id) {
                Some(pane) => {
                    let snap = snapshot::build_snapshot(pane);
                    Some(MuxPdu::Subscribed { snapshot: snap })
                }
                None => Some(MuxPdu::Error {
                    message: format!("pane not found: {pane_id}"),
                }),
            }
        }

        MuxPdu::Unsubscribe { pane_id } => {
            conn.unsubscribe(pane_id);
            Some(MuxPdu::Unsubscribed)
        }

        MuxPdu::ListWindows => {
            let windows = snapshot::build_window_list(mux.session());
            Some(MuxPdu::WindowList { windows })
        }

        MuxPdu::ListTabs { window_id } => {
            let tabs =
                snapshot::build_tab_list(mux.session(), mux.pane_registry(), panes, window_id);
            Some(MuxPdu::TabList { tabs })
        }

        MuxPdu::GetPaneSnapshot { pane_id } => match panes.get(&pane_id) {
            Some(pane) => {
                let snap = snapshot::build_snapshot(pane);
                Some(MuxPdu::PaneSnapshotResp { snapshot: snap })
            }
            None => Some(MuxPdu::Error {
                message: format!("pane not found: {pane_id}"),
            }),
        },

        MuxPdu::SplitPane {
            tab_id,
            pane_id,
            direction,
            shell,
            cwd,
            theme,
        } => {
            let config = SpawnConfig {
                shell,
                cwd: cwd.map(PathBuf::from),
                ..SpawnConfig::default()
            };
            let theme = parse_theme(theme.as_deref());
            match mux.split_pane(tab_id, pane_id, direction, &config, theme, wakeup) {
                Ok((new_pane_id, pane)) => {
                    panes.insert(new_pane_id, pane);
                    let domain_id = mux.default_domain();
                    log::debug!("split {pane_id} -> {new_pane_id}");
                    Some(MuxPdu::PaneSplit {
                        new_pane_id,
                        domain_id,
                    })
                }
                Err(e) => Some(MuxPdu::Error {
                    message: format!("split_pane failed: {e}"),
                }),
            }
        }

        MuxPdu::CycleTab { window_id, delta } => {
            match mux.cycle_active_tab(window_id, delta as isize) {
                Some(tab_id) => Some(MuxPdu::ActiveTabChanged { tab_id }),
                None => Some(MuxPdu::Error {
                    message: format!("cycle_tab failed for {window_id}"),
                }),
            }
        }

        MuxPdu::SetActiveTab { window_id, tab_id } => {
            if mux.switch_active_tab(window_id, tab_id) {
                Some(MuxPdu::ActiveTabChanged { tab_id })
            } else {
                Some(MuxPdu::Error {
                    message: format!("set_active_tab failed: {window_id}/{tab_id}"),
                })
            }
        }

        // Response/notification variants from a client are protocol violations.
        _ => {
            log::warn!(
                "unexpected PDU from client {}: {:?}",
                conn.id(),
                pdu.msg_type()
            );
            Some(MuxPdu::Error {
                message: "unexpected PDU type from client".to_string(),
            })
        }
    }
}

/// Parse a wire theme string into a [`Theme`].
///
/// `None` or unrecognized strings default to [`Theme::Dark`].
pub(crate) fn parse_theme(s: Option<&str>) -> Theme {
    match s {
        Some("light") => Theme::Light,
        _ => Theme::Dark,
    }
}

/// Remove all pane subscriptions from the global subscriptions map for a
/// disconnecting client.
pub fn remove_client_subscriptions(
    subscriptions: &mut HashMap<PaneId, Vec<ClientId>>,
    client_id: ClientId,
    subscribed_panes: &std::collections::HashSet<PaneId>,
) {
    for pane_id in subscribed_panes {
        if let Some(subs) = subscriptions.get_mut(pane_id) {
            subs.retain(|&c| c != client_id);
            if subs.is_empty() {
                subscriptions.remove(pane_id);
            }
        }
    }
}
