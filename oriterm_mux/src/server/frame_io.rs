//! Non-blocking frame I/O for mio streams.
//!
//! [`FrameReader`] accumulates bytes from non-blocking reads and greedily
//! decodes complete frames. [`send_frame`] is a thin wrapper around
//! [`ProtocolCodec::encode_frame`] for clarity at call sites.

use std::io::{self, Write};

use crate::protocol::{HEADER_LEN, MAX_PAYLOAD};
use crate::{DecodeError, DecodedFrame, FrameHeader, MsgType, MuxPdu};

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

/// Per-connection outgoing frame buffer.
///
/// Frames are serialized to an internal buffer via [`queue`]. The caller
/// then calls [`flush_to`] to write as much as possible to the non-blocking
/// stream. If a write returns `WouldBlock`, the remaining bytes stay in the
/// buffer and are retried on the next writable event.
pub struct FrameWriter {
    buf: Vec<u8>,
}

impl FrameWriter {
    /// Create a new empty writer.
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(256),
        }
    }

    /// Serialize a frame and append it to the outgoing buffer.
    pub fn queue(&mut self, seq: u32, pdu: &MuxPdu) -> io::Result<()> {
        let payload =
            bincode::serialize(pdu).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let payload_len: u32 = payload.len().try_into().map_err(|_overflow| {
            io::Error::new(io::ErrorKind::InvalidData, "payload exceeds u32 capacity")
        })?;

        if payload_len > MAX_PAYLOAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("payload too large: {payload_len} bytes (max {MAX_PAYLOAD})"),
            ));
        }

        let header = FrameHeader {
            msg_type: pdu.msg_type() as u16,
            seq,
            payload_len,
        };

        self.buf.extend_from_slice(&header.encode());
        self.buf.extend_from_slice(&payload);
        Ok(())
    }

    /// Write as much buffered data as possible to the stream.
    ///
    /// Returns `Ok(())` even if some data remains (caller should check
    /// [`has_pending`] and register `WRITABLE` interest if so).
    pub fn flush_to<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        while !self.buf.is_empty() {
            match writer.write(&self.buf) {
                Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero)),
                Ok(n) => {
                    self.buf.drain(..n);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Whether there is unsent data in the buffer.
    pub fn has_pending(&self) -> bool {
        !self.buf.is_empty()
    }
}
