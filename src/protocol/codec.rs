//! Codec for encoding/decoding protocol messages to/from frames
//!
//! This module provides the bridge between typed messages and binary frames.

use super::frame::{Frame, FrameType};
use super::messages::*;
use bytes::Bytes;
use std::io::{self, Error as IoError, ErrorKind};

/// Trait for messages that can be encoded to frames
pub trait Encodable {
    /// Get the frame type for this message
    fn frame_type(&self) -> FrameType;

    /// Encode the message payload to bytes
    fn encode_payload(&self) -> io::Result<Bytes>;

    /// Encode the complete frame
    fn encode_frame(&self) -> io::Result<Frame> {
        Ok(Frame::new(self.frame_type(), self.encode_payload()?))
    }
}

/// Trait for messages that can be decoded from frames
pub trait Decodable: Sized {
    /// Expected frame type for this message
    fn expected_frame_type() -> FrameType;

    /// Decode the message from a payload
    fn decode_payload(payload: &[u8]) -> io::Result<Self>;

    /// Decode from a complete frame, validating the frame type
    fn decode_frame(frame: &Frame) -> io::Result<Self> {
        if frame.frame_type != Self::expected_frame_type() {
            return Err(IoError::new(
                ErrorKind::InvalidData,
                format!(
                    "Expected frame type {:?}, got {:?}",
                    Self::expected_frame_type(),
                    frame.frame_type
                ),
            ));
        }
        Self::decode_payload(&frame.payload)
    }
}

/// Helper macro to implement Encodable and Decodable for a message type
macro_rules! impl_codec {
    ($type:ty, $frame_type:expr) => {
        impl Encodable for $type {
            fn frame_type(&self) -> FrameType {
                $frame_type
            }

            fn encode_payload(&self) -> io::Result<Bytes> {
                serde_json::to_vec(self)
                    .map(Bytes::from)
                    .map_err(|e| IoError::new(ErrorKind::InvalidData, e))
            }
        }

        impl Decodable for $type {
            fn expected_frame_type() -> FrameType {
                $frame_type
            }

            fn decode_payload(payload: &[u8]) -> io::Result<Self> {
                serde_json::from_slice(payload).map_err(|e| IoError::new(ErrorKind::InvalidData, e))
            }
        }
    };
}

// Control messages
impl_codec!(Hello, FrameType::Hello);
impl_codec!(HelloAck, FrameType::HelloAck);
impl_codec!(Auth, FrameType::Auth);
impl_codec!(AuthOk, FrameType::AuthOk);
impl_codec!(AuthFailed, FrameType::AuthFailed);
impl_codec!(Ping, FrameType::Ping);
impl_codec!(Pong, FrameType::Pong);
impl_codec!(Throttle, FrameType::Throttle);
impl_codec!(Goodbye, FrameType::Goodbye);
impl_codec!(ServerCommand, FrameType::ServerCommand);

// Chat command messages
impl_codec!(SendMessage, FrameType::SendMessage);
impl_codec!(EditMessage, FrameType::EditMessage);
impl_codec!(DeleteMessage, FrameType::DeleteMessage);
impl_codec!(AddReaction, FrameType::AddReaction);
impl_codec!(RemoveReaction, FrameType::RemoveReaction);
impl_codec!(JoinRoom, FrameType::JoinRoom);
impl_codec!(LeaveRoom, FrameType::LeaveRoom);
impl_codec!(CreateRoom, FrameType::CreateRoom);

// Room messages
impl_codec!(RoomMessage, FrameType::RoomMessage);
impl_codec!(RoomMessageEdited, FrameType::RoomMessageEdited);
impl_codec!(RoomMessageDeleted, FrameType::RoomMessageDeleted);
impl_codec!(RoomReactionAdded, FrameType::RoomReactionAdded);
impl_codec!(RoomReactionRemoved, FrameType::RoomReactionRemoved);
impl_codec!(RoomUserJoined, FrameType::RoomUserJoined);
impl_codec!(RoomUserLeft, FrameType::RoomUserLeft);
impl_codec!(RoomInit, FrameType::RoomInit);
impl_codec!(RoomClose, FrameType::RoomClose);

// Shard management
impl_codec!(ShardAssignment, FrameType::ShardAssignment);
impl_codec!(RoomPromoted, FrameType::RoomPromoted);
impl_codec!(RoomDemoted, FrameType::RoomDemoted);

// ACK messages
impl_codec!(MessageDelivered, FrameType::MessageDelivered);
impl_codec!(MessageRead, FrameType::MessageRead);
impl_codec!(MessageAck, FrameType::MessageAck);

// Upload messages
impl_codec!(UploadStart, FrameType::UploadStart);
impl_codec!(UploadChunk, FrameType::UploadChunk);
impl_codec!(UploadComplete, FrameType::UploadComplete);
impl_codec!(UploadCancel, FrameType::UploadCancel);
impl_codec!(UploadAck, FrameType::UploadAck);

// Datagram messages
impl_codec!(Typing, FrameType::Typing);
impl_codec!(StopTyping, FrameType::StopTyping);
impl_codec!(PresenceOnline, FrameType::PresenceOnline);
impl_codec!(PresenceOffline, FrameType::PresenceOffline);
impl_codec!(PresenceAway, FrameType::PresenceAway);

// Error message
impl_codec!(Error, FrameType::Error);

/// Decode any frame into a typed message enum
#[derive(Debug, Clone)]
pub enum DecodedMessage {
    // Control
    Hello(Hello),
    HelloAck(HelloAck),
    Auth(Auth),
    AuthOk(AuthOk),
    AuthFailed(AuthFailed),
    Ping(Ping),
    Pong(Pong),
    Throttle(Throttle),
    Goodbye(Goodbye),
    ServerCommand(ServerCommand),

    // Chat commands
    SendMessage(SendMessage),
    EditMessage(EditMessage),
    DeleteMessage(DeleteMessage),
    AddReaction(AddReaction),
    RemoveReaction(RemoveReaction),
    JoinRoom(JoinRoom),
    LeaveRoom(LeaveRoom),
    CreateRoom(CreateRoom),

    // Room messages
    RoomMessage(RoomMessage),
    RoomMessageEdited(RoomMessageEdited),
    RoomMessageDeleted(RoomMessageDeleted),
    RoomReactionAdded(RoomReactionAdded),
    RoomReactionRemoved(RoomReactionRemoved),
    RoomUserJoined(RoomUserJoined),
    RoomUserLeft(RoomUserLeft),
    RoomInit(RoomInit),
    RoomClose(RoomClose),

    // Shard management
    ShardAssignment(ShardAssignment),
    RoomPromoted(RoomPromoted),
    RoomDemoted(RoomDemoted),

    // ACK
    MessageDelivered(MessageDelivered),
    MessageRead(MessageRead),
    MessageAck(MessageAck),

    // Upload
    UploadStart(UploadStart),
    UploadChunk(UploadChunk),
    UploadComplete(UploadComplete),
    UploadCancel(UploadCancel),
    UploadAck(UploadAck),

    // Datagram
    Typing(Typing),
    StopTyping(StopTyping),
    PresenceOnline(PresenceOnline),
    PresenceOffline(PresenceOffline),
    PresenceAway(PresenceAway),

    // Error
    Error(Error),
}

impl DecodedMessage {
    /// Decode a frame into a typed message
    pub fn decode(frame: &Frame) -> io::Result<Self> {
        let payload = &frame.payload;

        match frame.frame_type {
            // Control
            FrameType::Hello => Ok(Self::Hello(serde_json::from_slice(payload)?)),
            FrameType::HelloAck => Ok(Self::HelloAck(serde_json::from_slice(payload)?)),
            FrameType::Auth => Ok(Self::Auth(serde_json::from_slice(payload)?)),
            FrameType::AuthOk => Ok(Self::AuthOk(serde_json::from_slice(payload)?)),
            FrameType::AuthFailed => Ok(Self::AuthFailed(serde_json::from_slice(payload)?)),
            FrameType::Ping => Ok(Self::Ping(serde_json::from_slice(payload)?)),
            FrameType::Pong => Ok(Self::Pong(serde_json::from_slice(payload)?)),
            FrameType::Throttle => Ok(Self::Throttle(serde_json::from_slice(payload)?)),
            FrameType::Goodbye => Ok(Self::Goodbye(serde_json::from_slice(payload)?)),
            FrameType::ServerCommand => Ok(Self::ServerCommand(serde_json::from_slice(payload)?)),

            // Chat commands
            FrameType::SendMessage => Ok(Self::SendMessage(serde_json::from_slice(payload)?)),
            FrameType::EditMessage => Ok(Self::EditMessage(serde_json::from_slice(payload)?)),
            FrameType::DeleteMessage => Ok(Self::DeleteMessage(serde_json::from_slice(payload)?)),
            FrameType::AddReaction => Ok(Self::AddReaction(serde_json::from_slice(payload)?)),
            FrameType::RemoveReaction => Ok(Self::RemoveReaction(serde_json::from_slice(payload)?)),
            FrameType::JoinRoom => Ok(Self::JoinRoom(serde_json::from_slice(payload)?)),
            FrameType::LeaveRoom => Ok(Self::LeaveRoom(serde_json::from_slice(payload)?)),
            FrameType::CreateRoom => Ok(Self::CreateRoom(serde_json::from_slice(payload)?)),

            // Room messages
            FrameType::RoomMessage => Ok(Self::RoomMessage(serde_json::from_slice(payload)?)),
            FrameType::RoomMessageEdited => {
                Ok(Self::RoomMessageEdited(serde_json::from_slice(payload)?))
            }
            FrameType::RoomMessageDeleted => {
                Ok(Self::RoomMessageDeleted(serde_json::from_slice(payload)?))
            }
            FrameType::RoomReactionAdded => {
                Ok(Self::RoomReactionAdded(serde_json::from_slice(payload)?))
            }
            FrameType::RoomReactionRemoved => {
                Ok(Self::RoomReactionRemoved(serde_json::from_slice(payload)?))
            }
            FrameType::RoomUserJoined => Ok(Self::RoomUserJoined(serde_json::from_slice(payload)?)),
            FrameType::RoomUserLeft => Ok(Self::RoomUserLeft(serde_json::from_slice(payload)?)),
            FrameType::RoomInit => Ok(Self::RoomInit(serde_json::from_slice(payload)?)),
            FrameType::RoomClose => Ok(Self::RoomClose(serde_json::from_slice(payload)?)),

            // Shard management
            FrameType::ShardAssignment => {
                Ok(Self::ShardAssignment(serde_json::from_slice(payload)?))
            }
            FrameType::RoomPromoted => Ok(Self::RoomPromoted(serde_json::from_slice(payload)?)),
            FrameType::RoomDemoted => Ok(Self::RoomDemoted(serde_json::from_slice(payload)?)),

            // ACK
            FrameType::MessageDelivered => {
                Ok(Self::MessageDelivered(serde_json::from_slice(payload)?))
            }
            FrameType::MessageRead => Ok(Self::MessageRead(serde_json::from_slice(payload)?)),
            FrameType::MessageAck => Ok(Self::MessageAck(serde_json::from_slice(payload)?)),

            // Upload
            FrameType::UploadStart => Ok(Self::UploadStart(serde_json::from_slice(payload)?)),
            FrameType::UploadChunk => Ok(Self::UploadChunk(serde_json::from_slice(payload)?)),
            FrameType::UploadComplete => Ok(Self::UploadComplete(serde_json::from_slice(payload)?)),
            FrameType::UploadCancel => Ok(Self::UploadCancel(serde_json::from_slice(payload)?)),
            FrameType::UploadAck => Ok(Self::UploadAck(serde_json::from_slice(payload)?)),

            // Datagram
            FrameType::Typing => Ok(Self::Typing(serde_json::from_slice(payload)?)),
            FrameType::StopTyping => Ok(Self::StopTyping(serde_json::from_slice(payload)?)),
            FrameType::PresenceOnline => Ok(Self::PresenceOnline(serde_json::from_slice(payload)?)),
            FrameType::PresenceOffline => {
                Ok(Self::PresenceOffline(serde_json::from_slice(payload)?))
            }
            FrameType::PresenceAway => Ok(Self::PresenceAway(serde_json::from_slice(payload)?)),

            // Error
            FrameType::Error => Ok(Self::Error(serde_json::from_slice(payload)?)),
        }
    }

    /// Get the frame type of this message
    pub fn frame_type(&self) -> FrameType {
        match self {
            Self::Hello(_) => FrameType::Hello,
            Self::HelloAck(_) => FrameType::HelloAck,
            Self::Auth(_) => FrameType::Auth,
            Self::AuthOk(_) => FrameType::AuthOk,
            Self::AuthFailed(_) => FrameType::AuthFailed,
            Self::Ping(_) => FrameType::Ping,
            Self::Pong(_) => FrameType::Pong,
            Self::Throttle(_) => FrameType::Throttle,
            Self::Goodbye(_) => FrameType::Goodbye,
            Self::ServerCommand(_) => FrameType::ServerCommand,
            Self::SendMessage(_) => FrameType::SendMessage,
            Self::EditMessage(_) => FrameType::EditMessage,
            Self::DeleteMessage(_) => FrameType::DeleteMessage,
            Self::AddReaction(_) => FrameType::AddReaction,
            Self::RemoveReaction(_) => FrameType::RemoveReaction,
            Self::JoinRoom(_) => FrameType::JoinRoom,
            Self::LeaveRoom(_) => FrameType::LeaveRoom,
            Self::CreateRoom(_) => FrameType::CreateRoom,
            Self::RoomMessage(_) => FrameType::RoomMessage,
            Self::RoomMessageEdited(_) => FrameType::RoomMessageEdited,
            Self::RoomMessageDeleted(_) => FrameType::RoomMessageDeleted,
            Self::RoomReactionAdded(_) => FrameType::RoomReactionAdded,
            Self::RoomReactionRemoved(_) => FrameType::RoomReactionRemoved,
            Self::RoomUserJoined(_) => FrameType::RoomUserJoined,
            Self::RoomUserLeft(_) => FrameType::RoomUserLeft,
            Self::RoomInit(_) => FrameType::RoomInit,
            Self::RoomClose(_) => FrameType::RoomClose,
            Self::ShardAssignment(_) => FrameType::ShardAssignment,
            Self::RoomPromoted(_) => FrameType::RoomPromoted,
            Self::RoomDemoted(_) => FrameType::RoomDemoted,
            Self::MessageDelivered(_) => FrameType::MessageDelivered,
            Self::MessageRead(_) => FrameType::MessageRead,
            Self::MessageAck(_) => FrameType::MessageAck,
            Self::UploadStart(_) => FrameType::UploadStart,
            Self::UploadChunk(_) => FrameType::UploadChunk,
            Self::UploadComplete(_) => FrameType::UploadComplete,
            Self::UploadCancel(_) => FrameType::UploadCancel,
            Self::UploadAck(_) => FrameType::UploadAck,
            Self::Typing(_) => FrameType::Typing,
            Self::StopTyping(_) => FrameType::StopTyping,
            Self::PresenceOnline(_) => FrameType::PresenceOnline,
            Self::PresenceOffline(_) => FrameType::PresenceOffline,
            Self::PresenceAway(_) => FrameType::PresenceAway,
            Self::Error(_) => FrameType::Error,
        }
    }

    /// Check if this is a control message
    pub fn is_control(&self) -> bool {
        self.frame_type().is_control()
    }

    /// Check if this is a chat command
    pub fn is_chat_command(&self) -> bool {
        self.frame_type().is_chat_command()
    }

    /// Check if this is a room message
    pub fn is_room_message(&self) -> bool {
        self.frame_type().is_room_message()
    }

    /// Check if this is a datagram message
    pub fn is_datagram(&self) -> bool {
        self.frame_type().is_datagram()
    }
}

/// Encode a message directly to bytes (convenience function)
pub fn encode<T: Encodable>(msg: &T) -> io::Result<Bytes> {
    msg.encode_frame().map(|f| f.encode_to_bytes())
}

/// Decode a frame to a specific message type (convenience function)
pub fn decode<T: Decodable>(frame: &Frame) -> io::Result<T> {
    T::decode_frame(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = SendMessage {
            room_id: 123,
            content: "Hello, World!".to_string(),
            nonce: "abc123".to_string(),
            reply_to: None,
            attachments: vec![],
        };

        let frame = original.encode_frame().unwrap();
        assert_eq!(frame.frame_type, FrameType::SendMessage);

        let decoded = SendMessage::decode_frame(&frame).unwrap();
        assert_eq!(original.room_id, decoded.room_id);
        assert_eq!(original.content, decoded.content);
        assert_eq!(original.nonce, decoded.nonce);
    }

    #[test]
    fn test_decoded_message_enum() {
        let msg = Ping { timestamp: 12345 };
        let frame = msg.encode_frame().unwrap();

        let decoded = DecodedMessage::decode(&frame).unwrap();
        assert!(decoded.is_control());

        match decoded {
            DecodedMessage::Ping(ping) => {
                assert_eq!(ping.timestamp, 12345);
            }
            _ => panic!("Expected Ping message"),
        }
    }

    #[test]
    fn test_wrong_frame_type() {
        let msg = Ping { timestamp: 12345 };
        let frame = msg.encode_frame().unwrap();

        // Try to decode as Pong (wrong type)
        let result = Pong::decode_frame(&frame);
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_helper() {
        let msg = Hello::default();
        let bytes = encode(&msg).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_error_message_encoding() {
        let err = Error::auth_failed("Invalid credentials");
        let frame = err.encode_frame().unwrap();

        let decoded = Error::decode_frame(&frame).unwrap();
        assert_eq!(decoded.code, Error::AUTH_FAILED);
        assert_eq!(decoded.message, "Invalid credentials");
    }

    #[test]
    fn test_room_message_encoding() {
        let msg = RoomMessage {
            message_id: 456,
            room_id: 123,
            sender: UserInfo {
                user_id: 1,
                username: "alice".to_string(),
                avatar_url: None,
            },
            content: "Test message".to_string(),
            timestamp: 1234567890,
            reply_to: Some(100),
            attachments: vec!["https://example.com/file.png".to_string()],
            nonce: Some("nonce123".to_string()),
        };

        let frame = msg.encode_frame().unwrap();
        let decoded = RoomMessage::decode_frame(&frame).unwrap();

        assert_eq!(decoded.message_id, 456);
        assert_eq!(decoded.reply_to, Some(100));
        assert_eq!(decoded.attachments.len(), 1);
    }
}
