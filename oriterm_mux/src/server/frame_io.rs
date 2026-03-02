//! Non-blocking frame I/O for mio streams.
//!
//! [`FrameReader`] accumulates bytes from non-blocking reads and greedily
//! decodes complete frames. [`send_frame`] is a thin wrapper around
//! [`ProtocolCodec::encode_frame`] for clarity at call sites.

use std::io::{self, Write};

use crate::protocol::{HEADER_LEN, MAX_PAYLOAD};
use crate::{DecodeError, DecodedFrame, FrameHeader, MsgType, MuxPdu, ProtocolCodec};

/// Result of a single `read_from` call.
#[derive(Debug, PartialEq, Eq)]
pub enum ReadStatus {
    /// At least one byte was read.
    GotData,
    /// The peer closed the connection (EOF).
    Closed,
    /// No data available right now (`WouldBlock`).
    WouldBlock,
}

/// Accumulates bytes from a non-blocking stream and decodes frames.
///
/// The reader buffers partial headers and payloads across `read_from` calls.
/// After each `read_from`, call `try_decode` in a loop to drain all complete
/// frames before returning to the event loop.
pub struct FrameReader {
    buf: Vec<u8>,
}

impl FrameReader {
    /// Create a new empty reader.
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(4096),
        }
    }

    /// Append raw bytes to the internal buffer.
    ///
    /// Called after the caller reads bytes from the stream separately (to
    /// avoid double-mutable-borrow of `ClientConnection`).
    pub fn extend(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// Try to decode one complete frame from the buffer.
    ///
    /// Returns `Some(Ok(frame))` if a full frame was decoded and consumed,
    /// `Some(Err(e))` on a decode error (the malformed bytes are consumed),
    /// or `None` if there aren't enough bytes yet.
    pub fn try_decode(&mut self) -> Option<Result<DecodedFrame, DecodeError>> {
        if self.buf.len() < HEADER_LEN {
            return None;
        }

        let header = FrameHeader::decode(
            self.buf[..HEADER_LEN]
                .try_into()
                .expect("checked length >= HEADER_LEN"),
        );

        // Validate payload size.
        if header.payload_len > MAX_PAYLOAD {
            // Drain the header bytes and report the error.
            self.buf.drain(..HEADER_LEN);
            return Some(Err(DecodeError::PayloadTooLarge(header.payload_len)));
        }

        // Validate message type.
        if MsgType::from_u16(header.msg_type).is_none() {
            self.buf.drain(..HEADER_LEN);
            return Some(Err(DecodeError::UnknownMsgType(header.msg_type)));
        }

        let total = HEADER_LEN + header.payload_len as usize;
        if self.buf.len() < total {
            return None;
        }

        // Deserialize the payload.
        let payload = &self.buf[HEADER_LEN..total];
        let result: Result<MuxPdu, _> = bincode::deserialize(payload);
        self.buf.drain(..total);

        match result {
            Ok(pdu) => Some(Ok(DecodedFrame {
                seq: header.seq,
                pdu,
            })),
            Err(e) => Some(Err(DecodeError::Deserialize(e))),
        }
    }
}

/// Encode and send a single frame to a writer.
pub fn send_frame<W: Write>(writer: &mut W, seq: u32, pdu: &MuxPdu) -> io::Result<()> {
    ProtocolCodec::encode_frame(writer, seq, pdu)
}
