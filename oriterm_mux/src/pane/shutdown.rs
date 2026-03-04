//! Pane shutdown and cleanup.
//!
//! Implements `Drop` for [`Pane`] following the same pattern as
//! `tab::Tab::drop`: signal shutdown, kill the child process to unblock
//! pending PTY reads, then join the reader thread with a timeout.

use std::time::{Duration, Instant};

use crate::PaneId;

use super::Pane;

/// Maximum time to wait for the reader thread during drop.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// Poll interval while waiting for the reader thread to finish.
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

impl Drop for Pane {
    fn drop(&mut self) {
        // 1. Signal the writer thread to stop (sets the shutdown flag for the
        //    reader thread too).
        self.notifier.shutdown();

        // 2. Kill the child process to unblock any pending PTY read.
        let _ = self.pty.kill();

        // 3. Join both threads with a shared timeout.
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        Self::join_thread(&mut self.writer_thread, "writer", self.id, deadline);
        Self::join_thread(&mut self.reader_thread, "reader", self.id, deadline);

        // 4. Reap the child process.
        let _ = self.pty.wait();
    }
}

impl Pane {
    /// Join a thread handle with a deadline, logging warnings on timeout or panic.
    fn join_thread(
        slot: &mut Option<std::thread::JoinHandle<()>>,
        name: &str,
        pane_id: PaneId,
        deadline: Instant,
    ) {
        let Some(handle) = slot.take() else { return };
        while !handle.is_finished() {
            if Instant::now() >= deadline {
                log::warn!("pane {pane_id}: {name} thread did not exit in time");
                return;
            }
            std::thread::sleep(SHUTDOWN_POLL_INTERVAL);
        }
        if let Err(_payload) = handle.join() {
            log::warn!("pane {pane_id}: {name} thread panicked");
        }
    }
}
