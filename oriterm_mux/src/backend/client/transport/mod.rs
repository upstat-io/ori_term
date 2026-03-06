//! IPC transport layer: connection, reader thread, RPC roundtrip.
//!
//! [`ClientTransport`] manages the IPC connection to the mux daemon.
//! A background reader thread owns the stream and multiplexes outbound
//! requests with inbound responses and push notifications.

// Platform FFI for self-pipe wakeup (pipe2, poll, read, write, close).
#![allow(unsafe_code)]

mod reader;

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

#[cfg(unix)]
use std::os::fd::RawFd;

use oriterm_ipc::ClientStream;

use crate::id::ClientId;
use crate::mux_event::MuxNotification;
use crate::protocol::{MuxPdu, ProtocolCodec};
use crate::{PaneId, PaneSnapshot};

/// RPC timeout for blocking responses.
const RPC_TIMEOUT: Duration = Duration::from_secs(5);

/// Read timeout on the socket for interleaving reads with send-channel drains.
///
/// 1ms keeps RPC round-trips fast (sub-2ms median) at negligible CPU cost
/// for a single reader thread (~1000 polls/s when idle).
const READ_POLL_INTERVAL: Duration = Duration::from_millis(1);

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
    /// Shared snapshot slot: reader thread inserts pushed snapshots,
    /// main thread takes them at render time. Bounds memory to `O(num_panes)`.
    pushed_snapshots: Arc<Mutex<HashMap<PaneId, PaneSnapshot>>>,
    /// Coalescing flag: prevents redundant `PostMessage` wakeup syscalls
    /// during flood output. Set by the guarded wakeup closure, cleared
    /// by [`clear_wakeup_pending`](Self::clear_wakeup_pending) in `poll_events`.
    wakeup_pending: Arc<AtomicBool>,
    /// Write end of the self-pipe used to wake the reader thread instantly
    /// when an outbound request is queued (Unix only).
    #[cfg(unix)]
    wake_write: RawFd,
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

        // Advertise capabilities (fire-and-forget, no ack expected).
        let cap_seq = 2; // seq 1 was Hello; we'll start RPC seqs from 3.
        ProtocolCodec::encode_frame(
            &mut stream,
            cap_seq,
            &MuxPdu::SetCapabilities {
                flags: crate::protocol::messages::CAP_SNAPSHOT_PUSH,
            },
        )?;

        // Set up channels.
        let (send_tx, send_rx) = mpsc::channel::<SendRequest>();
        let (notif_tx, notif_rx) = mpsc::channel::<MuxNotification>();
        let alive = Arc::new(AtomicBool::new(true));
        let alive_flag = alive.clone();
        let pushed_snapshots = Arc::new(Mutex::new(HashMap::new()));
        let pushed_snapshots_reader = Arc::clone(&pushed_snapshots);

        // Wrap wakeup with coalescing flag to prevent redundant PostMessage
        // syscalls during flood output (hundreds per second → at most one).
        let wakeup_pending = Arc::new(AtomicBool::new(false));
        let guarded_wakeup = {
            let pending = wakeup_pending.clone();
            Arc::new(move || {
                if !pending.swap(true, Ordering::Release) {
                    (wakeup)();
                }
            }) as Arc<dyn Fn() + Send + Sync>
        };

        // Create self-pipe for waking the reader thread on outbound requests.
        #[cfg(unix)]
        let (wake_read, wake_write) = {
            let mut fds = [0i32; 2];
            let ret = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_NONBLOCK | libc::O_CLOEXEC) };
            if ret != 0 {
                return Err(io::Error::last_os_error());
            }
            #[expect(
                clippy::tuple_array_conversions,
                reason = "pipe2 returns [i32; 2], need (RawFd, RawFd)"
            )]
            (fds[0], fds[1])
        };

        // Spawn reader thread.
        let handle = std::thread::Builder::new()
            .name("mux-client-reader".into())
            .spawn(move || {
                reader::reader_loop(
                    stream,
                    send_rx,
                    notif_tx,
                    guarded_wakeup,
                    alive_flag,
                    pushed_snapshots_reader,
                    #[cfg(unix)]
                    wake_read,
                );
                #[cfg(unix)]
                unsafe {
                    libc::close(wake_read);
                }
            })
            .map_err(|e| io::Error::other(format!("failed to spawn reader thread: {e}")))?;

        Ok(Self {
            send_tx: Some(send_tx),
            notif_rx,
            next_seq: 3, // seq 1 = Hello, seq 2 = SetCapabilities
            reader_handle: Some(handle),
            client_id,
            alive,
            pushed_snapshots,
            wakeup_pending,
            #[cfg(unix)]
            wake_write,
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
        let (reply_tx, reply_rx) = mpsc::channel();

        let tx = self
            .send_tx
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "transport shut down"))?;
        tx.send(SendRequest {
            seq,
            pdu,
            reply_tx: Some(reply_tx),
        })
        .map_err(|_send_err| io::Error::new(io::ErrorKind::BrokenPipe, "reader thread gone"))?;
        self.signal_wake();

        match reply_rx.recv_timeout(RPC_TIMEOUT) {
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
        }
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
            self.signal_wake();
        }
    }

    /// Drain pending push notifications into `out`.
    pub(super) fn poll_notifications(&self, out: &mut Vec<MuxNotification>) {
        while let Ok(n) = self.notif_rx.try_recv() {
            out.push(n);
        }
    }

    /// Take a pushed snapshot for a specific pane, if one exists.
    ///
    /// Called at render time (not poll time) so that bare-dirty
    /// invalidations arriving between poll and render are respected.
    pub(super) fn take_pushed_snapshot(&self, pane_id: PaneId) -> Option<PaneSnapshot> {
        self.pushed_snapshots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&pane_id)
    }

    /// Remove any pushed snapshot for a pane (fire-and-forget invalidation).
    ///
    /// Called by fire-and-forget mutation methods so that a stale push
    /// from before the mutation is not used on the next render.
    pub(super) fn invalidate_pushed_snapshot(&self, pane_id: PaneId) {
        self.pushed_snapshots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&pane_id);
    }

    /// Whether the reader thread is still running.
    pub(super) fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    /// Clear the wakeup-pending flag so the next notification posts a wakeup.
    ///
    /// Called at the start of `poll_events` on the main thread.
    pub(super) fn clear_wakeup_pending(&self) {
        self.wakeup_pending.store(false, Ordering::Release);
    }

    /// Write a byte to the self-pipe to wake the reader thread immediately.
    #[cfg(unix)]
    fn signal_wake(&self) {
        unsafe {
            libc::write(self.wake_write, [1u8].as_ptr().cast(), 1);
        }
    }

    /// No-op on non-Unix platforms (reader uses timeout-based polling).
    #[cfg(not(unix))]
    fn signal_wake(&self) {
        let _ = self;
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
        // Signal the wake pipe to unblock the reader thread from poll(2).
        self.signal_wake();
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
        #[cfg(unix)]
        unsafe {
            libc::close(self.wake_write);
        }
    }
}
