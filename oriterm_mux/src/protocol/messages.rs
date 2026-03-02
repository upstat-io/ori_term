//! Protocol data unit definitions.
//!
//! [`MuxPdu`] is the unified enum covering all messages: requests from window
//! to daemon, responses from daemon to window, and push notifications from
//! daemon to window. Each variant maps to a [`MsgType`] ID.

use serde::{Deserialize, Serialize};

use oriterm_core::Theme;

use crate::id::{ClientId, DomainId, PaneId, TabId, WindowId};
use crate::layout::SplitDirection;

use super::snapshot::{MuxTabInfo, MuxWindowInfo, PaneSnapshot};

/// Message type IDs for the wire header.
///
/// Ranges: `0x01xx` = requests, `0x02xx` = responses, `0x03xx` = notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum MsgType {
    // Requests (window → daemon).
    Hello = 0x0101,
    CreateWindow = 0x0102,
    CreateTab = 0x0103,
    CloseTab = 0x0104,
    ClosePane = 0x0105,
    Input = 0x0106,
    Resize = 0x0107,
    MoveTabToWindow = 0x0108,
    Subscribe = 0x0109,
    Unsubscribe = 0x010A,
    ListWindows = 0x010B,
    ListTabs = 0x010C,
    GetPaneSnapshot = 0x010D,
    SplitPane = 0x010E,
    CycleTab = 0x010F,
    SetActiveTab = 0x0110,
    CloseWindow = 0x0111,
    ClaimWindow = 0x0112,

    // Responses (daemon → window).
    HelloAck = 0x0201,
    WindowCreated = 0x0202,
    TabCreated = 0x0203,
    TabClosed = 0x0204,
    PaneClosedAck = 0x0205,
    TabMovedAck = 0x0206,
    Subscribed = 0x0207,
    Unsubscribed = 0x0208,
    WindowList = 0x0209,
    TabList = 0x020A,
    PaneSnapshotResp = 0x020B,
    PaneSplit = 0x020C,
    ActiveTabChanged = 0x020D,
    WindowClosed = 0x020E,
    WindowClaimed = 0x020F,
    Error = 0x02FF,

    // Push notifications (daemon → window).
    NotifyPaneOutput = 0x0301,
    NotifyPaneExited = 0x0302,
    NotifyPaneTitleChanged = 0x0303,
    NotifyPaneBell = 0x0304,
    NotifyWindowTabsChanged = 0x0305,
    NotifyTabMoved = 0x0306,
}

impl MsgType {
    /// Construct from raw `u16`, returning `None` for unknown values.
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0x0101 => Some(Self::Hello),
            0x0102 => Some(Self::CreateWindow),
            0x0103 => Some(Self::CreateTab),
            0x0104 => Some(Self::CloseTab),
            0x0105 => Some(Self::ClosePane),
            0x0106 => Some(Self::Input),
            0x0107 => Some(Self::Resize),
            0x0108 => Some(Self::MoveTabToWindow),
            0x0109 => Some(Self::Subscribe),
            0x010A => Some(Self::Unsubscribe),
            0x010B => Some(Self::ListWindows),
            0x010C => Some(Self::ListTabs),
            0x010D => Some(Self::GetPaneSnapshot),
            0x010E => Some(Self::SplitPane),
            0x010F => Some(Self::CycleTab),
            0x0110 => Some(Self::SetActiveTab),
            0x0111 => Some(Self::CloseWindow),
            0x0112 => Some(Self::ClaimWindow),
            0x0201 => Some(Self::HelloAck),
            0x0202 => Some(Self::WindowCreated),
            0x0203 => Some(Self::TabCreated),
            0x0204 => Some(Self::TabClosed),
            0x0205 => Some(Self::PaneClosedAck),
            0x0206 => Some(Self::TabMovedAck),
            0x0207 => Some(Self::Subscribed),
            0x0208 => Some(Self::Unsubscribed),
            0x0209 => Some(Self::WindowList),
            0x020A => Some(Self::TabList),
            0x020B => Some(Self::PaneSnapshotResp),
            0x020C => Some(Self::PaneSplit),
            0x020D => Some(Self::ActiveTabChanged),
            0x020E => Some(Self::WindowClosed),
            0x020F => Some(Self::WindowClaimed),
            0x02FF => Some(Self::Error),
            0x0301 => Some(Self::NotifyPaneOutput),
            0x0302 => Some(Self::NotifyPaneExited),
            0x0303 => Some(Self::NotifyPaneTitleChanged),
            0x0304 => Some(Self::NotifyPaneBell),
            0x0305 => Some(Self::NotifyWindowTabsChanged),
            0x0306 => Some(Self::NotifyTabMoved),
            _ => None,
        }
    }
}

/// All protocol messages — requests, responses, and notifications.
///
/// Each variant carries its own data. The bincode encoding includes the
/// enum discriminant, so the `msg_type` in the frame header is redundant
/// for deserialization but useful for pre-routing and debugging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MuxPdu {
    // -- Requests (window → daemon) --
    /// Client handshake. Sent immediately after connecting.
    Hello {
        /// OS process ID of the connecting window.
        pid: u32,
    },

    /// Create a new mux window (no tabs yet).
    CreateWindow,

    /// Spawn a new tab (with a single pane) in a window.
    CreateTab {
        /// Target window.
        window_id: WindowId,
        /// Shell program override (uses default shell if `None`).
        shell: Option<String>,
        /// Working directory override (uses current dir if `None`).
        cwd: Option<String>,
        /// Color theme: `"dark"`, `"light"`, or `None` for server default.
        theme: Option<String>,
    },

    /// Close a tab and all its panes.
    CloseTab {
        /// Tab to close.
        tab_id: TabId,
    },

    /// Close a single pane (may close the tab if it was the last pane).
    ClosePane {
        /// Pane to close.
        pane_id: PaneId,
    },

    /// Write input data to a pane's PTY. Fire-and-forget.
    Input {
        /// Target pane.
        pane_id: PaneId,
        /// Raw bytes to write.
        data: Vec<u8>,
    },

    /// Resize a pane's terminal grid. Fire-and-forget.
    Resize {
        /// Target pane.
        pane_id: PaneId,
        /// New column count.
        cols: u16,
        /// New row count.
        rows: u16,
    },

    /// Move a tab to a different window.
    MoveTabToWindow {
        /// Tab to move.
        tab_id: TabId,
        /// Destination window.
        target_window_id: WindowId,
    },

    /// Subscribe to a pane's output. Returns current snapshot.
    Subscribe {
        /// Pane to subscribe to.
        pane_id: PaneId,
    },

    /// Unsubscribe from a pane's output.
    Unsubscribe {
        /// Pane to unsubscribe from.
        pane_id: PaneId,
    },

    /// List all mux windows.
    ListWindows,

    /// List tabs in a window.
    ListTabs {
        /// Target window.
        window_id: WindowId,
    },

    /// Get a full snapshot of a pane's state.
    GetPaneSnapshot {
        /// Target pane.
        pane_id: PaneId,
    },

    /// Split an existing pane, creating a new one adjacent to it.
    SplitPane {
        /// Tab containing the pane.
        tab_id: TabId,
        /// Pane to split.
        pane_id: PaneId,
        /// Split direction.
        direction: SplitDirection,
        /// Shell program override.
        shell: Option<String>,
        /// Working directory override.
        cwd: Option<String>,
        /// Color theme: `"dark"`, `"light"`, or `None` for server default.
        theme: Option<String>,
    },

    /// Cycle the active tab in a window.
    CycleTab {
        /// Target window.
        window_id: WindowId,
        /// Cycle direction: positive = forward, negative = backward.
        delta: i32,
    },

    /// Set a specific tab as active in a window.
    SetActiveTab {
        /// Target window.
        window_id: WindowId,
        /// Tab to activate.
        tab_id: TabId,
    },

    /// Close a window and all its tabs/panes.
    CloseWindow {
        /// Window to close.
        window_id: WindowId,
    },

    /// Tell the daemon which mux window this client renders.
    ///
    /// Sent after window ID is resolved (init or `create_window`). Enables
    /// the daemon to route `WindowTabsChanged` notifications to this client.
    ClaimWindow {
        /// Window this client is rendering.
        window_id: WindowId,
    },

    // -- Responses (daemon → window) --
    /// Handshake acknowledgment.
    HelloAck {
        /// Assigned client ID for this connection.
        client_id: ClientId,
    },

    /// New window created.
    WindowCreated {
        /// Assigned window ID.
        window_id: WindowId,
    },

    /// New tab created with its initial pane.
    TabCreated {
        /// Assigned tab ID.
        tab_id: TabId,
        /// ID of the initial pane in the tab.
        pane_id: PaneId,
        /// Domain that owns the pane.
        domain_id: DomainId,
    },

    /// Tab closed successfully.
    TabClosed,

    /// Pane closed successfully.
    PaneClosedAck,

    /// Tab moved successfully.
    TabMovedAck,

    /// Subscription established with current pane state.
    Subscribed {
        /// Current state of the subscribed pane.
        snapshot: PaneSnapshot,
    },

    /// Unsubscription confirmed.
    Unsubscribed,

    /// List of all mux windows.
    WindowList {
        /// Window summaries.
        windows: Vec<MuxWindowInfo>,
    },

    /// List of tabs in a window.
    TabList {
        /// Tab summaries.
        tabs: Vec<MuxTabInfo>,
    },

    /// Full pane state snapshot.
    PaneSnapshotResp {
        /// Pane state.
        snapshot: PaneSnapshot,
    },

    /// New pane created via split.
    PaneSplit {
        /// ID of the newly created pane.
        new_pane_id: PaneId,
        /// Domain that owns the pane.
        domain_id: DomainId,
    },

    /// Active tab changed in a window.
    ActiveTabChanged {
        /// Now-active tab.
        tab_id: TabId,
    },

    /// Window closed with all its panes.
    WindowClosed {
        /// IDs of panes that were removed.
        pane_ids: Vec<PaneId>,
    },

    /// Window claim acknowledged.
    WindowClaimed,

    /// Error response for a failed request.
    Error {
        /// Human-readable error description.
        message: String,
    },

    // -- Push notifications (daemon → window) --
    /// Pane has new output — the client should re-fetch the snapshot.
    NotifyPaneOutput {
        /// Pane with new output.
        pane_id: PaneId,
    },

    /// Pane's shell process exited.
    NotifyPaneExited {
        /// Pane that exited.
        pane_id: PaneId,
    },

    /// Pane title changed (OSC 0/2).
    NotifyPaneTitleChanged {
        /// Pane with new title.
        pane_id: PaneId,
        /// New title text.
        title: String,
    },

    /// Bell fired in a pane.
    NotifyPaneBell {
        /// Pane that belled.
        pane_id: PaneId,
    },

    /// Window's tab list changed (tab added, removed, or reordered).
    NotifyWindowTabsChanged {
        /// Affected window.
        window_id: WindowId,
    },

    /// A tab migrated between windows.
    NotifyTabMoved {
        /// The migrated tab.
        tab_id: TabId,
        /// Window the tab left.
        from_window: WindowId,
        /// Window the tab arrived in.
        to_window: WindowId,
    },
}

impl MuxPdu {
    /// Message type ID for the wire header.
    pub fn msg_type(&self) -> MsgType {
        match self {
            Self::Hello { .. } => MsgType::Hello,
            Self::CreateWindow => MsgType::CreateWindow,
            Self::CreateTab { .. } => MsgType::CreateTab,
            Self::CloseTab { .. } => MsgType::CloseTab,
            Self::ClosePane { .. } => MsgType::ClosePane,
            Self::Input { .. } => MsgType::Input,
            Self::Resize { .. } => MsgType::Resize,
            Self::MoveTabToWindow { .. } => MsgType::MoveTabToWindow,
            Self::Subscribe { .. } => MsgType::Subscribe,
            Self::Unsubscribe { .. } => MsgType::Unsubscribe,
            Self::ListWindows => MsgType::ListWindows,
            Self::ListTabs { .. } => MsgType::ListTabs,
            Self::GetPaneSnapshot { .. } => MsgType::GetPaneSnapshot,
            Self::SplitPane { .. } => MsgType::SplitPane,
            Self::CycleTab { .. } => MsgType::CycleTab,
            Self::SetActiveTab { .. } => MsgType::SetActiveTab,
            Self::CloseWindow { .. } => MsgType::CloseWindow,
            Self::ClaimWindow { .. } => MsgType::ClaimWindow,
            Self::HelloAck { .. } => MsgType::HelloAck,
            Self::WindowCreated { .. } => MsgType::WindowCreated,
            Self::TabCreated { .. } => MsgType::TabCreated,
            Self::TabClosed => MsgType::TabClosed,
            Self::PaneClosedAck => MsgType::PaneClosedAck,
            Self::TabMovedAck => MsgType::TabMovedAck,
            Self::Subscribed { .. } => MsgType::Subscribed,
            Self::Unsubscribed => MsgType::Unsubscribed,
            Self::WindowList { .. } => MsgType::WindowList,
            Self::TabList { .. } => MsgType::TabList,
            Self::PaneSnapshotResp { .. } => MsgType::PaneSnapshotResp,
            Self::PaneSplit { .. } => MsgType::PaneSplit,
            Self::ActiveTabChanged { .. } => MsgType::ActiveTabChanged,
            Self::WindowClosed { .. } => MsgType::WindowClosed,
            Self::WindowClaimed => MsgType::WindowClaimed,
            Self::Error { .. } => MsgType::Error,
            Self::NotifyPaneOutput { .. } => MsgType::NotifyPaneOutput,
            Self::NotifyPaneExited { .. } => MsgType::NotifyPaneExited,
            Self::NotifyPaneTitleChanged { .. } => MsgType::NotifyPaneTitleChanged,
            Self::NotifyPaneBell { .. } => MsgType::NotifyPaneBell,
            Self::NotifyWindowTabsChanged { .. } => MsgType::NotifyWindowTabsChanged,
            Self::NotifyTabMoved { .. } => MsgType::NotifyTabMoved,
        }
    }

    /// Whether this PDU is a fire-and-forget message (no response expected).
    pub fn is_fire_and_forget(&self) -> bool {
        matches!(self, Self::Input { .. } | Self::Resize { .. })
    }

    /// Whether this PDU is a push notification from the daemon.
    pub fn is_notification(&self) -> bool {
        matches!(
            self,
            Self::NotifyPaneOutput { .. }
                | Self::NotifyPaneExited { .. }
                | Self::NotifyPaneTitleChanged { .. }
                | Self::NotifyPaneBell { .. }
                | Self::NotifyWindowTabsChanged { .. }
                | Self::NotifyTabMoved { .. }
        )
    }
}

/// Convert a [`Theme`] to its wire representation.
///
/// Returns `Some("dark")` or `Some("light")`, or `None` for
/// [`Theme::Unknown`] (server uses its default).
pub(crate) fn theme_to_wire(theme: Theme) -> Option<String> {
    match theme {
        Theme::Dark => Some("dark".to_owned()),
        Theme::Light => Some("light".to_owned()),
        Theme::Unknown => None,
    }
}
