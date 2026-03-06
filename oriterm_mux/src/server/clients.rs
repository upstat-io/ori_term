//! Client connection lifecycle — accept, read, dispatch, disconnect.
//!
//! Extracted from the server event loop to keep `mod.rs` focused on
//! the mio poll loop and mux-event draining.

use std::io::{self, Read};
use std::time::Instant;

use mio::{Interest, Token};

use crate::id::ClientId;
use crate::{DecodedFrame, MuxPdu};

use super::connection::ClientConnection;
use super::frame_io::ReadStatus;
use super::{MuxServer, dispatch, push};

impl MuxServer {
    /// Accept pending connections from the IPC listener.
    pub(super) fn accept_connections(&mut self) -> io::Result<()> {
        loop {
            match self.listener.accept() {
                Ok(mut stream) => {
                    let id = self.client_alloc.alloc();
                    let token = Token(self.next_token);
                    self.next_token += 1;

                    self.poll
                        .registry()
                        .register(&mut stream, token, Interest::READABLE)?;

                    let conn = ClientConnection::new(id, stream, token);
                    self.connections.insert(id, conn);
                    self.token_to_client.insert(token, id);
                    self.had_client = true;
                    log::info!("client {id} connected");
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Handle a readable/writable event from a connected client.
    pub(super) fn handle_client_event(&mut self, token: Token) {
        let Some(&client_id) = self.token_to_client.get(&token) else {
            log::warn!("event for unknown token {}", token.0);
            return;
        };

        // Flush any pending writes first (writable event or combined event).
        {
            let Some(conn) = self.connections.get_mut(&client_id) else {
                return;
            };
            if conn.has_pending_writes() {
                log::trace!("{client_id}: flushing pending writes");
                if let Err(e) = conn.flush_writes() {
                    log::warn!("flush error for client {client_id}: {e}");
                    self.disconnect_client(client_id);
                    return;
                }
                self.update_write_interest(client_id);
            }
        }

        // Read available bytes from the stream into the frame reader.
        let read_status = {
            let Some(conn) = self.connections.get_mut(&client_id) else {
                return;
            };
            let mut tmp = [0u8; 4096];
            match conn.stream_mut().read(&mut tmp) {
                Ok(0) => ReadStatus::Closed,
                Ok(n) => {
                    log::trace!("{client_id}: read {n} bytes");
                    conn.frame_reader_mut().extend(&tmp[..n]);
                    ReadStatus::GotData
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    log::trace!("{client_id}: read WouldBlock");
                    ReadStatus::WouldBlock
                }
                Err(e) => {
                    log::warn!("read error from client {client_id}: {e}");
                    self.disconnect_client(client_id);
                    return;
                }
            }
        };

        if read_status == ReadStatus::Closed {
            log::info!("client {client_id} disconnected (EOF)");
            self.disconnect_client(client_id);
            return;
        }

        self.dispatch_frames(client_id);
    }

    /// Decode and dispatch all complete frames from a client's buffer.
    fn dispatch_frames(&mut self, client_id: ClientId) {
        loop {
            let frame = {
                let Some(conn) = self.connections.get_mut(&client_id) else {
                    return;
                };
                conn.frame_reader_mut().try_decode()
            };

            let Some(decode_result) = frame else {
                break; // No more complete frames.
            };

            match decode_result {
                Ok(decoded) => self.handle_decoded_frame(client_id, decoded),
                Err(e) => {
                    if matches!(e, crate::DecodeError::UnknownMsgType(_)) {
                        log::debug!("unknown msg_type from client {client_id}, skipping");
                    } else {
                        log::warn!("decode error from client {client_id}: {e}");
                        let err_pdu = MuxPdu::Error {
                            message: format!("decode error: {e}"),
                        };
                        if let Some(conn) = self.connections.get_mut(&client_id) {
                            let _ = conn.queue_frame(0, &err_pdu);
                            self.update_write_interest(client_id);
                        }
                    }
                    if is_fatal_decode_error(&e) {
                        self.disconnect_client(client_id);
                        return;
                    }
                }
            }
        }
    }

    /// Handle a single successfully decoded frame from a client.
    fn handle_decoded_frame(&mut self, client_id: ClientId, decoded: DecodedFrame) {
        log::trace!(
            "{client_id}: dispatch seq={} pdu={:?}",
            decoded.seq,
            decoded.pdu
        );
        let seq = decoded.seq;
        self.scratch_panes.clear();
        self.scratch_immediate_push.clear();
        let Some(conn) = self.connections.get_mut(&client_id) else {
            return;
        };
        let mut ctx = dispatch::DispatchContext {
            mux: &mut self.mux,
            panes: &mut self.panes,
            wakeup: &self.wakeup,
            closed_panes: &mut self.scratch_panes,
            snapshot_cache: &mut self.snapshot_cache,
            immediate_push: &mut self.scratch_immediate_push,
        };
        let result = dispatch::dispatch_request(&mut ctx, conn, decoded.pdu);

        // Purge stale subscriptions for closed panes.
        if !self.scratch_panes.is_empty() {
            self.purge_closed_pane_subscriptions();
        }

        // Sync subscription tracking (only on subscription changes).
        if result.sub_changed {
            self.sync_subscriptions(client_id);
        }

        // Immediate push for fire-and-forget mutations that change visible state.
        if !self.scratch_immediate_push.is_empty() {
            let now = Instant::now();
            let mut push_ctx = push::PushContext {
                last_snapshot_push: &mut self.last_snapshot_push,
                subscriptions: &self.subscriptions,
                connections: &mut self.connections,
                panes: &self.panes,
                snapshot_cache: &mut self.snapshot_cache,
                pending_push: &mut self.pending_push,
                scratch: &mut self.scratch_clients,
            };
            for &push_pane_id in &self.scratch_immediate_push {
                push::push_or_defer_pane(&mut push_ctx, now, push_pane_id);
            }
        }

        // Prune pending_push on Unsubscribe.
        if let Some(unsub_pid) = result.unsubscribed_pane {
            if let Some(deferred) = self.pending_push.get_mut(&unsub_pid) {
                deferred.remove(&client_id);
                if deferred.is_empty() {
                    self.pending_push.remove(&unsub_pid);
                }
            }
        }

        if let Some(resp_pdu) = result.response {
            let is_shutdown = matches!(resp_pdu, MuxPdu::ShutdownAck);
            let Some(conn) = self.connections.get_mut(&client_id) else {
                return;
            };
            if let Err(e) = conn.queue_frame(seq, &resp_pdu) {
                log::warn!("write error to client {client_id}: {e}");
                self.disconnect_client(client_id);
                return;
            }
            self.update_write_interest(client_id);
            if is_shutdown {
                log::info!("shutdown flag set via IPC");
                self.shutdown
                    .store(true, std::sync::atomic::Ordering::Release);
            }
        }
    }

    /// Add or remove `WRITABLE` interest based on pending write buffer.
    ///
    /// Called after `queue_frame` or `flush_writes` to ensure the event loop
    /// delivers writable events only when needed.
    pub(super) fn update_write_interest(&mut self, client_id: ClientId) {
        let Some(conn) = self.connections.get_mut(&client_id) else {
            return;
        };
        let interest = if conn.has_pending_writes() {
            Interest::READABLE | Interest::WRITABLE
        } else {
            Interest::READABLE
        };
        let token = conn.token();
        let _ = self
            .poll
            .registry()
            .reregister(conn.stream_mut(), token, interest);
    }

    /// Disconnect a client, cleaning up all associated state.
    ///
    /// Closes any panes the client owned (the client process is gone, so
    /// its panes are orphaned). This allows `should_exit()` to fire
    /// when the last client disconnects.
    pub(super) fn disconnect_client(&mut self, client_id: ClientId) {
        let Some(mut conn) = self.connections.remove(&client_id) else {
            return;
        };

        // Deregister from mio.
        let _ = self.poll.registry().deregister(conn.stream_mut());

        // Remove token mapping.
        self.token_to_client.remove(&conn.token());

        // Subscription-based cleanup: for each pane the disconnecting client
        // was subscribed to, check if any other client is still subscribed.
        // If not, close the pane (it has no remaining consumers).
        let subscribed: Vec<_> = conn.subscribed_panes().iter().copied().collect();
        for pid in &subscribed {
            let other_subscribers = self
                .subscriptions
                .get(pid)
                .is_some_and(|subs| subs.iter().any(|&c| c != client_id));
            if !other_subscribers {
                self.mux.close_pane(*pid);
                if let Some(pane) = self.panes.remove(pid) {
                    std::thread::spawn(move || drop(pane));
                }
                self.cleanup_pane_state(*pid);
                log::debug!("closed orphaned {pid} (last subscriber {client_id} disconnected)");
            }
        }

        // Clean up subscription state.
        dispatch::remove_client_subscriptions(
            &mut self.subscriptions,
            client_id,
            conn.subscribed_panes(),
        );

        // Remove disconnecting client from all pending_push sets.
        self.pending_push.retain(|_pane_id, deferred| {
            deferred.remove(&client_id);
            !deferred.is_empty()
        });

        log::info!("client {client_id} fully disconnected");
    }

    /// Sync per-connection subscription state to the global subscriptions map.
    ///
    /// Called after dispatch to ensure the global map stays in sync with
    /// per-connection tracking.
    fn sync_subscriptions(&mut self, client_id: ClientId) {
        let Some(conn) = self.connections.get(&client_id) else {
            return;
        };
        for &pane_id in conn.subscribed_panes() {
            let subs = self.subscriptions.entry(pane_id).or_default();
            if !subs.contains(&client_id) {
                subs.push(client_id);
            }
        }
        // Remove entries where the client unsubscribed.
        self.scratch_panes.clear();
        self.scratch_panes
            .extend(conn.subscribed_panes().iter().copied());
        self.subscriptions.retain(|pane_id, subs| {
            if !self.scratch_panes.contains(pane_id) {
                subs.retain(|&c| c != client_id);
            }
            !subs.is_empty()
        });
    }

    /// Purge subscription entries for panes that have been removed.
    ///
    /// Reads closed pane IDs from `scratch_panes` (filled by dispatch),
    /// removes them from the global subscription map and all connections'
    /// subscribed-pane sets, then clears `scratch_panes`.
    fn purge_closed_pane_subscriptions(&mut self) {
        // Copy IDs out of scratch_panes so cleanup_pane_state can borrow self.
        let count = self.scratch_panes.len();
        for i in 0..count {
            let pane_id = self.scratch_panes[i];
            self.cleanup_pane_state(pane_id);
        }
        self.scratch_panes.clear();
    }
}

/// Whether a decode error should cause the client to be disconnected.
fn is_fatal_decode_error(err: &crate::DecodeError) -> bool {
    matches!(err, crate::DecodeError::PayloadTooLarge(_))
}
