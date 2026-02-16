//! Background PTY reader thread.
//!
//! Reads PTY output in a dedicated thread and sends chunks to the main
//! thread via a standard library channel.

use std::io::{self, Read};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

/// Events sent from the PTY reader thread.
#[derive(Debug)]
pub enum PtyEvent {
    /// A chunk of data read from PTY output.
    Data(Vec<u8>),
    /// The PTY closed (child exited or unrecoverable read error).
    Closed,
}

/// Background thread that reads PTY output and forwards it via channel.
///
/// The thread exits when the PTY closes (EOF), the channel receiver is
/// dropped, or an unrecoverable read error occurs. In all cases a
/// [`PtyEvent::Closed`] is sent before the thread exits.
pub struct PtyReader {
    handle: Option<JoinHandle<()>>,
}

impl PtyReader {
    /// Spawn a reader thread for the given PTY output stream.
    ///
    /// Data chunks are sent through `tx`. The thread runs until the PTY
    /// closes or the receiver is dropped.
    pub fn spawn(
        mut reader: Box<dyn Read + Send>,
        tx: mpsc::Sender<PtyEvent>,
    ) -> io::Result<Self> {
        let handle = thread::Builder::new()
            .name("pty-reader".into())
            .spawn(move || {
                Self::read_loop(&mut *reader, &tx);
                let _ = tx.send(PtyEvent::Closed);
            })
            .map_err(|e| io::Error::other(e.to_string()))?;

        Ok(Self {
            handle: Some(handle),
        })
    }

    /// Block until the reader thread exits.
    pub fn join(mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }

    /// Core read loop: reads chunks and sends them through the channel.
    fn read_loop(reader: &mut dyn Read, tx: &mpsc::Sender<PtyEvent>) {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => return,
                Ok(n) => {
                    if tx.send(PtyEvent::Data(buf[..n].to_vec())).is_err() {
                        return;
                    }
                }
                // EINTR: fall through to retry.
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => return,
            }
        }
    }
}
