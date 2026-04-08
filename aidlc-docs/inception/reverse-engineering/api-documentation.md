# Garou - Protocol & API Documentation

## Transport Layer

**Protocol**: QUIC (RFC 9000) via Quinn 0.11
**TLS**: rustls 0.23 with ring provider
**ALPN**: `"chat"`
**Default Port**: 4433

## Frame Protocol

All messages are sent as binary frames:

```
+--------+------------------+------------------+
| type   | length           | payload          |
| 1 byte | 4 bytes (big-end)| variable         |
+--------+------------------+------------------+
```

- Max frame size: 16 MB
- Header size: 5 bytes

## Stream Layout

| Stream | Direction | Purpose | Initiator |
|--------|-----------|---------|-----------|
| Control | Bidirectional | Auth, Ping/Pong, Commands, Throttle | Client |
| ChatCommands | Client→Server | Send/Edit/Delete messages, reactions, join/leave rooms | Client |
| BulkUpload | Client→Server | File/image/voice uploads | Client |
| ACKs | Client→Server | Delivery and read receipts | Client |
| Shard[0-7] | Server→Client | Room messages grouped by shard | Server |
| HotRoom[id] | Server→Client | Dedicated stream for high-traffic room | Server |
| Datagrams | Unreliable | Typing indicators, presence | Both |

## Connection Handshake

```
Client                              Server
  |--- Hello{version:1} ----------->|
  |<-- HelloAck{session_id, ...} ---|
  |--- Auth{method, credentials} -->|
  |<-- AuthOk{user_id, username} ---|  (or AuthFailed)
  |<-- ShardAssignment{shards} -----|  Server opens shard streams
  |   (connection established)      |
```

## Frame Type Reference

### Control Stream (0x00-0x0F)
| Frame | Code | Direction | Description |
|-------|------|-----------|-------------|
| Hello | 0x00 | C→S | Initial handshake, protocol version |
| HelloAck | 0x01 | S→C | Server acknowledges hello |
| Auth | 0x02 | C→S | Authentication credentials |
| AuthOk | 0x03 | S→C | Auth successful, user info |
| AuthFailed | 0x04 | S→C | Auth failed, error code |
| Ping | 0x05 | Both | Keep-alive ping |
| Pong | 0x06 | Both | Keep-alive response |
| Throttle | 0x07 | S→C | Rate limit notification |
| Goodbye | 0x08 | Both | Graceful disconnect |
| ServerCommand | 0x09 | S→C | Server-side command |

### Chat Commands (0x10-0x2F)
| Frame | Code | Description |
|-------|------|-------------|
| SendMessage | 0x10 | Send a text message to a room |
| EditMessage | 0x11 | Edit an existing message |
| DeleteMessage | 0x12 | Delete a message |
| AddReaction | 0x13 | Add emoji reaction to message |
| RemoveReaction | 0x14 | Remove emoji reaction |
| JoinRoom | 0x15 | Join a chat room |
| LeaveRoom | 0x16 | Leave a chat room |
| CreateRoom | 0x17 | Create a new room |

### Room Messages (0x30-0x4F)
| Frame | Code | Description |
|-------|------|-------------|
| RoomMessage | 0x30 | New message in a room |
| RoomMessageEdited | 0x31 | Message was edited |
| RoomMessageDeleted | 0x32 | Message was deleted |
| RoomReactionAdded | 0x33 | Reaction added to message |
| RoomReactionRemoved | 0x34 | Reaction removed from message |
| RoomUserJoined | 0x35 | User joined the room |
| RoomUserLeft | 0x36 | User left the room |
| RoomInit | 0x37 | Room state on join (members + recent messages) |
| RoomClose | 0x38 | Room closed or user removed |

### Shard Management (0x50-0x5F)
| Frame | Code | Description |
|-------|------|-------------|
| ShardAssignment | 0x50 | Server assigns shards to client |
| RoomPromoted | 0x51 | Room moved to dedicated hot stream |
| RoomDemoted | 0x52 | Room moved back to shard stream |

### ACK Messages (0x60-0x6F)
| Frame | Code | Description |
|-------|------|-------------|
| MessageDelivered | 0x60 | Message delivered to recipient |
| MessageRead | 0x61 | Message read by recipient |
| MessageAck | 0x62 | Server confirms message received |

### Uploads (0x70-0x7F)
| Frame | Code | Description |
|-------|------|-------------|
| UploadStart | 0x70 | Begin file upload |
| UploadChunk | 0x71 | Upload data chunk |
| UploadComplete | 0x72 | Upload finished |
| UploadCancel | 0x73 | Cancel upload |
| UploadAck | 0x74 | Server ACK for upload |

### Datagrams (0x80-0x8F)
| Frame | Code | Description |
|-------|------|-------------|
| Typing | 0x80 | User is typing |
| StopTyping | 0x81 | User stopped typing |
| PresenceOnline | 0x82 | User came online |
| PresenceOffline | 0x83 | User went offline |
| PresenceAway | 0x84 | User is away |

## Room Shard Assignment

Rooms are assigned to shards using: `shard_id = room_id % 8`

Hot rooms (>100 msg/sec for 10s window) get promoted to dedicated streams.
Cool rooms (<20 msg/sec for 60s) get demoted back to shard streams.

## Server Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| bind_addr | 127.0.0.1:4433 | Bind address |
| max_connections | 10,000 | Maximum concurrent connections |
| idle_timeout | 300s | Connection idle timeout |
| enable_datagrams | true | Enable QUIC datagrams |
| num_shards | 8 | Number of shard streams |
| hot_room_threshold | 100 msg/s | Threshold for hot room promotion |
| cool_down_threshold | 20 msg/s | Threshold for demotion |
| cool_down_period | 60s | Time before demotion |
| max_hot_rooms | 10 | Max hot rooms per connection |
