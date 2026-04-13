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
│   │   └── mod.rs               TelemetryGuard, init_telemetry(), Metrics (OTel instruments: counters, histograms, gauges)
│   │
│   ├── health/
│   │   └── mod.rs               HealthDeps, spawn_health_server() (ntex: /health/live, /health/ready)
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
futures-core = "0.3"  # poll_fn stream drain (avoids futures-util macro conflict)

# Redis (NOTE: fred was replaced — fred crate is abandoned; redis-rs is the maintained alternative)
redis = { version = "0.27", features = ["tokio-comp", "connection-manager"] }

# FlatBuffers
flatbuffers = "24"
# flatc binary downloaded to /tmp at build time (no sudo required)

# OpenTelemetry (replaces metrics + metrics-exporter-prometheus)
opentelemetry = "0.31.0"
opentelemetry_sdk = { version = "0.31.0", features = ["rt-tokio"] }
opentelemetry-otlp = { version = "0.31.1", features = ["grpc-tonic", "metrics", "logs"] }
opentelemetry-semantic-conventions = "0.31.0"
opentelemetry-appender-tracing = "0.31.1"
tracing-opentelemetry = "0.32.1"
tracing-subscriber = { version = "0.3.23", features = ["env-filter"] }

# HTTP health server (NOTE: axum replaced by ntex — lighter weight, consistent with chat server stack)
ntex = { version = "3.7.2", features = ["tokio"] }
tokio = { version = "1.0", features = ["full"] }   # already present; signal feature used for shutdown
```

## Key Design Decisions Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Module organization | Single crate, new submodules | Simpler build, no cross-crate versioning |
| NATS failure mode | Non-fatal (log + continue) | Non-blocking startup; server degrades without NATS |
| FlatBuffers codegen | Build-time via build.rs | Always in sync with schemas; no stale generated code |
| Redis failure mode | Degrade gracefully | Resilience over strict caching; JWT re-validated inline |
| NATS connection model | Single client (async-nats) | async-nats handles internal connection pooling |
| Redis crate | redis = "0.27" (redis-rs) | fred crate was abandoned; redis-rs is actively maintained |
| Metrics export | OTLP push via gRPC | Single pipeline; no Prometheus scrape endpoint needed |
| Health/metrics HTTP server | ntex | Lightweight, consistent with chat server stack; axum dependency dropped |
| Server Arc pattern | Arc<Self> at construction | Fixes BUG-002; no per-connection re-wrapping |
| Message ID | AtomicU64 | Replaces RwLock<u64> for lock-free ID generation |
| Message buffer | VecDeque | O(1) front removal fixes BUG-003 |
| Channels | Bounded mpsc | Backpressure fixes BUG-004 |
| Shutdown architecture | Single ShutdownCoordinator in mod.rs | Simpler than 3-file design; broadcast channel fans out to all tasks |
| JWT validate_async | Separate from validate() | validate() stays sync for tests; caching layer added on top |

## Component Initialization Order

```
1. Config               (no deps)
2. TelemetryGuard       (no deps — sets global OTel providers; held for process lifetime)
3. Metrics              (needs TelemetryGuard — instruments from global meter)
4. Redis                (needs Config; non-fatal if unreachable)
5. NATS                 (needs Config; non-fatal if unreachable)
6. Auth                 (needs Config + optional Redis for JWT cache)
7. RoomManager          (no deps)
8. ShardRouter          (needs Config)
9. MultiStreamServer    (needs Config, Auth, NATS, Redis, RoomManager, ShardRouter)
10. HealthDeps          (needs NATS + Redis handles from MultiStreamServer)
11. spawn_health_server (needs HealthDeps; starts ntex on health_addr)
12. ShutdownCoordinator (needs Config.server.shutdown_timeout_secs)
13. QUIC Endpoint       (needs Config + TLS cert; started inside MultiStreamServer.run())
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
