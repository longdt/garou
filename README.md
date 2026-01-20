# Garou - High-Performance QUIC Chat Server

A production-grade chat server implementation using the QUIC protocol with optimized multi-stream architecture for ultra-low-latency messaging.

## Features

- **Multi-Stream Architecture**: Separate QUIC streams for different message types eliminate head-of-line blocking
- **Shard-Based Room Distribution**: Rooms are distributed across configurable shards for scalability
- **Hot Room Optimization**: High-traffic rooms automatically get dedicated streams
- **Datagram Support**: Typing indicators and presence use unreliable datagrams for real-time updates
- **Role-Based Permissions**: Owner, Admin, Moderator, Member, Guest roles per room
- **Room Types**: Direct messages, group chats, and public channels

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              QUIC Connection                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Control Stream (Bidirectional)                                             │
│  • Auth handshake (Hello → HelloAck → Auth → AuthOk)                       │
│  • Ping/Pong keepalive                                                      │
│  • Shard assignment, Hot room promotion/demotion                            │
│                                                                             │
│  Client → Server (Unidirectional)                                          │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐              │
│  │ Chat Commands   │ │ ACKs            │ │ Bulk Upload     │              │
│  │ • SendMessage   │ │ • Delivered     │ │ • Files         │              │
│  │ • Reactions     │ │ • Read          │ │ • Images        │              │
│  │ • Join/Leave    │ │                 │ │                 │              │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘              │
│                                                                             │
│  Server → Client (Unidirectional)                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ Shard Streams (8 by default) - Room messages grouped by shard       │   │
│  │ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐       │   │
│  │ │Shard 0│ │Shard 1│ │Shard 2│ │Shard 3│ │Shard 4│ │Shard 5│ ...   │   │
│  │ └───────┘ └───────┘ └───────┘ └───────┘ └───────┘ └───────┘       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Hot Room Streams (on-demand) - Dedicated streams for high-traffic rooms   │
│                                                                             │
│  Datagrams (Unreliable) - Typing indicators, presence updates              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Quick Start

### Starting the Server

```bash
# Default configuration (port 4433)
cargo run -- server

# Custom port
cargo run -- server --port 5000

# With debug logging
RUST_LOG=debug cargo run -- server
```

### Programmatic Usage

```rust
use garou::{MultiStreamServer, ServerConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig {
        bind_addr: "0.0.0.0:4433".parse()?,
        max_connections: 10000,
        idle_timeout: Duration::from_secs(300),
        enable_datagrams: true,
        ..Default::default()
    };

    let mut server = MultiStreamServer::new(config);
    server.start().await?;
    Ok(())
}
```

## Configuration

### Server Configuration

```rust
ServerConfig {
    bind_addr: "0.0.0.0:4433",     // Listen address
    max_connections: 10000,         // Maximum concurrent connections
    idle_timeout: Duration::from_secs(300),  // Connection idle timeout
    enable_datagrams: true,         // Enable typing/presence datagrams
    stream_config: StreamConfig { ... },
    shard_config: ShardConfig { ... },
}
```

### Stream Configuration

```rust
StreamConfig {
    max_concurrent_streams: 100,    // Max streams per connection
    num_shards: 8,                  // Number of shard streams
    max_hot_rooms: 10,              // Max dedicated hot room streams
    send_buffer_size: 65536,        // 64KB send buffer
    recv_buffer_size: 65536,        // 64KB receive buffer
    idle_timeout_secs: 300,         // Stream idle timeout
}
```

### Shard Configuration

```rust
ShardConfig {
    num_shards: 8,                  // Number of shards
    hot_room_threshold: 100,        // msgs/sec to promote to hot
    cool_down_threshold: 20,        // msgs/sec to demote from hot
    cool_down_period_secs: 60,      // Wait time before demoting
    max_hot_rooms: 10,              // Max hot rooms per connection
    rate_window_secs: 10,           // Rate calculation window
}
```

## Protocol

### Frame Format

```
+--------+--------+------------------+
| type   | length | payload          |
| 1 byte | 4 bytes BE | variable     |
+--------+--------+------------------+
```

### Frame Types

| Range | Category | Examples |
|-------|----------|----------|
| 0x00-0x0F | Control | Hello, Auth, Ping/Pong, Throttle |
| 0x10-0x2F | Chat Commands | SendMessage, EditMessage, JoinRoom |
| 0x30-0x4F | Room Messages | RoomMessage, RoomUserJoined |
| 0x50-0x5F | Shard Management | ShardAssignment, RoomPromoted |
| 0x60-0x6F | ACKs | MessageDelivered, MessageRead |
| 0x70-0x7F | Uploads | UploadStart, UploadChunk |
| 0x80-0x8F | Datagrams | Typing, PresenceOnline |
| 0xFF | Error | Error |

### Handshake Flow

```
Client                              Server
   |                                   |
   |-------- Hello (version) --------->|
   |<------- HelloAck (session) -------|
   |-------- Auth (credentials) ------>|
   |<------- AuthOk (user_id) ---------|
   |<------- ShardAssignment ----------|
   |                                   |
   |   [Connection Established]        |
```

## Room Management

### Room Types

- **Direct**: Private 1:1 conversations
- **Group**: Multi-user chat rooms
- **Channel**: Public broadcast channels

### Member Roles

- **Owner**: Full control, can delete room
- **Admin**: Can manage members and settings
- **Moderator**: Can mute/kick members
- **Member**: Standard permissions
- **Guest**: Read-only access

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_server_config
```

## Examples

```bash
# Run the basic usage example
cargo run --example basic_usage
```

## Project Structure

```
src/
├── lib.rs                    # Library exports
├── main.rs                   # CLI entry point
├── error.rs                  # Error types
├── server/
│   ├── mod.rs
│   ├── multi_stream_server.rs  # Main server
│   ├── connection_handler.rs   # Per-connection logic
│   └── room_manager.rs         # Room state
├── protocol/
│   ├── mod.rs
│   ├── frame.rs              # Binary frame format
│   ├── messages.rs           # Message definitions
│   └── codec.rs              # Encode/decode
└── transport/
    ├── mod.rs
    ├── connection.rs         # Connection management
    ├── streams.rs            # Stream types
    └── shards.rs             # Shard routing
```

## Dependencies

- **quinn**: QUIC protocol implementation
- **rustls**: TLS encryption
- **tokio**: Async runtime
- **serde/serde_json**: Serialization
- **bytes**: Buffer management
- **tracing**: Logging

## Performance

- QUIC eliminates TCP head-of-line blocking
- Shard-based distribution limits per-stream message rate
- Hot room optimization for viral content
- Datagrams for transient data (no blocking)

## Security

### Current (Development)
- Self-signed TLS certificates
- Username-based authentication

### Production Recommendations
- CA-signed certificates
- Token/JWT authentication
- Rate limiting
- Message persistence

## Roadmap

- [ ] Multi-stream client implementation
- [ ] Protobuf encoding (replace JSON)
- [ ] Token-based authentication
- [ ] Prometheus metrics
- [ ] Message persistence layer
- [ ] Horizontal scaling

## License

MIT