# Garou Chat Server — Production Requirements

**Version**: 1.0  
**Date**: 2026-04-08  
**Status**: Approved (pending workflow plan approval)

---

## 1. Business Context

Garou is being refactored from a functional prototype to a production-grade high-performance chat server. The existing QUIC multi-stream architecture is sound; this refactoring addresses correctness bugs, missing production features, and the need to scale horizontally on Kubernetes.

---

## 2. Functional Requirements

### FR-001: JWT Authentication
- Server must validate JWT tokens presented by clients on the Auth frame
- JWT validation: verify signature, expiry (exp claim), and required claims (user_id, username)
- Support configurable JWT secret (HMAC-SHA256) or public key (RS256) via config file
- Reject connections with invalid/expired tokens with AuthFailed frame

### FR-002: TOML Configuration File
- Server must load configuration from a TOML file (default: `config.toml`)
- CLI flag `--config <path>` to specify alternate config file
- CLI flags override config file values
- Configuration covers: bind address, TLS cert paths, JWT secret, NATS URL, Redis URL, max connections, shard count, hot room thresholds, log level

### FR-003: Message Persistence via NATS JetStream
- All sent messages must be persisted to NATS JetStream before broadcast
- Each room maps to a NATS subject: `chat.rooms.{room_id}`
- JetStream stream: `CHAT_MESSAGES`, retention: limits-based (configurable max age + max bytes)
- Message history retrieval: replay last N messages from JetStream on room join
- Room state (members, metadata) stored as NATS KV bucket: `CHAT_ROOMS`
- User data stored as NATS KV bucket: `CHAT_USERS`

### FR-004: Cross-Node Pub/Sub via NATS
- Room message broadcasts must be routed via NATS subjects for cross-node delivery
- Each chat node subscribes to rooms its connected users are members of
- On SendMessage: publish to NATS subject → all nodes with members in that room receive and deliver
- Presence and typing events: published to NATS subjects with no persistence (core NATS, not JetStream)

### FR-005: FlatBuffers Wire Protocol
- Replace JSON serialization with FlatBuffers for all protocol messages
- FlatBuffers schemas defined in `fbs/` directory
- Generated Rust code in `src/protocol/generated/`
- Zero-copy access: messages read directly from frame payload without heap allocation
- Fallback: keep JSON for development/debug mode (config flag)

### FR-006: OpenTelemetry Metrics
- ~~Expose `/metrics` endpoint on a separate HTTP port (default: 9090)~~ **[REVISED]**: Metrics are exported via OTLP push (gRPC) to an OpenTelemetry Collector — no Prometheus scrape endpoint is served. The health port (9090) exposes only `/health/live` and `/health/ready`.
- Metrics instrumentation and export must use OpenTelemetry metrics
- Required metrics (all instrumented via OTel instruments in `Metrics` struct):
  - `garou_connections_total` (counter): total connections accepted
  - `garou_connections_active` (up-down counter): current active connections
  - `garou_authenticated_connections` (counter): successfully authenticated connections
  - `garou_messages_total` (counter): messages processed
  - `garou_message_latency_ms` (histogram): end-to-end message processing latency
  - `garou_errors_total` (counter, label: `kind`): errors by type (covers `garou_nats_publish_errors_total` via `kind=nats_publish`)
  - `garou_rooms_active` (up-down counter): rooms with at least one member
- Note: `garou_hot_rooms_active` was not instrumented in this release; deferred to a future milestone

### FR-007: OpenTelemetry Logging
- All log output must be structured and integrated with OpenTelemetry logs (or OpenTelemetry-compatible bridge where direct SDK support is limited)
- Log fields: timestamp (ISO 8601), level, target, span fields, message
- Log level configurable via config file and `RUST_LOG` env var

### FR-008: Health Check Endpoints
- HTTP server (same as metrics port) exposes:
  - `GET /health/live` — returns 200 if server process is running
  - `GET /health/ready` — returns 200 if NATS + Redis connections are healthy, 503 otherwise
- Required for K8s liveness and readiness probes

### FR-009: Bug Fixes (Critical)
- **BUG-001**: `handle_edit_message`, `handle_delete_message`, `handle_add_reaction`, `handle_remove_reaction` must look up the message's actual `room_id` from NATS KV (not hardcode `room_id: 0`)
- **BUG-002**: Replace `clone_ref()` antipattern — `MultiStreamServer` should be wrapped in `Arc<MultiStreamServer>` at construction time, not re-wrapped per connection
- **BUG-003**: Replace `Vec::remove(0)` in `Room::add_message` with `VecDeque` for O(1) front removal
- **BUG-004**: Replace all `mpsc::unbounded_channel()` with bounded channels (`mpsc::channel(capacity)`) for backpressure

### FR-010: Graceful Shutdown
- Server must handle SIGTERM and SIGINT signals
- On shutdown signal: stop accepting new connections, drain in-flight messages, close NATS connection gracefully, flush metrics
- K8s `terminationGracePeriodSeconds` compatible (default 30s drain window)

### FR-011: Redis Caching Layer
- Redis used for hot-path caching only (not primary storage)
- JWT validation cache: cache decoded JWT claims by token hash (TTL = token expiry)
- User online presence: `presence:{user_id}` keys with 60s TTL (refreshed on activity)
- Room membership roster: cached in Redis for fast broadcast lookups

---

## 3. Non-Functional Requirements

### NFR-001: Performance
- Target: support 100,000+ concurrent QUIC connections per cluster
- Per-node target: 10,000-20,000 concurrent connections per pod (depends on pod sizing)
- Message broadcast latency p99 < 10ms within a single node
- Message broadcast latency p99 < 50ms across nodes (including NATS round-trip)
- NATS publish latency budget: < 5ms p99

### NFR-002: Reliability
- No message loss: all sent messages must be persisted to JetStream before ACK to sender
- Exactly-once delivery semantics: use NATS sequence numbers + client-side dedup
- Connection failures: graceful reconnect to NATS with exponential backoff

### NFR-003: Security
- JWT tokens validated on every authentication
- No auth = no message access (enforced in ConnectionHandler)
- TLS required for all QUIC connections (currently self-signed; config supports loading cert from file for production upgrade)
- No unauthenticated users can send messages

### NFR-004: Observability
- OpenTelemetry is mandatory for traces, metrics, and logs across services/components
- ~~Prometheus scrape compatibility remains supported via OpenTelemetry metrics export path (15s default)~~ **[REVISED]**: Prometheus scrape model replaced by OTLP push to an OTel Collector. Prometheus compatibility can be restored via the Collector's Prometheus exporter if needed.
- Structured logs must be emitted through OpenTelemetry logs pipeline (or OpenTelemetry-compatible bridge)
- Distributed tracing must be OpenTelemetry-native with context propagation enabled
- `OTEL_EXPORTER_OTLP_ENDPOINT` env var or `config.toml [observability].otlp_endpoint` configures the OTLP destination

### NFR-005: Kubernetes Compatibility
- Container image: multi-stage Dockerfile (builder + minimal runtime)
- Health probes: `/health/live` and `/health/ready`
- Configuration: TOML config file mounted as ConfigMap
- Secrets: JWT secret, TLS certs via K8s Secrets
- Horizontal Pod Autoscaler: scale on `garou_connections_active` metric
- QUIC (UDP): K8s Service type `LoadBalancer` with UDP protocol
- Metrics: Service annotated for Prometheus ServiceMonitor

---

## 4. Architecture Decision: NATS JetStream + Redis

### Why NATS JetStream (not PostgreSQL/Redis-as-store/RocksDB)
| Criterion | NATS JetStream | PostgreSQL | RocksDB | Redis AOF |
|-----------|---------------|-----------|---------|-----------|
| Chat workload fit | Excellent (append-only stream) | Good | Good | Fair |
| Cross-node pub/sub | Built-in | No | No | Separate (Redis pub/sub) |
| K8s operator | Yes (official) | Yes | No | Yes |
| Stateless chat nodes | Yes | With connection pool | No (embedded) | Yes |
| Horizontal scale | Yes (NATS cluster) | Read replicas | No | Redis Cluster |
| Ops complexity | Low | High | Medium | Medium |

### Why Redis (supplementary)
- JWT validation cache: avoids re-parsing JWT on every operation
- Presence: TTL-based keys are ideal for online/offline tracking
- Roster cache: fast membership lookups for broadcast routing

---

## 5. Constraints & Exclusions

- **No rate limiting** (FR-008 explicitly excluded per Q8=C)
- TLS: self-signed acceptable for this milestone; production upgrade path via cert-manager
- No multi-tenancy in this phase
- No message search (full-text) in this phase
- No end-to-end encryption in this phase

---

## 6. Extension Rules

- **Security Baseline**: ENABLED — all security rules enforced as blocking constraints
- **Property-Based Testing**: ENABLED — all PBT rules enforced as blocking constraints

## 7. Dependency Governance Policy

### NFR-006: Dependency Maintenance and Adoption
- All new third-party dependencies must be popular and actively maintained
- Abandoned or inactive libraries are disallowed by default
- Any exception requires explicit approval and documented rationale
- Dependency selection must consider release recency, maintainer/community activity, issue/PR responsiveness, and ecosystem adoption

### Applied Decisions
- `fred` crate (Redis client) was evaluated and found to be abandoned; replaced by `redis = "0.27"` (redis-rs), which is the primary maintained Rust Redis client
- `axum` was evaluated for the health HTTP server but replaced by `ntex` for consistency with the existing chat server stack and its lower dependency footprint
