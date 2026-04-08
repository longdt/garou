# Application Design — Consolidated View

## Architecture Summary

Garou is a single Rust crate refactored into 11 components across 9 module namespaces. The server is stateless at the application layer — all durable state is owned by NATS JetStream (messages) and NATS KV (room/user state). Redis provides fast-path caching. Chat pods can be scaled horizontally as Kubernetes Deployments.

## Module Structure

```
garou/
├── src/
│   ├── main.rs                  Bootstrap service: init + signal handling
│   ├── lib.rs                   Public re-exports
│   ├── error.rs                 ChatError enum (extended)
│   │
│   ├── config/
│   │   └── mod.rs               Config, ServerConfig, AuthConfig, NatsConfig,
│   │                            RedisConfig, MetricsConfig, ShardConfigSettings,
│   │                            ProtocolConfig
│   │
│   ├── auth/
│   │   └── mod.rs               AuthValidator, AuthClaims
│   │
│   ├── storage/
│   │   ├── mod.rs               Storage module
│   │   ├── nats.rs              NatsClient, RoomState
│   │   └── redis.rs             RedisClient
│   │
│   ├── metrics/
│   │   └── mod.rs               init_metrics(), increment_*/record_*/set_* helpers
│   │
│   ├── health/
│   │   └── mod.rs               HealthServer (Axum: /health/live, /health/ready, /metrics)
│   │
│   ├── protocol/
│   │   ├── mod.rs               Protocol module
│   │   ├── frame.rs             FrameType, Frame, FrameCodec (unchanged)
│   │   ├── codec.rs             Encodable, Decodable traits (updated for FlatBuffers)
│   │   ├── messages.rs          Type aliases, constants, serde types (legacy/debug)
│   │   └── generated/           FlatBuffers generated code (build.rs output)
│   │
│   ├── transport/
│   │   ├── mod.rs               Transport module
│   │   ├── streams.rs           StreamType, StreamState, StreamStats, StreamHandle, StreamSet
│   │   ├── shards.rs            ShardConfig, ShardRouter, RoomStats
│   │   └── connection.rs        ConnectionBuilder, ManagedConnection
│   │
│   └── server/
│       ├── mod.rs               Server module
│       ├── multi_stream_server.rs  MultiStreamServer, ServerConfig, ServerStats
│       ├── connection_handler.rs   ConnectionHandler, ServerEvent, ConnectionCommand
│       └── room_manager.rs         RoomManager, Room (VecDeque fix), RoomMember, RoomType
│
├── fbs/                         FlatBuffers schema files
│   ├── control.fbs              Hello, HelloAck, Auth, AuthOk, AuthFailed, Ping, Pong, ...
│   ├── chat.fbs                 SendMessage, EditMessage, DeleteMessage, ...
│   ├── room.fbs                 RoomMessage, RoomInit, RoomUserJoined, ...
│   ├── shard.fbs                ShardAssignment, RoomPromoted, RoomDemoted
│   ├── ack.fbs                  MessageDelivered, MessageRead, MessageAck
│   ├── upload.fbs               UploadStart, UploadChunk, ...
│   └── presence.fbs             Typing, StopTyping, PresenceOnline, ...
│
├── deploy/                      Kubernetes deployment artifacts
│   ├── deployment.yaml
│   ├── service-quic.yaml
│   ├── service-metrics.yaml
│   ├── configmap.yaml
│   ├── secret.yaml
│   ├── hpa.yaml
│   └── servicemonitor.yaml
│
├── build.rs                     FlatBuffers codegen (calls flatc)
├── Cargo.toml                   Dependencies (updated)
├── config.toml.example          Documented example configuration
└── Dockerfile                   Multi-stage container image
```

## New Dependencies (Cargo.toml additions)

```toml
# Configuration
toml = "0.8"
serde = { version = "1.0", features = ["derive"] }  # already present

# Authentication
jsonwebtoken = "9.3"

# NATS
async-nats = "0.35"

# Redis
fred = { version = "9", features = ["tokio-runtime", "pool"] }

# FlatBuffers
flatbuffers = "24"
# build dependency:
# flatc is a system binary (installed in Dockerfile)

# Metrics
metrics = "0.23"
metrics-exporter-prometheus = "0.15"

# HTTP (health + metrics server)
axum = { version = "0.7", features = ["tokio"] }
tokio = { version = "1.0", features = ["full"] }   # already present

# Signal handling
tokio-util = { version = "0.7", features = ["rt"] }

# Cancellation
tokio-util = { version = "0.7", features = ["sync"] }  # CancellationToken
```

## Key Design Decisions Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Module organization | Single crate, new submodules | Simpler build, no cross-crate versioning |
| NATS failure mode | Fail fast | Simpler, client retries; no data loss risk |
| FlatBuffers codegen | Build-time via build.rs | Always in sync with schemas; no stale generated code |
| Redis failure mode | Degrade gracefully | Resilience over strict caching; JWT re-validated inline |
| NATS connection model | Pool (configurable size) | Higher throughput under concurrent publishes |
| Server Arc pattern | Arc<Self> at construction | Fixes BUG-002; no per-connection re-wrapping |
| Message ID | AtomicU64 | Replaces RwLock<u64> for lock-free ID generation |
| Message buffer | VecDeque | O(1) front removal fixes BUG-003 |
| Channels | Bounded mpsc | Backpressure fixes BUG-004 |

## Component Initialization Order

```
1. Config           (no deps)
2. Metrics          (no deps — global registry)
3. Redis            (needs Config)
4. NATS             (needs Config)
5. Auth             (needs Config + Redis)
6. RoomManager      (no deps)
7. ShardRouter      (needs Config)
8. MultiStreamServer(needs all above)
9. HealthServer     (needs NATS + Redis + Metrics)
10. QUIC Endpoint   (needs Config + TLS cert)
```

## Security Baseline Compliance

- JWT validated cryptographically before any command accepted (FR-001)
- No unauthenticated message access
- TLS on all QUIC connections
- No secrets in config.toml.example (template values only)
- K8s Secrets used for JWT key and TLS certs

## Property-Based Testing Targets

- `Protocol`: FlatBuffers encode→decode round-trip for all message types
- `Frame`: `FrameCodec` streaming decode invariant (any byte sequence, no panic)
- `Auth`: JWT claims round-trip (create → sign → validate → extract)
- `RoomManager`: member join/leave/reconnect state consistency
- `ShardRouter`: `room_shard(room_id)` always in `[0, NUM_SHARDS)` range
