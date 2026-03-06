//! Background reader thread for the IPC transport.
//!
//! The reader thread owns the IPC stream. It drains outbound requests from
//! an mpsc channel, writes them to the stream, reads responses and
//! notifications, and dispatches them to the correct reply channel or
//! notification channel.

// Platform FFI for poll(2), pipe read/drain.
#![allow(unsafe_code)]

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[cfg(unix)]
use std::os::fd::{AsRawFd, RawFd};

use oriterm_ipc::ClientStream;

use crate::mux_event::MuxNotification;
use crate::protocol::{DecodedFrame, MuxPdu, ProtocolCodec};
use crate::{PaneId, PaneSnapshot};

use super::super::notification::pdu_to_notification;
use super::{PING_INTERVAL, READ_POLL_INTERVAL, SendRequest};

/// Dispatch a received notification PDU.
///
/// `NotifyPaneSnapshot` and `NotifyPaneOutput` are intercepted here
/// (stored/invalidated in the shared snapshot map). Other notifications
/// go through [`pdu_to_notification`].
fn dispatch_notification(
    pdu: MuxPdu,
    pushed_snapshots: &Mutex<HashMap<PaneId, PaneSnapshot>>,
    notif_tx: &mpsc::Sender<MuxNotification>,
    wakeup: &dyn Fn(),
) {
    match pdu {
        MuxPdu::NotifyPaneSnapshot { pane_id, snapshot } => {
            pushed_snapshots
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(pane_id, snapshot);
            let _ = notif_tx.send(MuxNotification::PaneOutput(pane_id));
            (wakeup)();
        }
        MuxPdu::NotifyPaneOutput { pane_id } => {
            pushed_snapshots
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&pane_id);
            let _ = notif_tx.send(MuxNotification::PaneOutput(pane_id));
            (wakeup)();
        }
        other => {
            if let Some(notif) = pdu_to_notification(other) {
                let _ = notif_tx.send(notif);
                (wakeup)();
            }
        }
    }
}

/// Non-blocking check for data available on the socket.
///
/// Returns `true` if the socket has data ready to read, without blocking.
/// Used after successfully decoding a frame to avoid the expensive blocking
/// `decode_frame` retry that would sleep for `READ_POLL_INTERVAL` (1ms+
/// due to kernel timer granularity) when no more data is available.
#[cfg(unix)]
fn socket_has_data(stream: &ClientStream) -> bool {
    let mut pfd = libc::pollfd {
        fd: stream.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    (unsafe { libc::poll(&raw mut pfd, 1, 0) }) > 0
}

/// Wait for readability on the socket or a wake signal from the main thread.
///
/// Uses `poll(2)` to block until either the IPC socket has data or the
/// wake pipe is signalled. Returns immediately if either fd is already ready.
/// Falls back to a short sleep on non-Unix platforms.
#[cfg(unix)]
fn wait_for_readable(stream: &ClientStream, wake_read: RawFd, timeout_ms: i32) -> bool {
    let mut fds = [
        libc::pollfd {
            fd: stream.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: wake_read,
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    let ret = unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout_ms) };
    if ret > 0 && fds[1].revents & libc::POLLIN != 0 {
        // Drain the wake pipe (non-blocking, consume all pending bytes).
        let mut buf = [0u8; 64];
        unsafe { while libc::read(wake_read, buf.as_mut_ptr().cast(), buf.len()) > 0 {} }
    }
    // Return whether the socket has data.
    ret > 0 && fds[0].revents & libc::POLLIN != 0
}

/// Background reader thread event loop.
///
/// Owns the IPC stream. Drains outbound requests from the send channel,
/// writes them to the stream, reads responses and notifications, and
/// dispatches them to the correct reply channel or notification channel.
///
/// Uses `poll(2)` via the wake pipe to instantly unblock when the main
/// thread queues an outbound request, eliminating polling latency.
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "ownership required — values are moved into the spawned thread"
)]
pub(super) fn reader_loop(
    mut stream: ClientStream,
    send_rx: mpsc::Receiver<SendRequest>,
    notif_tx: mpsc::Sender<MuxNotification>,
    wakeup: Arc<dyn Fn() + Send + Sync>,
    alive: Arc<AtomicBool>,
    pushed_snapshots: Arc<Mutex<HashMap<PaneId, PaneSnapshot>>>,
    #[cfg(unix)] wake_read: RawFd,
) {
    // Set a short read timeout as a safety net for edge cases where poll(2)
    // says readable but the full frame hasn't arrived yet.
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

        // 3. Wait for socket data or wake signal (or health-check timeout).
        //
        // First try a non-blocking poll (0ms timeout) to avoid any timer
        // granularity overhead. Only fall back to a blocking poll if there
        // are no pending outbound requests (send_rx is empty).
        #[cfg(unix)]
        let socket_ready = {
            // Fast path: non-blocking check.
            if wait_for_readable(&stream, wake_read, 0) {
                true
            } else {
                // Slow path: block up to the remaining ping interval.
                let timeout_ms = PING_INTERVAL
                    .saturating_sub(last_ping_sent.elapsed())
                    .as_millis() as i32;
                wait_for_readable(&stream, wake_read, timeout_ms.max(1))
            }
        };
        #[cfg(not(unix))]
        let socket_ready = {
            std::thread::sleep(READ_POLL_INTERVAL);
            true // Always try to read on non-Unix.
        };

        if !socket_ready {
            // Woken by pipe or timeout — loop back to drain outbound + ping.
            continue;
        }

        // 4. Read and dispatch all available frames.
        if !read_and_dispatch_frames(
            &mut stream,
            &mut codec,
            &mut pending,
            &mut outstanding_ping_seq,
            &pushed_snapshots,
            &notif_tx,
            &*wakeup,
        ) {
            alive.store(false, Ordering::Release);
            return;
        }
    }
}

/// Read and dispatch all available frames from the socket.
///
/// After each successful decode, checks socket readability via `poll(0)` to
/// avoid the expensive blocking `decode_frame` retry (1ms+ due to kernel
/// timer granularity on WSL2). Returns `false` if the connection is dead.
#[allow(
    clippy::too_many_arguments,
    reason = "reader thread state — grouping would add indirection"
)]
fn read_and_dispatch_frames(
    stream: &mut ClientStream,
    codec: &mut ProtocolCodec,
    pending: &mut HashMap<u32, mpsc::Sender<MuxPdu>>,
    outstanding_ping_seq: &mut Option<u32>,
    pushed_snapshots: &Mutex<HashMap<PaneId, PaneSnapshot>>,
    notif_tx: &mpsc::Sender<MuxNotification>,
    wakeup: &dyn Fn(),
) -> bool {
    loop {
        match codec.decode_frame(stream) {
            Ok(DecodedFrame { seq, pdu }) => {
                if *outstanding_ping_seq == Some(seq) && pdu == MuxPdu::PingAck {
                    *outstanding_ping_seq = None;
                    #[cfg(unix)]
                    if !socket_has_data(stream) {
                        break;
                    }
                    continue;
                }

                if seq == 0 || pdu.is_notification() {
                    dispatch_notification(pdu, pushed_snapshots, notif_tx, wakeup);
                } else if let Some(reply_tx) = pending.remove(&seq) {
                    let _ = reply_tx.send(pdu);
                } else {
                    log::warn!(
                        "mux-client-reader: no pending request for seq={seq}, dropping response"
                    );
                }

                // Break early if no more data — avoids blocking decode_frame retry.
                #[cfg(unix)]
                if !socket_has_data(stream) {
                    break;
                }
            }
            Err(crate::protocol::DecodeError::UnknownMsgType(t)) => {
                log::warn!("mux-client-reader: unknown msg_type 0x{t:04x}, skipping");
            }
            Err(crate::protocol::DecodeError::Io(ref e))
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                break;
            }
            Err(crate::protocol::DecodeError::Io(ref e))
                if e.kind() == io::ErrorKind::UnexpectedEof =>
            {
                log::info!("mux-client-reader: daemon disconnected (EOF)");
                return false;
            }
            Err(e) => {
                log::error!("mux-client-reader: decode error: {e}");
                return false;
            }
        }
    }
    true
}
