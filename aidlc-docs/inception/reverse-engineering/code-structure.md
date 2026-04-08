# Garou - Code Structure Analysis

## Module Inventory

### `src/error.rs`
- `ChatError` enum: 13 error variants (Network, Auth, Protocol, Connection, Serialization, InvalidMessage, UserNotFound, RoomNotFound, PermissionDenied, Internal, Config, Timeout, ResourceLimit)
- `Result<T>` type alias
- From implementations for: `io::Error`, `quinn::ConnectError`, `quinn::ConnectionError`, `quinn::ReadError`, `quinn::WriteError`, `quinn::ReadToEndError`, `serde_json::Error`, `uuid::Error`, `anyhow::Error`, `quinn::ClosedStream`

### `src/protocol/frame.rs`
- `FrameType` enum: 33 frame types across 8 categories (control, chat commands, room messages, shard mgmt, ACKs, bulk upload, datagrams, error)
- `Frame` struct: `frame_type: FrameType`, `payload: Bytes`
- Frame encoding: `[type:u8][length:u32 BE][payload:variable]` — 5-byte header
- `FrameCodec` struct: streaming decoder with internal `BytesMut` buffer
- Constants: `FRAME_HEADER_SIZE = 5`, `MAX_FRAME_SIZE = 16MB`

### `src/protocol/messages.rs`
Type aliases:
- `UserId = u64`, `RoomId = u64`, `MessageId = u64`, `UploadId = u64`, `ShardId = u8`
- `NUM_SHARDS = 8`, `room_shard(room_id) = room_id % 8`

Message structs (all serde Serialize/Deserialize):
- Control: `Hello`, `HelloAck`, `Auth`, `AuthOk`, `AuthFailed`, `Ping`, `Pong`, `Throttle`, `Goodbye`, `ServerCommand`
- Chat Commands: `SendMessage`, `EditMessage`, `DeleteMessage`, `AddReaction`, `RemoveReaction`, `JoinRoom`, `LeaveRoom`, `CreateRoom`
- Room Messages: `RoomMessage`, `RoomMessageEdited`, `RoomMessageDeleted`, `RoomReactionAdded`, `RoomReactionRemoved`, `RoomUserJoined`, `RoomUserLeft`, `RoomInit`, `RoomClose`
- Shard Mgmt: `ShardAssignment`, `ShardStreamInfo`, `RoomPromoted`, `RoomDemoted`
- ACKs: `MessageDelivered`, `MessageRead`, `MessageAck`
- Uploads: `UploadStart`, `UploadChunk`, `UploadComplete`, `UploadCancel`, `UploadAck`
- Presence: `Typing`, `StopTyping`, `PresenceOnline`, `PresenceOffline`, `PresenceAway`
- Other: `UserInfo`, `Error`

### `src/protocol/codec.rs`
- `Encodable` trait: `fn encode(&self) -> Result<Bytes>`
- `Decodable` trait: `fn decode(payload: &Bytes) -> Result<Self>`
- `DecodedMessage` enum: wraps all message types after decoding

### `src/transport/streams.rs`
- `StreamType` enum: `Control`, `ChatCommands`, `BulkUpload`, `Acks`, `Shard(ShardId)`, `HotRoom(u64)`
- `StreamState` enum: `Initializing`, `Open`, `HalfClosed`, `Closed`, `Error`
- `StreamStats` struct: atomic counters for bytes_sent/received, frames_sent/received
- `StreamHandle` struct: stream metadata + state + stats
- `StreamSet` struct: manages a set of named streams
- `StreamConfig` struct: buffer sizes, timeouts

### `src/transport/shards.rs`
- `ShardConfig`: num_shards=8, hot_room_threshold=100 msg/sec, cool_down_threshold=20 msg/sec, cool_down_period=60s, max_hot_rooms=10, rate_window=10s
- `RoomStats`: per-room atomic counters for message rate tracking
- `ShardRouter`: manages room→shard assignments, detects hot rooms, promotes/demotes rooms

### `src/server/room_manager.rs`
- `RoomType`: `Direct`, `Group`, `Channel`
- `MemberRole`: `Owner`, `Admin`, `Moderator`, `Member`, `Guest`
- `RoomMember`: user_id, username, avatar_url, joined_at, role, last_activity
- `Room`: id, name, room_type, members (RwLock<HashMap>), recent_messages (RwLock<Vec>), max_recent_messages=100, timestamps, message_count, metadata
- `RoomManager`: rooms (RwLock<HashMap>), user_rooms (RwLock<HashMap>), next_room_id (RwLock), uptime tracking

### `src/server/connection_handler.rs`
- `ServerEvent` enum: events emitted to server (Authenticated, JoinRoom, LeaveRoom, SendMessage, EditMessage, DeleteMessage, AddReaction, RemoveReaction, CreateRoom, Typing, StopTyping, PresenceUpdate, Disconnected)
- `ConnectionCommand` enum: commands received from server (SendRoomMessage, SendRoomInit, SendRoomClose, SendUserJoined, SendUserLeft, SendMessageEdited, SendMessageDeleted, SendReactionAdded, SendReactionRemoved, SendMessageAck, SendTyping, SendStopTyping, SendPresenceOnline, SendPresenceOffline, SendThrottle, PromoteRoom, DemoteRoom, Close)
- `ConnectionHandler`: per-connection state machine, manages streams, protocol handshake, message routing

### `src/server/multi_stream_server.rs`
- `ServerConfig`: bind_addr, max_connections=10000, stream_config, shard_config, idle_timeout=300s, enable_datagrams=true
- `ActiveConnection`: user_id, username, command_tx, remote_addr, connected_at
- `MultiStreamServer`: main server struct, owns endpoint, room_manager, shard_router, connections HashMap, user_connections HashMap, message ID counter
- `ServerStats`: total_connections, authenticated_connections, total_rooms, total_users, bind_address
- `clone_ref()`: creates Arc-wrapped clone — currently wraps WHOLE struct (problematic)

## Known Issues & Technical Debt

### Critical Bugs
1. **Message edit/delete/reaction room_id = 0**: `handle_edit_message`, `handle_delete_message`, `handle_add_reaction`, `handle_remove_reaction` all set `room_id: 0` — incorrect, should look up actual room from message ID
2. **clone_ref() antipattern**: Creates a new `Arc<MultiStreamServer>` wrapping all fields every time a connection is handled — this creates duplicate Arc layers rather than proper shared ownership

### Missing Production Features
3. **Authentication is a stub**: `ConnectionHandler` accepts any auth credentials — no real auth
4. **In-memory only**: No persistence layer — all rooms, messages, user data lost on restart
5. **No rate limiting**: `UnboundedSender` channels everywhere — no backpressure
6. **O(n) message buffer**: `Vec::remove(0)` in `add_message` is O(n) — should use `VecDeque`
7. **No message store**: Message IDs are in-memory atomic counter — resets on restart
8. **Single node**: No clustering support — cannot scale horizontally
9. **Self-signed certificate**: Generated on every startup — not production-suitable
10. **No metrics/observability**: No Prometheus metrics, no health endpoint
11. **No config file**: Only CLI args — no TOML/YAML configuration
12. **No graceful shutdown**: `shutdown()` method exists but isn't called on SIGTERM
13. **Broadcast O(n) with global lock**: `broadcast_to_room` holds two `RwLock` read guards while iterating — contention at scale
14. **Username lookup on every message**: `connections.read()` for username on every SendMessage
15. **JSON serialization**: For production high performance, binary encoding (MessagePack, FlatBuffers, or protobuf) is preferred
