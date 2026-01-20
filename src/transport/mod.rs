//! Transport layer for QUIC stream management
//!
//! This module provides:
//! - Stream type definitions and lifecycle management
//! - Shard routing for room-to-stream distribution
//! - Connection handling with multi-stream support

pub mod connection;
pub mod shards;
pub mod streams;

// Re-export commonly used types
pub use connection::{ConnectionBuilder, ConnectionCommand, ConnectionEvent, ManagedConnection};
pub use shards::{RoutingAction, ShardConfig, ShardRouter};
pub use streams::{StreamConfig, StreamHandle, StreamSet, StreamState, StreamStats, StreamType};
