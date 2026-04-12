# Unit 8: Graceful Shutdown — Code Generation Plan

## Status: PENDING

## Unit Context
- **Stories Implemented**: FR-012 (Graceful shutdown on SIGTERM/SIGINT), FR-007 (Error recovery)
- **Dependencies**: Unit 2 (config), Unit 5 (NATS), Unit 6 (Redis), Unit 7 (metrics/tracing)
- **Scope**: Signal handling (SIGTERM/SIGINT), ordered shutdown sequence, configurable drain timeout
- **Deployment Target**: Main server lifecycle management

## Steps

- [ ] Add dependencies to `Cargo.toml` (if not already present):
  - [ ] `tokio = { version = "1.35", features = ["signal", "sync"] }` (ensure `signal` feature enabled)
  - [ ] `tracing = "0.1"` (already present, for shutdown logging)

- [ ] Update `src/config/mod.rs`:
  - [ ] Add `[server]` subsection to `Config` struct:
    - [ ] `shutdown_timeout_secs: u64` (default: 30)
  - [ ] Add TOML parsing for `shutdown_timeout_secs`
  - [ ] Update `config.toml.example` with shutdown timeout example

- [ ] Create `src/shutdown/mod.rs` (module declarations and re-exports)

- [ ] Create `src/shutdown/coordinator.rs`:
  - [ ] `ShutdownCoordinator` struct:
    - [ ] `shutdown_signal: tokio::sync::broadcast::Sender<()>` (broadcast shutdown event to all tasks)
    - [ ] `active_connections: Arc<AtomicUsize>` (track active connections)
    - [ ] `drain_timeout: Duration`
  - [ ] `ShutdownCoordinator::new(drain_timeout_secs) -> Self`
  - [ ] `ShutdownCoordinator::signal_shutdown() -> broadcast::Receiver<()>` (subscribe to shutdown event)
  - [ ] `ShutdownCoordinator::increment_connections() -> ()` (called on connection accept)
  - [ ] `ShutdownCoordinator::decrement_connections() -> ()` (called on connection close)
  - [ ] `ShutdownCoordinator::active_connection_count() -> usize` (for monitoring)
  - [ ] `ShutdownCoordinator::wait_all_connections_closed(timeout) -> Result<()>` (block until all connections drain, or timeout)
  - [ ] Unit tests:
    - [ ] `test_shutdown_coordinator_new()`
    - [ ] `test_increment_decrement_connections()`
    - [ ] `test_wait_connections_timeout()`
    - [ ] `test_wait_connections_all_close()`

- [ ] Create `src/shutdown/signal_handler.rs`:
  - [ ] `install_signal_handlers() -> Result<tokio::sync::broadcast::Receiver<ShutdownSignal>>`
  - [ ] `ShutdownSignal` enum: `Sigterm | Sigint`
  - [ ] Use `tokio::signal::unix::signal(SignalKind::Terminate)` and `SignalKind::Interrupt`
  - [ ] Create broadcast channel to fan out signals to all listeners
  - [ ] Spawn background task that waits for signals and broadcasts them
  - [ ] Return receiver for main loop to monitor
  - [ ] Unit tests:
    - [ ] `test_signal_handler_install_success()`
    - [ ] `test_signal_broadcast_to_multiple_receivers()`

- [ ] Create `src/shutdown/sequence.rs`:
  - [ ] `execute_shutdown_sequence(server, nats_client, redis_client, health_checker, coordinator, metrics_provider) -> Result<()>` async function
  - [ ] Shutdown sequence (ordered):
    - [ ] Step 1: Log "Shutdown sequence initiated" with tracing span
    - [ ] Step 2: Stop accepting new connections (signal `MultiStreamServer::accepting` flag)
    - [ ] Step 3: Notify connected clients with `ConnectionClosed` frame (or wait for graceful close)
    - [ ] Step 4: Wait for all active connections to close (with timeout from `coordinator.drain_timeout`)
    - [ ] Step 5: Close NATS subscription (if connected)
    - [ ] Step 6: Close Redis connection (if connected)
    - [ ] Step 7: Flush all metrics and traces (call `metrics_provider.force_flush()`, `tracer_provider.force_flush()`)
    - [ ] Step 8: Flush all logs
    - [ ] Step 9: Log "Shutdown sequence complete"
  - [ ] Each step wrapped in tracing span with error logging
  - [ ] Non-fatal errors: log warning, continue to next step (e.g., if NATS already disconnected)
  - [ ] Return Result (overall success if all critical steps complete)
  - [ ] Unit tests:
    - [ ] `test_shutdown_sequence_order()` (mock all dependencies)
    - [ ] `test_shutdown_sequence_timeout_on_drain()` (verify timeout works)

- [ ] Update `src/lib.rs`:
  - [ ] Add `pub mod shutdown`
  - [ ] Re-export `ShutdownCoordinator`, `install_signal_handlers`, `execute_shutdown_sequence`

- [ ] Update `src/main.rs`:
  - [ ] Import shutdown module and `tokio::signal`
  - [ ] In `main()` or `tokio::main`:
    - [ ] Create `ShutdownCoordinator` with timeout from config
    - [ ] Call `shutdown::install_signal_handlers()` to register SIGTERM/SIGINT
    - [ ] Pass `coordinator` clone to `MultiStreamServer::new()` (for connection tracking)
    - [ ] Create `tokio::select!` or spawn background task to monitor signal receiver
    - [ ] On signal received:
      - [ ] Log signal (info-level)
      - [ ] Call `shutdown::execute_shutdown_sequence(...)` with all components
      - [ ] Exit cleanly with code 0
    - [ ] Handle signal handler errors (log and continue)

- [ ] Update `src/server/multi_stream_server.rs`:
  - [ ] Add `shutdown_coordinator: Arc<ShutdownCoordinator>` field
  - [ ] Add `accepting: Arc<AtomicBool>` field (flag to stop accepting new connections)
  - [ ] In `from_config()`:
    - [ ] Create `ShutdownCoordinator` with config.server.shutdown_timeout_secs
    - [ ] Pass to constructor
  - [ ] In `run()` or accept loop:
    - [ ] Before `accept()`, check `self.accepting` flag
    - [ ] On accept success: call `shutdown_coordinator.increment_connections()`
    - [ ] On shutdown signal: set `self.accepting = false` (stop new accepts)
  - [ ] Add `shutdown_accepting()` method:
    - [ ] Set `self.accepting = false`
    - [ ] Log "Server stopped accepting new connections"
  - [ ] Ensure all connections call `shutdown_coordinator.decrement_connections()` on close
  - [ ] Update tests (if any mock `MultiStreamServer`)

- [ ] Update `src/server/connection_handler.rs`:
  - [ ] Add `shutdown_signal: broadcast::Receiver<()>` field (derived from `ShutdownCoordinator`)
  - [ ] In main message loop:
    - [ ] Use `tokio::select!` to monitor both message reads and shutdown signal
    - [ ] On shutdown signal received: send `ConnectionClosed` frame and break (graceful close)
  - [ ] Ensure `drop()` always calls `shutdown_coordinator.decrement_connections()`
  - [ ] Log connection close with user_id and session duration

- [ ] Update `src/storage/nats.rs`:
  - [ ] Add `shutdown()` method to `NatsClient`:
    - [ ] Unsubscribe from all subscriptions
    - [ ] Drain connection (if supported by async-nats): `self.client.drain().await?`
    - [ ] Log "NATS client closed"
  - [ ] Handle case where NATS already disconnected (non-fatal)

- [ ] Update `src/storage/redis.rs`:
  - [ ] Add `shutdown()` method to `RedisClient`:
    - [ ] Close connection: `self.conn.close().await` (or drop the `ConnectionManager`)
    - [ ] Log "Redis connection closed"
  - [ ] Handle case where Redis already disconnected (non-fatal)

- [ ] Update `src/health/mod.rs`:
  - [ ] `HealthChecker::set_accepting(bool)` method (update accepting flag for `/health/ready`)
  - [ ] `/health/ready` returns 503 if `accepting == false` (server shutting down)

- [ ] Create integration test: `tests/graceful_shutdown_integration.rs`
  - [ ] Test: Server accepts connections → shutdown signal → drains all connections
  - [ ] Test: Server rejects new connections after shutdown initiated
  - [ ] Test: All resources (NATS, Redis, metrics) are flushed before exit
  - [ ] Test: Timeout mechanism works (force close after timeout)
  - [ ] Test: Signals are properly handled (SIGTERM and SIGINT both work)

- [ ] Update `config.toml.example`:
  - [ ] Add `[server]` section with `shutdown_timeout_secs = 30`
  - [ ] Document graceful shutdown behavior

- [ ] Documentation:
  - [ ] Create `aidlc-docs/construction/unit-8-graceful-shutdown/code/shutdown-summary.md`:
    - [ ] Describe shutdown sequence in order
    - [ ] Document timeout behavior
    - [ ] Show example signals and logs
    - [ ] Explain monitoring and drain window

- [ ] Verify `cargo test` passes (all shutdown unit tests + existing tests)

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
