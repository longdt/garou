//! Protocol message types for the chat system
//!
//! All message payloads that can be serialized/deserialized within frames.
//! Uses serde for JSON serialization (can be swapped for protobuf/flatbuffers).

use serde::{Deserialize, Serialize};

/// Unique identifier types
pub type UserId = u64;
pub type RoomId = u64;
pub type MessageId = u64;
pub type UploadId = u64;
pub type ShardId = u8;

/// Number of shards for room distribution
pub const NUM_SHARDS: u8 = 8;

/// Calculate which shard a room belongs to
#[inline]
pub fn room_shard(room_id: RoomId) -> ShardId {
    (room_id % NUM_SHARDS as u64) as ShardId
}

// =============================================================================
// Control Messages (0x00 - 0x0F)
// =============================================================================

/// Initial handshake from client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    /// Protocol version
    pub version: u32,
    /// Client capabilities
    pub capabilities: Vec<String>,
}

impl Default for Hello {
    fn default() -> Self {
        Self {
            version: 1,
            capabilities: vec!["chat".to_string(), "presence".to_string()],
        }
    }
}

/// Server response to Hello
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAck {
    /// Server protocol version
    pub version: u32,
    /// Session ID assigned to this connection
    pub session_id: String,
    /// Number of shards the server uses
    pub num_shards: u8,
}

/// Authentication request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Auth {
    /// Authentication method: "token", "username", etc.
    pub method: String,
    /// Authentication credentials
    pub credentials: String,
}

/// Successful authentication response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthOk {
    /// Authenticated user ID
    pub user_id: UserId,
    /// Username
    pub username: String,
    /// List of room IDs the user is a member of
    pub rooms: Vec<RoomId>,
}

/// Authentication failure response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthFailed {
    /// Error code
    pub code: u32,
    /// Human-readable error message
    pub message: String,
}

/// Ping message for keepalive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ping {
    /// Timestamp when ping was sent (for RTT measurement)
    pub timestamp: u64,
}

/// Pong response to Ping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pong {
    /// Echo back the timestamp from Ping
    pub timestamp: u64,
}

/// Server throttle signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Throttle {
    /// Room ID to throttle (None = global throttle)
    pub room_id: Option<RoomId>,
    /// Delay in milliseconds before sending more messages
    pub delay_ms: u32,
    /// Reason for throttling
    pub reason: String,
}

/// Graceful disconnect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goodbye {
    /// Reason for disconnect
    pub reason: String,
}

/// Generic server command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCommand {
    /// Command name
    pub command: String,
    /// Command parameters as JSON
    pub params: serde_json::Value,
}

// =============================================================================
// Chat Command Messages (0x10 - 0x2F) - Client -> Server
// =============================================================================

/// Send a new message to a room
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessage {
    /// Target room ID
    pub room_id: RoomId,
    /// Message content
    pub content: String,
    /// Client-generated nonce for deduplication
    pub nonce: String,
    /// Optional reply to message ID
    pub reply_to: Option<MessageId>,
    /// Optional attachments (upload IDs)
    pub attachments: Vec<UploadId>,
}

/// Edit an existing message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditMessage {
    /// Message ID to edit
    pub message_id: MessageId,
    /// New content
    pub content: String,
}

/// Delete a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteMessage {
    /// Message ID to delete
    pub message_id: MessageId,
}

/// Add a reaction to a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddReaction {
    /// Message ID to react to
    pub message_id: MessageId,
    /// Emoji reaction (e.g., "👍", "❤️")
    pub emoji: String,
}

/// Remove a reaction from a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveReaction {
    /// Message ID
    pub message_id: MessageId,
    /// Emoji to remove
    pub emoji: String,
}

/// Join a room
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRoom {
    /// Room ID to join
    pub room_id: RoomId,
}

/// Leave a room
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveRoom {
    /// Room ID to leave
    pub room_id: RoomId,
}

/// Create a new room
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoom {
    /// Room name
    pub name: String,
    /// Room type: "public", "private", "dm"
    pub room_type: String,
    /// Initial members to invite
    pub members: Vec<UserId>,
}

// =============================================================================
// Room Messages (0x30 - 0x4F) - Server -> Client
// =============================================================================

/// User information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub user_id: UserId,
    pub username: String,
    pub avatar_url: Option<String>,
}

/// A chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMessage {
    /// Message ID
    pub message_id: MessageId,
    /// Room ID
    pub room_id: RoomId,
    /// Sender information
    pub sender: UserInfo,
    /// Message content
    pub content: String,
    /// Timestamp (Unix ms)
    pub timestamp: u64,
    /// Reply to message ID (if any)
    pub reply_to: Option<MessageId>,
    /// Attachment URLs
    pub attachments: Vec<String>,
    /// Client nonce for matching sent messages
    pub nonce: Option<String>,
}

/// Message edit notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMessageEdited {
    /// Message ID
    pub message_id: MessageId,
    /// Room ID
    pub room_id: RoomId,
    /// Editor user ID
    pub editor_id: UserId,
    /// New content
    pub content: String,
    /// Edit timestamp
    pub edited_at: u64,
}

/// Message deletion notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMessageDeleted {
    /// Message ID
    pub message_id: MessageId,
    /// Room ID
    pub room_id: RoomId,
    /// Who deleted it
    pub deleted_by: UserId,
    /// Deletion timestamp
    pub deleted_at: u64,
}

/// Reaction added notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomReactionAdded {
    /// Message ID
    pub message_id: MessageId,
    /// Room ID
    pub room_id: RoomId,
    /// User who reacted
    pub user_id: UserId,
    /// Emoji
    pub emoji: String,
}

/// Reaction removed notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomReactionRemoved {
    /// Message ID
    pub message_id: MessageId,
    /// Room ID
    pub room_id: RoomId,
    /// User who removed reaction
    pub user_id: UserId,
    /// Emoji
    pub emoji: String,
}

/// User joined room notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomUserJoined {
    /// Room ID
    pub room_id: RoomId,
    /// User who joined
    pub user: UserInfo,
    /// Join timestamp
    pub joined_at: u64,
}

/// User left room notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomUserLeft {
    /// Room ID
    pub room_id: RoomId,
    /// User ID who left
    pub user_id: UserId,
    /// Leave timestamp
    pub left_at: u64,
}

/// Room initialization (sent when joining a room)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInit {
    /// Room ID
    pub room_id: RoomId,
    /// Room name
    pub name: String,
    /// Room type
    pub room_type: String,
    /// Current members
    pub members: Vec<UserInfo>,
    /// Recent messages (for initial sync)
    pub recent_messages: Vec<RoomMessage>,
}

/// Room stream closing (user left or room deleted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomClose {
    /// Room ID
    pub room_id: RoomId,
    /// Reason: "left", "kicked", "deleted"
    pub reason: String,
}

// =============================================================================
// Shard Management (0x50 - 0x5F)
// =============================================================================

/// Shard assignment notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardAssignment {
    /// Mapping of shard ID to stream ID (on the server->client side)
    pub shard_streams: Vec<ShardStreamInfo>,
}

/// Information about a shard stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardStreamInfo {
    /// Shard ID (0-7 typically)
    pub shard_id: ShardId,
    /// Rooms in this shard that the user is subscribed to
    pub room_ids: Vec<RoomId>,
}

/// Room promoted to dedicated stream (high traffic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomPromoted {
    /// Room ID
    pub room_id: RoomId,
    /// Old shard it was on
    pub from_shard: ShardId,
}

/// Room demoted back to shard (traffic normalized)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomDemoted {
    /// Room ID
    pub room_id: RoomId,
    /// Shard it's returning to
    pub to_shard: ShardId,
}

// =============================================================================
// ACK Messages (0x60 - 0x6F) - Client -> Server
// =============================================================================

/// Message delivered acknowledgment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDelivered {
    /// Message ID
    pub message_id: MessageId,
    /// Room ID
    pub room_id: RoomId,
    /// Delivery timestamp
    pub delivered_at: u64,
}

/// Message read acknowledgment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRead {
    /// Message ID (last read message in the room)
    pub message_id: MessageId,
    /// Room ID
    pub room_id: RoomId,
    /// Read timestamp
    pub read_at: u64,
}

/// Generic message acknowledgment from server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAck {
    /// Client nonce being acknowledged
    pub nonce: String,
    /// Assigned message ID
    pub message_id: MessageId,
    /// Server timestamp
    pub timestamp: u64,
}

// =============================================================================
// Bulk Upload Messages (0x70 - 0x7F)
// =============================================================================

/// Start a file upload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadStart {
    /// Room ID for the upload
    pub room_id: RoomId,
    /// File name
    pub filename: String,
    /// MIME type
    pub mime_type: String,
    /// Total file size in bytes
    pub size: u64,
    /// SHA-256 checksum of the file
    pub checksum: String,
}

/// Upload chunk (binary payload in frame, this is just metadata)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadChunk {
    /// Upload ID (assigned by server in UploadAck)
    pub upload_id: UploadId,
    /// Offset in bytes
    pub offset: u64,
    /// Chunk size (the actual data follows in the frame payload)
    pub size: u32,
}

/// Upload complete signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadComplete {
    /// Upload ID
    pub upload_id: UploadId,
}

/// Cancel an upload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadCancel {
    /// Upload ID
    pub upload_id: UploadId,
    /// Reason for cancellation
    pub reason: String,
}

/// Upload acknowledgment from server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadAck {
    /// Assigned upload ID (for UploadStart)
    pub upload_id: UploadId,
    /// Bytes received so far
    pub bytes_received: u64,
    /// Final URL (only set when upload is complete)
    pub url: Option<String>,
}

// =============================================================================
// Datagram Messages (0x80 - 0x8F) - Unreliable
// =============================================================================

/// User is typing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Typing {
    /// Room ID
    pub room_id: RoomId,
    /// User ID (filled by server for outgoing)
    pub user_id: Option<UserId>,
}

/// User stopped typing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopTyping {
    /// Room ID
    pub room_id: RoomId,
    /// User ID (filled by server for outgoing)
    pub user_id: Option<UserId>,
}

/// User came online
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceOnline {
    /// User ID
    pub user_id: UserId,
    /// Optional status message
    pub status: Option<String>,
}

/// User went offline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceOffline {
    /// User ID
    pub user_id: UserId,
    /// Last seen timestamp
    pub last_seen: u64,
}

/// User is away
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceAway {
    /// User ID
    pub user_id: UserId,
    /// Away since timestamp
    pub away_since: u64,
}

// =============================================================================
// Error Message (0xFF)
// =============================================================================

/// Error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    /// Error code
    pub code: u32,
    /// Error message
    pub message: String,
    /// Related entity (message_id, room_id, etc.)
    pub context: Option<String>,
}

impl Error {
    // Common error codes
    pub const UNKNOWN: u32 = 1000;
    pub const INVALID_FRAME: u32 = 1001;
    pub const AUTH_REQUIRED: u32 = 1002;
    pub const AUTH_FAILED: u32 = 1003;
    pub const PERMISSION_DENIED: u32 = 1004;
    pub const NOT_FOUND: u32 = 1005;
    pub const RATE_LIMITED: u32 = 1006;
    pub const INVALID_MESSAGE: u32 = 1007;
    pub const ROOM_FULL: u32 = 1008;
    pub const UPLOAD_FAILED: u32 = 1009;
    pub const SERVER_ERROR: u32 = 1010;

    pub fn new(code: u32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            context: None,
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn unknown(message: impl Into<String>) -> Self {
        Self::new(Self::UNKNOWN, message)
    }

    pub fn invalid_frame(message: impl Into<String>) -> Self {
        Self::new(Self::INVALID_FRAME, message)
    }

    pub fn auth_required() -> Self {
        Self::new(Self::AUTH_REQUIRED, "Authentication required")
    }

    pub fn auth_failed(message: impl Into<String>) -> Self {
        Self::new(Self::AUTH_FAILED, message)
    }

    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::new(Self::PERMISSION_DENIED, message)
    }

    pub fn not_found(entity: impl Into<String>) -> Self {
        Self::new(Self::NOT_FOUND, format!("{} not found", entity.into()))
    }

    pub fn rate_limited(delay_ms: u32) -> Self {
        Self::new(
            Self::RATE_LIMITED,
            format!("Rate limited. Retry after {}ms", delay_ms),
        )
    }

    pub fn server_error(message: impl Into<String>) -> Self {
        Self::new(Self::SERVER_ERROR, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_shard() {
        assert_eq!(room_shard(0), 0);
        assert_eq!(room_shard(1), 1);
        assert_eq!(room_shard(7), 7);
        assert_eq!(room_shard(8), 0);
        assert_eq!(room_shard(15), 7);
        assert_eq!(room_shard(100), 4); // 100 % 8 = 4
    }

    #[test]
    fn test_serialize_send_message() {
        let msg = SendMessage {
            room_id: 123,
            content: "Hello, World!".to_string(),
            nonce: "abc123".to_string(),
            reply_to: None,
            attachments: vec![],
        };

        let json = serde_json::to_string(&msg).unwrap();
        let decoded: SendMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(msg.room_id, decoded.room_id);
        assert_eq!(msg.content, decoded.content);
        assert_eq!(msg.nonce, decoded.nonce);
    }

    #[test]
    fn test_serialize_room_message() {
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
            reply_to: None,
            attachments: vec![],
            nonce: Some("nonce123".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let decoded: RoomMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(msg.message_id, decoded.message_id);
        assert_eq!(msg.sender.username, decoded.sender.username);
    }

    #[test]
    fn test_error_constructors() {
        let err = Error::auth_failed("Invalid token");
        assert_eq!(err.code, Error::AUTH_FAILED);

        let err = Error::not_found("Room").with_context("room_id=123");
        assert_eq!(err.code, Error::NOT_FOUND);
        assert_eq!(err.context, Some("room_id=123".to_string()));
    }
}
