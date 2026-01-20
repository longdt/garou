//! Binary frame protocol with length-prefixed messages
//!
//! Frame format:
//! ```text
//! +--------+--------+------------------+
//! | type   | length | payload          |
//! | (1 byte)| (4 bytes, BE) | (variable)  |
//! +--------+--------+------------------+
//! ```

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::io::{self, Cursor};

/// Frame header size: 1 byte type + 4 bytes length
pub const FRAME_HEADER_SIZE: usize = 5;

/// Maximum frame payload size (16 MB)
pub const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

/// Frame types for different message categories
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameType {
    // Control stream messages (0x00 - 0x0F)
    Hello = 0x00,
    HelloAck = 0x01,
    Auth = 0x02,
    AuthOk = 0x03,
    AuthFailed = 0x04,
    Ping = 0x05,
    Pong = 0x06,
    Throttle = 0x07,
    Goodbye = 0x08,
    ServerCommand = 0x09,

    // Chat command messages (0x10 - 0x2F)
    SendMessage = 0x10,
    EditMessage = 0x11,
    DeleteMessage = 0x12,
    AddReaction = 0x13,
    RemoveReaction = 0x14,
    JoinRoom = 0x15,
    LeaveRoom = 0x16,
    CreateRoom = 0x17,

    // Server -> Client room messages (0x30 - 0x4F)
    RoomMessage = 0x30,
    RoomMessageEdited = 0x31,
    RoomMessageDeleted = 0x32,
    RoomReactionAdded = 0x33,
    RoomReactionRemoved = 0x34,
    RoomUserJoined = 0x35,
    RoomUserLeft = 0x36,
    RoomInit = 0x37,
    RoomClose = 0x38,

    // Shard management (0x50 - 0x5F)
    ShardAssignment = 0x50,
    RoomPromoted = 0x51,
    RoomDemoted = 0x52,

    // ACK messages (0x60 - 0x6F)
    MessageDelivered = 0x60,
    MessageRead = 0x61,
    MessageAck = 0x62,

    // Bulk upload messages (0x70 - 0x7F)
    UploadStart = 0x70,
    UploadChunk = 0x71,
    UploadComplete = 0x72,
    UploadCancel = 0x73,
    UploadAck = 0x74,

    // Datagram messages (0x80 - 0x8F)
    Typing = 0x80,
    StopTyping = 0x81,
    PresenceOnline = 0x82,
    PresenceOffline = 0x83,
    PresenceAway = 0x84,

    // Error (0xFF)
    Error = 0xFF,
}

impl FrameType {
    /// Convert from u8, returns None for unknown types
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(FrameType::Hello),
            0x01 => Some(FrameType::HelloAck),
            0x02 => Some(FrameType::Auth),
            0x03 => Some(FrameType::AuthOk),
            0x04 => Some(FrameType::AuthFailed),
            0x05 => Some(FrameType::Ping),
            0x06 => Some(FrameType::Pong),
            0x07 => Some(FrameType::Throttle),
            0x08 => Some(FrameType::Goodbye),
            0x09 => Some(FrameType::ServerCommand),

            0x10 => Some(FrameType::SendMessage),
            0x11 => Some(FrameType::EditMessage),
            0x12 => Some(FrameType::DeleteMessage),
            0x13 => Some(FrameType::AddReaction),
            0x14 => Some(FrameType::RemoveReaction),
            0x15 => Some(FrameType::JoinRoom),
            0x16 => Some(FrameType::LeaveRoom),
            0x17 => Some(FrameType::CreateRoom),

            0x30 => Some(FrameType::RoomMessage),
            0x31 => Some(FrameType::RoomMessageEdited),
            0x32 => Some(FrameType::RoomMessageDeleted),
            0x33 => Some(FrameType::RoomReactionAdded),
            0x34 => Some(FrameType::RoomReactionRemoved),
            0x35 => Some(FrameType::RoomUserJoined),
            0x36 => Some(FrameType::RoomUserLeft),
            0x37 => Some(FrameType::RoomInit),
            0x38 => Some(FrameType::RoomClose),

            0x50 => Some(FrameType::ShardAssignment),
            0x51 => Some(FrameType::RoomPromoted),
            0x52 => Some(FrameType::RoomDemoted),

            0x60 => Some(FrameType::MessageDelivered),
            0x61 => Some(FrameType::MessageRead),
            0x62 => Some(FrameType::MessageAck),

            0x70 => Some(FrameType::UploadStart),
            0x71 => Some(FrameType::UploadChunk),
            0x72 => Some(FrameType::UploadComplete),
            0x73 => Some(FrameType::UploadCancel),
            0x74 => Some(FrameType::UploadAck),

            0x80 => Some(FrameType::Typing),
            0x81 => Some(FrameType::StopTyping),
            0x82 => Some(FrameType::PresenceOnline),
            0x83 => Some(FrameType::PresenceOffline),
            0x84 => Some(FrameType::PresenceAway),

            0xFF => Some(FrameType::Error),
            _ => None,
        }
    }

    /// Check if this frame type is valid for the control stream
    pub fn is_control(&self) -> bool {
        (*self as u8) < 0x10
    }

    /// Check if this frame type is a chat command
    pub fn is_chat_command(&self) -> bool {
        let val = *self as u8;
        (0x10..0x30).contains(&val)
    }

    /// Check if this frame type is a room message
    pub fn is_room_message(&self) -> bool {
        let val = *self as u8;
        (0x30..0x50).contains(&val)
    }

    /// Check if this frame type is a shard management message
    pub fn is_shard_management(&self) -> bool {
        let val = *self as u8;
        (0x50..0x60).contains(&val)
    }

    /// Check if this frame type is an ACK
    pub fn is_ack(&self) -> bool {
        let val = *self as u8;
        (0x60..0x70).contains(&val)
    }

    /// Check if this frame type is for bulk upload
    pub fn is_upload(&self) -> bool {
        let val = *self as u8;
        (0x70..0x80).contains(&val)
    }

    /// Check if this frame type is a datagram message
    pub fn is_datagram(&self) -> bool {
        let val = *self as u8;
        (0x80..0x90).contains(&val)
    }
}

/// A single protocol frame
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub frame_type: FrameType,
    pub payload: Bytes,
}

impl Frame {
    /// Create a new frame with the given type and payload
    pub fn new(frame_type: FrameType, payload: impl Into<Bytes>) -> Self {
        Self {
            frame_type,
            payload: payload.into(),
        }
    }

    /// Create an empty frame (no payload)
    pub fn empty(frame_type: FrameType) -> Self {
        Self {
            frame_type,
            payload: Bytes::new(),
        }
    }

    /// Get the total encoded size of this frame
    pub fn encoded_size(&self) -> usize {
        FRAME_HEADER_SIZE + self.payload.len()
    }

    /// Encode this frame into a buffer
    pub fn encode(&self, buf: &mut BytesMut) {
        buf.reserve(self.encoded_size());
        buf.put_u8(self.frame_type as u8);
        buf.put_u32(self.payload.len() as u32);
        buf.put_slice(&self.payload);
    }

    /// Encode this frame into a new Bytes
    pub fn encode_to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(self.encoded_size());
        self.encode(&mut buf);
        buf.freeze()
    }

    /// Try to decode a frame from a buffer
    /// Returns Ok(Some(frame)) if successful, Ok(None) if more data needed
    pub fn decode(buf: &mut BytesMut) -> io::Result<Option<Frame>> {
        // Check if we have enough data for the header
        if buf.len() < FRAME_HEADER_SIZE {
            return Ok(None);
        }

        // Peek at the header without consuming
        let mut cursor = Cursor::new(&buf[..]);
        let frame_type_byte = cursor.get_u8();
        let payload_len = cursor.get_u32() as usize;

        // Validate frame type
        let frame_type = FrameType::from_u8(frame_type_byte).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown frame type: 0x{:02X}", frame_type_byte),
            )
        })?;

        // Validate payload size
        if payload_len > MAX_FRAME_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Frame payload too large: {} bytes (max: {})",
                    payload_len, MAX_FRAME_SIZE
                ),
            ));
        }

        // Check if we have the full frame
        let total_size = FRAME_HEADER_SIZE + payload_len;
        if buf.len() < total_size {
            return Ok(None);
        }

        // Consume the header
        buf.advance(FRAME_HEADER_SIZE);

        // Extract payload
        let payload = buf.split_to(payload_len).freeze();

        Ok(Some(Frame {
            frame_type,
            payload,
        }))
    }

    /// Decode a single frame from a complete buffer (no streaming)
    pub fn decode_complete(data: &[u8]) -> io::Result<Frame> {
        if data.len() < FRAME_HEADER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Incomplete frame header",
            ));
        }

        let frame_type_byte = data[0];
        let payload_len = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;

        let frame_type = FrameType::from_u8(frame_type_byte).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown frame type: 0x{:02X}", frame_type_byte),
            )
        })?;

        if payload_len > MAX_FRAME_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Frame payload too large: {} bytes (max: {})",
                    payload_len, MAX_FRAME_SIZE
                ),
            ));
        }

        let expected_len = FRAME_HEADER_SIZE + payload_len;
        if data.len() < expected_len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "Incomplete frame: expected {} bytes, got {}",
                    expected_len,
                    data.len()
                ),
            ));
        }

        let payload = Bytes::copy_from_slice(&data[FRAME_HEADER_SIZE..expected_len]);

        Ok(Frame {
            frame_type,
            payload,
        })
    }
}

/// Frame encoder/decoder for streaming use
#[derive(Debug, Default)]
pub struct FrameCodec {
    buffer: BytesMut,
}

impl FrameCodec {
    /// Create a new frame codec
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(4096),
        }
    }

    /// Create a new frame codec with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: BytesMut::with_capacity(capacity),
        }
    }

    /// Feed data into the codec
    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Try to decode the next frame
    pub fn decode_next(&mut self) -> io::Result<Option<Frame>> {
        Frame::decode(&mut self.buffer)
    }

    /// Encode a frame
    pub fn encode(&self, frame: &Frame) -> Bytes {
        frame.encode_to_bytes()
    }

    /// Get the current buffer length
    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_type_roundtrip() {
        let types = [
            FrameType::Hello,
            FrameType::Auth,
            FrameType::Ping,
            FrameType::SendMessage,
            FrameType::RoomMessage,
            FrameType::MessageDelivered,
            FrameType::UploadStart,
            FrameType::Typing,
            FrameType::Error,
        ];

        for frame_type in types {
            let byte = frame_type as u8;
            let recovered = FrameType::from_u8(byte).unwrap();
            assert_eq!(frame_type, recovered);
        }
    }

    #[test]
    fn test_frame_encode_decode() {
        let original = Frame::new(FrameType::SendMessage, "Hello, World!");
        let encoded = original.encode_to_bytes();

        let decoded = Frame::decode_complete(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_frame_codec_streaming() {
        let mut codec = FrameCodec::new();

        // Use larger payloads to ensure partial frame scenario
        let frame1 = Frame::new(FrameType::Ping, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let frame2 = Frame::new(
            FrameType::Pong,
            vec![11, 12, 13, 14, 15, 16, 17, 18, 19, 20],
        );

        // Encode both frames
        let mut data = BytesMut::new();
        frame1.encode(&mut data);
        frame2.encode(&mut data);

        // Feed just the header (incomplete frame)
        codec.feed(&data[..3]);

        // Should not have a complete frame yet (not even header complete)
        assert!(codec.decode_next().unwrap().is_none());

        // Feed the rest
        codec.feed(&data[3..]);

        // Now we should get both frames
        let decoded1 = codec.decode_next().unwrap().unwrap();
        let decoded2 = codec.decode_next().unwrap().unwrap();

        assert_eq!(frame1, decoded1);
        assert_eq!(frame2, decoded2);

        // No more frames
        assert!(codec.decode_next().unwrap().is_none());
    }

    #[test]
    fn test_frame_type_categories() {
        assert!(FrameType::Hello.is_control());
        assert!(FrameType::Auth.is_control());
        assert!(!FrameType::SendMessage.is_control());

        assert!(FrameType::SendMessage.is_chat_command());
        assert!(FrameType::AddReaction.is_chat_command());
        assert!(!FrameType::RoomMessage.is_chat_command());

        assert!(FrameType::RoomMessage.is_room_message());
        assert!(!FrameType::SendMessage.is_room_message());

        assert!(FrameType::MessageDelivered.is_ack());
        assert!(!FrameType::Ping.is_ack());

        assert!(FrameType::UploadStart.is_upload());
        assert!(FrameType::UploadChunk.is_upload());

        assert!(FrameType::Typing.is_datagram());
        assert!(FrameType::PresenceOnline.is_datagram());
    }

    #[test]
    fn test_empty_frame() {
        let frame = Frame::empty(FrameType::Ping);
        assert!(frame.payload.is_empty());
        assert_eq!(frame.encoded_size(), FRAME_HEADER_SIZE);

        let encoded = frame.encode_to_bytes();
        let decoded = Frame::decode_complete(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn test_invalid_frame_type() {
        let mut data = BytesMut::new();
        data.put_u8(0xFE); // Invalid type
        data.put_u32(0);

        let result = Frame::decode_complete(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_frame_too_large() {
        let mut data = BytesMut::new();
        data.put_u8(FrameType::SendMessage as u8);
        data.put_u32((MAX_FRAME_SIZE + 1) as u32);

        let result = Frame::decode_complete(&data);
        assert!(result.is_err());
    }
}
