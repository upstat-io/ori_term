//! IPC wire protocol for daemon ↔ window communication.
//!
//! Binary framing with a fixed 10-byte header followed by a bincode-encoded
//! payload. Designed for low-latency local IPC (Unix sockets / named pipes).
//!
//! # Frame format
//!
//! ```text
//! ┌──────────┬──────────┬──────────────────────┐
//! │ type(u16)│ seq(u32) │ payload_len(u32)      │
//! ├──────────┴──────────┴──────────────────────┤
//! │ payload (bincode-encoded MuxPdu variant)    │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! - `type`: message type ID for pre-routing and debugging.
//! - `seq`: request/response correlation. Notifications use `seq = 0`.
//! - `payload_len`: u32, max 16 MiB.
//! - payload: bincode-serialized variant fields.

mod codec;
pub(crate) mod messages;
pub(crate) mod msg_type;
mod snapshot;

pub use codec::{DecodeError, DecodedFrame, ProtocolCodec};
pub use messages::MuxPdu;
pub use msg_type::MsgType;
pub use snapshot::{
    PaneSnapshot, WireCell, WireCellFlags, WireColor, WireCursor, WireCursorShape, WireRgb,
    WireSearchMatch, WireSelection,
};

/// Frame header size in bytes.
pub const HEADER_LEN: usize = 10;

/// Maximum payload size (16 MiB).
pub const MAX_PAYLOAD: u32 = 16 * 1024 * 1024;

/// Frame header on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Message type ID (for routing and debugging).
    pub msg_type: u16,
    /// Request/response correlation. `0` for fire-and-forget and notifications.
    pub seq: u32,
    /// Length of the bincode-encoded payload in bytes.
    pub payload_len: u32,
}

impl FrameHeader {
    /// Encode the header into a 10-byte buffer.
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut buf = [0u8; HEADER_LEN];
        buf[0..2].copy_from_slice(&self.msg_type.to_le_bytes());
        buf[2..6].copy_from_slice(&self.seq.to_le_bytes());
        buf[6..10].copy_from_slice(&self.payload_len.to_le_bytes());
        buf
    }

    /// Decode a header from a 10-byte buffer.
    pub fn decode(buf: &[u8; HEADER_LEN]) -> Self {
        let msg_type = u16::from_le_bytes([buf[0], buf[1]]);
        let seq = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]);
        let payload_len = u32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]);
        Self {
            msg_type,
            seq,
            payload_len,
        }
    }
}

#[cfg(test)]
mod tests;
