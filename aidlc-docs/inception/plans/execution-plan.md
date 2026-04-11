# Execution Plan — Garou Production Refactoring

## Detailed Analysis Summary

### Transformation Scope
- **Transformation Type**: Architectural transformation — adding distributed infrastructure (NATS, Redis), replacing protocol serialization (JSON→FlatBuffers), adding auth, OpenTelemetry observability (traces/metrics/logs), and K8s deployment
- **Primary Changes**: Protocol layer, authentication, persistence/pub-sub (NATS JetStream), caching (Redis), observability (Prometheus), configuration management, K8s deployment artifacts
- **Related Components**: All modules (protocol, transport, server) + new infrastructure integrations

### Change Impact Assessment
- **User-facing changes**: Yes — wire protocol changes (FlatBuffers replaces JSON; clients must update)
- **Structural changes**: Yes — new NATS and Redis integration layers; server becomes stateless
- **Data model changes**: Yes — FlatBuffers schemas replace serde structs for wire format; NATS KV replaces in-memory room state
- **API changes**: Yes — Auth flow enforced (was stub); message history from JetStream replay
- **NFR impact**: Yes — 100k+ scale, sub-10ms p99 latency, OpenTelemetry traces/metrics/logs, K8s probes

### Component Relationships
```
Primary:  src/server/multi_stream_server.rs    [Major - fixes + NATS pub/sub + metrics]
Primary:  src/server/connection_handler.rs     [Major - JWT auth + FlatBuffers decoding]
Primary:  src/protocol/messages.rs             [Major - FlatBuffers replaces JSON serde]
Primary:  src/protocol/frame.rs               [Minor - no format change, FlatBuffers is payload]
New:      src/config/mod.rs                   [New - TOML config layer]
New:      src/auth/mod.rs                     [New - JWT validation + Redis cache]
New:      src/storage/nats.rs                 [New - NATS JetStream client + room persistence]
New:      src/storage/redis.rs                [New - Redis cache layer]
New:      src/metrics/mod.rs                  [New - Prometheus metrics]
New:      src/health/mod.rs                   [New - HTTP health endpoints]
New:      fbs/                                [New - FlatBuffers schema files]
New:      deploy/                             [New - Dockerfile + K8s manifests]
```

### Risk Assessment
- **Risk Level**: HIGH
- **Rollback Complexity**: Moderate (NATS/Redis are additive; FlatBuffers is breaking protocol change)
- **Testing Complexity**: Complex (integration tests require NATS + Redis; property-based tests for FlatBuffers codec)

---

## Workflow Visualization

### Text Representation
```
INCEPTION PHASE:
  [x] Workspace Detection      - COMPLETED
  [x] Reverse Engineering      - COMPLETED
  [x] Requirements Analysis    - COMPLETED
  [-] User Stories             - SKIP (internal refactoring, no user persona work needed)
  [x] Workflow Planning        - IN PROGRESS
  [ ] Application Design       - EXECUTE (new components: auth, storage, metrics, health, config)
  [ ] Units Generation         - EXECUTE (9 units of work across 4 domains)

CONSTRUCTION PHASE (per-unit loop):
  For each unit:
  [ ] Functional Design        - EXECUTE (complex business logic per unit)
  [ ] NFR Requirements         - EXECUTE (performance, security, scalability per unit)
  [ ] NFR Design               - EXECUTE (NFR patterns per unit)
  [ ] Infrastructure Design    - EXECUTE (K8s, NATS, Redis mappings)
  [ ] Code Generation          - EXECUTE (always)
  [ ] Build and Test           - EXECUTE (always, after all units)

OPERATIONS PHASE:
  [ ] Operations               - PLACEHOLDER
```

---

## Phases to Execute

### INCEPTION PHASE
- [x] Workspace Detection — COMPLETED
- [x] Reverse Engineering — COMPLETED
- [x] Requirements Analysis — COMPLETED
- [-] User Stories — **SKIP**
  - *Rationale*: Pure architectural refactoring with no new user-facing workflows. No user personas needed. Requirements are fully specified by the engineering team.
- [x] Workflow Planning — IN PROGRESS (this document)
- [ ] Application Design — **EXECUTE**
  - *Rationale*: 5 new components needed (config, auth, storage, metrics, health). Component interfaces and service layer contracts must be defined before code generation.
- [ ] Units Generation — **EXECUTE**
  - *Rationale*: 9 distinct units of work. Sequential dependency ordering required (config before auth before NATS before everything else).

### CONSTRUCTION PHASE

For each of the 9 units below:
- [ ] Functional Design — **EXECUTE** (per unit)
  - *Rationale*: Each unit involves new business logic (JWT validation flow, NATS stream management, FlatBuffers codec), data models (config structs, NATS subject naming), and error handling patterns
- [ ] NFR Requirements — **EXECUTE** (per unit)
  - *Rationale*: Performance (FlatBuffers zero-copy, NATS async client), security (JWT, no plaintext secrets), scalability (NATS clustering) requirements per unit
- [ ] NFR Design — **EXECUTE** (per unit)
  - *Rationale*: Backpressure patterns (bounded channels), connection pooling (Redis), retry/reconnect (NATS), OpenTelemetry instrumentation and propagation (traces/metrics/logs)
- [ ] Infrastructure Design — **EXECUTE** (Unit 9 only — K8s deployment)
  - *Rationale*: K8s manifests, Dockerfile, Service types (UDP for QUIC, ClusterIP for metrics), ConfigMap/Secret patterns
- [ ] Code Generation — **EXECUTE** (per unit, always)
- [ ] Build and Test — **EXECUTE** (after all units complete)

---

## Unit Definitions (9 Units)

Units are sequenced by dependency — earlier units must be complete before later units begin.

### Unit 1: Core Bug Fixes & Foundation
**Scope**: Fix all 4 critical bugs from reverse engineering
**Changes**:
- Replace `MultiStreamServer::clone_ref()` with proper `Arc<MultiStreamServer>` at construction
- Replace `Vec::remove(0)` in `Room::add_message` with `VecDeque` for O(1) operations
- Replace all `mpsc::unbounded_channel()` with bounded `mpsc::channel(1024)` for backpressure
- Fix `room_id: 0` bug in edit/delete/reaction handlers (defer room lookup to Unit 5 NATS integration)
**Dependencies**: None — must be done first
**Risk**: Low — isolated fixes

### Unit 2: Configuration Layer
**Scope**: TOML config file + CLI overrides
**Changes**:
- New `src/config/mod.rs`: `Config` struct with all server settings
- TOML deserialization (serde + toml crate)
- CLI parsing updated to load config file first, then apply overrides
- Config sections: server, tls, auth, nats, redis, metrics, shards
**Dependencies**: Unit 1
**Risk**: Low

### Unit 3: FlatBuffers Protocol Layer
**Scope**: Replace JSON with FlatBuffers for all wire protocol messages
**Changes**:
- New `fbs/` directory with `.fbs` schema files for all 40+ message types
- Generated Rust code via `flatc` build script (`build.rs`)
- Update `Encodable`/`Decodable` traits to use FlatBuffers builders/verifiers
- Keep `FrameType` enum and frame codec unchanged (FlatBuffers is the payload)
- Add debug/JSON fallback mode (config flag `protocol.debug_json = true`)
**Dependencies**: Unit 2 (config for debug mode flag)
**Risk**: High — breaking protocol change; requires client updates

### Unit 4: JWT Authentication
**Scope**: Real JWT validation replacing the auth stub
**Changes**:
- New `src/auth/mod.rs`: JWT validator using `jsonwebtoken` crate
- Support HMAC-SHA256 (symmetric) and RS256 (asymmetric public key) algorithms
- Config-driven: `auth.algorithm`, `auth.secret` or `auth.public_key_path`
- JWT claims: `sub` (user_id as string), `name` (username), `exp`
- Redis JWT cache (Unit 6 dependency, graceful degradation without Redis)
- Reject unauthenticated users with `AuthFailed` frame
**Dependencies**: Unit 2 (config), Unit 3 (FlatBuffers)
**Risk**: Medium

### Unit 5: NATS JetStream Storage
**Scope**: Message persistence and cross-node pub/sub via NATS JetStream
**Changes**:
- New `src/storage/nats.rs`: async NATS client wrapper (`async-nats` crate)
- JetStream stream: `CHAT_MESSAGES` with subject `chat.rooms.>` (wildcard)
- Per-room subjects: `chat.rooms.{room_id}`
- KV buckets: `CHAT_ROOMS` (room state), `CHAT_USERS` (user data)
- Message persistence before broadcast: `nats_client.publish().await` then fan-out
- Room join replay: fetch last 50 messages from JetStream sequence
- Fix BUG-001: message metadata (room_id) stored in NATS message headers
- Cross-node subscription: each pod subscribes to rooms with connected users
**Dependencies**: Unit 2 (config), Unit 3 (FlatBuffers serialization)
**Risk**: High — new external dependency, changes message flow

### Unit 6: Redis Cache Layer
**Scope**: Fast-path caching for JWT, presence, roster
**Changes**:
- New `src/storage/redis.rs`: Redis client wrapper (`fred` crate — async, connection pooling)
- JWT cache: hash token → cached claims, TTL from token `exp`
- Presence cache: `presence:{user_id}` keys, 60s TTL, refreshed on activity
- Room roster cache: `roster:{room_id}` sorted set of user_ids, used in broadcast
- Graceful degradation: if Redis unavailable, fall back to in-memory (log warning)
**Dependencies**: Unit 2 (config), Unit 4 (JWT validation uses cache)
**Risk**: Low — caching layer; non-critical path

### Unit 7: Observability
**Scope**: OpenTelemetry traces + metrics + logs + health endpoints
**Changes**:
- Add OpenTelemetry SDK setup for tracing, metrics, and logging pipelines
- Configure OTLP export endpoints and resource attributes from config
- Instrument request/message lifecycle spans and context propagation
- Define required counters/gauges/histograms per FR-006 through OpenTelemetry metrics
- Route structured logs through OpenTelemetry log signal (or approved OTel-compatible bridge when needed)
- New `src/health/mod.rs`: small Axum HTTP server on observability port (9090)
  - `GET /metrics` → endpoint compatible with configured telemetry scraping/export strategy
  - `GET /health/live` → 200 always
  - `GET /health/ready` → 200 if NATS+Redis healthy, 503 otherwise
- Instrument `handle_send_message` with end-to-end latency measurement
**Dependencies**: Unit 2 (config for observability/export settings), Unit 5 + Unit 6 (health checks)
**Risk**: Medium

### Unit 8: Graceful Shutdown
**Scope**: SIGTERM/SIGINT handling and clean drain
**Changes**:
- `main.rs`: install `tokio::signal` handlers for SIGTERM and SIGINT
- Shutdown sequence: stop accepting → notify connections → drain NATS → close Redis → flush metrics
- Configurable drain window: `server.shutdown_timeout_secs` (default 30)
- `MultiStreamServer::shutdown()` updated to use drain sequence
**Dependencies**: Unit 5 (NATS), Unit 6 (Redis), Unit 7 (metrics flush)
**Risk**: Low

### Unit 9: Kubernetes Deployment
**Scope**: Container image + K8s manifests
**Changes**:
- New `Dockerfile`: multi-stage (rust:1.78-slim builder → debian:bookworm-slim runtime)
- New `deploy/` directory:
  - `deployment.yaml`: K8s Deployment with liveness/readiness probes
  - `service-quic.yaml`: LoadBalancer Service with UDP port 4433
  - `service-metrics.yaml`: ClusterIP Service for metrics scraping
  - `configmap.yaml`: ConfigMap with config.toml template
  - `secret.yaml`: Secret template for JWT key and TLS certs
  - `hpa.yaml`: HorizontalPodAutoscaler on connection gauge metric
  - `servicemonitor.yaml`: Prometheus ServiceMonitor (if using kube-prometheus-stack)
- `config.toml.example`: documented example configuration file
**Dependencies**: All other units complete
**Risk**: Low

---

## Package Change Sequence

```
Unit 1 (Core Fixes)
  └─> Unit 2 (Config)
        ├─> Unit 3 (FlatBuffers) ─────────────────┐
        ├─> Unit 4 (JWT Auth) ─────────> Unit 6 (Redis)
        └─> Unit 5 (NATS JetStream) ──────────────┤
                                                   └─> Unit 7 (Observability)
                                                         └─> Unit 8 (Graceful Shutdown)
                                                               └─> Unit 9 (K8s Deploy)
```

---

## Success Criteria

- **Primary Goal**: Production-grade QUIC chat server deployable on Kubernetes at 100k+ users
- **Key Deliverables**:
  1. All 4 critical bugs fixed
  2. JWT authentication enforced
  3. Messages persisted to NATS JetStream before ACK
  4. Cross-node message delivery via NATS pub/sub
  5. FlatBuffers wire protocol (with JSON debug fallback)
  6. OpenTelemetry-based observability for traces, metrics, and logs, plus health probes at `/health/live` and `/health/ready`
  7. TOML configuration file support
  8. Graceful SIGTERM shutdown
  9. Multi-stage Dockerfile + K8s manifests
- **Quality Gates**:
  - All property-based tests pass (FlatBuffers codec round-trips, frame codec invariants)
  - Security baseline rules met (JWT validated, no plaintext secrets)
  - Integration tests pass with live NATS + Redis (Docker Compose test environment)
  - OpenTelemetry pipelines validated for traces, metrics, and logs in local and K8s-like environments
  - Server handles 10k concurrent connections in load test without OOM

---

## Cross-Cutting Dependency Governance (All Future Work)

- All new third-party dependencies MUST be popular and actively maintained.
- Abandoned/inactive libraries are disallowed unless explicitly approved with a documented exception.
- Dependency selection must include a brief maintenance-health check (release recency, maintainer activity, issue/PR responsiveness, ecosystem adoption).
- This policy applies to all future units and backlog items.

---

## Full Observability Migration Backlog (Mandatory)

1. Replace existing non-OTel metrics pipeline with OpenTelemetry metrics implementation.
2. Replace/bridge existing logging pipeline to OpenTelemetry logs with structured fields.
3. Ensure tracing is OpenTelemetry-native with propagation across async boundaries and inter-service calls.
4. Standardize exporter/collector configuration for local development and Kubernetes deployment.
5. Add telemetry verification checklist and automated validation tests for traces, metrics, and logs.
