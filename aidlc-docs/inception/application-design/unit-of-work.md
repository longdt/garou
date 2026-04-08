# Units of Work

## Overview

9 units of work, sequenced by dependency. Each unit is a self-contained set of file changes that can be designed, implemented, and tested independently before the next unit begins.

---

## Unit 1: Core Bug Fixes & Foundation

**Type**: Refactoring  
**Risk**: Low  
**Estimated Complexity**: Small

### Scope
Fix all 4 critical bugs identified in reverse engineering. Establishes a clean, correct foundation for all subsequent units.

### Files Changed
| File | Change Type | Description |
|------|------------|-------------|
| `src/server/multi_stream_server.rs` | Modify | Replace `clone_ref()` with `Arc<Self>` at construction; change `next_message_id` from `RwLock<u64>` to `AtomicU64` |
| `src/server/room_manager.rs` | Modify | Replace `Vec<RoomMessage>` with `VecDeque<RoomMessage>` in `Room`; update `add_message()` and `get_recent_messages()` |
| `src/server/connection_handler.rs` | Modify | Replace all `mpsc::unbounded_channel()` with `mpsc::channel(1024)` |
| `src/server/multi_stream_server.rs` | Modify | Fix `handle_edit_message`, `handle_delete_message`, `handle_add_reaction`, `handle_remove_reaction` — remove `room_id: 0` placeholder (add TODO comment for Unit 5 NATS lookup) |

### Acceptance Criteria
- [ ] `clone_ref()` method removed; server created as `Arc<MultiStreamServer>` in `start()`
- [ ] `next_message_id` uses `AtomicU64::fetch_add(1, Ordering::SeqCst)` — no lock contention
- [ ] `Room.recent_messages` is `VecDeque<RoomMessage>` — `push_back` + `pop_front` for O(1) rotation
- [ ] All `unbounded_channel` replaced with `channel(1024)` — 4 channel pairs per connection
- [ ] Edit/delete/reaction handlers return `ChatError::Internal` with TODO message until Unit 5
- [ ] All existing unit tests pass

---

## Unit 2: Configuration Layer

**Type**: New Feature  
**Risk**: Low  
**Estimated Complexity**: Small

### Scope
TOML configuration file loading with CLI overrides. All subsequent units receive `Arc<Config>` rather than hardcoded values.

### Files Changed
| File | Change Type | Description |
|------|------------|-------------|
| `src/config/mod.rs` | New | `Config`, `ServerConfig`, `AuthConfig`, `NatsConfig`, `RedisConfig`, `MetricsConfig`, `ShardConfigSettings`, `ProtocolConfig` structs; `Config::load()`, `Config::validate()` |
| `Cargo.toml` | Modify | Add `toml = "0.8"` dependency |
| `src/main.rs` | Modify | Load config at startup, pass `Arc<Config>` to server; update CLI to accept `--config <path>` |
| `src/server/multi_stream_server.rs` | Modify | Accept `Arc<Config>` instead of `ServerConfig`; derive all settings from config |
| `src/transport/shards.rs` | Modify | Derive `ShardConfig` values from `Arc<Config>` |
| `config.toml.example` | New | Fully documented example config file |

### Acceptance Criteria
- [ ] `cargo run -- --config config.toml server` loads config from file
- [ ] Missing required fields produce clear error messages at startup
- [ ] CLI `--port` overrides `server.bind_addr` port
- [ ] `config.toml.example` has every field documented with description and default
- [ ] Unit test: valid config file parses correctly
- [ ] Unit test: invalid config (bad URL) returns `ChatError::Config`

---

## Unit 3: FlatBuffers Protocol Layer

**Type**: New Feature (Breaking Change)  
**Risk**: High  
**Estimated Complexity**: Large

### Scope
Replace JSON serde serialization with FlatBuffers for all wire protocol messages. Frame format (5-byte header) unchanged — FlatBuffers is the payload.

### Files Changed
| File | Change Type | Description |
|------|------------|-------------|
| `fbs/control.fbs` | New | Schemas: Hello, HelloAck, Auth, AuthOk, AuthFailed, Ping, Pong, Throttle, Goodbye, ServerCommand |
| `fbs/chat.fbs` | New | Schemas: SendMessage, EditMessage, DeleteMessage, AddReaction, RemoveReaction, JoinRoom, LeaveRoom, CreateRoom |
| `fbs/room.fbs` | New | Schemas: RoomMessage, RoomMessageEdited, RoomMessageDeleted, RoomReactionAdded, RoomReactionRemoved, RoomUserJoined, RoomUserLeft, RoomInit, RoomClose |
| `fbs/shard.fbs` | New | Schemas: ShardAssignment, ShardStreamInfo, RoomPromoted, RoomDemoted |
| `fbs/ack.fbs` | New | Schemas: MessageDelivered, MessageRead, MessageAck |
| `fbs/upload.fbs` | New | Schemas: UploadStart, UploadChunk, UploadComplete, UploadCancel, UploadAck |
| `fbs/presence.fbs` | New | Schemas: Typing, StopTyping, PresenceOnline, PresenceOffline, PresenceAway |
| `src/protocol/generated/` | New | Generated Rust code (committed, regenerated via `cargo build`) |
| `build.rs` | New | Invoke `flatc` to generate Rust from all `.fbs` files |
| `src/protocol/codec.rs` | Modify | Update `Encodable`/`Decodable` traits to use FlatBuffers builder/verifier pattern |
| `src/protocol/messages.rs` | Modify | Keep type aliases (`UserId`, `RoomId`, etc.) and constants; mark serde structs as `#[cfg(feature = "debug-json")]` |
| `Cargo.toml` | Modify | Add `flatbuffers = "24"`; add optional `serde_json` behind `debug-json` feature |
| `src/server/connection_handler.rs` | Modify | Use FlatBuffers decode for incoming frames; use FlatBuffers encode for outgoing frames |

### Acceptance Criteria
- [ ] `build.rs` regenerates `src/protocol/generated/` without errors on `cargo build`
- [ ] All 40+ message types have FlatBuffers schemas
- [ ] Property-based test: encode→decode round-trip for every message type with random inputs
- [ ] Frame codec unchanged: existing `test_frame_encode_decode` tests still pass
- [ ] `protocol.debug_json = true` config flag enables JSON fallback (dev mode)
- [ ] Zero-copy decode: `verify_root::<T>()` used (not `get_root_as_*` with copy)

---

## Unit 4: JWT Authentication

**Type**: New Feature  
**Risk**: Medium  
**Estimated Complexity**: Medium

### Scope
Replace the auth stub in `ConnectionHandler` with real JWT validation. No unauthenticated commands accepted after this unit.

### Files Changed
| File | Change Type | Description |
|------|------------|-------------|
| `src/auth/mod.rs` | New | `AuthValidator`, `AuthClaims`; `validate()` with Redis cache + inline fallback |
| `Cargo.toml` | Modify | Add `jsonwebtoken = "9.3"` |
| `src/server/connection_handler.rs` | Modify | Inject `Arc<AuthValidator>`; reject all commands before `AuthOk`; send `AuthFailed` on invalid token |
| `src/server/multi_stream_server.rs` | Modify | Pass `Arc<AuthValidator>` to each `ConnectionHandler` at construction |
| `src/lib.rs` | Modify | Re-export `AuthValidator`, `AuthClaims` |

### Acceptance Criteria
- [ ] Valid JWT (HS256, not expired) → `AuthOk` frame sent, connection proceeds
- [ ] Invalid signature → `AuthFailed{code: 401, reason: "invalid token"}` frame, connection closed
- [ ] Expired JWT → `AuthFailed{code: 401, reason: "token expired"}` frame, connection closed
- [ ] Redis unavailable → auth succeeds via inline validation (log warning)
- [ ] Any chat command before auth → connection closed with `ChatError::Auth`
- [ ] Unit test: HS256 valid/invalid/expired token scenarios
- [ ] Unit test: RS256 valid token (using test key pair)
- [ ] Property-based test: any JWT with wrong signature always fails validation

---

## Unit 5: NATS JetStream Storage

**Type**: New Feature  
**Risk**: High  
**Estimated Complexity**: Large

### Scope
Integrate NATS JetStream for message persistence and cross-node pub/sub. Fixes BUG-001 (room_id lookup via NATS message headers).

### Files Changed
| File | Change Type | Description |
|------|------------|-------------|
| `src/storage/mod.rs` | New | Storage module declaration |
| `src/storage/nats.rs` | New | `NatsClient`, `RoomState`; connection pool, JetStream stream init, publish, subscribe, history replay, KV operations |
| `Cargo.toml` | Modify | Add `async-nats = "0.35"` |
| `src/server/multi_stream_server.rs` | Modify | Inject `Arc<NatsClient>`; persist message before broadcast; subscribe rooms on join; fix edit/delete/reaction room_id via NATS message headers; handle cross-node subscription events |
| `src/lib.rs` | Modify | Re-export `NatsClient`, `RoomState` |

### Acceptance Criteria
- [ ] Message published to NATS JetStream before `MessageAck` sent to client
- [ ] NATS publish failure → `ChatError::Internal` returned to sender; message NOT broadcast
- [ ] Room join → last 50 messages replayed from JetStream
- [ ] Two pods: message sent via Pod 1 delivered to user on Pod 2 within 50ms (integration test)
- [ ] Edit/delete/reaction: `room_id` correctly read from NATS message header (BUG-001 fixed)
- [ ] NATS KV: room state persisted on creation, retrieved on join
- [ ] Integration test: `docker-compose up nats` + two server instances, cross-node delivery verified
- [ ] Unit test: `NatsClient::health_check()` returns false when NATS unreachable

---

## Unit 6: Redis Cache Layer

**Type**: New Feature  
**Risk**: Low  
**Estimated Complexity**: Small

### Scope
Add Redis connection pool for JWT caching, user presence, and room roster. All failures non-fatal.

### Files Changed
| File | Change Type | Description |
|------|------------|-------------|
| `src/storage/redis.rs` | New | `RedisClient` with `fred` pool; JWT cache, presence, roster operations; graceful degradation |
| `Cargo.toml` | Modify | Add `fred = { version = "9", features = ["tokio-runtime", "pool"] }` |
| `src/auth/mod.rs` | Modify | Pass `Arc<RedisClient>` to `AuthValidator`; cache claims after successful validation |
| `src/server/multi_stream_server.rs` | Modify | Update presence on activity; populate roster cache on room join; clean up on disconnect |
| `src/lib.rs` | Modify | Re-export `RedisClient` |

### Acceptance Criteria
- [ ] Second auth with same JWT hits Redis cache (verified by mock/spy)
- [ ] JWT cache entry TTL matches token `exp` claim
- [ ] Redis down → auth succeeds inline; presence/roster operations silently skipped (warn log)
- [ ] User disconnect → presence key deleted, user removed from all roster sets
- [ ] Unit test: cache hit, cache miss, cache expiry scenarios
- [ ] Unit test: all Redis operations return `Ok` when Redis unavailable (graceful degrade)

---

## Unit 7: Observability

**Type**: New Feature  
**Risk**: Low  
**Estimated Complexity**: Medium

### Scope
Prometheus metrics endpoint, structured JSON logging, and HTTP health endpoints.

### Files Changed
| File | Change Type | Description |
|------|------------|-------------|
| `src/metrics/mod.rs` | New | `init_metrics()`, Prometheus handle, all metric definitions and helpers from FR-006 |
| `src/health/mod.rs` | New | `HealthServer` with Axum; `/health/live`, `/health/ready`, `/metrics` routes |
| `Cargo.toml` | Modify | Add `metrics = "0.23"`, `metrics-exporter-prometheus = "0.15"`, `axum = "0.7"`, `tokio-util = "0.7"` |
| `src/main.rs` | Modify | Init metrics; spawn `HealthServer` task; pass `CancellationToken` for shutdown |
| `src/server/multi_stream_server.rs` | Modify | Instrument: `increment_connections_total()` on accept, `set_connections_active()` on change, `record_message_latency_ms()` in `handle_send_message()` |
| `src/server/connection_handler.rs` | Modify | Call `increment_authenticated_connections()` on AuthOk |

### Acceptance Criteria
- [ ] `GET /metrics` returns Prometheus text format with all 7 metrics from FR-006
- [ ] `GET /health/live` always returns HTTP 200
- [ ] `GET /health/ready` returns HTTP 200 when NATS + Redis healthy, HTTP 503 when either down
- [ ] Log output is JSON format: `{"timestamp":"...","level":"INFO","message":"...","fields":{...}}`
- [ ] `garou_message_latency_ms` histogram records latency from frame received to ACK sent
- [ ] Integration test: send 100 messages, verify `garou_messages_total` counter = 100

---

## Unit 8: Graceful Shutdown

**Type**: New Feature  
**Risk**: Low  
**Estimated Complexity**: Small

### Scope
SIGTERM/SIGINT signal handling with configurable drain window. K8s `terminationGracePeriodSeconds` compatible.

### Files Changed
| File | Change Type | Description |
|------|------------|-------------|
| `src/main.rs` | Modify | Install `tokio::signal` handlers; trigger `CancellationToken.cancel()` on signal |
| `src/server/multi_stream_server.rs` | Modify | `shutdown()` uses drain sequence: stop accept → send Goodbye frames → wait drain window → close NATS → close Redis |
| `Cargo.toml` | Modify | `tokio` already has signal feature in `full` |

### Acceptance Criteria
- [ ] SIGTERM → server stops accepting new connections within 100ms
- [ ] Connected clients receive `Goodbye` frame before connection closed
- [ ] Drain window: server waits up to `shutdown_timeout_secs` for in-flight messages to complete
- [ ] NATS connection closed gracefully (pending publishes flushed) before process exits
- [ ] `GET /health/ready` returns 503 immediately after shutdown signal (K8s stops routing)
- [ ] Integration test: `kill -TERM <pid>` → server exits cleanly with code 0

---

## Unit 9: Kubernetes Deployment

**Type**: New Feature  
**Risk**: Low  
**Estimated Complexity**: Medium

### Scope
Production container image and Kubernetes manifests for deploying on K8s with QUIC (UDP), metrics scraping, and autoscaling.

### Files Changed
| File | Change Type | Description |
|------|------------|-------------|
| `Dockerfile` | New | Multi-stage: `rust:1.82-slim` builder (with `flatc`) → `debian:bookworm-slim` runtime |
| `deploy/deployment.yaml` | New | K8s Deployment: liveness (`/health/live`) + readiness (`/health/ready`) probes |
| `deploy/service-quic.yaml` | New | LoadBalancer Service: UDP 4433 (QUIC), protocol: UDP |
| `deploy/service-metrics.yaml` | New | ClusterIP Service: TCP 9090 (metrics/health) |
| `deploy/configmap.yaml` | New | ConfigMap with `config.toml` template |
| `deploy/secret.yaml` | New | Secret template: JWT key, TLS cert (base64 placeholders) |
| `deploy/hpa.yaml` | New | HPA: scale on `garou_connections_active` custom metric via KEDA or Prometheus Adapter |
| `deploy/servicemonitor.yaml` | New | Prometheus ServiceMonitor for kube-prometheus-stack |
| `.dockerignore` | New | Exclude `target/`, `aidlc-docs/`, `.claude/` |

### Acceptance Criteria
- [ ] `docker build -t garou:latest .` succeeds
- [ ] Container starts and passes `/health/ready` within 30s
- [ ] `kubectl apply -f deploy/` deploys all resources without errors
- [ ] QUIC service is reachable via UDP LoadBalancer IP on port 4433
- [ ] Metrics scraped by Prometheus via ServiceMonitor
- [ ] HPA scales pods up when `garou_connections_active` exceeds threshold
- [ ] Pod restarts cleanly (SIGTERM → graceful drain → exit 0)
