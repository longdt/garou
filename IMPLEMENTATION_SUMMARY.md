# QUIC Chat Server Implementation Summary

## Overview

This project implements a high-performance chat server using the QUIC protocol for communication. The implementation demonstrates modern Rust async programming patterns, network protocol handling, and real-time message distribution with an optimized multi-stream architecture for ultra-low-latency messaging.

## Architecture

### Multi-Stream QUIC Architecture

The server uses a sophisticated stream layout to eliminate head-of-line blocking:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              QUIC Connection                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ Control Stream (Bidirectional)                                       │   │
│  │ • Auth handshake (Hello → HelloAck → Auth → AuthOk)                 │   │
│  │ • Ping/Pong keepalive                                                │   │
│  │ • Shard assignment                                                   │   │
│  │ • Hot room promotion/demotion                                        │   │
│  │ • Error messages                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Client → Server (Unidirectional)                                          │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐              │
│  │ Chat Commands   │ │ ACKs            │ │ Bulk Upload     │              │
│  │ • SendMessage   │ │ • Delivered     │ │ • Files         │              │
│  │ • EditMessage   │ │ • Read          │ │ • Images        │              │
│  │ • DeleteMessage │ │                 │ │ • Voice         │              │
│  │ • Reactions     │ │                 │ │                 │              │
│  │ • Join/Leave    │ │                 │ │                 │              │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘              │
│                                                                             │
│  Server → Client (Unidirectional)                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ Shard Streams (8 by default)                                        │   │
│  │ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐       │   │
│  │ │Shard 0│ │Shard 1│ │Shard 2│ │Shard 3│ │Shard 4│ │Shard 5│ ...   │   │
│  │ │Room 0 │ │Room 1 │ │Room 2 │ │Room 3 │ │Room 4 │ │Room 5 │       │   │
│  │ │Room 8 │ │Room 9 │ │Room 10│ │Room 11│ │Room 12│ │Room 13│       │   │
│  │ └───────┘ └───────┘ └───────┘ └───────┘ └───────┘ └───────┘       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ Hot Room Streams (on-demand)                                        │   │
│  │ • Dedicated stream per high-traffic room                            │   │
│  │ • Auto-promotion at 100 msgs/sec                                    │   │
│  │ • Auto-demotion below 20 msgs/sec after cooldown                    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ Datagrams (Unreliable)                                              │   │
│  │ • Typing indicators                                                  │   │
│  │ • Presence updates (online/offline/away)                            │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Core Components

1. **MultiStreamServer** (`src/server/multi_stream_server.rs`)
   - Production-grade QUIC-based server using Quinn library
   - Handles multiple concurrent client connections
   - Room management with member tracking
   - Message routing through shard or hot room streams
   - Automatic hot room promotion/demotion

2. **ConnectionHandler** (`src/server/connection_handler.rs`)
   - Per-connection state management
   - Protocol handshake (Hello/HelloAck/Auth/AuthOk)
   - Multi-stream lifecycle management
   - Frame encoding/decoding per stream type
   - Datagram handling for presence

3. **RoomManager** (`src/server/room_manager.rs`)
   - Server-side room state management
   - Member tracking with roles (Owner, Admin, Moderator, Member, Guest)
   - Message history (circular buffer)
   - Room types: Direct, Group, Channel

4. **ShardRouter** (`src/transport/shards.rs`)
   - Room-to-shard mapping (room_id % num_shards)
   - Message rate tracking per room
   - Hot room detection and promotion logic
   - Cooldown-based demotion

5. **Protocol Layer** (`src/protocol/`)
   - Binary frame format (1 byte type + 4 bytes length + payload)
   - Typed message definitions for all protocol messages
   - Encodable/Decodable traits with serde_json (swappable to protobuf)

### Frame Types

| Range | Category | Frame Types |
|-------|----------|-------------|
| 0x00-0x0F | Control | Hello, HelloAck, Auth, AuthOk, AuthFailed, Ping, Pong, Throttle, Goodbye, ServerCommand |
| 0x10-0x2F | Chat Commands | SendMessage, EditMessage, DeleteMessage, AddReaction, RemoveReaction, JoinRoom, LeaveRoom, CreateRoom |
| 0x30-0x4F | Room Messages | RoomMessage, RoomMessageEdited, RoomMessageDeleted, RoomReactionAdded, RoomReactionRemoved, RoomUserJoined, RoomUserLeft, RoomInit, RoomClose |
| 0x50-0x5F | Shard Management | ShardAssignment, RoomPromoted, RoomDemoted |
| 0x60-0x6F | ACKs | MessageDelivered, MessageRead, MessageAck |
| 0x70-0x7F | Uploads | UploadStart, UploadChunk, UploadComplete, UploadCancel, UploadAck |
| 0x80-0x8F | Datagrams | Typing, StopTyping, PresenceOnline, PresenceOffline, PresenceAway |
| 0xFF | Error | Error |

## Key Features Implemented

### 1. Multi-Stream Protocol
- Separate streams for different message types eliminate head-of-line blocking
- Priority-based stream handling (Control > Shard > ChatCommands > ACKs > BulkUpload)
- Automatic shard assignment based on room ID

### 2. Hot Room Optimization
- Rooms with >100 msgs/sec get dedicated streams
- Reduces latency for viral/active rooms
- Automatic demotion after 60s cooldown below 20 msgs/sec

### 3. Datagram Support
- Typing indicators use unreliable datagrams
- Presence updates (online/offline/away) are datagrams
- No blocking of reliable streams for transient data

### 4. Room Management
- Full room lifecycle (create, join, leave, delete)
- Role-based permissions
- Message history with configurable buffer size
- Direct messages, group chats, and channels

### 5. Protocol Handshake
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

## Code Structure

```
garou/
├── src/
│   ├── lib.rs                 # Library exports and types
│   ├── main.rs                # CLI interface
│   ├── error.rs               # Error handling
│   ├── server/
│   │   ├── mod.rs             # Server module exports
│   │   ├── multi_stream_server.rs  # Multi-stream server
│   │   ├── connection_handler.rs   # Per-connection handler
│   │   └── room_manager.rs    # Room state management
│   ├── protocol/
│   │   ├── mod.rs             # Protocol exports
│   │   ├── frame.rs           # Binary frame format
│   │   ├── messages.rs        # Typed message definitions
│   │   └── codec.rs           # Encode/decode traits
│   └── transport/
│       ├── mod.rs             # Transport exports
│       ├── connection.rs      # ManagedConnection
│       ├── streams.rs         # Stream types and management
│       └── shards.rs          # Shard routing logic
├── examples/
│   └── basic_usage.rs         # Usage examples
├── tests/                     # Integration tests
├── README.md                  # User documentation
└── Cargo.toml                 # Dependencies
```

## Dependencies

### Core Libraries
- **quinn**: QUIC protocol implementation
- **rustls**: TLS cryptography
- **tokio**: Async runtime
- **serde/serde_json**: Serialization
- **uuid**: Unique identifiers
- **bytes**: Buffer management
- **tracing**: Logging and diagnostics

### Development Dependencies
- **rcgen**: Certificate generation
- **anyhow**: Error handling utilities

## Usage

### Starting the Multi-Stream Server
```bash
cargo run -- server-ms
```

### Starting the Legacy Server
```bash
cargo run -- server
```

### Connecting a Client (Legacy)
```bash
cargo run -- client Alice
```

### Running Tests
```bash
cargo test
```

## Configuration

### Server Configuration
```rust
ServerConfig {
    bind_addr: "127.0.0.1:4433",
    max_connections: 10000,
    idle_timeout: Duration::from_secs(300),
    enable_datagrams: true,
    stream_config: StreamConfig {
        max_concurrent_streams: 100,
        num_shards: 8,
        max_hot_rooms: 10,
        send_buffer_size: 64 * 1024,
        recv_buffer_size: 64 * 1024,
    },
    shard_config: ShardConfig {
        num_shards: 8,
        hot_room_threshold: 100,      // msgs/sec to promote
        cool_down_threshold: 20,      // msgs/sec to demote
        cool_down_period_secs: 60,
        max_hot_rooms: 10,
    },
}
```

## Performance Characteristics

- QUIC provides better performance than TCP in many scenarios
- 0-RTT connection establishment after initial handshake
- Multiple streams per connection reduce head-of-line blocking
- Shard-based room distribution limits per-stream message rate
- Datagrams avoid blocking for transient presence data

## Security Considerations

### Current Implementation (Development)
- Self-signed certificates
- Simple username-based authentication
- TLS encryption for all communication

### Production Recommendations
- CA-signed certificates
- Token/JWT-based authentication
- Rate limiting and abuse prevention
- Message persistence layer
- Horizontal scaling with state externalization

## Future Enhancements

### Short Term
- Multi-stream client implementation
- Replace serde_json with Protobuf/FlatBuffers
- Implement proper token-based authentication
- Add Prometheus metrics

### Long Term
- Horizontal scaling with edge nodes
- Message persistence layer
- Voice/video chat capabilities
- Mobile client SDKs

## Test Results

All 40 tests pass:
- Protocol frame encoding/decoding
- Message serialization
- Shard routing
- Hot room promotion/demotion
- Room management
- Stream statistics
- Connection building
- Server configuration

## License

MIT