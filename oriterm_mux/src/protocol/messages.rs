//! Protocol data unit definitions.
//!
//! [`MuxPdu`] is the unified enum covering all messages: requests from client
//! to daemon, responses from daemon to client, and push notifications from
//! daemon to client. Each variant maps to a [`MsgType`] ID.

use serde::{Deserialize, Serialize};

use oriterm_core::Theme;

use crate::id::{ClientId, DomainId, PaneId};

use super::snapshot::{PaneSnapshot, WireSelection};

/// Client supports receiving `NotifyPaneSnapshot` pushed snapshots.
pub const CAP_SNAPSHOT_PUSH: u32 = 1;

/// Message type IDs for the wire header.
///
/// Ranges: `0x01xx` = requests, `0x02xx` = responses, `0x03xx` = notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum MsgType {
    // Requests (client → daemon).
    Hello = 0x0101,
    ClosePane = 0x0105,
    Input = 0x0106,
    Resize = 0x0107,
    Subscribe = 0x0109,
    Unsubscribe = 0x010A,
    GetPaneSnapshot = 0x010D,
    Ping = 0x0113,
    Shutdown = 0x0114,
    ScrollDisplay = 0x0115,
    ScrollToBottom = 0x0116,
    ScrollToPrompt = 0x0117,
    SetTheme = 0x0118,
    SetCursorShape = 0x0119,
    MarkAllDirty = 0x011A,
    OpenSearch = 0x011B,
    CloseSearch = 0x011C,
    SearchSetQuery = 0x011D,
    SearchNextMatch = 0x011E,
    SearchPrevMatch = 0x011F,
    ExtractText = 0x0120,
    ExtractHtml = 0x0121,
    SetCapabilities = 0x0122,
    SpawnPane = 0x0124,
    ListPanes = 0x0125,

    // Responses (daemon → client).
    HelloAck = 0x0201,
    PaneClosedAck = 0x0205,
    Subscribed = 0x0207,
    Unsubscribed = 0x0208,
    PaneSnapshotResp = 0x020B,
    PingAck = 0x0210,
    ShutdownAck = 0x0211,
    ScrollToPromptAck = 0x0212,
    ExtractTextResp = 0x0213,
    ExtractHtmlResp = 0x0214,
    SpawnPaneResponse = 0x0216,
    ListPanesResponse = 0x0217,
    Error = 0x02FF,

    // Push notifications (daemon → client).
    NotifyPaneOutput = 0x0301,
    NotifyPaneExited = 0x0302,
    NotifyPaneTitleChanged = 0x0303,
    NotifyPaneBell = 0x0304,
    NotifyPaneSnapshot = 0x0307,
}

impl MsgType {
    /// Construct from raw `u16`, returning `None` for unknown values.
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0x0101 => Some(Self::Hello),
            0x0105 => Some(Self::ClosePane),
            0x0106 => Some(Self::Input),
            0x0107 => Some(Self::Resize),
            0x0109 => Some(Self::Subscribe),
            0x010A => Some(Self::Unsubscribe),
            0x010D => Some(Self::GetPaneSnapshot),
            0x0113 => Some(Self::Ping),
            0x0114 => Some(Self::Shutdown),
            0x0115 => Some(Self::ScrollDisplay),
            0x0116 => Some(Self::ScrollToBottom),
            0x0117 => Some(Self::ScrollToPrompt),
            0x0118 => Some(Self::SetTheme),
            0x0119 => Some(Self::SetCursorShape),
            0x011A => Some(Self::MarkAllDirty),
            0x011B => Some(Self::OpenSearch),
            0x011C => Some(Self::CloseSearch),
            0x011D => Some(Self::SearchSetQuery),
            0x011E => Some(Self::SearchNextMatch),
            0x011F => Some(Self::SearchPrevMatch),
            0x0120 => Some(Self::ExtractText),
            0x0121 => Some(Self::ExtractHtml),
            0x0122 => Some(Self::SetCapabilities),
            0x0124 => Some(Self::SpawnPane),
            0x0125 => Some(Self::ListPanes),
            0x0201 => Some(Self::HelloAck),
            0x0205 => Some(Self::PaneClosedAck),
            0x0207 => Some(Self::Subscribed),
            0x0208 => Some(Self::Unsubscribed),
            0x020B => Some(Self::PaneSnapshotResp),
            0x0210 => Some(Self::PingAck),
            0x0211 => Some(Self::ShutdownAck),
            0x0212 => Some(Self::ScrollToPromptAck),
            0x0213 => Some(Self::ExtractTextResp),
            0x0214 => Some(Self::ExtractHtmlResp),
            0x0216 => Some(Self::SpawnPaneResponse),
            0x0217 => Some(Self::ListPanesResponse),
            0x02FF => Some(Self::Error),
            0x0301 => Some(Self::NotifyPaneOutput),
            0x0302 => Some(Self::NotifyPaneExited),
            0x0303 => Some(Self::NotifyPaneTitleChanged),
            0x0304 => Some(Self::NotifyPaneBell),
            0x0307 => Some(Self::NotifyPaneSnapshot),
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
    // -- Requests (client → daemon) --
    /// Client handshake. Sent immediately after connecting.
    Hello {
        /// OS process ID of the connecting client.
        pid: u32,
    },

    /// Close a single pane.
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

    /// Get a full snapshot of a pane's state.
    GetPaneSnapshot {
        /// Target pane.
        pane_id: PaneId,
    },

    /// Liveness check. The daemon replies with [`PingAck`](Self::PingAck).
    Ping,

    /// Request graceful daemon shutdown. The daemon replies with
    /// [`ShutdownAck`](Self::ShutdownAck) and then exits.
    Shutdown,

    /// Scroll a pane's viewport by `delta` lines (positive = toward history).
    /// Fire-and-forget.
    ScrollDisplay {
        /// Target pane.
        pane_id: PaneId,
        /// Lines to scroll (positive = up into scrollback, negative = down).
        delta: i32,
    },

    /// Scroll a pane to the live terminal position (bottom). Fire-and-forget.
    ScrollToBottom {
        /// Target pane.
        pane_id: PaneId,
    },

    /// Scroll to the nearest prompt in the given direction.
    ScrollToPrompt {
        /// Target pane.
        pane_id: PaneId,
        /// Direction: `-1` = previous (up), `+1` = next (down).
        direction: i8,
    },

    /// Set the theme and palette for a pane. Fire-and-forget.
    SetTheme {
        /// Target pane.
        pane_id: PaneId,
        /// Theme name: `"dark"` or `"light"`.
        theme: String,
        /// Full palette as 270 RGB triplets (same format as snapshot).
        palette_rgb: Vec<[u8; 3]>,
    },

    /// Set the cursor shape for a pane. Fire-and-forget.
    SetCursorShape {
        /// Target pane.
        pane_id: PaneId,
        /// Cursor shape discriminant (maps to `WireCursorShape`).
        shape: u8,
    },

    /// Mark all grid lines dirty in a pane (forces full re-render).
    /// Fire-and-forget.
    MarkAllDirty {
        /// Target pane.
        pane_id: PaneId,
    },

    /// Open search for a pane (initializes empty search state).
    /// Fire-and-forget.
    OpenSearch {
        /// Target pane.
        pane_id: PaneId,
    },

    /// Close search and clear search state. Fire-and-forget.
    CloseSearch {
        /// Target pane.
        pane_id: PaneId,
    },

    /// Update the search query. Recomputes matches against the full grid.
    /// Fire-and-forget.
    SearchSetQuery {
        /// Target pane.
        pane_id: PaneId,
        /// New search query text.
        query: String,
    },

    /// Navigate to the next search match. Fire-and-forget.
    SearchNextMatch {
        /// Target pane.
        pane_id: PaneId,
    },

    /// Navigate to the previous search match. Fire-and-forget.
    SearchPrevMatch {
        /// Target pane.
        pane_id: PaneId,
    },

    /// Extract plain text from a selection.
    ExtractText {
        /// Target pane.
        pane_id: PaneId,
        /// Selection to extract from.
        selection: WireSelection,
    },

    /// Extract HTML and plain text from a selection.
    ExtractHtml {
        /// Target pane.
        pane_id: PaneId,
        /// Selection to extract from.
        selection: WireSelection,
        /// Font family name for the HTML wrapper.
        font_family: String,
        /// Font size in points × 100 (integer for deterministic comparison).
        font_size_x100: u16,
    },

    /// Client advertises protocol capabilities. Fire-and-forget.
    SetCapabilities {
        /// Bitmask of capability flags (e.g. [`CAP_SNAPSHOT_PUSH`]).
        flags: u32,
    },

    /// Spawn a new pane (shell process).
    SpawnPane {
        /// Shell program override (uses default shell if `None`).
        shell: Option<String>,
        /// Working directory override (uses current dir if `None`).
        cwd: Option<String>,
        /// Color theme: `"dark"`, `"light"`, or `None` for server default.
        theme: Option<String>,
    },

    /// List all live pane IDs.
    ListPanes,

    // -- Responses (daemon → client) --
    /// Handshake acknowledgment.
    HelloAck {
        /// Assigned client ID for this connection.
        client_id: ClientId,
    },

    /// Pane closed successfully.
    PaneClosedAck,

    /// Subscription established with current pane state.
    Subscribed {
        /// Current state of the subscribed pane.
        snapshot: PaneSnapshot,
    },

    /// Unsubscription confirmed.
    Unsubscribed,

    /// Full pane state snapshot.
    PaneSnapshotResp {
        /// Pane state.
        snapshot: PaneSnapshot,
    },

    /// Reply to a [`Ping`](Self::Ping) request.
    PingAck,

    /// Acknowledgment that the daemon will shut down.
    ShutdownAck,

    /// Response to [`ScrollToPrompt`](Self::ScrollToPrompt).
    ScrollToPromptAck {
        /// Whether a prompt was found and the viewport scrolled.
        scrolled: bool,
    },

    /// Response to [`ExtractText`](Self::ExtractText).
    ExtractTextResp {
        /// Extracted plain text.
        text: String,
    },

    /// Response to [`ExtractHtml`](Self::ExtractHtml).
    ExtractHtmlResp {
        /// Extracted HTML with inline styles.
        html: String,
        /// Plain text (same as `ExtractTextResp::text`).
        text: String,
    },

    /// Response to [`SpawnPane`](Self::SpawnPane).
    SpawnPaneResponse {
        /// ID of the newly created pane.
        pane_id: PaneId,
        /// Domain that owns the pane.
        domain_id: DomainId,
    },

    /// Response to [`ListPanes`](Self::ListPanes).
    ListPanesResponse {
        /// IDs of all live panes.
        pane_ids: Vec<PaneId>,
    },

    /// Error response for a failed request.
    Error {
        /// Human-readable error description.
        message: String,
    },

    // -- Push notifications (daemon → client) --
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

    /// Server-pushed pane snapshot (proactive, throttled to ~60fps).
    ///
    /// Only sent to clients that advertised [`CAP_SNAPSHOT_PUSH`].
    NotifyPaneSnapshot {
        /// Pane this snapshot belongs to.
        pane_id: PaneId,
        /// Full pane state snapshot.
        snapshot: PaneSnapshot,
    },
    // Wire-compat: append-only — new variants must go at the end.
}

impl MuxPdu {
    /// Message type ID for the wire header.
    pub fn msg_type(&self) -> MsgType {
        match self {
            Self::Hello { .. } => MsgType::Hello,
            Self::ClosePane { .. } => MsgType::ClosePane,
            Self::Input { .. } => MsgType::Input,
            Self::Resize { .. } => MsgType::Resize,
            Self::Subscribe { .. } => MsgType::Subscribe,
            Self::Unsubscribe { .. } => MsgType::Unsubscribe,
            Self::GetPaneSnapshot { .. } => MsgType::GetPaneSnapshot,
            Self::Ping => MsgType::Ping,
            Self::Shutdown => MsgType::Shutdown,
            Self::ScrollDisplay { .. } => MsgType::ScrollDisplay,
            Self::ScrollToBottom { .. } => MsgType::ScrollToBottom,
            Self::ScrollToPrompt { .. } => MsgType::ScrollToPrompt,
            Self::SetTheme { .. } => MsgType::SetTheme,
            Self::SetCursorShape { .. } => MsgType::SetCursorShape,
            Self::MarkAllDirty { .. } => MsgType::MarkAllDirty,
            Self::OpenSearch { .. } => MsgType::OpenSearch,
            Self::CloseSearch { .. } => MsgType::CloseSearch,
            Self::SearchSetQuery { .. } => MsgType::SearchSetQuery,
            Self::SearchNextMatch { .. } => MsgType::SearchNextMatch,
            Self::SearchPrevMatch { .. } => MsgType::SearchPrevMatch,
            Self::ExtractText { .. } => MsgType::ExtractText,
            Self::ExtractHtml { .. } => MsgType::ExtractHtml,
            Self::SetCapabilities { .. } => MsgType::SetCapabilities,
            Self::SpawnPane { .. } => MsgType::SpawnPane,
            Self::ListPanes => MsgType::ListPanes,
            Self::HelloAck { .. } => MsgType::HelloAck,
            Self::PaneClosedAck => MsgType::PaneClosedAck,
            Self::Subscribed { .. } => MsgType::Subscribed,
            Self::Unsubscribed => MsgType::Unsubscribed,
            Self::PaneSnapshotResp { .. } => MsgType::PaneSnapshotResp,
            Self::PingAck => MsgType::PingAck,
            Self::ShutdownAck => MsgType::ShutdownAck,
            Self::ScrollToPromptAck { .. } => MsgType::ScrollToPromptAck,
            Self::ExtractTextResp { .. } => MsgType::ExtractTextResp,
            Self::ExtractHtmlResp { .. } => MsgType::ExtractHtmlResp,
            Self::SpawnPaneResponse { .. } => MsgType::SpawnPaneResponse,
            Self::ListPanesResponse { .. } => MsgType::ListPanesResponse,
            Self::Error { .. } => MsgType::Error,
            Self::NotifyPaneOutput { .. } => MsgType::NotifyPaneOutput,
            Self::NotifyPaneExited { .. } => MsgType::NotifyPaneExited,
            Self::NotifyPaneTitleChanged { .. } => MsgType::NotifyPaneTitleChanged,
            Self::NotifyPaneBell { .. } => MsgType::NotifyPaneBell,
            Self::NotifyPaneSnapshot { .. } => MsgType::NotifyPaneSnapshot,
        }
    }

    /// Whether this PDU is a fire-and-forget message (no response expected).
    pub fn is_fire_and_forget(&self) -> bool {
        matches!(
            self,
            Self::Input { .. }
                | Self::Resize { .. }
                | Self::ScrollDisplay { .. }
                | Self::ScrollToBottom { .. }
                | Self::SetTheme { .. }
                | Self::SetCursorShape { .. }
                | Self::MarkAllDirty { .. }
                | Self::OpenSearch { .. }
                | Self::CloseSearch { .. }
                | Self::SearchSetQuery { .. }
                | Self::SearchNextMatch { .. }
                | Self::SearchPrevMatch { .. }
                | Self::SetCapabilities { .. }
        )
    }

    /// Whether this PDU is a push notification from the daemon.
    pub fn is_notification(&self) -> bool {
        matches!(
            self,
            Self::NotifyPaneOutput { .. }
                | Self::NotifyPaneExited { .. }
                | Self::NotifyPaneTitleChanged { .. }
                | Self::NotifyPaneBell { .. }
                | Self::NotifyPaneSnapshot { .. }
        )
    }
}

/// Convert a [`Theme`] to its wire representation.
///
/// Returns `Some("dark")` or `Some("light")`, or `None` for
/// [`Theme::Unknown`] (server uses its default). Callers `.map(str::to_owned)`
/// at the serialization boundary when building PDU fields.
pub(crate) fn theme_to_wire(theme: Theme) -> Option<&'static str> {
    match theme {
        Theme::Dark => Some("dark"),
        Theme::Light => Some("light"),
        Theme::Unknown => None,
    }
}
