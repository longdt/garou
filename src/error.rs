//! Error handling for the chat server

use std::fmt;

/// Result type alias for chat operations
pub type Result<T> = std::result::Result<T, ChatError>;

/// Chat server error types
#[derive(Debug, Clone)]
pub enum ChatError {
    /// Network-related errors
    Network(String),
    /// Serialization/deserialization errors
    Serialization(String),
    /// Authentication errors
    Auth(String),
    /// Protocol errors
    Protocol(String),
    /// Connection errors
    Connection(String),
    /// Invalid message format
    InvalidMessage(String),
    /// User not found
    UserNotFound(String),
    /// Room not found
    RoomNotFound(String),
    /// Permission denied
    PermissionDenied(String),
    /// Server internal error
    Internal(String),
    /// Configuration error
    Config(String),
    /// Timeout error
    Timeout(String),
    /// Resource limit exceeded
    ResourceLimit(String),
}

impl ChatError {
    /// Get error code for this error type
    pub fn code(&self) -> u32 {
        match self {
            ChatError::Network(_) => 1000,
            ChatError::Serialization(_) => 1001,
            ChatError::Auth(_) => 1002,
            ChatError::Protocol(_) => 1003,
            ChatError::Connection(_) => 1004,
            ChatError::InvalidMessage(_) => 1005,
            ChatError::UserNotFound(_) => 1006,
            ChatError::RoomNotFound(_) => 1007,
            ChatError::PermissionDenied(_) => 1008,
            ChatError::Internal(_) => 1009,
            ChatError::Config(_) => 1010,
            ChatError::Timeout(_) => 1011,
            ChatError::ResourceLimit(_) => 1012,
        }
    }

    /// Get human-readable error message
    pub fn message(&self) -> &str {
        match self {
            ChatError::Network(msg) => msg,
            ChatError::Serialization(msg) => msg,
            ChatError::Auth(msg) => msg,
            ChatError::Protocol(msg) => msg,
            ChatError::Connection(msg) => msg,
            ChatError::InvalidMessage(msg) => msg,
            ChatError::UserNotFound(msg) => msg,
            ChatError::RoomNotFound(msg) => msg,
            ChatError::PermissionDenied(msg) => msg,
            ChatError::Internal(msg) => msg,
            ChatError::Config(msg) => msg,
            ChatError::Timeout(msg) => msg,
            ChatError::ResourceLimit(msg) => msg,
        }
    }

    /// Create a network error
    pub fn network<T: Into<String>>(msg: T) -> Self {
        ChatError::Network(msg.into())
    }

    /// Create a serialization error
    pub fn serialization<T: Into<String>>(msg: T) -> Self {
        ChatError::Serialization(msg.into())
    }

    /// Create an authentication error
    pub fn auth<T: Into<String>>(msg: T) -> Self {
        ChatError::Auth(msg.into())
    }

    /// Create a protocol error
    pub fn protocol<T: Into<String>>(msg: T) -> Self {
        ChatError::Protocol(msg.into())
    }

    /// Create a connection error
    pub fn connection<T: Into<String>>(msg: T) -> Self {
        ChatError::Connection(msg.into())
    }

    /// Create an invalid message error
    pub fn invalid_message<T: Into<String>>(msg: T) -> Self {
        ChatError::InvalidMessage(msg.into())
    }

    /// Create a user not found error
    pub fn user_not_found<T: Into<String>>(msg: T) -> Self {
        ChatError::UserNotFound(msg.into())
    }

    /// Create a room not found error
    pub fn room_not_found<T: Into<String>>(msg: T) -> Self {
        ChatError::RoomNotFound(msg.into())
    }

    /// Create a permission denied error
    pub fn permission_denied<T: Into<String>>(msg: T) -> Self {
        ChatError::PermissionDenied(msg.into())
    }

    /// Create an internal error
    pub fn internal<T: Into<String>>(msg: T) -> Self {
        ChatError::Internal(msg.into())
    }

    /// Create a configuration error
    pub fn config<T: Into<String>>(msg: T) -> Self {
        ChatError::Config(msg.into())
    }

    /// Create a timeout error
    pub fn timeout<T: Into<String>>(msg: T) -> Self {
        ChatError::Timeout(msg.into())
    }

    /// Create a resource limit error
    pub fn resource_limit<T: Into<String>>(msg: T) -> Self {
        ChatError::ResourceLimit(msg.into())
    }
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatError::Network(msg) => write!(f, "Network error: {}", msg),
            ChatError::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            ChatError::Auth(msg) => write!(f, "Authentication error: {}", msg),
            ChatError::Protocol(msg) => write!(f, "Protocol error: {}", msg),
            ChatError::Connection(msg) => write!(f, "Connection error: {}", msg),
            ChatError::InvalidMessage(msg) => write!(f, "Invalid message: {}", msg),
            ChatError::UserNotFound(msg) => write!(f, "User not found: {}", msg),
            ChatError::RoomNotFound(msg) => write!(f, "Room not found: {}", msg),
            ChatError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            ChatError::Internal(msg) => write!(f, "Internal error: {}", msg),
            ChatError::Config(msg) => write!(f, "Configuration error: {}", msg),
            ChatError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            ChatError::ResourceLimit(msg) => write!(f, "Resource limit exceeded: {}", msg),
        }
    }
}

impl std::error::Error for ChatError {}

impl From<std::io::Error> for ChatError {
    fn from(err: std::io::Error) -> Self {
        ChatError::Network(format!("IO error: {}", err))
    }
}

impl From<quinn::ConnectError> for ChatError {
    fn from(err: quinn::ConnectError) -> Self {
        ChatError::Connection(format!("QUIC connection error: {}", err))
    }
}

impl From<quinn::ConnectionError> for ChatError {
    fn from(err: quinn::ConnectionError) -> Self {
        ChatError::Connection(format!("QUIC connection error: {}", err))
    }
}

impl From<quinn::ReadError> for ChatError {
    fn from(err: quinn::ReadError) -> Self {
        ChatError::Network(format!("QUIC read error: {}", err))
    }
}

impl From<quinn::WriteError> for ChatError {
    fn from(err: quinn::WriteError) -> Self {
        ChatError::Network(format!("QUIC write error: {}", err))
    }
}

impl From<quinn::ReadToEndError> for ChatError {
    fn from(err: quinn::ReadToEndError) -> Self {
        ChatError::Network(format!("QUIC read to end error: {}", err))
    }
}

impl From<serde_json::Error> for ChatError {
    fn from(err: serde_json::Error) -> Self {
        ChatError::Serialization(format!("JSON error: {}", err))
    }
}

impl From<uuid::Error> for ChatError {
    fn from(err: uuid::Error) -> Self {
        ChatError::Internal(format!("UUID error: {}", err))
    }
}

impl From<anyhow::Error> for ChatError {
    fn from(err: anyhow::Error) -> Self {
        ChatError::Internal(format!("Anyhow error: {}", err))
    }
}

impl From<quinn::ClosedStream> for ChatError {
    fn from(err: quinn::ClosedStream) -> Self {
        ChatError::Connection(format!("Stream closed: {}", err))
    }
}
