//! IPC transport layer: connection, reader thread, RPC roundtrip.
//!
//! [`ClientTransport`] manages the IPC connection to the mux daemon.
//! A background reader thread owns the stream and multiplexes outbound
//! requests with inbound responses and push notifications.

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use oriterm_ipc::ClientStream;

use crate::id::ClientId;
use crate::mux_event::MuxNotification;
use crate::protocol::{DecodedFrame, MuxPdu, ProtocolCodec};

use super::notification::pdu_to_notification;

/// RPC timeout for blocking responses.
const RPC_TIMEOUT: Duration = Duration::from_secs(5);

/// Read timeout on the socket for interleaving reads with send-channel drains.
const READ_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Interval between health-check pings.
///
/// If the previous ping is still outstanding when the next interval fires,
/// the connection is declared dead (implicit timeout = `PING_INTERVAL`).
const PING_INTERVAL: Duration = Duration::from_secs(5);

/// A request queued for the reader thread to send.
struct SendRequest {
    /// Sequence number assigned by the transport.
    seq: u32,
    /// PDU to encode and write.
    pdu: MuxPdu,
    /// Reply channel. `None` for fire-and-forget messages.
    reply_tx: Option<mpsc::Sender<MuxPdu>>,
}

/// IPC transport to the mux daemon.
///
/// Manages a background reader thread that owns the stream. The main thread
/// sends requests via an mpsc channel and blocks on per-request oneshot
/// replies. Push notifications are buffered in a separate channel.
pub(super) struct ClientTransport {
    /// Channel to queue outbound requests for the reader thread.
    /// Wrapped in `Option` so `Drop` can close the channel before joining
    /// the reader thread (Rust drops fields *after* `Drop::drop` runs).
    send_tx: Option<mpsc::Sender<SendRequest>>,
    /// Channel to receive push notifications from the reader thread.
    notif_rx: mpsc::Receiver<MuxNotification>,
    /// Monotonic sequence counter for request/response correlation.
    next_seq: u32,
    /// Reader thread handle (joined on drop).
    reader_handle: Option<JoinHandle<()>>,
    /// Client ID assigned by the daemon during handshake.
    client_id: ClientId,
    /// Set to `false` when the reader thread exits.
    alive: Arc<AtomicBool>,
}

impl ClientTransport {
    /// Connect to the daemon at `path` and perform the Hello handshake.
    ///
    /// `wakeup` is called when push notifications arrive, so the event loop
    /// can wake and process them.
    pub(super) fn connect(path: &Path, wakeup: Arc<dyn Fn() + Send + Sync>) -> io::Result<Self> {
        let mut stream = ClientStream::connect(path)?;

        // Send Hello handshake.
        let pid = std::process::id();
        ProtocolCodec::encode_frame(&mut stream, 1, &MuxPdu::Hello { pid })?;

        // Read HelloAck (blocking, no timeout — daemon should respond quickly).
        let frame = ProtocolCodec::new()
            .decode_frame(&mut stream)
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    format!("handshake failed: {e}"),
                )
            })?;

        let client_id = match frame.pdu {
            MuxPdu::HelloAck { client_id } => client_id,
            MuxPdu::Error { message } => {
                return Err(io::Error::other(format!(
                    "daemon rejected handshake: {message}"
                )));
            }
            other => {
                return Err(io::Error::other(format!(
                    "unexpected handshake response: {other:?}"
                )));
            }
        };

        log::info!(
            "connected to daemon at {}, assigned {client_id}",
            path.display()
        );

        // Set up channels.
        let (send_tx, send_rx) = mpsc::channel::<SendRequest>();
        let (notif_tx, notif_rx) = mpsc::channel::<MuxNotification>();
        let alive = Arc::new(AtomicBool::new(true));
        let alive_flag = alive.clone();

        // Spawn reader thread.
        let handle = std::thread::Builder::new()
            .name("mux-client-reader".into())
            .spawn(move || {
                reader_loop(stream, send_rx, notif_tx, wakeup, alive_flag);
            })
            .map_err(|e| io::Error::other(format!("failed to spawn reader thread: {e}")))?;

        Ok(Self {
            send_tx: Some(send_tx),
            notif_rx,
            next_seq: 2, // seq 1 was used for Hello
            reader_handle: Some(handle),
            client_id,
            alive,
        })
    }

    /// The client ID assigned by the daemon.
    pub(super) fn client_id(&self) -> ClientId {
        self.client_id
    }

    /// Send a request and block until the response arrives.
    ///
    /// Returns `Err` on timeout, transport death, or daemon error response.
    pub(super) fn rpc(&mut self, pdu: MuxPdu) -> io::Result<MuxPdu> {
        if !self.is_alive() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "daemon connection lost",
            ));
        }

        let seq = self.alloc_seq();
        let pdu_type = pdu.msg_type();
        let (reply_tx, reply_rx) = mpsc::channel();

        let tx = self
            .send_tx
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "transport shut down"))?;
        let send_start = Instant::now();
        tx.send(SendRequest {
            seq,
            pdu,
            reply_tx: Some(reply_tx),
        })
        .map_err(|_send_err| io::Error::new(io::ErrorKind::BrokenPipe, "reader thread gone"))?;
        let send_elapsed = send_start.elapsed();

        let wait_start = Instant::now();
        let result = match reply_rx.recv_timeout(RPC_TIMEOUT) {
            Ok(MuxPdu::Error { message }) => Err(io::Error::other(message)),
            Ok(response) => Ok(response),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "RPC timed out after 5s",
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "reply channel disconnected",
            )),
        };
        let wait_elapsed = wait_start.elapsed();
        if wait_elapsed.as_millis() > 5 {
            log::warn!(
                "[DIAG] transport.rpc seq={seq} type={pdu_type:?}: \
                 send={send_elapsed:?} wait={wait_elapsed:?} ok={}",
                result.is_ok()
            );
        }
        result
    }

    /// Send a message without waiting for a response.
    ///
    /// Used for `Input` and `Resize` PDUs once snapshot-based rendering
    /// is wired (the app will send input/resize through the client transport
    /// rather than through the local `Pane`).
    pub(super) fn fire_and_forget(&mut self, pdu: MuxPdu) {
        if !self.is_alive() {
            return;
        }
        let seq = self.alloc_seq();
        if let Some(ref tx) = self.send_tx {
            let _ = tx.send(SendRequest {
                seq,
                pdu,
                reply_tx: None,
            });
        }
    }

    /// Drain pending push notifications into `out`.
    pub(super) fn poll_notifications(&self, out: &mut Vec<MuxNotification>) {
        while let Ok(n) = self.notif_rx.try_recv() {
            out.push(n);
        }
    }

    /// Whether the reader thread is still running.
    pub(super) fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    /// Allocate the next sequence number.
    fn alloc_seq(&mut self) -> u32 {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        if self.next_seq == 0 {
            // Skip 0 — reserved for notifications.
            self.next_seq = 1;
        }
        seq
    }
}

#[cfg(test)]
impl ClientTransport {
    /// Set the next sequence number for testing wraparound behavior.
    pub(super) fn test_set_next_seq(&mut self, val: u32) {
        self.next_seq = val;
    }
}

impl Drop for ClientTransport {
    fn drop(&mut self) {
        // Close the send channel first so the reader thread sees
        // `Disconnected` on `try_recv` and exits. This must happen before
        // joining, because Rust drops fields *after* `Drop::drop` returns.
        self.send_tx.take();
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Background reader thread event loop.
///
/// Owns the IPC stream. Drains outbound requests from the send channel,
/// writes them to the stream, reads responses and notifications, and
/// dispatches them to the correct reply channel or notification channel.
#[allow(
    clippy::needless_pass_by_value,
    reason = "ownership required — values are moved into the spawned thread"
)]
fn reader_loop(
    mut stream: ClientStream,
    send_rx: mpsc::Receiver<SendRequest>,
    notif_tx: mpsc::Sender<MuxNotification>,
    wakeup: Arc<dyn Fn() + Send + Sync>,
    alive: Arc<AtomicBool>,
) {
    // Set read timeout so we can interleave reads with send-channel drains.
    if let Err(e) = stream.set_read_timeout(Some(READ_POLL_INTERVAL)) {
        log::error!("mux-client-reader: failed to set read timeout: {e}");
        alive.store(false, Ordering::Release);
        return;
    }

    let mut pending: HashMap<u32, mpsc::Sender<MuxPdu>> = HashMap::new();
    let mut codec = ProtocolCodec::new();

    // Health-check ping state.
    let mut last_ping_sent = Instant::now();
    let mut outstanding_ping_seq: Option<u32> = None;
    // Ping seqs count down from u32::MAX to avoid colliding with RPC seqs.
    let mut ping_seq_counter = u32::MAX;

    loop {
        // 1. Drain outbound requests.
        loop {
            match send_rx.try_recv() {
                Ok(req) => {
                    if let Some(reply_tx) = req.reply_tx {
                        pending.insert(req.seq, reply_tx);
                    }
                    if let Err(e) = ProtocolCodec::encode_frame(&mut stream, req.seq, &req.pdu) {
                        log::error!("mux-client-reader: write error: {e}");
                        alive.store(false, Ordering::Release);
                        return;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // Transport dropped — shut down.
                    alive.store(false, Ordering::Release);
                    return;
                }
            }
        }

        // 2. Health-check ping.
        if last_ping_sent.elapsed() >= PING_INTERVAL {
            if let Some(_seq) = outstanding_ping_seq {
                // Previous ping unanswered — daemon is unresponsive.
                log::warn!("mux-client-reader: ping timeout, marking connection dead");
                alive.store(false, Ordering::Release);
                return;
            }
            let seq = ping_seq_counter;
            ping_seq_counter = ping_seq_counter.wrapping_sub(1);
            if let Err(e) = ProtocolCodec::encode_frame(&mut stream, seq, &MuxPdu::Ping) {
                log::error!("mux-client-reader: ping write error: {e}");
                alive.store(false, Ordering::Release);
                return;
            }
            outstanding_ping_seq = Some(seq);
            last_ping_sent = Instant::now();
        }

        // 3. Attempt to read a frame (may timeout after READ_POLL_INTERVAL).
        match codec.decode_frame(&mut stream) {
            Ok(DecodedFrame { seq, pdu }) => {
                // Check if this is a PingAck for our health check.
                if outstanding_ping_seq == Some(seq) && pdu == MuxPdu::PingAck {
                    outstanding_ping_seq = None;
                    continue;
                }

                if seq == 0 || pdu.is_notification() {
                    // Push notification from daemon.
                    if let Some(notif) = pdu_to_notification(pdu) {
                        let _ = notif_tx.send(notif);
                        (wakeup)();
                    }
                } else if let Some(reply_tx) = pending.remove(&seq) {
                    // Response to a pending RPC.
                    log::trace!(
                        "[DIAG] reader_loop: forwarding response seq={seq}, pending_left={}",
                        pending.len()
                    );
                    let _ = reply_tx.send(pdu);
                } else {
                    log::warn!(
                        "mux-client-reader: no pending request for seq={seq}, dropping response"
                    );
                }
            }
            Err(crate::protocol::DecodeError::Io(ref e))
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                // Normal timeout — loop back to drain send channel.
            }
            Err(crate::protocol::DecodeError::Io(ref e))
                if e.kind() == io::ErrorKind::UnexpectedEof =>
            {
                log::info!("mux-client-reader: daemon disconnected (EOF)");
                alive.store(false, Ordering::Release);
                return;
            }
            Err(e) => {
                log::error!("mux-client-reader: decode error: {e}");
                alive.store(false, Ordering::Release);
                return;
            }
        }
    }
}
