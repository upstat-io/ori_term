//! Application-level event types.
//!
//! [`TermEvent`] is the winit user-event type that flows from background
//! threads (PTY reader, config watcher, mux event proxy) into the main
//! event loop. Defined here rather than in `tab` so that non-tab modules
//! (like `config::monitor`) can reference it without backwards dependencies.

/// Events sent from background threads to the winit event loop.
///
/// The mux event proxy and config watcher produce these. The event loop
/// dispatches them in `user_event()`.
#[derive(Debug)]
pub(crate) enum TermEvent {
    /// The config file watcher detected a change.
    ConfigReload,
    /// The mux layer has events to process.
    ///
    /// Sent by [`MuxEventProxy`](oriterm_mux::mux_event::MuxEventProxy) to wake
    /// the winit event loop when pane events arrive over the mpsc channel.
    MuxWakeup,
}
