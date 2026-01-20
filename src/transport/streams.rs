//! Stream type definitions and management for QUIC transport
//!
//! This module defines the different stream types used in the chat protocol
//! and provides utilities for managing stream lifecycles.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::protocol::messages::ShardId;

/// Stream type identifiers for the chat protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamType {
    /// Bidirectional control stream for auth, ping/pong, commands
    Control,

    /// Client -> Server: Chat commands (messages, reactions, edits)
    ChatCommands,

    /// Client -> Server: Bulk uploads (files, images, voice)
    BulkUpload,

    /// Client -> Server: ACKs (delivered, read receipts)
    Acks,

    /// Server -> Client: Shard stream for room messages
    Shard(ShardId),

    /// Server -> Client: Dedicated stream for a hot room
    HotRoom(u64),
}

impl StreamType {
    /// Check if this stream type is client-initiated
    pub fn is_client_initiated(&self) -> bool {
        matches!(
            self,
            StreamType::Control
                | StreamType::ChatCommands
                | StreamType::BulkUpload
                | StreamType::Acks
        )
    }

    /// Check if this stream type is server-initiated
    pub fn is_server_initiated(&self) -> bool {
        matches!(self, StreamType::Shard(_) | StreamType::HotRoom(_))
    }

    /// Check if this stream type is bidirectional
    pub fn is_bidirectional(&self) -> bool {
        matches!(self, StreamType::Control)
    }

    /// Get the priority level (higher = more important)
    /// Used for QUIC stream prioritization
    pub fn priority(&self) -> u8 {
        match self {
            StreamType::Control => 255,      // Highest priority
            StreamType::Shard(_) => 200,     // Room messages
            StreamType::HotRoom(_) => 200,   // Same as shard
            StreamType::ChatCommands => 150, // User sending messages
            StreamType::Acks => 100,         // Receipts
            StreamType::BulkUpload => 50,    // Lowest - uploads shouldn't block chat
        }
    }
}

/// Stream state tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// Stream is being initialized
    Initializing,
    /// Stream is open and active
    Open,
    /// Stream is half-closed (one direction finished)
    HalfClosed,
    /// Stream is fully closed
    Closed,
    /// Stream encountered an error
    Error,
}

/// Statistics for a stream
#[derive(Debug, Default)]
pub struct StreamStats {
    /// Bytes sent on this stream
    pub bytes_sent: AtomicU64,
    /// Bytes received on this stream
    pub bytes_received: AtomicU64,
    /// Frames sent
    pub frames_sent: AtomicU64,
    /// Frames received
    pub frames_received: AtomicU64,
}

impl StreamStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_send(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
        self.frames_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_recv(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
        self.frames_received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent.load(Ordering::Relaxed)
    }

    pub fn bytes_received(&self) -> u64 {
        self.bytes_received.load(Ordering::Relaxed)
    }

    pub fn frames_sent(&self) -> u64 {
        self.frames_sent.load(Ordering::Relaxed)
    }

    pub fn frames_received(&self) -> u64 {
        self.frames_received.load(Ordering::Relaxed)
    }
}

impl Clone for StreamStats {
    fn clone(&self) -> Self {
        Self {
            bytes_sent: AtomicU64::new(self.bytes_sent.load(Ordering::Relaxed)),
            bytes_received: AtomicU64::new(self.bytes_received.load(Ordering::Relaxed)),
            frames_sent: AtomicU64::new(self.frames_sent.load(Ordering::Relaxed)),
            frames_received: AtomicU64::new(self.frames_received.load(Ordering::Relaxed)),
        }
    }
}

/// Handle for a managed stream
#[derive(Debug, Clone)]
pub struct StreamHandle {
    /// Stream type
    pub stream_type: StreamType,
    /// Current state
    pub state: StreamState,
    /// Stream statistics
    pub stats: Arc<StreamStats>,
    /// Creation timestamp
    pub created_at: u64,
}

impl StreamHandle {
    pub fn new(stream_type: StreamType, created_at: u64) -> Self {
        Self {
            stream_type,
            state: StreamState::Initializing,
            stats: Arc::new(StreamStats::new()),
            created_at,
        }
    }

    pub fn set_state(&mut self, state: StreamState) {
        self.state = state;
    }

    pub fn is_open(&self) -> bool {
        self.state == StreamState::Open
    }

    pub fn is_closed(&self) -> bool {
        matches!(self.state, StreamState::Closed | StreamState::Error)
    }
}

/// Configuration for stream management
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Maximum number of concurrent streams per connection
    pub max_concurrent_streams: u32,

    /// Maximum number of shard streams (typically 8 or 16)
    pub num_shards: u8,

    /// Maximum hot room streams that can be active
    pub max_hot_rooms: u32,

    /// Buffer size for stream send queues
    pub send_buffer_size: usize,

    /// Buffer size for stream receive queues
    pub recv_buffer_size: usize,

    /// Idle timeout for streams in seconds
    pub idle_timeout_secs: u64,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            max_concurrent_streams: 100,
            num_shards: 8,
            max_hot_rooms: 10,
            send_buffer_size: 64 * 1024, // 64KB
            recv_buffer_size: 64 * 1024, // 64KB
            idle_timeout_secs: 300,      // 5 minutes
        }
    }
}

/// Stream set for tracking active streams in a connection
#[derive(Debug)]
pub struct StreamSet {
    /// Control stream (always present after handshake)
    pub control: Option<StreamHandle>,

    /// Chat commands stream (client -> server)
    pub chat_commands: Option<StreamHandle>,

    /// Bulk upload stream (client -> server, on-demand)
    pub bulk_upload: Option<StreamHandle>,

    /// ACK stream (client -> server)
    pub acks: Option<StreamHandle>,

    /// Shard streams (server -> client)
    /// Index is shard ID
    pub shards: Vec<Option<StreamHandle>>,

    /// Hot room streams (server -> client)
    /// Key is room ID
    pub hot_rooms: std::collections::HashMap<u64, StreamHandle>,

    /// Configuration
    config: StreamConfig,
}

impl StreamSet {
    pub fn new(config: StreamConfig) -> Self {
        let num_shards = config.num_shards as usize;
        Self {
            control: None,
            chat_commands: None,
            bulk_upload: None,
            acks: None,
            shards: vec![None; num_shards],
            hot_rooms: std::collections::HashMap::new(),
            config,
        }
    }

    /// Get the total number of active streams
    pub fn active_count(&self) -> usize {
        let mut count = 0;
        if self.control.as_ref().map_or(false, |s| s.is_open()) {
            count += 1;
        }
        if self.chat_commands.as_ref().map_or(false, |s| s.is_open()) {
            count += 1;
        }
        if self.bulk_upload.as_ref().map_or(false, |s| s.is_open()) {
            count += 1;
        }
        if self.acks.as_ref().map_or(false, |s| s.is_open()) {
            count += 1;
        }
        count += self
            .shards
            .iter()
            .filter(|s| s.as_ref().map_or(false, |s| s.is_open()))
            .count();
        count += self.hot_rooms.values().filter(|s| s.is_open()).count();
        count
    }

    /// Check if we can open more streams
    pub fn can_open_stream(&self) -> bool {
        self.active_count() < self.config.max_concurrent_streams as usize
    }

    /// Check if we can promote a room to hot status
    pub fn can_promote_room(&self) -> bool {
        self.hot_rooms.len() < self.config.max_hot_rooms as usize
    }

    /// Get the shard ID for a room
    pub fn get_shard_for_room(&self, room_id: u64) -> ShardId {
        (room_id % self.config.num_shards as u64) as ShardId
    }

    /// Check if a room is currently hot (has dedicated stream)
    pub fn is_room_hot(&self, room_id: u64) -> bool {
        self.hot_rooms.contains_key(&room_id)
    }

    /// Get aggregate statistics for all streams
    pub fn aggregate_stats(&self) -> StreamStats {
        let stats = StreamStats::new();

        let mut add_stats = |handle: &Option<StreamHandle>| {
            if let Some(h) = handle {
                stats
                    .bytes_sent
                    .fetch_add(h.stats.bytes_sent(), Ordering::Relaxed);
                stats
                    .bytes_received
                    .fetch_add(h.stats.bytes_received(), Ordering::Relaxed);
                stats
                    .frames_sent
                    .fetch_add(h.stats.frames_sent(), Ordering::Relaxed);
                stats
                    .frames_received
                    .fetch_add(h.stats.frames_received(), Ordering::Relaxed);
            }
        };

        add_stats(&self.control);
        add_stats(&self.chat_commands);
        add_stats(&self.bulk_upload);
        add_stats(&self.acks);

        for shard in &self.shards {
            add_stats(shard);
        }

        for handle in self.hot_rooms.values() {
            stats
                .bytes_sent
                .fetch_add(handle.stats.bytes_sent(), Ordering::Relaxed);
            stats
                .bytes_received
                .fetch_add(handle.stats.bytes_received(), Ordering::Relaxed);
            stats
                .frames_sent
                .fetch_add(handle.stats.frames_sent(), Ordering::Relaxed);
            stats
                .frames_received
                .fetch_add(handle.stats.frames_received(), Ordering::Relaxed);
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_type_properties() {
        assert!(StreamType::Control.is_client_initiated());
        assert!(StreamType::Control.is_bidirectional());
        assert!(!StreamType::ChatCommands.is_bidirectional());

        assert!(StreamType::Shard(0).is_server_initiated());
        assert!(StreamType::HotRoom(123).is_server_initiated());

        // Priority ordering
        assert!(StreamType::Control.priority() > StreamType::ChatCommands.priority());
        assert!(StreamType::ChatCommands.priority() > StreamType::BulkUpload.priority());
    }

    #[test]
    fn test_stream_stats() {
        let stats = StreamStats::new();

        stats.record_send(100);
        stats.record_send(50);
        stats.record_recv(200);

        assert_eq!(stats.bytes_sent(), 150);
        assert_eq!(stats.bytes_received(), 200);
        assert_eq!(stats.frames_sent(), 2);
        assert_eq!(stats.frames_received(), 1);
    }

    #[test]
    fn test_stream_set() {
        let config = StreamConfig::default();
        let mut set = StreamSet::new(config);

        assert_eq!(set.active_count(), 0);
        assert!(set.can_open_stream());

        // Add control stream
        let mut control = StreamHandle::new(StreamType::Control, 0);
        control.set_state(StreamState::Open);
        set.control = Some(control);

        assert_eq!(set.active_count(), 1);

        // Test shard calculation
        assert_eq!(set.get_shard_for_room(0), 0);
        assert_eq!(set.get_shard_for_room(8), 0);
        assert_eq!(set.get_shard_for_room(1), 1);
    }

    #[test]
    fn test_stream_handle() {
        let mut handle = StreamHandle::new(StreamType::ChatCommands, 12345);

        assert_eq!(handle.state, StreamState::Initializing);
        assert!(!handle.is_open());
        assert!(!handle.is_closed());

        handle.set_state(StreamState::Open);
        assert!(handle.is_open());

        handle.set_state(StreamState::Closed);
        assert!(handle.is_closed());
    }
}
