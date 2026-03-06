//! Request dispatch for the mux server.
//!
//! Matches incoming [`MuxPdu`] request variants and calls the appropriate
//! [`InProcessMux`] methods, returning response PDUs.

mod helpers;
mod types;

pub(in crate::server) use helpers::parse_theme;
pub(in crate::server) use helpers::remove_client_subscriptions;
pub(super) use types::{DispatchContext, DispatchResult};

use std::path::PathBuf;

use oriterm_core::selection::{extract_html_with_text, extract_text};
use oriterm_core::{CursorShape, Rgb};

use crate::MuxPdu;
use crate::domain::SpawnConfig;

use super::connection::ClientConnection;
use super::snapshot;

use self::helpers::drop_pane_background;

/// Dispatch a client request PDU to the mux, returning a [`DispatchResult`].
///
/// The result contains the response PDU and side-effect flags that the
/// caller uses for subscription sync and pending-push cleanup.
#[allow(
    clippy::too_many_lines,
    reason = "exhaustive match dispatch — splitting would scatter the routing table"
)]
pub fn dispatch_request(
    ctx: &mut DispatchContext<'_>,
    conn: &mut ClientConnection,
    pdu: MuxPdu,
) -> DispatchResult {
    // Extract side-effect signals before consuming the PDU in the match.
    let sub_changed = matches!(&pdu, MuxPdu::Subscribe { .. } | MuxPdu::Unsubscribe { .. });
    let unsub_pane = match &pdu {
        MuxPdu::Unsubscribe { pane_id } => Some(*pane_id),
        _ => None,
    };

    let response = match pdu {
        MuxPdu::Hello { pid } => {
            log::info!("client {} handshake (pid={pid})", conn.id());
            Some(MuxPdu::HelloAck {
                client_id: conn.id(),
            })
        }

        MuxPdu::SpawnPane { shell, cwd, theme } => {
            let config = SpawnConfig {
                shell,
                cwd: cwd.map(PathBuf::from),
                ..SpawnConfig::default()
            };
            let theme = parse_theme(theme.as_deref());
            match ctx.mux.spawn_standalone_pane(&config, theme, ctx.wakeup) {
                Ok((pane_id, pane)) => {
                    ctx.panes.insert(pane_id, pane);
                    let domain_id = ctx.mux.default_domain();
                    log::debug!("spawned {pane_id}");
                    Some(MuxPdu::SpawnPaneResponse { pane_id, domain_id })
                }
                Err(e) => Some(MuxPdu::Error {
                    message: format!("spawn_pane failed: {e}"),
                }),
            }
        }

        MuxPdu::ListPanes => {
            let pane_ids: Vec<_> = ctx.panes.keys().copied().collect();
            Some(MuxPdu::ListPanesResponse { pane_ids })
        }

        MuxPdu::ClosePane { pane_id } => {
            ctx.mux.close_pane(pane_id);
            drop_pane_background(ctx.panes.remove(&pane_id));
            ctx.snapshot_cache.remove(pane_id);
            ctx.closed_panes.push(pane_id);
            log::debug!("closed {pane_id}");
            Some(MuxPdu::PaneClosedAck)
        }

        MuxPdu::Input { pane_id, data } => {
            if let Some(pane) = ctx.panes.get(&pane_id) {
                pane.write_input(&data);
            }
            None // Fire-and-forget.
        }

        MuxPdu::Resize {
            pane_id,
            cols,
            rows,
        } => {
            if let Some(pane) = ctx.panes.get(&pane_id) {
                pane.resize_grid(rows, cols);
                pane.resize_pty(rows, cols);
                ctx.immediate_push.push(pane_id);
            }
            None // Fire-and-forget.
        }

        MuxPdu::ScrollDisplay { pane_id, delta } => {
            if let Some(pane) = ctx.panes.get(&pane_id) {
                pane.scroll_display(delta as isize);
                ctx.immediate_push.push(pane_id);
            }
            None // Fire-and-forget.
        }

        MuxPdu::ScrollToBottom { pane_id } => {
            if let Some(pane) = ctx.panes.get(&pane_id) {
                pane.scroll_to_bottom();
                ctx.immediate_push.push(pane_id);
            }
            None // Fire-and-forget.
        }

        MuxPdu::ScrollToPrompt { pane_id, direction } => {
            let scrolled = ctx.panes.get(&pane_id).is_some_and(|pane| {
                if direction < 0 {
                    pane.scroll_to_previous_prompt()
                } else {
                    pane.scroll_to_next_prompt()
                }
            });
            Some(MuxPdu::ScrollToPromptAck { scrolled })
        }

        MuxPdu::SetTheme {
            pane_id,
            theme,
            palette_rgb,
        } => {
            if let Some(pane) = ctx.panes.get(&pane_id) {
                let theme = parse_theme(Some(&theme));
                let mut term = pane.terminal().lock();
                term.set_theme(theme);
                let palette = term.palette_mut();
                for (i, rgb) in palette_rgb.iter().enumerate().take(270) {
                    palette.set_indexed(
                        i,
                        Rgb {
                            r: rgb[0],
                            g: rgb[1],
                            b: rgb[2],
                        },
                    );
                }
                term.grid_mut().dirty_mut().mark_all();
                drop(term);
                ctx.immediate_push.push(pane_id);
            }
            None // Fire-and-forget.
        }

        MuxPdu::SetCursorShape { pane_id, shape } => {
            if let Some(pane) = ctx.panes.get(&pane_id) {
                let core_shape = match shape {
                    1 => CursorShape::Underline,
                    2 => CursorShape::Bar,
                    3 => CursorShape::HollowBlock,
                    4 => CursorShape::Hidden,
                    _ => CursorShape::Block,
                };
                pane.terminal().lock().set_cursor_shape(core_shape);
                ctx.immediate_push.push(pane_id);
            }
            None // Fire-and-forget.
        }

        MuxPdu::MarkAllDirty { pane_id } => {
            if let Some(pane) = ctx.panes.get(&pane_id) {
                pane.terminal().lock().grid_mut().dirty_mut().mark_all();
                ctx.immediate_push.push(pane_id);
            }
            None // Fire-and-forget.
        }

        MuxPdu::OpenSearch { pane_id } => {
            if let Some(pane) = ctx.panes.get_mut(&pane_id) {
                pane.open_search();
                ctx.immediate_push.push(pane_id);
            }
            None // Fire-and-forget.
        }

        MuxPdu::CloseSearch { pane_id } => {
            if let Some(pane) = ctx.panes.get_mut(&pane_id) {
                pane.close_search();
                ctx.immediate_push.push(pane_id);
            }
            None // Fire-and-forget.
        }

        MuxPdu::SearchSetQuery { pane_id, query } => {
            if let Some(pane) = ctx.panes.get_mut(&pane_id) {
                let grid_ref = pane.terminal().clone();
                if let Some(search) = pane.search_mut() {
                    let term = grid_ref.lock();
                    search.set_query(query, term.grid());
                }
                ctx.immediate_push.push(pane_id);
            }
            None // Fire-and-forget.
        }

        MuxPdu::SearchNextMatch { pane_id } => {
            if let Some(pane) = ctx.panes.get_mut(&pane_id) {
                if let Some(search) = pane.search_mut() {
                    search.next_match();
                }
                ctx.immediate_push.push(pane_id);
            }
            None // Fire-and-forget.
        }

        MuxPdu::SearchPrevMatch { pane_id } => {
            if let Some(pane) = ctx.panes.get_mut(&pane_id) {
                if let Some(search) = pane.search_mut() {
                    search.prev_match();
                }
                ctx.immediate_push.push(pane_id);
            }
            None // Fire-and-forget.
        }

        MuxPdu::SetCapabilities { flags } => {
            conn.set_capabilities(flags);
            log::info!("client {} capabilities: 0x{flags:08x}", conn.id());
            None // Fire-and-forget — no ack.
        }

        MuxPdu::Ping => Some(MuxPdu::PingAck),

        MuxPdu::Shutdown => {
            log::info!("shutdown requested by client {}", conn.id());
            Some(MuxPdu::ShutdownAck)
        }

        MuxPdu::Subscribe { pane_id } => {
            conn.subscribe(pane_id);
            match ctx.panes.get(&pane_id) {
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

        MuxPdu::GetPaneSnapshot { pane_id } => match ctx.panes.get(&pane_id) {
            Some(pane) => {
                let snap = ctx.snapshot_cache.build_and_take(pane_id, pane);
                Some(MuxPdu::PaneSnapshotResp { snapshot: snap })
            }
            None => Some(MuxPdu::Error {
                message: format!("pane not found: {pane_id}"),
            }),
        },

        MuxPdu::ExtractText { pane_id, selection } => {
            let sel = selection.to_selection();
            let text = ctx.panes.get(&pane_id).map_or_else(String::new, |pane| {
                let term = pane.terminal().lock();
                extract_text(term.grid(), &sel)
            });
            Some(MuxPdu::ExtractTextResp { text })
        }

        MuxPdu::ExtractHtml {
            pane_id,
            selection,
            font_family,
            font_size_x100,
        } => {
            let sel = selection.to_selection();
            let font_size = f32::from(font_size_x100) / 100.0;
            let (html, text) = ctx.panes.get(&pane_id).map_or_else(
                || (String::new(), String::new()),
                |pane| {
                    let term = pane.terminal().lock();
                    extract_html_with_text(
                        term.grid(),
                        &sel,
                        term.palette(),
                        &font_family,
                        font_size,
                    )
                },
            );
            Some(MuxPdu::ExtractHtmlResp { html, text })
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
    };

    DispatchResult {
        sub_changed,
        unsubscribed_pane: unsub_pane,
        response,
    }
}
