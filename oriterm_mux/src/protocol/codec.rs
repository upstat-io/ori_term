//! Frame codec for encoding and decoding protocol messages.
//!
//! Provides [`ProtocolCodec`] which reads/writes framed [`MuxPdu`] messages
//! from any `Read`/`Write` stream. Handles partial reads via an internal
//! buffer (non-blocking streams may deliver data incrementally).

use std::io::{self, Read, Write};

use super::messages::MuxPdu;
use super::{FrameHeader, HEADER_LEN, MAX_PAYLOAD};

/// A decoded frame: sequence number + PDU.
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    /// Sequence number from the header (for request/response correlation).
    pub seq: u32,
    /// Decoded protocol message.
    pub pdu: MuxPdu,
}

/// Errors from frame decoding.
#[derive(Debug)]
pub enum DecodeError {
    /// I/O error reading from the stream.
    Io(io::Error),
    /// Payload exceeds [`MAX_PAYLOAD`].
    PayloadTooLarge(u32),
    /// Unknown message type ID in the header.
    UnknownMsgType(u16),
    /// Bincode deserialization failed.
    Deserialize(bincode::Error),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::PayloadTooLarge(n) => {
                write!(f, "payload too large: {n} bytes (max {MAX_PAYLOAD})")
            }
            Self::UnknownMsgType(t) => write!(f, "unknown message type: 0x{t:04X}"),
            Self::Deserialize(e) => write!(f, "deserialize error: {e}"),
        }
    }
}

impl std::error::Error for DecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Deserialize(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for DecodeError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<bincode::Error> for DecodeError {
    fn from(e: bincode::Error) -> Self {
        Self::Deserialize(e)
    }
}

/// Codec for encoding and decoding framed protocol messages.
///
/// Encoding is straightforward (serialize + write header + payload).
/// Decoding reads the full header then the full payload from the stream,
/// blocking until complete. For non-blocking streams, callers should ensure
/// the stream is readable before calling `decode_frame`.
///
/// The codec reuses a single payload buffer across `decode_frame` calls,
/// growing to the high-water mark and staying there. This avoids per-frame
/// allocation on the reader hot path.
pub struct ProtocolCodec {
    /// Reusable payload buffer for decoding. Grows to the largest frame
    /// seen and stays allocated across calls.
    decode_buf: Vec<u8>,
}

impl Default for ProtocolCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolCodec {
    /// Create a new codec with an empty decode buffer.
    pub fn new() -> Self {
        Self {
            decode_buf: Vec::new(),
        }
    }

    /// Encode a PDU and write it as a framed message.
    ///
    /// Writes the 10-byte header followed by the bincode payload atomically
    /// (single `write_all` call for each segment).
    pub fn encode_frame<W: Write>(writer: &mut W, seq: u32, pdu: &MuxPdu) -> io::Result<()> {
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

        writer.write_all(&header.encode())?;
        writer.write_all(&payload)?;
        writer.flush()
    }

    /// Decode a single framed message from a stream.
    ///
    /// Blocks until the full header and payload are read. Returns
    /// `DecodeError::Io` with `UnexpectedEof` if the stream closes
    /// mid-frame. Reuses an internal buffer that grows to the high-water
    /// mark, avoiding per-frame allocation.
    pub fn decode_frame<R: Read>(&mut self, reader: &mut R) -> Result<DecodedFrame, DecodeError> {
        // Read the 10-byte header.
        let mut hdr_buf = [0u8; HEADER_LEN];
        reader.read_exact(&mut hdr_buf)?;
        let header = FrameHeader::decode(&hdr_buf);

        // Validate payload size.
        if header.payload_len > MAX_PAYLOAD {
            return Err(DecodeError::PayloadTooLarge(header.payload_len));
        }

        // Validate message type. Read and discard the payload for unknown
        // types to keep the stream aligned for the next frame.
        if super::msg_type::MsgType::from_u16(header.msg_type).is_none() {
            let len = header.payload_len as usize;
            self.decode_buf.resize(len, 0);
            reader.read_exact(&mut self.decode_buf[..len])?;
            return Err(DecodeError::UnknownMsgType(header.msg_type));
        }

        // Read the payload into the reusable buffer. `resize` only
        // allocates when the frame is larger than any previously seen;
        // smaller frames reuse the existing capacity.
        let len = header.payload_len as usize;
        self.decode_buf.resize(len, 0);
        reader.read_exact(&mut self.decode_buf[..len])?;

        // Deserialize the PDU from bincode.
        let pdu: MuxPdu = bincode::deserialize(&self.decode_buf[..len])?;

        Ok(DecodedFrame {
            seq: header.seq,
            pdu,
        })
    }
}
