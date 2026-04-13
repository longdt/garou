# Unit 8: Graceful Shutdown — Code Generation Plan

## Status: COMPLETED

## Unit Context
- **Requirements**: FR-010 (Graceful shutdown on SIGTERM/SIGINT)
- **Dependencies**: Unit 2 (config), Unit 5 (NATS), Unit 6 (Redis), Unit 7 (metrics/tracing)
- **Scope**: Signal handling (SIGTERM/SIGINT), ordered shutdown sequence, configurable drain timeout
- **Deployment Target**: Main server lifecycle management

## Architectural Deviation Note

> The original plan specified three separate files (`coordinator.rs`, `signal_handler.rs`, `sequence.rs`).
> The final implementation consolidated all shutdown logic into a single `src/shutdown/mod.rs` for simplicity.
> The `ShutdownSignal` enum and `execute_shutdown_sequence()` function were not created as standalone types —
> shutdown orchestration is handled inline in `main.rs` using `ShutdownCoordinator::initiate()` + `wait_drained()`.
> Method names were also simplified: `signal_shutdown()` → `subscribe()`, `increment_connections()` → `on_connect()`,
> `decrement_connections()` → `on_disconnect()`, `active_connection_count()` → `active_count()`,
> `wait_all_connections_closed()` → `wait_drained()`.

## Steps

- [x] Add dependencies to `Cargo.toml` (if not already present):
  - [x] `tokio` `signal` feature enabled (already present)
  - [x] `tracing = "0.1"` (already present)

- [x] Update `src/config/mod.rs`:
  - [x] Add `shutdown_timeout_secs: u64` to `ServerSettings` struct (default: 30)
  - [x] TOML parsing via serde `Default` impl
  - [x] `config.toml.example` updated with `shutdown_timeout_secs`

- [x] Create `src/shutdown/mod.rs` (all shutdown logic consolidated here):
  - [x] `ShutdownCoordinator` struct:
    - [x] `tx: broadcast::Sender<()>`
    - [x] `active: Arc<AtomicUsize>`
    - [x] `shutting_down: Arc<AtomicBool>`
    - [x] `drain_timeout: Duration`
  - [x] `ShutdownCoordinator::new(drain_timeout_secs) -> Self`
  - [x] `ShutdownCoordinator::subscribe() -> broadcast::Receiver<()>`
  - [x] `ShutdownCoordinator::is_shutting_down() -> bool`
  - [x] `ShutdownCoordinator::initiate()` — sets flag + broadcasts signal
  - [x] `ShutdownCoordinator::on_connect()` / `on_disconnect()`
  - [x] `ShutdownCoordinator::active_count() -> usize`
  - [x] `ShutdownCoordinator::wait_drained()` — polls until 0 connections or timeout
  - [x] `install_signal_handlers() -> broadcast::Receiver<()>` — SIGTERM/SIGINT via tokio; `ctrl_c` fallback on non-Unix
  - [x] Unit tests: `test_connection_counting`, `test_initiate_sets_flag`, `test_wait_drained_immediate_when_zero`, `test_wait_drained_timeout`, `test_subscriber_receives_signal`

- [x] Update `src/lib.rs`:
  - [x] `pub mod shutdown`

- [x] Update `src/main.rs`:
  - [x] Create `ShutdownCoordinator` with timeout from `config.server.shutdown_timeout_secs`
  - [x] Call `install_signal_handlers()` to register SIGTERM/SIGINT
  - [x] `tokio::select!` monitors signal receiver
  - [x] On signal: `health_deps.set_accepting(false)`, `coordinator.initiate()`, `coordinator.wait_drained()`, telemetry guard dropped on exit

- [x] Update `src/server/multi_stream_server.rs`:
  - [x] `shutdown_coordinator: Arc<ShutdownCoordinator>` field
  - [x] `accepting: Arc<AtomicBool>` field
  - [x] Accept loop checks `accepting` flag before each `endpoint.accept()`
  - [x] `on_connect()` / `on_disconnect()` called on connection accept/close
  - [x] `storage_handles()` method exposes NATS/Redis for health + shutdown

- [x] Update `src/server/connection_handler.rs`:
  - [x] `shutdown_rx: broadcast::Receiver<()>` field
  - [x] `tokio::select!` in main message loop monitors shutdown signal
  - [x] On shutdown: breaks loop cleanly; connection counted down via coordinator

- [x] Update `src/health/mod.rs`:
  - [x] `HealthDeps::set_accepting(bool)` — sets `accepting` AtomicBool
  - [x] `/health/ready` returns 503 if `accepting == false`

- [x] Verify `cargo test` passes (all shutdown unit tests + existing tests)

## Implementation Notes

- `tokio::signal::unix::signal()` requires Unix target; for cross-platform support, use `tokio::signal::ctrl_c()` + Unix-specific in cfg blocks
- Broadcast channel allows multiple tasks to independently monitor shutdown (no synchronization needed)
- Connection counting uses `AtomicUsize` for lock-free increments/decrements
- Drain timeout prevents server from hanging indefinitely on unresponsive clients
- All shutdown steps are wrapped in tracing spans for observability (can be viewed in Jaeger)
- Graceful close sends `ConnectionClosed` frame to client (requires client-side handling)
- Redis and NATS drain/close are non-fatal: if already disconnected, skip (log at DEBUG level)
- Metrics and traces flushed last to ensure all lifecycle events are captured

## Extension Rule Compliance

- **Security Baseline** (enabled): Shutdown process does not expose internal state; all logs at INFO/DEBUG level
- **Property-Based Testing** (enabled): Connection counter invariants (increment == decrement by end of test)
