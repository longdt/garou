# Component Definitions

## Overview

Garou is organized as a single Rust crate with 11 logical components across 7 module namespaces. New modules (`config`, `auth`, `storage`, `metrics`, `health`) are added alongside the existing (`error`, `protocol`, `transport`, `server`) modules.

---

## Component 1: Config (`src/config/`)

**Purpose**: Load, validate, and distribute server configuration from TOML file and CLI overrides.

**Responsibilities**:
- Parse `config.toml` using `serde` + `toml` crate
- Apply CLI argument overrides on top of file config
- Provide typed config structs to all other components
- Validate config at startup (fail fast on missing required fields)
- Expose config as `Arc<Config>` for sharing across async tasks

**Boundaries**:
- Owns all configuration — other components receive `Arc<Config>` at construction, never read files themselves
- No runtime config reloading in this phase

---

## Component 2: Auth (`src/auth/`)

**Purpose**: Validate JWT tokens and cache decoded claims in Redis.

**Responsibilities**:
- Parse and cryptographically verify JWT tokens (HMAC-SHA256 or RS256)
- Extract user identity from validated claims (`sub` → `UserId`, `name` → username)
- Cache validated claims in Redis with TTL matching token expiry
- Degrade gracefully when Redis is unavailable (validate inline, no cache)
- Return structured `AuthClaims` on success, `ChatError::Auth` on failure

**Boundaries**:
- Stateless validator — holds config and optional Redis handle
- Does not manage sessions or connection state (owned by ConnectionHandler)

---

## Component 3: NATS Storage (`src/storage/nats.rs`)

**Purpose**: Durable message persistence and cross-node pub/sub via NATS JetStream.

**Responsibilities**:
- Maintain a connection pool to NATS server (`async-nats` crate)
- Create and manage JetStream stream `CHAT_MESSAGES` (subjects: `chat.rooms.>`)
- Persist messages to JetStream before confirming ACK to sender
- Subscribe to room subjects for cross-node message delivery
- Provide message history replay (last N messages from JetStream sequence)
- Manage NATS KV buckets: `CHAT_ROOMS` (room state) and `CHAT_USERS` (user data)
- Store message metadata (room_id, sender_id) in NATS message headers (fixes BUG-001)
- Reconnect with exponential backoff on connection loss

**Boundaries**:
- Owns all durable state — in-memory `Room` is only a cache
- Fail fast on publish failure (no local buffering per Q2=A)

---

## Component 4: Redis Cache (`src/storage/redis.rs`)

**Purpose**: Fast-path in-memory cache for JWT claims, user presence, and room roster.

**Responsibilities**:
- Maintain a configurable connection pool to Redis (`fred` crate)
- JWT claim cache: store decoded claims by token hash, TTL = token expiry delta
- Presence cache: `presence:{user_id}` keys with 60s TTL (refreshed on activity)
- Room roster cache: `roster:{room_id}` set of online user_ids for fast broadcast
- Graceful degradation: all operations return `Ok(None)` (cache miss) on Redis failure
- Health check: `PING` command for readiness probe

**Boundaries**:
- Pure cache — no data is authoritative here; NATS KV is the source of truth
- All Redis failures are non-fatal (logged as warnings)

---

## Component 5: Metrics (`src/metrics/`)

**Purpose**: Define and update Prometheus metrics throughout the server lifecycle.

**Responsibilities**:
- Initialize global metrics registry at startup (`metrics` + `metrics-exporter-prometheus` crates)
- Define all counters, gauges, and histograms from FR-006
- Provide thin `increment_*` / `record_*` / `set_*` helper functions called from other components
- Expose Prometheus text-format output via `MetricsExporter::render()`

**Boundaries**:
- No HTTP serving (Health component handles the HTTP endpoint)
- Metrics are global (using the `metrics` crate global registry)

---

## Component 6: Health (`src/health/`)

**Purpose**: Serve HTTP health and metrics endpoints for K8s probes and Prometheus scraping.

**Responsibilities**:
- Run a lightweight Axum HTTP server on the configured metrics port (default: 9090)
- `GET /health/live` → 200 always (process is running)
- `GET /health/ready` → 200 if NATS + Redis are healthy, 503 otherwise
- `GET /metrics` → Prometheus text format from metrics registry
- Check NATS health: verify JetStream stream exists and is accessible
- Check Redis health: send `PING` command

**Boundaries**:
- Separate Tokio task, does not block the QUIC server
- Holds `Arc<NatsClient>` and `Arc<RedisClient>` for health checks only

---

## Component 7: Protocol (`src/protocol/`)

**Purpose**: FlatBuffers wire encoding/decoding and binary frame framing.

**Responsibilities**:
- Define FlatBuffers schemas for all 40+ message types in `fbs/` directory
- `build.rs` generates Rust code from schemas using `flatc` at build time
- `Encodable` trait: serialize a message struct → `Bytes` (FlatBuffers bytes)
- `Decodable` trait: deserialize `&[u8]` → typed message struct (zero-copy verification)
- `FrameCodec`: unchanged 5-byte header framing (type + length-prefixed payload)
- JSON debug mode: if `protocol.debug_json = true`, fall back to serde_json (dev only)

**Boundaries**:
- Pure data transformation — no I/O, no state
- FlatBuffers generated code lives in `src/protocol/generated/` (git-tracked)

---

## Component 8: Transport (`src/transport/`)

**Purpose**: QUIC stream lifecycle management and shard routing logic.

**Responsibilities**:
- `StreamType` classification and priority assignment (unchanged)
- `ShardRouter`: room→shard assignment, hot room detection, promote/demote (unchanged)
- `StreamHandle` and `StreamSet`: track stream state and statistics (unchanged)
- `StreamConfig`: buffer sizes and timeout configuration (extended with config values)

**Boundaries**:
- No changes to core logic — receives updated config via `Arc<Config>`
- Hot room threshold and shard count now driven by config (not hardcoded)

---

## Component 9: Connection Handler (`src/server/connection_handler.rs`)

**Purpose**: Per-connection protocol state machine managing all QUIC streams for one client.

**Responsibilities**:
- Protocol handshake: Hello → HelloAck → Auth (via Auth component) → AuthOk/AuthFailed
- Stream setup: open shard streams, accept ChatCommands/ACK/BulkUpload streams from client
- FlatBuffers decode incoming frames on each stream, emit `ServerEvent` to server
- Encode outgoing `ConnectionCommand` messages as FlatBuffers frames, write to appropriate stream
- Datagram handling: decode/encode typing indicators and presence
- Enforce authentication: no commands accepted before AuthOk
- Bounded channel for `ConnectionCommand` (backpressure, fixes BUG-004)

**Boundaries**:
- One instance per QUIC connection
- Communicates with server via `mpsc::channel` (events up, commands down)
- Holds `Arc<AuthValidator>` and `Arc<ShardRouter>` (injected at construction)

---

## Component 10: Room Manager (`src/server/room_manager.rs`)

**Purpose**: In-memory cache of room state for fast local lookups and broadcast routing.

**Responsibilities**:
- Cache active rooms and their current member lists (warm cache of NATS KV data)
- Track user→room memberships for fast broadcast and cleanup on disconnect
- `VecDeque`-based recent message buffer (fixes BUG-003)
- Provide member_ids for local broadcast fanout (complements NATS cross-node delivery)
- Room lifecycle: create, join, leave, delete

**Boundaries**:
- In-memory only — NATS KV is the source of truth; room manager is a read-through cache
- Populated from NATS KV on room join; invalidated on disconnect

---

## Component 11: Multi-Stream Server (`src/server/multi_stream_server.rs`)

**Purpose**: Main server orchestrator — owns the QUIC endpoint and coordinates all components.

**Responsibilities**:
- Bootstrap: load config, initialize all components, start QUIC endpoint
- Accept QUIC connections, spawn `ConnectionHandler` per connection (fixed: `Arc<Self>` pattern)
- Process `ServerEvent` messages from connection handlers
- Coordinate message flow: receive SendMessage event → persist via NATS → broadcast
- Connection lifecycle: register/deregister connections, cleanup on disconnect
- Graceful shutdown: drain connections, flush NATS, close Redis pool
- Expose `get_stats()` for metrics collection

**Boundaries**:
- Owns `Arc` handles to: Config, AuthValidator, NatsClient, RedisClient, MetricsRegistry, RoomManager, ShardRouter
- Fixed: wrapped in `Arc<MultiStreamServer>` at construction, not re-wrapped per connection (fixes BUG-002)
