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
//! use garou::{MultiStreamServer, server::ServerConfig};
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
pub mod transport;

// Server modules
pub mod server;
pub mod server_legacy;

// Client module (legacy, for backward compatibility)
pub mod client;
pub mod simple_test;

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

// Re-export new server types
pub use server::{
    ConnectionHandler, MultiStreamServer, Room, RoomManager, RoomMember,
    connection_handler::{ConnectionCommand as ServerConnectionCommand, ServerEvent},
    multi_stream_server::{ServerConfig, ServerStats},
    room_manager::{MemberRole, RoomType},
};

// Re-export legacy types (for backward compatibility)
pub use client::{ChatClient, ChatClientConfig};
pub use server_legacy::ChatServer;

use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Chat server configuration (legacy, for backward compatibility)
#[derive(Clone, Debug)]
pub struct ChatConfig {
    /// Server listen address
    pub bind_addr: std::net::SocketAddr,
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Connection idle timeout in seconds
    pub idle_timeout_secs: u64,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Number of shards for room distribution
    pub num_shards: u8,
    /// Stream configuration
    pub stream_config: StreamConfig,
    /// Shard configuration
    pub shard_config: ShardConfig,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:4433".parse().unwrap(),
            max_connections: 1000,
            idle_timeout_secs: 300,
            max_message_size: 1024 * 1024, // 1MB
            num_shards: NUM_SHARDS,
            stream_config: StreamConfig::default(),
            shard_config: ShardConfig::default(),
        }
    }
}

/// Generate a unique message ID
pub fn generate_message_id() -> String {
    Uuid::new_v4().to_string()
}

/// Get current timestamp in milliseconds since UNIX epoch
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// User information (legacy, for backward compatibility)
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub joined_at: u64,
}

impl User {
    pub fn new(username: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            username,
            joined_at: current_timestamp(),
        }
    }

    /// Convert to protocol UserInfo
    pub fn to_user_info(&self) -> UserInfo {
        UserInfo {
            user_id: self.id.parse().unwrap_or(0),
            username: self.username.clone(),
            avatar_url: None,
        }
    }
}

/// Chat message types (legacy, for backward compatibility)
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ChatMessageType {
    Text { content: String },
    Join { user: User },
    Leave { user_id: String, username: String },
    UserList { users: Vec<User> },
    Error { code: u32, message: String },
}

/// Chat message (legacy, for backward compatibility)
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub sender: Option<User>,
    pub message_type: ChatMessageType,
    pub timestamp: u64,
}

impl ChatMessage {
    pub fn new_text(sender: User, content: String) -> Self {
        Self {
            id: generate_message_id(),
            sender: Some(sender),
            message_type: ChatMessageType::Text { content },
            timestamp: current_timestamp(),
        }
    }

    pub fn new_join(user: User) -> Self {
        Self {
            id: generate_message_id(),
            sender: None,
            message_type: ChatMessageType::Join { user },
            timestamp: current_timestamp(),
        }
    }

    pub fn new_leave(user_id: String, username: String) -> Self {
        Self {
            id: generate_message_id(),
            sender: None,
            message_type: ChatMessageType::Leave { user_id, username },
            timestamp: current_timestamp(),
        }
    }

    pub fn new_user_list(users: Vec<User>) -> Self {
        Self {
            id: generate_message_id(),
            sender: None,
            message_type: ChatMessageType::UserList { users },
            timestamp: current_timestamp(),
        }
    }

    pub fn new_error(code: u32, message: String) -> Self {
        Self {
            id: generate_message_id(),
            sender: None,
            message_type: ChatMessageType::Error { code, message },
            timestamp: current_timestamp(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = ChatConfig::default();
        assert_eq!(config.bind_addr.port(), 4433);
        assert_eq!(config.num_shards, NUM_SHARDS);
    }

    #[test]
    fn test_user_creation() {
        let user = User::new("alice".to_string());
        assert_eq!(user.username, "alice");
        assert!(!user.id.is_empty());
    }

    #[test]
    fn test_message_creation() {
        let user = User::new("bob".to_string());
        let msg = ChatMessage::new_text(user.clone(), "Hello!".to_string());

        assert!(msg.sender.is_some());
        assert_eq!(msg.sender.unwrap().username, "bob");

        match msg.message_type {
            ChatMessageType::Text { content } => assert_eq!(content, "Hello!"),
            _ => panic!("Expected Text message type"),
        }
    }

    #[test]
    fn test_server_config() {
        let config = ServerConfig::default();
        assert_eq!(config.bind_addr.port(), 4433);
        assert_eq!(config.max_connections, 10000);
    }
}
