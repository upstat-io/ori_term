//! Application-level event types.
//!
//! [`TermEvent`] is the winit user-event type that flows from background
//! threads (PTY reader, config watcher) into the main event loop. Defined
//! here rather than in `tab` so that non-tab modules (like `config::monitor`)
//! can reference it without creating backwards dependencies.

use oriterm_core::Event;

use crate::tab::TabId;

/// Events sent from background threads to the winit event loop.
///
/// The PTY reader thread and child-watcher thread produce these.
/// The event loop dispatches them to the appropriate tab handler.
#[derive(Debug)]
pub(crate) enum TermEvent {
    /// A terminal state-machine event from the PTY reader thread.
    ///
    /// Wraps `oriterm_core::Event` with the originating tab's identity
    /// so the event loop knows which tab to update.
    Terminal {
        /// Which tab produced this event.
        #[allow(dead_code, reason = "tab routing in Section 15")]
        tab_id: TabId,
        /// The terminal event (wakeup, title change, bell, etc.).
        event: Event,
    },
    /// The config file watcher detected a change.
    ConfigReload,
}
