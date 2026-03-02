//! Pane shutdown and cleanup.
//!
//! Implements `Drop` for [`Pane`] following the same pattern as
//! `tab::Tab::drop`: signal shutdown, kill the child process to unblock
//! pending PTY reads, then join the reader thread with a timeout.

use std::time::{Duration, Instant};

use super::Pane;

/// Maximum time to wait for the reader thread during drop.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// Poll interval while waiting for the reader thread to finish.
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

impl Drop for Pane {
    fn drop(&mut self) {
        // 1. Signal the reader thread to stop.
        self.notifier.shutdown();

        // 2. Kill the child process to unblock any pending PTY read.
        let _ = self.pty.kill();

        // 3. Join the reader thread with a timeout.
        if let Some(handle) = self.reader_thread.take() {
            let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
            while !handle.is_finished() {
                if Instant::now() >= deadline {
                    log::warn!("pane {}: reader thread did not exit within 2s", self.id);
                    break;
                }
                std::thread::sleep(SHUTDOWN_POLL_INTERVAL);
            }
            if handle.is_finished() {
                if let Err(_payload) = handle.join() {
                    log::warn!("pane {}: reader thread panicked", self.id);
                }
            }
        }

        // 4. Reap the child process.
        let _ = self.pty.wait();
    }
}
