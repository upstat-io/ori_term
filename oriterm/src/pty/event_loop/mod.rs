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

use oriterm_core::{EventListener, FairMutex, Term};

use super::Msg;

/// Maximum bytes parsed under one lock acquisition.
///
/// Prevents holding the `FairMutex` for too long during large output
/// bursts (e.g. `cat` of a big file). After this many bytes, the lock
/// is released and re-acquired to give the render thread a chance.
const MAX_LOCKED_PARSE: usize = 0x1_0000; // 64 KB

/// PTY read buffer size.
const READ_BUFFER_SIZE: usize = 0x1_0000; // 64 KB

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
    /// VTE parser state machine.
    processor: vte::ansi::Processor,
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
        }
    }

    /// Parse PTY output through VTE, updating terminal state.
    ///
    /// Acquires the `FairMutex` using the lease + `lock_unfair` pattern.
    /// Parses in chunks of [`MAX_LOCKED_PARSE`] to avoid starving the
    /// render thread.
    fn parse_pty_output(&mut self, data: &[u8]) {
        let mut offset = 0;

        while offset < data.len() {
            let chunk_end = (offset + MAX_LOCKED_PARSE).min(data.len());
            let chunk = &data[offset..chunk_end];

            let _lease = self.terminal.lease();
            let mut term = self.terminal.lock_unfair();
            self.processor.advance(&mut *term, chunk);
            let sync_bytes = self.processor.sync_bytes_count();
            if sync_bytes > 0 {
                log::warn!("sync buffer: {sync_bytes} bytes pending");
            }
            // Notify the main thread after each chunk so the renderer can
            // pick up partial updates during large output bursts.
            term.event_listener()
                .send_event(oriterm_core::Event::Wakeup);
            drop(term);

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
