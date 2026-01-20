//! Basic Usage Example for Garou Multi-Stream Chat Server
//!
//! This example demonstrates how to start and configure the multi-stream
//! QUIC chat server with its optimized architecture.
//!
//! Run with: cargo run --example basic_usage

use garou::{
    MultiStreamServer, RoomManager, RoomMember, RoomType, ShardConfig, StreamConfig,
    server::multi_stream_server::ServerConfig,
};
use std::time::Duration;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Garou Multi-Stream Chat Server - Basic Usage Example");
    info!("=====================================================");

    // Example 1: Create server with default configuration
    example_default_server().await?;

    // Example 2: Create server with custom configuration
    example_custom_config();

    // Example 3: Room management demonstration
    example_room_management().await;

    // Example 4: Shard configuration explanation
    example_shard_config();

    info!("Examples completed!");
    Ok(())
}

/// Example 1: Start server with default configuration
async fn example_default_server() -> Result<(), Box<dyn std::error::Error>> {
    info!("\n--- Example 1: Default Server Configuration ---");

    let config = ServerConfig::default();

    info!("Default configuration:");
    info!("  Bind address: {}", config.bind_addr);
    info!("  Max connections: {}", config.max_connections);
    info!("  Idle timeout: {:?}", config.idle_timeout);
    info!("  Datagrams enabled: {}", config.enable_datagrams);
    info!("  Number of shards: {}", config.shard_config.num_shards);

    // Create server (but don't start it in this example)
    let server = MultiStreamServer::new(config);
    let stats = server.get_stats().await;

    info!("Server created:");
    info!("  Total connections: {}", stats.total_connections);
    info!("  Total rooms: {}", stats.total_rooms);

    Ok(())
}

/// Example 2: Custom server configuration
fn example_custom_config() {
    info!("\n--- Example 2: Custom Server Configuration ---");

    // Configure streams
    let stream_config = StreamConfig {
        max_concurrent_streams: 200,
        num_shards: 16, // More shards for higher scale
        max_hot_rooms: 20,
        send_buffer_size: 128 * 1024, // 128KB
        recv_buffer_size: 128 * 1024,
        idle_timeout_secs: 600, // 10 minutes
    };

    // Configure shard routing
    let shard_config = ShardConfig {
        num_shards: 16,
        hot_room_threshold: 200,    // Promote at 200 msgs/sec
        cool_down_threshold: 50,    // Demote below 50 msgs/sec
        cool_down_period_secs: 120, // Wait 2 minutes before demoting
        max_hot_rooms: 20,
        rate_window_secs: 10,
    };

    // Create server config
    let config = ServerConfig {
        bind_addr: "0.0.0.0:5000".parse().unwrap(),
        max_connections: 50000,
        stream_config,
        shard_config,
        idle_timeout: Duration::from_secs(600),
        enable_datagrams: true,
    };

    info!("Custom configuration:");
    info!("  Bind address: {}", config.bind_addr);
    info!("  Max connections: {}", config.max_connections);
    info!("  Number of shards: {}", config.shard_config.num_shards);
    info!(
        "  Hot room threshold: {} msgs/sec",
        config.shard_config.hot_room_threshold
    );
    info!(
        "  Cool down threshold: {} msgs/sec",
        config.shard_config.cool_down_threshold
    );
    info!("  Max hot rooms: {}", config.shard_config.max_hot_rooms);
}

/// Example 3: Room management
async fn example_room_management() {
    info!("\n--- Example 3: Room Management ---");

    let room_manager = RoomManager::new();

    // Create a group chat room
    let creator = RoomMember::new(1, "alice".to_string());
    let room = room_manager
        .create_room("Engineering Team".to_string(), RoomType::Group, creator)
        .await;

    info!("Created room:");
    info!("  ID: {}", room.id);
    info!("  Name: {}", room.name);
    info!("  Type: {:?}", room.room_type);

    // Add members
    let bob = RoomMember::new(2, "bob".to_string());
    let charlie = RoomMember::new(3, "charlie".to_string());

    room_manager.join_room(room.id, bob).await;
    room_manager.join_room(room.id, charlie).await;

    info!("Members in room: {}", room.member_count().await);

    // Create a channel
    let creator2 = RoomMember::new(1, "alice".to_string());
    let channel = room_manager
        .create_room("Announcements".to_string(), RoomType::Channel, creator2)
        .await;

    info!("Created channel:");
    info!("  ID: {}", channel.id);
    info!("  Name: {}", channel.name);

    // Create a DM
    let dm = room_manager
        .get_or_create_dm_room(1, "alice".to_string(), 2, "bob".to_string())
        .await;

    info!("Created DM:");
    info!("  ID: {}", dm.id);
    info!("  Type: {:?}", dm.room_type);

    // Stats
    info!("Total rooms: {}", room_manager.room_count().await);
    info!(
        "Alice's rooms: {}",
        room_manager.get_user_room_ids(1).await.len()
    );
}

/// Example 4: Shard configuration explanation
fn example_shard_config() {
    info!("\n--- Example 4: Understanding Shard Configuration ---");

    info!("Shard routing distributes rooms across multiple streams:");
    info!("  - room_id % num_shards = shard_id");
    info!("  - This prevents head-of-line blocking");
    info!("  - Hot rooms get dedicated streams");
    info!("");

    let config = ShardConfig::default();

    info!("Default shard configuration:");
    info!(
        "  num_shards: {} (rooms distributed across 8 streams)",
        config.num_shards
    );
    info!(
        "  hot_room_threshold: {} msgs/sec (promote to dedicated stream)",
        config.hot_room_threshold
    );
    info!(
        "  cool_down_threshold: {} msgs/sec (demote back to shard)",
        config.cool_down_threshold
    );
    info!(
        "  cool_down_period: {} seconds (wait before demoting)",
        config.cool_down_period_secs
    );
    info!(
        "  max_hot_rooms: {} (limit dedicated streams)",
        config.max_hot_rooms
    );

    // Show shard distribution
    info!("");
    info!("Example shard distribution (8 shards):");
    for room_id in 0..16u64 {
        let shard = room_id % config.num_shards as u64;
        info!("  Room {} -> Shard {}", room_id, shard);
    }
}

/// To actually run the server, uncomment and use this function
#[allow(dead_code)]
async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig::default();
    let mut server = MultiStreamServer::new(config);

    info!("Starting server...");
    info!("Press Ctrl+C to stop");

    // Handle shutdown signal
    let shutdown = tokio::signal::ctrl_c();

    tokio::select! {
        result = server.start() => {
            if let Err(e) = result {
                warn!("Server error: {}", e);
            }
        }
        _ = shutdown => {
            info!("Shutdown signal received");
            server.shutdown().await?;
        }
    }

    Ok(())
}
