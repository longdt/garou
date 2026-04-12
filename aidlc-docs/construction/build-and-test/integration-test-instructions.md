# Integration Test Instructions

## Purpose

Verify that the QUIC server, NATS JetStream, Redis, and the OTel Collector work correctly together end-to-end.

## Prerequisites — Start Dependencies with Docker Compose

```bash
# Start NATS, Redis, and OTel Collector
docker compose -f docker-compose.test.yml up -d

# Verify services are healthy
docker compose -f docker-compose.test.yml ps
```

**Minimal `docker-compose.test.yml`:**

```yaml
services:
  nats:
    image: nats:2.10-alpine
    ports: ["4222:4222"]
    command: ["--jetstream"]

  redis:
    image: redis:7-alpine
    ports: ["6379:6379"]

  otel-collector:
    image: otel/opentelemetry-collector-contrib:latest
    ports: ["4317:4317", "55679:55679"]
    volumes:
      - ./otel-collector-config.yaml:/etc/otel/config.yaml
    command: ["--config=/etc/otel/config.yaml"]
```

## Scenario 1: Server Startup + Health Probes

```bash
# Start server with test config
RUST_LOG=debug cargo run -- server --config config.toml.example

# In another terminal — verify health probes
curl -f http://localhost:9090/health/live   # expect 200
curl -f http://localhost:9090/health/ready  # expect 200 (when NATS+Redis up)
```

## Scenario 2: QUIC Connection + Authentication

```bash
# Build and run the example client
cargo run --example basic_usage

# Expected: client connects, authenticates with JWT, exchanges messages
```

## Scenario 3: Readiness Degrades When NATS Goes Down

```bash
# Stop NATS
docker compose -f docker-compose.test.yml stop nats

# Readiness should return 503 within ~2s (probe timeout)
curl -v http://localhost:9090/health/ready  # expect 503

# Restart NATS
docker compose -f docker-compose.test.yml start nats

# Readiness should recover within ~2s
curl -v http://localhost:9090/health/ready  # expect 200
```

## Scenario 4: Graceful Shutdown

```bash
# Start server, send SIGTERM
cargo run -- server &
SERVER_PID=$!
sleep 2
kill -TERM $SERVER_PID

# Expected log output:
# INFO Received SIGTERM
# INFO Shutdown signal broadcast to all tasks
# INFO All connections drained
# INFO Shutdown complete
```

## Scenario 5: OTel Trace + Metric Export

```bash
# Start server with OTel collector running
RUST_LOG=debug cargo run -- server

# Connect a client, send messages, then check collector
curl http://localhost:55679/debug/tracez   # traces
curl http://localhost:55679/debug/pipelinez # metrics pipeline
```

## Cleanup

```bash
docker compose -f docker-compose.test.yml down -v
```
