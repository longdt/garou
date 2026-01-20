//! Garou - High-Performance QUIC Chat Server
//!
//! This library provides a distributed chat server implementation using QUIC protocol
//! with optimized multi-stream architecture for ultra-low-latency messaging.
//!
//! ## Architecture
//!
//! The server uses a sophisticated stream layout to eliminate head-of-line blocking:
//!
//! - **Control Stream** (bidirectional): Auth, ping/pong, commands
//! - **Chat Commands Stream** (client→server): Messages, reactions, edits
//! - **Bulk Upload Stream** (client→server): Files, images, voice notes
//! - **ACK Stream** (client→server): Delivery/read receipts
//! - **Shard Streams** (server→client): Room messages grouped by shard
//! - **Hot Room Streams** (server→client): Dedicated streams for high-traffic rooms
//! - **Datagrams**: Typing indicators, presence (unreliable)
//!
//! ## Example
//!
//! ```rust,ignore
//! use garou::{MultiStreamServer, ServerConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ServerConfig::default();
//!     let mut server = MultiStreamServer::new(config);
//!     server.start().await?;
//!     Ok(())
//! }
//! ```

// Core modules
pub mod error;
pub mod protocol;
pub mod server;
pub mod transport;

// Re-export error types
pub use error::{ChatError, Result};

// Re-export protocol types
pub use protocol::{
    // Codec traits
    Decodable,
    DecodedMessage,
    Encodable,
    // Frame types
    Frame,
    FrameCodec,
    FrameType,
    // Message types
    messages::{
        // Chat commands
        AddReaction,
        // Control messages
        Auth,
        AuthFailed,
        AuthOk,
        CreateRoom,
        DeleteMessage,
        EditMessage,
        // Errors
        Error as ProtocolError,
        Goodbye,
        Hello,
        HelloAck,
        JoinRoom,
        LeaveRoom,
        // ACKs
        MessageAck,
        MessageDelivered,
        // IDs
        MessageId,
        MessageRead,
        // Constants
        NUM_SHARDS,
        Ping,
        Pong,
        // Presence (datagrams)
        PresenceAway,
        PresenceOffline,
        PresenceOnline,
        RemoveReaction,
        // Room messages
        RoomClose,
        // Shard management
        RoomDemoted,
        RoomId,
        RoomInit,
        RoomMessage,
        RoomMessageDeleted,
        RoomMessageEdited,
        RoomPromoted,
        RoomReactionAdded,
        RoomReactionRemoved,
        RoomUserJoined,
        RoomUserLeft,
        SendMessage,
        ServerCommand,
        ShardAssignment,
        ShardId,
        ShardStreamInfo,
        StopTyping,
        Throttle,
        Typing,
        // Uploads
        UploadAck,
        UploadCancel,
        UploadChunk,
        UploadComplete,
        UploadId,
        UploadStart,
        UserId,
        UserInfo,
        room_shard,
    },
};

// Re-export transport types
pub use transport::{
    // Connection management
    ConnectionBuilder,
    ConnectionCommand,
    ConnectionEvent,
    ManagedConnection,
    // Shard routing
    RoutingAction,
    ShardConfig,
    ShardRouter,
    // Stream management
    StreamConfig,
    StreamHandle,
    StreamSet,
    StreamState,
    StreamStats,
    StreamType,
};

// Re-export server types
pub use server::{
    ConnectionHandler, MultiStreamServer, Room, RoomManager, RoomMember,
    connection_handler::{ConnectionCommand as ServerConnectionCommand, ServerEvent},
    multi_stream_server::{ServerConfig, ServerStats},
    room_manager::{MemberRole, RoomType},
};

use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a unique message ID
pub fn generate_message_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Get current timestamp in milliseconds since UNIX epoch
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config() {
        let config = ServerConfig::default();
        assert_eq!(config.bind_addr.port(), 4433);
        assert_eq!(config.max_connections, 10000);
    }

    #[test]
    fn test_timestamp() {
        let ts = current_timestamp();
        assert!(ts > 0);
    }

    #[test]
    fn test_message_id() {
        let id1 = generate_message_id();
        let id2 = generate_message_id();
        assert_ne!(id1, id2);
        assert!(!id1.is_empty());
    }
}
