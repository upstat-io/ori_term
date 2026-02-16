//! Tests for PtyEventLoop.
//!
//! Uses anonymous pipes to test the event loop without real PTY processes,
//! avoiding platform-specific ConPTY issues with blocking reads.

use std::io::{Read, Write};
use std::sync::mpsc;
use std::sync::Arc;

use oriterm_core::{FairMutex, Term, VoidListener};

use super::{PtyEventLoop, MAX_LOCKED_PARSE, READ_BUFFER_SIZE};
use crate::pty::{Msg, PtyControl};

// ---------------------------------------------------------------------------
// Mock PTY control (resize only, no real PTY)
// ---------------------------------------------------------------------------

/// Minimal mock implementing `portable_pty::MasterPty` for tests.
struct MockControl;

impl portable_pty::MasterPty for MockControl {
    fn resize(&self, _size: portable_pty::PtySize) -> Result<(), anyhow::Error> {
        Ok(())
    }

    fn get_size(&self) -> Result<portable_pty::PtySize, anyhow::Error> {
        Ok(portable_pty::PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
    }

    fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>, anyhow::Error> {
        unimplemented!("not needed for tests")
    }

    fn take_writer(&self) -> Result<Box<dyn Write + Send>, anyhow::Error> {
        unimplemented!("not needed for tests")
    }

    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<i32> {
        None
    }

    #[cfg(unix)]
    fn as_raw_fd(&self) -> Option<i32> {
        None
    }

    #[cfg(unix)]
    fn tty_name(&self) -> Option<std::path::PathBuf> {
        None
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a PtyEventLoop with a mock control handle and the given reader/writer.
fn build_event_loop(
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
) -> (
    PtyEventLoop<VoidListener>,
    Arc<FairMutex<Term<VoidListener>>>,
    mpsc::Sender<Msg>,
) {
    let terminal = Arc::new(FairMutex::new(Term::new(24, 80, 1000, VoidListener)));
    let (tx, rx) = mpsc::channel();

    let event_loop = PtyEventLoop::new(
        Arc::clone(&terminal),
        reader,
        writer,
        rx,
        PtyControl::from_raw(Box::new(MockControl)),
    );

    (event_loop, terminal, tx)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn shutdown_on_reader_eof() {
    // Anonymous pipe where we control the write end — dropping it produces EOF.
    let (pipe_reader, pipe_writer) = std::io::pipe().expect("pipe");

    let (event_loop, _terminal, _tx) = build_event_loop(
        Box::new(pipe_reader),
        Box::new(Vec::<u8>::new()),
    );

    let join = event_loop.spawn().expect("spawn event loop");

    // Drop the write end → reader gets EOF → thread exits.
    drop(pipe_writer);

    join.join().expect("reader thread should exit on EOF");
}

#[test]
fn processes_pty_output_into_terminal() {
    let (pipe_reader, mut pipe_writer) = std::io::pipe().expect("pipe");

    let (event_loop, terminal, _tx) = build_event_loop(
        Box::new(pipe_reader),
        Box::new(Vec::<u8>::new()),
    );

    let join = event_loop.spawn().expect("spawn event loop");

    // Simulate shell output: write raw text to the reader pipe.
    pipe_writer.write_all(b"hello world").expect("write");

    // Close the pipe to trigger EOF so the thread exits.
    drop(pipe_writer);

    join.join().expect("reader thread should exit on EOF");

    // Verify terminal received the output.
    let term = terminal.lock();
    let grid = term.grid();
    let first_row = &grid[oriterm_core::Line(0)];
    let text: String = (0..80)
        .map(|col| first_row[oriterm_core::Column(col)].ch)
        .collect();
    assert!(
        text.contains("hello world"),
        "terminal grid should contain 'hello world', got: {text:?}",
    );
}

#[test]
fn processes_channel_input() {
    let (pipe_reader, mut pipe_writer) = std::io::pipe().expect("pipe");
    let (capture_reader, capture_writer) = std::io::pipe().expect("capture pipe");

    let (event_loop, _terminal, tx) = build_event_loop(
        Box::new(pipe_reader),
        Box::new(capture_writer),
    );

    let join = event_loop.spawn().expect("spawn event loop");

    // Write something to the PTY reader so the event loop wakes up and
    // drains the command channel on the next iteration.
    pipe_writer.write_all(b"prompt> ").expect("write");

    // Send input via channel.
    tx.send(Msg::Input(b"typed text".to_vec())).expect("send");

    // Give the event loop time to process, then close pipe for EOF.
    std::thread::sleep(std::time::Duration::from_millis(100));
    drop(pipe_writer);

    join.join().expect("reader thread should exit on EOF");

    // Read what was written to the capture writer.
    drop(tx);
    let mut captured = Vec::new();
    let mut reader = capture_reader;
    reader.read_to_end(&mut captured).expect("read captured");
    assert_eq!(captured, b"typed text", "input should be forwarded to writer");
}

#[test]
fn read_buffer_size_is_64kb() {
    assert_eq!(READ_BUFFER_SIZE, 65536);
}

#[test]
fn max_locked_parse_is_64kb() {
    assert_eq!(MAX_LOCKED_PARSE, 65536);
}
