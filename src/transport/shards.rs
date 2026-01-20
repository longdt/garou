//! Shard management for room-to-stream routing
//!
//! This module handles the distribution of rooms across shard streams
//! and manages hot room promotion/demotion.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use crate::protocol::messages::{NUM_SHARDS, RoomId, ShardId};

/// Configuration for shard management
#[derive(Debug, Clone)]
pub struct ShardConfig {
    /// Number of shards (default: 8)
    pub num_shards: u8,

    /// Messages per second threshold to promote a room to hot status
    pub hot_room_threshold: u32,

    /// Messages per second threshold to demote a hot room back to shard
    pub cool_down_threshold: u32,

    /// How long to wait before demoting a hot room (seconds)
    pub cool_down_period_secs: u64,

    /// Maximum number of hot rooms per connection
    pub max_hot_rooms: usize,

    /// Window size for rate calculation (seconds)
    pub rate_window_secs: u64,
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            num_shards: NUM_SHARDS,
            hot_room_threshold: 100,   // 100 msg/sec = hot
            cool_down_threshold: 20,   // Below 20 msg/sec = demote
            cool_down_period_secs: 60, // Wait 1 minute before demoting
            max_hot_rooms: 10,
            rate_window_secs: 10,
        }
    }
}

/// Statistics for a single room
#[derive(Debug)]
pub struct RoomStats {
    /// Room ID
    pub room_id: RoomId,

    /// Total messages processed
    pub message_count: AtomicU64,

    /// Messages in the current rate window
    pub window_message_count: AtomicU64,

    /// When the current rate window started
    pub window_start: RwLock<Instant>,

    /// Calculated message rate (msgs/sec)
    pub message_rate: AtomicU64, // Stored as rate * 100 for precision

    /// When the room became hot (if applicable)
    pub hot_since: RwLock<Option<Instant>>,

    /// When the room cooled down (for demotion timing)
    pub cool_since: RwLock<Option<Instant>>,
}

impl RoomStats {
    pub fn new(room_id: RoomId) -> Self {
        Self {
            room_id,
            message_count: AtomicU64::new(0),
            window_message_count: AtomicU64::new(0),
            window_start: RwLock::new(Instant::now()),
            message_rate: AtomicU64::new(0),
            hot_since: RwLock::new(None),
            cool_since: RwLock::new(None),
        }
    }

    /// Record a message and update rate
    pub async fn record_message(&self, window_secs: u64) {
        self.message_count.fetch_add(1, Ordering::Relaxed);
        self.window_message_count.fetch_add(1, Ordering::Relaxed);

        // Check if we need to rotate the window
        let should_rotate = {
            let window_start = self.window_start.read().await;
            window_start.elapsed() >= Duration::from_secs(window_secs)
        };

        if should_rotate {
            self.rotate_window(window_secs).await;
        }
    }

    /// Rotate the rate window and calculate new rate
    async fn rotate_window(&self, window_secs: u64) {
        let mut window_start = self.window_start.write().await;
        let elapsed = window_start.elapsed();

        if elapsed >= Duration::from_secs(window_secs) {
            let count = self.window_message_count.swap(0, Ordering::Relaxed);
            let elapsed_secs = elapsed.as_secs_f64();

            // Calculate rate (msgs/sec * 100 for precision)
            let rate = if elapsed_secs > 0.0 {
                ((count as f64 / elapsed_secs) * 100.0) as u64
            } else {
                0
            };

            self.message_rate.store(rate, Ordering::Relaxed);
            *window_start = Instant::now();
        }
    }

    /// Get the current message rate (msgs/sec)
    pub fn get_rate(&self) -> f64 {
        self.message_rate.load(Ordering::Relaxed) as f64 / 100.0
    }

    /// Get total message count
    pub fn total_messages(&self) -> u64 {
        self.message_count.load(Ordering::Relaxed)
    }

    /// Check if room is currently marked as hot
    pub async fn is_hot(&self) -> bool {
        self.hot_since.read().await.is_some()
    }

    /// Mark room as hot
    pub async fn mark_hot(&self) {
        let mut hot_since = self.hot_since.write().await;
        if hot_since.is_none() {
            *hot_since = Some(Instant::now());
        }
        // Clear cool_since when becoming hot
        let mut cool_since = self.cool_since.write().await;
        *cool_since = None;
    }

    /// Mark room as cooling down
    pub async fn mark_cooling(&self) {
        let mut cool_since = self.cool_since.write().await;
        if cool_since.is_none() {
            *cool_since = Some(Instant::now());
        }
    }

    /// Clear hot status
    pub async fn clear_hot(&self) {
        let mut hot_since = self.hot_since.write().await;
        *hot_since = None;
        let mut cool_since = self.cool_since.write().await;
        *cool_since = None;
    }

    /// Check if room has been cool long enough to demote
    pub async fn can_demote(&self, cool_down_period: Duration) -> bool {
        let cool_since = self.cool_since.read().await;
        if let Some(since) = *cool_since {
            since.elapsed() >= cool_down_period
        } else {
            false
        }
    }
}

/// Shard information
#[derive(Debug)]
pub struct Shard {
    /// Shard ID
    pub id: ShardId,

    /// Rooms assigned to this shard
    pub rooms: RwLock<HashSet<RoomId>>,

    /// Total messages routed through this shard
    pub message_count: AtomicU64,
}

impl Shard {
    pub fn new(id: ShardId) -> Self {
        Self {
            id,
            rooms: RwLock::new(HashSet::new()),
            message_count: AtomicU64::new(0),
        }
    }

    /// Add a room to this shard
    pub async fn add_room(&self, room_id: RoomId) {
        let mut rooms = self.rooms.write().await;
        rooms.insert(room_id);
    }

    /// Remove a room from this shard
    pub async fn remove_room(&self, room_id: RoomId) {
        let mut rooms = self.rooms.write().await;
        rooms.remove(&room_id);
    }

    /// Check if a room is in this shard
    pub async fn has_room(&self, room_id: RoomId) -> bool {
        let rooms = self.rooms.read().await;
        rooms.contains(&room_id)
    }

    /// Get the number of rooms in this shard
    pub async fn room_count(&self) -> usize {
        let rooms = self.rooms.read().await;
        rooms.len()
    }

    /// Record a message routed through this shard
    pub fn record_message(&self) {
        self.message_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total messages
    pub fn total_messages(&self) -> u64 {
        self.message_count.load(Ordering::Relaxed)
    }
}

/// Actions that can result from routing decisions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingAction {
    /// Route to regular shard
    RouteToShard(ShardId),

    /// Route to hot room dedicated stream
    RouteToHotRoom(RoomId),

    /// Promote room to hot status (create dedicated stream)
    PromoteRoom(RoomId, ShardId),

    /// Demote room back to shard (close dedicated stream)
    DemoteRoom(RoomId, ShardId),
}

/// Shard router for managing room-to-stream routing
#[derive(Debug)]
pub struct ShardRouter {
    /// Configuration
    config: ShardConfig,

    /// Shards
    shards: Vec<Arc<Shard>>,

    /// Room statistics
    room_stats: RwLock<HashMap<RoomId, Arc<RoomStats>>>,

    /// Currently hot rooms (room_id -> original shard)
    hot_rooms: RwLock<HashMap<RoomId, ShardId>>,
}

impl ShardRouter {
    /// Create a new shard router with the given configuration
    pub fn new(config: ShardConfig) -> Self {
        let shards = (0..config.num_shards)
            .map(|id| Arc::new(Shard::new(id)))
            .collect();

        Self {
            config,
            shards,
            room_stats: RwLock::new(HashMap::new()),
            hot_rooms: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new shard router with default configuration
    pub fn with_defaults() -> Self {
        Self::new(ShardConfig::default())
    }

    /// Calculate which shard a room belongs to
    #[inline]
    pub fn shard_for_room(&self, room_id: RoomId) -> ShardId {
        (room_id % self.config.num_shards as u64) as ShardId
    }

    /// Get a shard by ID
    pub fn get_shard(&self, shard_id: ShardId) -> Option<Arc<Shard>> {
        self.shards.get(shard_id as usize).cloned()
    }

    /// Register a room with the router
    pub async fn register_room(&self, room_id: RoomId) {
        let shard_id = self.shard_for_room(room_id);

        // Add to shard
        if let Some(shard) = self.get_shard(shard_id) {
            shard.add_room(room_id).await;
        }

        // Create room stats
        let mut stats = self.room_stats.write().await;
        if !stats.contains_key(&room_id) {
            stats.insert(room_id, Arc::new(RoomStats::new(room_id)));
        }
    }

    /// Unregister a room from the router
    pub async fn unregister_room(&self, room_id: RoomId) {
        let shard_id = self.shard_for_room(room_id);

        // Remove from shard
        if let Some(shard) = self.get_shard(shard_id) {
            shard.remove_room(room_id).await;
        }

        // Remove from hot rooms if applicable
        let mut hot_rooms = self.hot_rooms.write().await;
        hot_rooms.remove(&room_id);

        // Remove stats
        let mut stats = self.room_stats.write().await;
        stats.remove(&room_id);
    }

    /// Route a message and get the routing action
    pub async fn route_message(&self, room_id: RoomId) -> RoutingAction {
        // Record the message
        let stats = {
            let stats_map = self.room_stats.read().await;
            stats_map.get(&room_id).cloned()
        };

        if let Some(stats) = stats {
            stats.record_message(self.config.rate_window_secs).await;
        }

        // Check if room is hot
        let hot_rooms = self.hot_rooms.read().await;
        if hot_rooms.contains_key(&room_id) {
            return RoutingAction::RouteToHotRoom(room_id);
        }
        drop(hot_rooms);

        // Route to shard
        let shard_id = self.shard_for_room(room_id);

        // Record message in shard
        if let Some(shard) = self.get_shard(shard_id) {
            shard.record_message();
        }

        RoutingAction::RouteToShard(shard_id)
    }

    /// Check if a room should be promoted to hot status
    pub async fn should_promote(&self, room_id: RoomId) -> bool {
        // Check if we can have more hot rooms
        let hot_rooms = self.hot_rooms.read().await;
        if hot_rooms.len() >= self.config.max_hot_rooms {
            return false;
        }
        if hot_rooms.contains_key(&room_id) {
            return false;
        }
        drop(hot_rooms);

        // Check room rate
        let stats = {
            let stats_map = self.room_stats.read().await;
            stats_map.get(&room_id).cloned()
        };

        if let Some(stats) = stats {
            let rate = stats.get_rate();
            rate >= self.config.hot_room_threshold as f64
        } else {
            false
        }
    }

    /// Promote a room to hot status
    pub async fn promote_room(&self, room_id: RoomId) -> Option<RoutingAction> {
        if !self.should_promote(room_id).await {
            return None;
        }

        let shard_id = self.shard_for_room(room_id);

        // Mark as hot
        let mut hot_rooms = self.hot_rooms.write().await;
        hot_rooms.insert(room_id, shard_id);

        // Update room stats
        let stats = {
            let stats_map = self.room_stats.read().await;
            stats_map.get(&room_id).cloned()
        };

        if let Some(stats) = stats {
            stats.mark_hot().await;
        }

        Some(RoutingAction::PromoteRoom(room_id, shard_id))
    }

    /// Check if a room should be demoted back to its shard
    pub async fn should_demote(&self, room_id: RoomId) -> bool {
        let hot_rooms = self.hot_rooms.read().await;
        if !hot_rooms.contains_key(&room_id) {
            return false;
        }
        drop(hot_rooms);

        let stats = {
            let stats_map = self.room_stats.read().await;
            stats_map.get(&room_id).cloned()
        };

        if let Some(stats) = stats {
            let rate = stats.get_rate();

            if rate < self.config.cool_down_threshold as f64 {
                // Mark as cooling if not already
                stats.mark_cooling().await;

                // Check if cool long enough
                let cool_down_period = Duration::from_secs(self.config.cool_down_period_secs);
                stats.can_demote(cool_down_period).await
            } else {
                // Rate is back up, clear cooling status
                let mut cool_since = stats.cool_since.write().await;
                *cool_since = None;
                false
            }
        } else {
            false
        }
    }

    /// Demote a room back to its shard
    pub async fn demote_room(&self, room_id: RoomId) -> Option<RoutingAction> {
        let mut hot_rooms = self.hot_rooms.write().await;
        let original_shard = hot_rooms.remove(&room_id)?;

        // Clear hot status
        let stats = {
            let stats_map = self.room_stats.read().await;
            stats_map.get(&room_id).cloned()
        };

        if let Some(stats) = stats {
            stats.clear_hot().await;
        }

        Some(RoutingAction::DemoteRoom(room_id, original_shard))
    }

    /// Check all hot rooms for potential demotion
    pub async fn check_demotions(&self) -> Vec<RoutingAction> {
        let room_ids: Vec<RoomId> = {
            let hot_rooms = self.hot_rooms.read().await;
            hot_rooms.keys().copied().collect()
        };

        let mut actions = Vec::new();

        for room_id in room_ids {
            if self.should_demote(room_id).await {
                if let Some(action) = self.demote_room(room_id).await {
                    actions.push(action);
                }
            }
        }

        actions
    }

    /// Get the number of shards
    pub fn num_shards(&self) -> u8 {
        self.config.num_shards
    }

    /// Get the number of hot rooms
    pub async fn num_hot_rooms(&self) -> usize {
        let hot_rooms = self.hot_rooms.read().await;
        hot_rooms.len()
    }

    /// Check if a room is currently hot
    pub async fn is_room_hot(&self, room_id: RoomId) -> bool {
        let hot_rooms = self.hot_rooms.read().await;
        hot_rooms.contains_key(&room_id)
    }

    /// Get all hot rooms and their original shards
    pub async fn get_hot_rooms(&self) -> HashMap<RoomId, ShardId> {
        let hot_rooms = self.hot_rooms.read().await;
        hot_rooms.clone()
    }

    /// Get room statistics
    pub async fn get_room_stats(&self, room_id: RoomId) -> Option<Arc<RoomStats>> {
        let stats = self.room_stats.read().await;
        stats.get(&room_id).cloned()
    }

    /// Get statistics for all shards
    pub async fn get_shard_stats(&self) -> Vec<(ShardId, usize, u64)> {
        let mut stats = Vec::with_capacity(self.shards.len());

        for shard in &self.shards {
            let room_count = shard.room_count().await;
            let message_count = shard.total_messages();
            stats.push((shard.id, room_count, message_count));
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shard_routing() {
        let router = ShardRouter::with_defaults();

        // Test shard calculation
        assert_eq!(router.shard_for_room(0), 0);
        assert_eq!(router.shard_for_room(1), 1);
        assert_eq!(router.shard_for_room(8), 0);
        assert_eq!(router.shard_for_room(15), 7);
    }

    #[tokio::test]
    async fn test_room_registration() {
        let router = ShardRouter::with_defaults();

        router.register_room(100).await;

        let shard_id = router.shard_for_room(100);
        let shard = router.get_shard(shard_id).unwrap();

        assert!(shard.has_room(100).await);
        assert_eq!(shard.room_count().await, 1);
    }

    #[tokio::test]
    async fn test_message_routing() {
        let router = ShardRouter::with_defaults();

        router.register_room(100).await;

        let action = router.route_message(100).await;
        let expected_shard = router.shard_for_room(100);

        assert_eq!(action, RoutingAction::RouteToShard(expected_shard));
    }

    #[tokio::test]
    async fn test_hot_room_promotion() {
        let config = ShardConfig {
            hot_room_threshold: 2, // Low threshold for testing
            ..Default::default()
        };
        let router = ShardRouter::new(config);

        router.register_room(100).await;

        // Send enough messages to trigger promotion check
        for _ in 0..10 {
            router.route_message(100).await;
        }

        // Force rate calculation by getting stats
        if let Some(stats) = router.get_room_stats(100).await {
            // Manually set a high rate for testing
            stats.message_rate.store(300, Ordering::Relaxed); // 3.0 msgs/sec * 100
        }

        assert!(router.should_promote(100).await);

        let action = router.promote_room(100).await;
        assert!(action.is_some());

        // Now routing should go to hot room
        let action = router.route_message(100).await;
        assert_eq!(action, RoutingAction::RouteToHotRoom(100));
    }

    #[tokio::test]
    async fn test_shard_stats() {
        let router = ShardRouter::with_defaults();

        router.register_room(0).await;
        router.register_room(8).await; // Same shard as room 0
        router.register_room(1).await;

        let stats = router.get_shard_stats().await;

        // Shard 0 should have 2 rooms
        assert_eq!(stats[0].1, 2);
        // Shard 1 should have 1 room
        assert_eq!(stats[1].1, 1);
    }
}
