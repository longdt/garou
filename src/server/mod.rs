//! Multi-stream QUIC chat server implementation
//!
//! This module provides a production-grade chat server using the QUIC protocol
//! with optimized multi-stream architecture for ultra-low-latency messaging.
//!
//! ## Stream Layout
//!
//! - **Control Stream** (bidirectional): Auth, ping/pong, commands
//! - **Chat Commands Stream** (client→server uni): Messages, reactions, edits
//! - **ACK Stream** (client→server uni): Delivery/read receipts
//! - **Shard Streams** (server→client uni): Room messages grouped by shard
//! - **Hot Room Streams** (server→client uni): Dedicated streams for high-traffic rooms
//! - **Datagrams**: Typing indicators, presence (unreliable)

pub mod connection_handler;
pub mod multi_stream_server;
pub mod room_manager;

pub use connection_handler::ConnectionHandler;
pub use multi_stream_server::MultiStreamServer;
pub use room_manager::{Room, RoomManager, RoomMember};

// Re-export the legacy ChatServer for backward compatibility
pub use super::server_legacy::ChatServer;
