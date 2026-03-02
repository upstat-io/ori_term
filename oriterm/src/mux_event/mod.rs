//! Mux event types and the PTY-to-mux event bridge.
//!
//! [`MuxEvent`] carries pane events from PTY reader threads to the mux layer
//! via an mpsc channel. [`MuxEventProxy`] implements [`EventListener`] so it
//! can be plugged into `Term<MuxEventProxy>` as the event sink.
//!
//! [`MuxNotification`] carries mux-to-GUI notifications (pane dirty, closed,
//! layout changes). The GUI subscribes via a separate channel.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use oriterm_core::{ClipboardType, Event, EventListener};
use oriterm_mux::{PaneId, TabId, WindowId};

/// Events from pane PTY reader threads to the mux layer.
///
/// Sent over `mpsc::Sender<MuxEvent>`. The mux processes these on the main
/// thread after a winit wakeup.
pub(crate) enum MuxEvent {
    /// Pane has new terminal output — grid is dirty.
    PaneOutput(PaneId),
    /// PTY process exited.
    PaneExited {
        /// Which pane's process exited.
        pane_id: PaneId,
        /// Exit code from the child process.
        exit_code: i32,
    },
    /// Pane title changed (OSC 0/2).
    PaneTitleChanged {
        /// Which pane changed title.
        pane_id: PaneId,
        /// New title text.
        title: String,
    },
    /// Pane icon name changed (OSC 0/1).
    PaneIconChanged {
        /// Which pane changed icon name.
        pane_id: PaneId,
        /// New icon name text.
        icon_name: String,
    },
    /// Pane working directory changed (OSC 7).
    PaneCwdChanged {
        /// Which pane changed CWD.
        pane_id: PaneId,
        /// New working directory path.
        cwd: String,
    },
    /// A command completed in a pane (OSC 133;D) with the given duration.
    CommandComplete {
        /// Which pane completed a command.
        pane_id: PaneId,
        /// Time elapsed between OSC 133;C and OSC 133;D.
        duration: std::time::Duration,
    },
    /// Bell fired in a pane.
    PaneBell(PaneId),
    /// Data to write to a pane's PTY (DA responses, etc.).
    PtyWrite {
        /// Target pane.
        pane_id: PaneId,
        /// Bytes to write.
        data: String,
    },
    /// OSC 52 clipboard store request.
    ClipboardStore {
        /// Originating pane.
        pane_id: PaneId,
        /// Which clipboard to target.
        clipboard_type: ClipboardType,
        /// Text to store.
        text: String,
    },
    /// OSC 52 clipboard load request.
    ClipboardLoad {
        /// Originating pane.
        pane_id: PaneId,
        /// Which clipboard to read.
        clipboard_type: ClipboardType,
        /// Formats the clipboard text into a PTY response.
        formatter: Arc<dyn Fn(&str) -> String + Send + Sync>,
    },
}

impl fmt::Debug for MuxEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PaneOutput(id) => write!(f, "PaneOutput({id})"),
            Self::PaneExited { pane_id, exit_code } => {
                write!(f, "PaneExited({pane_id}, code={exit_code})")
            }
            Self::PaneTitleChanged { pane_id, title } => {
                write!(f, "PaneTitleChanged({pane_id}, {title:?})")
            }
            Self::PaneIconChanged { pane_id, icon_name } => {
                write!(f, "PaneIconChanged({pane_id}, {icon_name:?})")
            }
            Self::PaneCwdChanged { pane_id, cwd } => {
                write!(f, "PaneCwdChanged({pane_id}, {cwd:?})")
            }
            Self::CommandComplete { pane_id, duration } => {
                write!(f, "CommandComplete({pane_id}, {duration:?})")
            }
            Self::PaneBell(id) => write!(f, "PaneBell({id})"),
            Self::PtyWrite { pane_id, data } => {
                write!(f, "PtyWrite({pane_id}, {} bytes)", data.len())
            }
            Self::ClipboardStore {
                pane_id,
                clipboard_type,
                ..
            } => write!(f, "ClipboardStore({pane_id}, {clipboard_type:?})"),
            Self::ClipboardLoad {
                pane_id,
                clipboard_type,
                ..
            } => write!(f, "ClipboardLoad({pane_id}, {clipboard_type:?})"),
        }
    }
}

/// Bridges terminal events from the PTY reader thread to the mux layer.
///
/// Implements [`EventListener`] so it can be used as the event sink for
/// `Term<MuxEventProxy>`. On each event, maps it to a [`MuxEvent`] and
/// sends it over mpsc. Wakeup events are coalesced via an atomic flag to
/// avoid flooding the channel.
pub(crate) struct MuxEventProxy {
    /// Identity of the pane this proxy serves.
    pane_id: PaneId,
    /// Channel sender to the mux event processor.
    tx: mpsc::Sender<MuxEvent>,
    /// Coalesces wakeup events — set by reader thread, cleared by main thread.
    wakeup_pending: Arc<AtomicBool>,
    /// Set when the pane's grid has new content to render.
    grid_dirty: Arc<AtomicBool>,
    /// Wakes the event loop when events arrive.
    wakeup: Arc<dyn Fn() + Send + Sync>,
}

impl MuxEventProxy {
    /// Create a new event proxy for a pane.
    pub(crate) fn new(
        pane_id: PaneId,
        tx: mpsc::Sender<MuxEvent>,
        wakeup_pending: Arc<AtomicBool>,
        grid_dirty: Arc<AtomicBool>,
        wakeup: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        Self {
            pane_id,
            tx,
            wakeup_pending,
            grid_dirty,
            wakeup,
        }
    }

    /// Send a `MuxEvent` and wake the event loop.
    fn send(&self, event: MuxEvent) {
        let _ = self.tx.send(event);
        (self.wakeup)();
    }
}

impl EventListener for MuxEventProxy {
    fn send_event(&self, event: Event) {
        match event {
            Event::Wakeup => {
                // Always mark grid dirty, even when coalesced.
                self.grid_dirty.store(true, Ordering::Release);
                // Coalesce: only send if not already pending.
                if !self.wakeup_pending.swap(true, Ordering::AcqRel) {
                    self.send(MuxEvent::PaneOutput(self.pane_id));
                }
            }
            Event::Bell => {
                self.send(MuxEvent::PaneBell(self.pane_id));
            }
            Event::Title(title) => {
                self.send(MuxEvent::PaneTitleChanged {
                    pane_id: self.pane_id,
                    title,
                });
            }
            Event::ResetTitle => {
                self.send(MuxEvent::PaneTitleChanged {
                    pane_id: self.pane_id,
                    title: String::new(),
                });
            }
            Event::IconName(name) => {
                self.send(MuxEvent::PaneIconChanged {
                    pane_id: self.pane_id,
                    icon_name: name,
                });
            }
            Event::ResetIconName => {
                self.send(MuxEvent::PaneIconChanged {
                    pane_id: self.pane_id,
                    icon_name: String::new(),
                });
            }
            Event::ClipboardStore(clipboard_type, text) => {
                self.send(MuxEvent::ClipboardStore {
                    pane_id: self.pane_id,
                    clipboard_type,
                    text,
                });
            }
            Event::ClipboardLoad(clipboard_type, formatter) => {
                self.send(MuxEvent::ClipboardLoad {
                    pane_id: self.pane_id,
                    clipboard_type,
                    formatter,
                });
            }
            Event::PtyWrite(data) => {
                self.send(MuxEvent::PtyWrite {
                    pane_id: self.pane_id,
                    data,
                });
            }
            Event::Cwd(cwd) => {
                self.send(MuxEvent::PaneCwdChanged {
                    pane_id: self.pane_id,
                    cwd,
                });
            }
            Event::CommandComplete(duration) => {
                self.send(MuxEvent::CommandComplete {
                    pane_id: self.pane_id,
                    duration,
                });
            }
            Event::ChildExit(code) => {
                self.send(MuxEvent::PaneExited {
                    pane_id: self.pane_id,
                    exit_code: code,
                });
            }
            // Events that don't need mux routing — still wake the event loop.
            Event::ColorRequest(..) | Event::CursorBlinkingChange | Event::MouseCursorDirty => {
                (self.wakeup)();
            }
        }
    }
}

/// Notifications from the mux layer to the GUI.
///
/// These flow from the mux to the winit event loop after the mux has
/// processed incoming [`MuxEvent`]s and updated its state.
pub(crate) enum MuxNotification {
    /// A pane's title or icon name changed — re-sync tab bar.
    PaneTitleChanged(PaneId),
    /// A pane has new content to render.
    PaneDirty(PaneId),
    /// A pane was closed (PTY exited, removed from registry).
    PaneClosed(PaneId),
    /// A tab's split tree layout changed.
    TabLayoutChanged(TabId),
    /// A floating pane moved or resized (position-only, no PTY resize needed).
    FloatingPaneChanged(TabId),
    /// A window's tab list changed.
    WindowTabsChanged(WindowId),
    /// An alert fired in a pane (bell, urgent notification).
    Alert(PaneId),
    /// A long-running command completed in a pane.
    CommandComplete {
        /// Which pane completed a command.
        pane_id: PaneId,
        /// Command execution duration.
        duration: std::time::Duration,
    },
    /// A window was closed (but other windows remain).
    WindowClosed(WindowId),
    /// The last window was closed — application should exit.
    LastWindowClosed,
    /// OSC 52 clipboard store request forwarded from a pane.
    ClipboardStore {
        /// Originating pane.
        pane_id: PaneId,
        /// Which clipboard to target.
        clipboard_type: ClipboardType,
        /// Text to store.
        text: String,
    },
    /// OSC 52 clipboard load request forwarded from a pane.
    ClipboardLoad {
        /// Originating pane.
        pane_id: PaneId,
        /// Which clipboard to read.
        clipboard_type: ClipboardType,
        /// Formats the clipboard text into a PTY response.
        formatter: Arc<dyn Fn(&str) -> String + Send + Sync>,
    },
}

impl fmt::Debug for MuxNotification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PaneTitleChanged(id) => write!(f, "PaneTitleChanged({id})"),
            Self::PaneDirty(id) => write!(f, "PaneDirty({id})"),
            Self::PaneClosed(id) => write!(f, "PaneClosed({id})"),
            Self::TabLayoutChanged(id) => write!(f, "TabLayoutChanged({id})"),
            Self::FloatingPaneChanged(id) => write!(f, "FloatingPaneChanged({id})"),
            Self::WindowTabsChanged(id) => write!(f, "WindowTabsChanged({id})"),
            Self::WindowClosed(id) => write!(f, "WindowClosed({id})"),
            Self::Alert(id) => write!(f, "Alert({id})"),
            Self::CommandComplete { pane_id, duration } => {
                write!(f, "CommandComplete({pane_id}, {duration:?})")
            }
            Self::LastWindowClosed => write!(f, "LastWindowClosed"),
            Self::ClipboardStore {
                pane_id,
                clipboard_type,
                ..
            } => write!(f, "ClipboardStore({pane_id}, {clipboard_type:?})"),
            Self::ClipboardLoad {
                pane_id,
                clipboard_type,
                ..
            } => write!(f, "ClipboardLoad({pane_id}, {clipboard_type:?})"),
        }
    }
}

#[cfg(test)]
mod tests;
