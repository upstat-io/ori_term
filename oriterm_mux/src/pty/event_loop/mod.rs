//! PTY reader thread — reads shell output, parses VTE, updates terminal state.
//!
//! The [`PtyEventLoop`] runs on a dedicated thread. It reads from the PTY,
//! feeds bytes through the VTE processor into `Term<T>`, and drains the
//! command channel between reads.
//!
//! VTE responses (DA, CPR, DECRPM) flow back through
//! [`Event::PtyWrite`](oriterm_core::Event::PtyWrite) → `EventListener` →
//! winit event loop → `Notifier` → `Msg::Input` → this thread's
//! `process_commands` → PTY writer.

use std::io::{self, Read};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use oriterm_core::{EventListener, FairMutex, FairMutexGuard, Term};

use super::Msg;

/// Maximum bytes parsed under one lock acquisition.
///
/// Prevents holding the `FairMutex` for too long during large output
/// bursts (e.g. `cat` of a big file). After this many bytes, the lock
/// is released and re-acquired to give the render thread a chance.
const MAX_LOCKED_PARSE: usize = 0x1_0000; // 64 KB

/// PTY read buffer size.
const READ_BUFFER_SIZE: usize = 0x1_0000; // 64 KB

/// Minimum read size to trigger a coalesce delay.
///
/// Reads below this threshold are interactive (keystrokes, short prompts)
/// and skip the delay to preserve low latency. Reads above indicate bulk
/// output where the renderer needs breathing room.
const COALESCE_THRESHOLD: usize = 4096;

/// How long the reader pauses after processing a large read.
///
/// During flood output the reader's blocking `read()` returns instantly,
/// so it re-acquires the terminal lock before the renderer's `Wakeup`
/// event propagates through the winit event loop. Neither `unlock_fair`
/// nor `yield_now` reliably fix this on Windows — `unlock_fair` only
/// hands off to threads already parked on the mutex, and `yield_now`
/// only yields to same-priority threads. A deliberate sleep gives the
/// renderer time to receive the wakeup, lock the terminal, extract
/// cells, and release. `WezTerm` uses the same pattern (3 ms coalesce
/// timer); we use 1 ms for lower latency. On Windows `Sleep(1)` yields
/// at least one scheduler quantum (~1-15 ms), which is sufficient.
const COALESCE_DELAY: Duration = Duration::from_millis(1);

/// Coordinates PTY I/O, VTE parsing, and command processing.
///
/// Runs on a dedicated thread spawned by [`spawn`](PtyEventLoop::spawn).
/// The main loop:
/// 1. Drains the command channel (non-blocking).
/// 2. Reads from the PTY (blocking).
/// 3. Locks the terminal and parses through VTE in bounded chunks.
pub struct PtyEventLoop<T: EventListener> {
    /// Shared terminal state (also accessed by the render thread).
    terminal: Arc<FairMutex<Term<T>>>,
    /// PTY output reader (child → parent).
    reader: Box<dyn Read + Send>,
    /// Command receiver (shutdown from main thread).
    rx: mpsc::Receiver<Msg>,
    /// High-level VTE parser (routes to `Handler` trait methods).
    processor: vte::ansi::Processor,
    /// Raw VTE parser for shell integration sequences (OSC 7, 133, etc.)
    /// that the high-level processor drops.
    raw_parser: vte::Parser,
}

impl<T: EventListener> PtyEventLoop<T> {
    /// Create a new event loop with all PTY and terminal handles.
    pub fn new(
        terminal: Arc<FairMutex<Term<T>>>,
        reader: Box<dyn Read + Send>,
        rx: mpsc::Receiver<Msg>,
    ) -> Self {
        Self {
            terminal,
            reader,
            rx,
            processor: vte::ansi::Processor::new(),
            raw_parser: vte::Parser::new(),
        }
    }

    /// Spawn the event loop thread. Returns a join handle.
    pub fn spawn(self) -> io::Result<JoinHandle<()>> {
        thread::Builder::new()
            .name("pty-event-loop".into())
            .spawn(move || self.run())
    }

    /// Main event loop — runs until PTY closes or shutdown is received.
    fn run(mut self) {
        let mut buf = vec![0u8; READ_BUFFER_SIZE];

        loop {
            // 1. Drain pending commands (non-blocking).
            if !self.process_commands() {
                break;
            }

            // 2. Read from PTY (blocking).
            let n = match self.reader.read(&mut buf) {
                Ok(0) => {
                    log::info!("PTY EOF");
                    break;
                }
                Ok(n) => n,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    log::info!("PTY read error, closing reader: {e}");
                    break;
                }
            };

            log::trace!(
                "PTY read {n} bytes: {:?}",
                String::from_utf8_lossy(&buf[..n.min(200)])
            );

            // 3. Lock terminal and parse in bounded chunks.
            self.parse_pty_output(&buf[..n]);

            // 4. Adaptive coalesce: yield only when the renderer contended.
            //
            // During flood output read() returns instantly, so the reader
            // re-locks before the renderer's Wakeup event propagates
            // through winit. `take_contended()` checks whether the
            // renderer blocked on `lock()` since the last check. If so,
            // a brief sleep gives it time to receive the wakeup, lock
            // the terminal, extract cells, and release. When there's no
            // contention the reader runs at full speed.
            if n >= COALESCE_THRESHOLD && self.terminal.take_contended() {
                thread::sleep(COALESCE_DELAY);
            }
        }
    }

    /// Parse PTY output through VTE, updating terminal state.
    ///
    /// Acquires the `FairMutex` via fair `lock()` and parses in chunks of
    /// [`MAX_LOCKED_PARSE`]. After each chunk, `unlock_fair()` hands the
    /// fairness gate directly to the render thread (if waiting), guaranteeing
    /// it gets the next turn. Without fair unlock, `parking_lot`'s barging
    /// lets the reader re-acquire before the parked renderer wakes up.
    fn parse_pty_output(&mut self, data: &[u8]) {
        use crate::shell_integration::interceptor::RawInterceptor;

        let mut offset = 0;

        while offset < data.len() {
            let chunk_end = (offset + MAX_LOCKED_PARSE).min(data.len());
            let chunk = &data[offset..chunk_end];

            let mut term = self.terminal.lock();

            let evicted_before = term.grid().total_evicted();

            // 1. Raw interceptor catches OSC 7, 133, 9/99/777, XTVERSION
            //    before the high-level processor discards them.
            {
                let mut interceptor = RawInterceptor::new(&mut *term);
                self.raw_parser.advance(&mut interceptor, chunk);
            }

            // 2. High-level processor handles all standard VTE sequences.
            self.processor.advance(&mut *term, chunk);

            // 3. Deferred prompt marking: if the raw interceptor set
            //    pending flags, the cursor is now at the correct position
            //    after the high-level processor updated it.
            if term.prompt_mark_pending() {
                term.mark_prompt_row();
            }
            if term.command_start_mark_pending() {
                term.mark_command_start_row();
            }
            if term.output_start_mark_pending() {
                term.mark_output_start_row();
            }

            // 4. Prune prompt markers invalidated by scrollback eviction.
            let newly_evicted = term.grid().total_evicted() - evicted_before;
            if newly_evicted > 0 {
                term.prune_prompt_markers(newly_evicted);
            }

            let sync_bytes = self.processor.sync_bytes_count();
            if sync_bytes > 0 {
                log::warn!("sync buffer: {sync_bytes} bytes pending");
            }
            // Notify the main thread after each chunk so the renderer can
            // pick up partial updates during large output bursts.
            term.event_listener()
                .send_event(oriterm_core::Event::Wakeup);
            // Fair-unlock hands the fairness gate to the next waiter,
            // preventing the reader from barging back in.
            FairMutexGuard::unlock_fair(term);

            offset = chunk_end;
        }
    }

    /// Check the command channel for a shutdown signal (non-blocking).
    ///
    /// Returns `false` if a `Shutdown` message was received.
    fn process_commands(&self) -> bool {
        !matches!(self.rx.try_recv(), Ok(Msg::Shutdown))
    }
}

#[cfg(test)]
mod tests;
