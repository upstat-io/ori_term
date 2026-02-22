//! Config file change monitor — watches TOML and sends reload events.

use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use winit::event_loop::EventLoopProxy;

use super::config_path;
use crate::tab::TermEvent;

/// Watches the config file's parent directory for changes and sends
/// `TermEvent::ConfigReload` through the event loop proxy when the
/// config file is modified.
pub(crate) struct ConfigMonitor {
    shutdown_tx: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl ConfigMonitor {
    /// Start watching the config file for changes.
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

        let config_file = path;
        let thread = std::thread::Builder::new()
            .name("config-watcher".into())
            .spawn(move || {
                // Keep the watcher alive for the lifetime of this thread.
                let _watcher = watcher;
                Self::watch_loop(&config_file, &proxy, &notify_rx, &shutdown_rx);
            })
            .ok()?;

        Some(Self {
            shutdown_tx,
            thread: Some(thread),
        })
    }

    /// Shut down the watcher thread.
    pub(crate) fn shutdown(mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }

    /// Watch loop — runs on the watcher thread.
    fn watch_loop(
        config_file: &std::path::Path,
        proxy: &EventLoopProxy<TermEvent>,
        notify_rx: &mpsc::Receiver<Result<notify::Event, notify::Error>>,
        shutdown_rx: &mpsc::Receiver<()>,
    ) {
        while let Ok(event) = notify_rx.recv() {
            // Check for shutdown before processing.
            if shutdown_rx.try_recv().is_ok() {
                return;
            }

            // Filter: only process events for our config file.
            let is_config_event = match &event {
                Ok(ev) => ev.paths.iter().any(|p| p == config_file),
                Err(_) => false,
            };

            if !is_config_event {
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
            if shutdown_rx.try_recv().is_ok() {
                return;
            }

            log::info!("config_monitor: config file changed, sending reload event");
            if proxy.send_event(TermEvent::ConfigReload).is_err() {
                // Event loop closed — exit the watcher thread.
                return;
            }
        }
    }
}
