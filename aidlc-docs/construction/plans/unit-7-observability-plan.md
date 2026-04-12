# Unit 7: Observability (Traces, Metrics, Logs) + Health Endpoints — Code Generation Plan

## Status: COMPLETED

## Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Metrics/traces/logs export | OTLP push via gRPC | Single pipeline; no Prometheus scrape endpoint needed |
| Health server framework | **ntex** (not axum) | Lightweight, high-performance; consistent with chat server stack |
| Readiness probing | **Active NATS flush + Redis PING** per request | Reflects real dependency state, not stale flags |
| Health endpoint exposure | ClusterIP only (not LoadBalancer) | Internal k8s probes only; not reachable externally |
| `/metrics` endpoint | **Not exposed** | Metrics pushed via OTLP; Prometheus pull model replaced |

## Dependencies Added to `Cargo.toml`

- [x] `opentelemetry = "0.31.0"`
- [x] `opentelemetry_sdk = { version = "0.31.0", features = ["rt-tokio"] }`
- [x] `opentelemetry-otlp = { version = "0.31.1", features = ["grpc-tonic", "metrics", "logs"] }`
- [x] `opentelemetry-semantic-conventions = "0.31.0"`
- [x] `opentelemetry-appender-tracing = "0.31.1"`
- [x] `tracing-opentelemetry = "0.32.1"`
- [x] `tracing-subscriber = { version = "0.3.23", features = ["env-filter"] }`
- [x] `ntex = { version = "3.7.2", features = ["tokio"] }` — health HTTP server only

## Steps

### Telemetry initialisation (`src/metrics/mod.rs`)

- [x] `TelemetryGuard` struct: holds `SdkTracerProvider`, `SdkMeterProvider`, `SdkLoggerProvider`; flushes all on `Drop`
- [x] `init_telemetry(service_name, otlp_endpoint) -> TelemetryGuard`
  - [x] Build `Resource` with `service.name`
  - [x] Build OTLP span exporter (`SpanExporter::builder().with_tonic()`)
  - [x] Build `SdkTracerProvider` with batch exporter
  - [x] Build OTLP metric exporter (`MetricExporter::builder().with_tonic()`)
  - [x] Build `SdkMeterProvider` with periodic exporter
  - [x] Build OTLP log exporter (`LogExporter::builder().with_tonic()`)
  - [x] Build `SdkLoggerProvider` with batch exporter
  - [x] Wire `tracing` subscriber: `fmt` layer + `OpenTelemetryLayer` (traces) + `OpenTelemetryTracingBridge` (logs)
  - [x] Set global providers
- [x] `Metrics` struct: typed OTel instruments
  - [x] `connections_total: Counter<u64>`
  - [x] `connections_active: UpDownCounter<i64>`
  - [x] `authenticated_connections: Counter<u64>`
  - [x] `messages_total: Counter<u64>`
  - [x] `message_latency_ms: Histogram<f64>` (unit: ms)
  - [x] `rooms_active: UpDownCounter<i64>`
  - [x] `errors_total: Counter<u64>` (label: `kind`)
- [x] Helper methods on `Metrics`: `increment_connections_total`, `decrement_connections_active`, etc.

### Config (`src/config/mod.rs`)

- [x] Replace `MetricsSettings` with `ObservabilitySettings`:
  - [x] `otlp_endpoint: String` (default: `"http://localhost:4317"`)
  - [x] `service_name: String` (default: `"garou"`)
  - [x] `enabled: bool` (default: `true`)
  - [x] `health_addr: String` (default: `"0.0.0.0:9090"`)
- [x] `MetricsSettings` type alias kept for migration compatibility
- [x] Update `config.toml.example` with `[observability]` section

### Storage ping methods

- [x] `NatsClient::ping(&self) -> bool` — `flush()` with 2 s timeout
- [x] `RedisClient::ping(&self) -> bool` — `PING` command with 2 s timeout

### Health server (`src/health/mod.rs`)

- [x] `HealthDeps` struct:
  - [x] `nats: Option<Arc<NatsClient>>`
  - [x] `redis: Option<Arc<RedisClient>>`
  - [x] `accepting: AtomicBool`
- [x] `HealthDeps::new(nats, redis) -> Arc<HealthDeps>`
- [x] `HealthDeps::set_accepting(bool)`
- [x] `HealthDeps::is_ready() -> bool` — active probe: NATS flush + Redis PING; false if shutting down
- [x] ntex route: `GET /health/live` → always `200 OK`
- [x] ntex route: `GET /health/ready` → `200 OK` or `503 Service Unavailable` (via `is_ready()`)
- [x] `spawn_health_server(addr, Arc<HealthDeps>) -> io::Result<()>` — spawns ntex server as tokio task

### Server integration

- [x] `MultiStreamServer::storage_handles() -> (Option<Arc<NatsClient>>, Option<Arc<RedisClient>>)`
- [x] `src/lib.rs`: `pub mod health`, `pub mod metrics`

### `src/main.rs`

- [x] `init_telemetry("garou", &config.observability.otlp_endpoint)` — guard held for process lifetime
- [x] Fallback to `tracing_subscriber::fmt` when `observability.enabled = false`
- [x] `Metrics::new(&global::meter("garou"))` — instruments created once
- [x] `server.storage_handles()` → `HealthDeps::new(nats, redis)`
- [x] `spawn_health_server(health_addr, health_deps)`
- [x] On shutdown: `health_deps.set_accepting(false)` before drain

## Notes

- OTLP endpoint configurable via `OTEL_EXPORTER_OTLP_ENDPOINT` env var or `config.toml`
- ntex `HttpServer` spawned via `tokio::spawn` inside the existing tokio runtime (requires `features = ["tokio"]`)
- Active readiness probing: each k8s probe call triggers a real NATS flush + Redis PING (2 s timeout each)
- If NATS or Redis is not configured, those checks are skipped (dep treated as healthy)
- Health server is **not** registered as a LoadBalancer Service — only ClusterIP for k8s probe access
