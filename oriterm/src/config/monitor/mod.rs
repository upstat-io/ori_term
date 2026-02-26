//! Config file change monitor — watches TOML and sends reload events.

use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use winit::event_loop::EventLoopProxy;

use super::config_path;
use crate::event::TermEvent;

/// Watches the config file and themes directory for changes, sending
/// `TermEvent::ConfigReload` through the event loop proxy on modification.
///
/// Dropping the monitor signals the watcher thread to exit and joins it.
///
/// Drop order: `shutdown_tx` → `watcher` → `thread.join()`.
/// Dropping the watcher closes `notify_tx`, which causes `notify_rx.recv()`
/// in the thread to return `Err` and break the loop, allowing `join()`.
pub(crate) struct ConfigMonitor {
    /// Signals the watcher thread to exit.
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Filesystem watcher — dropped before joining thread to unblock `notify_rx.recv()`.
    watcher: Option<notify::RecommendedWatcher>,
    /// Watcher thread handle — joined on drop.
    thread: Option<JoinHandle<()>>,
}

impl ConfigMonitor {
    /// Start watching the config file and themes directory for changes.
    ///
    /// Returns `None` if the parent directory doesn't exist or the
    /// watcher cannot be created.
    pub(crate) fn new(proxy: EventLoopProxy<TermEvent>) -> Option<Self> {
        let path = config_path();
        let parent = path.parent()?.to_path_buf();

        if !parent.exists() {
            log::info!(
                "config_monitor: parent dir {} does not exist, skipping watch",
                parent.display()
            );
            return None;
        }

        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
        let (notify_tx, notify_rx) = mpsc::channel();

        let mut watcher = match notify::recommended_watcher(notify_tx) {
            Ok(w) => w,
            Err(e) => {
                log::warn!("config_monitor: failed to create watcher: {e}");
                return None;
            }
        };

        if let Err(e) = watcher.watch(&parent, RecursiveMode::NonRecursive) {
            log::warn!("config_monitor: failed to watch {}: {e}", parent.display());
            return None;
        }

        log::info!("config_monitor: watching {}", parent.display());

        // Also watch the themes subdirectory if it exists.
        let themes_dir = parent.join("themes");
        if themes_dir.is_dir() {
            if let Err(e) = watcher.watch(&themes_dir, RecursiveMode::NonRecursive) {
                log::warn!(
                    "config_monitor: failed to watch themes dir {}: {e}",
                    themes_dir.display()
                );
            } else {
                log::info!(
                    "config_monitor: watching themes dir {}",
                    themes_dir.display()
                );
            }
        }

        let config_file = path;
        let thread = std::thread::Builder::new()
            .name("config-watcher".into())
            .spawn(move || {
                Self::watch_loop(&config_file, &themes_dir, &proxy, &notify_rx, &shutdown_rx);
            })
            .ok()?;

        Some(Self {
            shutdown_tx: Some(shutdown_tx),
            watcher: Some(watcher),
            thread: Some(thread),
        })
    }

    /// Watch loop — runs on the watcher thread.
    fn watch_loop(
        config_file: &std::path::Path,
        themes_dir: &std::path::Path,
        proxy: &EventLoopProxy<TermEvent>,
        notify_rx: &mpsc::Receiver<Result<notify::Event, notify::Error>>,
        shutdown_rx: &mpsc::Receiver<()>,
    ) {
        while let Ok(event) = notify_rx.recv() {
            // Check for shutdown before processing.
            match shutdown_rx.try_recv() {
                Ok(()) | Err(mpsc::TryRecvError::Disconnected) => return,
                Err(mpsc::TryRecvError::Empty) => {}
            }

            // Filter: config file changes or `.toml` changes in themes dir.
            let is_relevant = match &event {
                Ok(ev) => ev
                    .paths
                    .iter()
                    .any(|p| p == config_file || is_theme_file(p, themes_dir)),
                Err(_) => false,
            };

            if !is_relevant {
                continue;
            }

            // Debounce: drain any further events within 200ms.
            // Editors save in multiple steps (write temp, rename) that
            // fire rapid-fire events.
            let debounce = Duration::from_millis(200);
            while notify_rx.recv_timeout(debounce).is_ok() {
                // Drain.
            }

            // Check for shutdown after debounce.
            match shutdown_rx.try_recv() {
                Ok(()) | Err(mpsc::TryRecvError::Disconnected) => return,
                Err(mpsc::TryRecvError::Empty) => {}
            }

            log::info!("config_monitor: change detected, sending reload event");
            if proxy.send_event(TermEvent::ConfigReload).is_err() {
                // Event loop closed — exit the watcher thread.
                return;
            }
        }
    }
}

/// Check if `path` is a `.toml` file inside `themes_dir`.
fn is_theme_file(path: &std::path::Path, themes_dir: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("toml") && path.parent() == Some(themes_dir)
}

impl Drop for ConfigMonitor {
    fn drop(&mut self) {
        // Signal shutdown — dropping the sender closes the channel,
        // causing try_recv to return Disconnected.
        drop(self.shutdown_tx.take());
        // Drop the watcher — closes notify_tx inside it, which makes
        // notify_rx.recv() in the thread return Err and break the loop.
        drop(self.watcher.take());
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests;
