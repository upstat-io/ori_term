//! Wire message type IDs.

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
