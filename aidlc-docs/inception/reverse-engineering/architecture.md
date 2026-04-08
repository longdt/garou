# Garou - System Architecture

## Business Overview

Garou is a high-performance QUIC-based chat server written in Rust. It provides real-time messaging using the QUIC transport protocol (via the `quinn` crate) with a multi-stream architecture designed to eliminate head-of-line (HOL) blocking — a fundamental limitation of TCP-based chat systems.

The server handles:
- Real-time chat messaging across multiple rooms
- User presence (online/away/offline)
- Typing indicators (via QUIC datagrams — unreliable but ultra-low-latency)
- Message delivery/read acknowledgements
- Room management (create, join, leave)
- Message operations (send, edit, delete, reactions)
- File/media uploads

## Multi-Stream QUIC Architecture

The core innovation is the segregation of message types across independent QUIC streams to prevent one message type from blocking another:

```
Client                              Server
  |                                   |
  |--- Control Stream (bidir) --------|  Auth, Ping/Pong, Commands
  |--- ChatCommands Stream (-->)------|  Send/Edit/Delete/React/Join/Leave
  |--- BulkUpload Stream (-->)--------|  File/Image/Voice uploads
  |--- ACK Stream (-->)---------------|  Delivery/Read receipts
  |                                   |
  |<-- Shard Stream 0 (server init)---|  Rooms in shard 0 (room_id % 8 == 0)
  |<-- Shard Stream 1 (server init)---|  Rooms in shard 1
  |  ...                              |
  |<-- Shard Stream 7 (server init)---|  Rooms in shard 7
  |<-- HotRoom Stream (server init)---|  Dedicated stream for high-traffic room
  |                                   |
  |<--> Datagrams (unreliable) -------|  Typing indicators, presence
```

**Key Design Rationale:**
- Shard streams group multiple rooms onto one QUIC stream, saving stream overhead
- Hot rooms (>100 msg/sec) get dedicated streams to prevent shard congestion
- Datagrams are used for ephemeral data that can be safely dropped

## Component Architecture

```
garou/
  main.rs               - Entry point: CLI argument parsing, server bootstrap
  lib.rs                - Library crate: re-exports all public APIs
  error.rs              - Error types (ChatError enum with 13 variants)
  
  protocol/
    mod.rs              - Protocol module
    messages.rs         - All message payload types (serde JSON)
    frame.rs            - Binary frame codec (1-byte type + 4-byte length + payload)
    codec.rs            - Encodable/Decodable trait abstractions
  
  transport/
    mod.rs              - Transport module
    streams.rs          - StreamType, StreamState, StreamStats, StreamHandle, StreamSet, StreamConfig
    shards.rs           - ShardConfig, ShardRouter, RoomStats (hot room detection)
    connection.rs       - ConnectionBuilder, ManagedConnection, ConnectionEvent, ConnectionCommand
  
  server/
    mod.rs              - Server module
    multi_stream_server.rs  - MultiStreamServer: main server, connection acceptance, event processing
    connection_handler.rs   - ConnectionHandler: per-connection stream management, protocol state
    room_manager.rs         - RoomManager, Room, RoomMember, RoomType, MemberRole
```

## Data Flow: Message Send

```
Client sends SendMessage frame on ChatCommands stream
  --> ConnectionHandler.handle_chat_command_stream()
      --> Parse frame, emit ServerEvent::SendMessage to server event channel
          --> MultiStreamServer.handle_event()
              --> handle_send_message()
                  --> Generate MessageId (atomic counter)
                  --> Create RoomMessage
                  --> room.add_message() -- add to in-memory history
                  --> shard_router.route_message() -- update rate stats
                  --> Send MessageAck to sender
                  --> broadcast_to_room() -- fan out to all room members
```

## Technology Stack

| Component | Technology | Version |
|-----------|-----------|---------|
| Language | Rust | 2024 edition |
| Async Runtime | Tokio | 1.x (full features) |
| QUIC | Quinn | 0.11 |
| TLS | Rustls + ring | 0.23 |
| Serialization | serde_json | 1.x |
| Logging | tracing + tracing-subscriber | 0.1 |
| Unique IDs | UUID v4 | 1.10 |
| TLS Cert Gen | rcgen | 0.12 |
| Byte buffers | bytes | 1.5 |
| Error handling | anyhow | 1.0 |
