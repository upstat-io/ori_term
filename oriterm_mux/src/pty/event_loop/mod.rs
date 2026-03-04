//! PTY reader thread — reads shell output, parses VTE, updates terminal state.
//!
//! The [`PtyEventLoop`] runs on a dedicated thread. It reads from the PTY,
//! feeds bytes through the VTE processor into `Term<T>`.
//!
//! **Read-ahead pattern** (matches Alacritty): the reader always drains PTY
//! data, even when the terminal lock is held by the renderer. Data is
//! buffered (up to [`READ_BUFFER_SIZE`]) and parsed in one shot when the
//! lock becomes available. This prevents `ConPTY` back-pressure on Windows
//! from cascading into a hang during flood output.
//!
//! PTY *writes* (keyboard input, DA responses) happen on a separate writer
//! thread spawned by [`spawn_pty_writer`](super::spawn_pty_writer), avoiding
//! a deadlock between blocking `read()` and pending writes.

use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

use oriterm_core::{EventListener, FairMutex, Term};

/// Maximum bytes parsed under one lease acquisition.
///
/// After this many bytes, the reader releases the fairness gate (drops
/// the lease) to give the render thread a turn. Alacritty uses 65 KB.
const MAX_LOCKED_PARSE: usize = 0x1_0000; // 64 KB

/// PTY read buffer size.
///
/// Large buffer (1 MB, matching Alacritty) so the reader can accumulate
/// data while the terminal lock is held by the renderer. During flood
/// output the reader drains the PTY into this buffer instead of blocking
/// on the lock, preventing `ConPTY` back-pressure on Windows.
const READ_BUFFER_SIZE: usize = 0x10_0000; // 1 MB

/// PTY reader — reads shell output, parses VTE, updates terminal state.
///
/// Runs on a dedicated thread spawned by [`spawn`](PtyEventLoop::spawn).
/// The main loop:
/// 1. Reads from the PTY (blocking) into a large buffer.
/// 2. Takes a fairness-gate lease (preventing the renderer from starting).
/// 3. Acquires the data lock via `try_lock` (non-blocking).
/// 4. If unavailable: releases the lease, reads more PTY data.
/// 5. If available: parses all buffered data, releases lock and lease.
/// 6. Sends `Wakeup` to trigger a renderer frame.
///
/// PTY writes are handled by a separate writer thread (see
/// [`spawn_pty_writer`](super::spawn_pty_writer)), avoiding deadlocks
/// between blocking `read()` and pending writes (e.g. DA1 responses).
pub struct PtyEventLoop<T: EventListener> {
    /// Shared terminal state (also accessed by the render thread).
    terminal: Arc<FairMutex<Term<T>>>,
    /// PTY output reader (child → parent).
    reader: Box<dyn Read + Send>,
    /// Set by the writer thread on `Msg::Shutdown`.
    shutdown: Arc<AtomicBool>,
    /// High-level VTE parser (routes to `Handler` trait methods).
    processor: vte::ansi::Processor,
    /// Raw VTE parser for shell integration sequences (OSC 7, 133, etc.)
    /// that the high-level processor drops.
    raw_parser: vte::Parser,
}

impl<T: EventListener> PtyEventLoop<T> {
    /// Create a new event loop with PTY reader and terminal state.
    pub fn new(
        terminal: Arc<FairMutex<Term<T>>>,
        reader: Box<dyn Read + Send>,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        Self {
            terminal,
            reader,
            shutdown,
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
    ///
    /// Uses a read-ahead pattern: the reader drains PTY data into a 1 MB
    /// buffer. Parsing is bounded to [`MAX_LOCKED_PARSE`] bytes per lock
    /// acquisition so the renderer/snapshot path gets frequent turns under
    /// sustained output.
    fn run(mut self) {
        let mut buf = vec![0u8; READ_BUFFER_SIZE];
        let mut unprocessed = 0;

        loop {
            if self.shutdown.load(Ordering::Acquire) {
                break;
            }

            // First, try to drain already-buffered bytes before reading more.
            if unprocessed > 0 {
                let parsed = self.try_parse(&buf[..unprocessed]);
                if parsed > 0 {
                    buf.copy_within(parsed..unprocessed, 0);
                    unprocessed -= parsed;
                    // Yield between parse cycles so the snapshot builder
                    // gets a turn at the terminal lock during floods.
                    thread::yield_now();
                    continue;
                }
            }

            // Read from PTY into remaining buffer space.
            let n = match self.reader.read(&mut buf[unprocessed..]) {
                Ok(0) => {
                    // EOF — parse any remaining data.
                    if unprocessed > 0 {
                        let terminal = Arc::clone(&self.terminal);
                        let mut term = terminal.lock_unfair();
                        self.parse_chunk(&mut *term, &buf[..unprocessed]);
                        term.event_listener()
                            .send_event(oriterm_core::Event::Wakeup);
                    }
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
            unprocessed += n;

            log::trace!(
                "PTY read {n} bytes (buffered {unprocessed}): {:?}",
                String::from_utf8_lossy(&buf[unprocessed - n..unprocessed.min(200)])
            );

            // Try to parse buffered data (bounded by MAX_LOCKED_PARSE).
            let parsed = self.try_parse(&buf[..unprocessed]);
            if parsed > 0 {
                buf.copy_within(parsed..unprocessed, 0);
                unprocessed -= parsed;
            }
            // If parsed == 0, the lock was unavailable and the buffer had
            // room. Continue reading to keep the PTY drained.
        }
    }

    /// Attempt to acquire the terminal lock and parse one bounded chunk.
    ///
    /// Takes a fairness-gate lease (blocking the renderer from starting a
    /// new fair lock), then tries to get the data lock without blocking.
    /// If the data lock is unavailable AND the buffer isn't full, returns
    /// `0` so the caller can read more PTY data. If the buffer is
    /// full, blocks on the data lock (must parse to free buffer space).
    ///
    /// Returns number of bytes parsed from the front of `data`.
    fn try_parse(&mut self, data: &[u8]) -> usize {
        if data.is_empty() {
            return 0;
        }

        // Clone the Arc so lease/lock borrows don't conflict with &mut self
        // (parse_chunk mutates the VTE processors).
        let terminal = Arc::clone(&self.terminal);

        // Reserve the fairness gate for this parse cycle. While held,
        // the renderer's lock() blocks on the gate — the reader controls
        // when the renderer gets a turn.
        let lease = terminal.lease();

        // Try to get the data lock without blocking (bypasses fairness
        // gate, which we hold via lease).
        let mut term = match terminal.try_lock() {
            Some(t) => t,
            None => {
                if data.len() >= READ_BUFFER_SIZE {
                    // Buffer full — must parse. Block on data lock.
                    terminal.lock_unfair()
                } else {
                    // Lock unavailable, buffer has space — yield to let
                    // the snapshot builder acquire the lock, then return
                    // to read more PTY data.
                    drop(lease);
                    thread::yield_now();
                    return 0;
                }
            }
        };

        // Parse a bounded chunk, then release lock/lease so snapshot and
        // render paths are not starved under sustained floods.
        let parse_len = data.len().min(MAX_LOCKED_PARSE);
        let parse_start = std::time::Instant::now();
        self.parse_chunk(&mut *term, &data[..parse_len]);
        let parse_elapsed = parse_start.elapsed();
        if parse_elapsed.as_millis() > 5 {
            log::warn!(
                "[DIAG] PTY parse_chunk: {:?} for {} bytes",
                parse_elapsed,
                parse_len,
            );
        }

        // Notify the renderer.
        let sync_bytes = self.processor.sync_bytes_count();
        if sync_bytes > 0 {
            log::warn!("sync buffer: {sync_bytes} bytes pending");
        }
        if sync_bytes < parse_len {
            term.event_listener()
                .send_event(oriterm_core::Event::Wakeup);
        }

        // Release data lock, then lease. Rust drops locals in reverse
        // declaration order: `term` (data) first, then `lease` (gate).
        // When the lease drops, the renderer's blocked lock() unblocks.
        drop(term);

        parse_len
    }

    /// Parse a single chunk of PTY output through both VTE parsers.
    fn parse_chunk(&mut self, term: &mut Term<T>, chunk: &[u8]) {
        use crate::shell_integration::interceptor::RawInterceptor;

        let evicted_before = term.grid().total_evicted();

        // 1. Raw interceptor catches OSC 7, 133, 9/99/777, XTVERSION
        //    before the high-level processor discards them.
        {
            let mut interceptor = RawInterceptor::new(term);
            self.raw_parser.advance(&mut interceptor, chunk);
        }

        // 2. High-level processor handles all standard VTE sequences.
        self.processor.advance(term, chunk);

        // 3. Deferred prompt marking.
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
    }
}

#[cfg(test)]
mod tests;
