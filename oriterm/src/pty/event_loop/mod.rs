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

use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use oriterm_core::{EventListener, FairMutex, Term};

use super::Msg;
use super::PtyControl;

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
    /// PTY input writer (parent → child).
    writer: Box<dyn Write + Send>,
    /// Command receiver (input, resize, shutdown from main thread).
    rx: mpsc::Receiver<Msg>,
    /// PTY control handle for resize operations.
    pty_control: PtyControl,
    /// VTE parser state machine.
    processor: vte::ansi::Processor,
}

impl<T: EventListener> PtyEventLoop<T> {
    /// Create a new event loop with all PTY and terminal handles.
    pub fn new(
        terminal: Arc<FairMutex<Term<T>>>,
        reader: Box<dyn Read + Send>,
        writer: Box<dyn Write + Send>,
        rx: mpsc::Receiver<Msg>,
        pty_control: PtyControl,
    ) -> Self {
        Self {
            terminal,
            reader,
            writer,
            rx,
            pty_control,
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
                Ok(0) => break,
                Ok(n) => n,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    log::debug!("PTY read error, closing reader: {e}");
                    break;
                }
            };

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
            drop(term);

            offset = chunk_end;
        }
    }

    /// Drain the command channel (non-blocking).
    ///
    /// Returns `false` if a `Shutdown` message was received.
    fn process_commands(&mut self) -> bool {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Input(bytes) => {
                    if let Err(e) = self.writer.write_all(&bytes) {
                        log::warn!("PTY write failed: {e}");
                    }
                    if let Err(e) = self.writer.flush() {
                        log::debug!("PTY flush failed: {e}");
                    }
                }
                Msg::Resize { rows, cols } => {
                    self.resize_pty(rows, cols);
                }
                Msg::Shutdown => return false,
            }
        }
        true
    }

    /// Resize the PTY dimensions.
    ///
    /// Terminal grid resize (reflow) is handled in Section 12.
    fn resize_pty(&self, rows: u16, cols: u16) {
        if let Err(e) = self.pty_control.resize(rows, cols) {
            log::warn!("PTY resize failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests;
