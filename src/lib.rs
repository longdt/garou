//! QUIC-based chat server with JSON serialization
//!
//! This library provides a high-performance chat server that uses QUIC protocol
//! for communication and JSON for serialization/deserialization.

pub mod client;
pub mod error;
pub mod server;
pub mod simple_test;

pub use client::{ChatClient, ChatClientConfig};
pub use error::{ChatError, Result};
pub use server::ChatServer;

use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

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

/// Chat server configuration
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
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:4433".parse().unwrap(),
            max_connections: 1000,
            idle_timeout_secs: 300,
            max_message_size: 1024 * 1024, // 1MB
        }
    }
}

/// User information
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
}

/// Chat message types
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ChatMessageType {
    Text { content: String },
    Join { user: User },
    Leave { user_id: String, username: String },
    UserList { users: Vec<User> },
    Error { code: u32, message: String },
}

/// Chat message
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
