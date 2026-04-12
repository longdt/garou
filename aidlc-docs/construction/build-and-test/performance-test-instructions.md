# Performance Test Instructions

## Performance Requirements

| Metric | Target | Notes |
|---|---|---|
| Concurrent connections | 10,000 | Per node (configurable via `max_connections`) |
| Message latency (p99) | < 10 ms | End-to-end, single node, LAN |
| Message throughput | > 100,000 msg/s | Aggregate across all rooms |
| Hot room promotion latency | < 100 ms | Time from threshold breach to dedicated stream |
| Memory per 1,000 connections | < 512 MB | Baseline without message history |
| Graceful shutdown drain | ≤ 30 s | All connections closed within timeout |

## Tools

- **[`quinn-perf`](https://github.com/quinn-rs/quinn)** — QUIC connection benchmarking
- **[`k6`](https://k6.io/)** — scripted load generation (WebSocket / custom protocols via xk6)
- **`cargo bench`** — micro-benchmarks for codec and routing hot paths

## Setup

```bash
# Release build with full optimisation
cargo build --release

# Start server
RUST_LOG=warn ./target/release/garou server --config config.toml.example
```

## Load Test: Connection Ramp-up

```bash
# Ramp to 10,000 concurrent QUIC connections over 60s
# (requires a custom QUIC load generator)
./tools/quic-load-gen \
  --endpoint 127.0.0.1:4433 \
  --connections 10000 \
  --ramp-secs 60 \
  --duration 300 \
  --cert-skip-verify
```

**Expected:** Server stays below 2 GB RAM, p99 auth latency < 50 ms.

## Throughput Test: Message Storm

```bash
# 1,000 clients each sending 100 msg/s for 60s → 100k msg/s total
./tools/quic-load-gen \
  --endpoint 127.0.0.1:4433 \
  --connections 1000 \
  --msg-rate 100 \
  --duration 60 \
  --room-count 10
```

**Expected:** `garou_message_latency_ms` p99 < 10 ms (visible in OTel/Grafana).

## Micro-benchmarks (Codec Hot Path)

```bash
cargo bench
```

Benchmarks cover FlatBuffer encode/decode for `RoomMessage`, `Auth`, `ChatCommand` frames.

## Analyse Results

OTel metrics are exported to the configured collector. Query in Grafana:

```
# Active connections gauge
garou_connections_active

# Message latency histogram (p99)
histogram_quantile(0.99, rate(garou_message_latency_ms_bucket[1m]))

# Error rate
rate(garou_errors_total[1m])
```

## Optimisation Checklist

If targets are not met:
- [ ] Profile with `cargo flamegraph` — identify hot paths
- [ ] Check shard count (`num_shards`): increase for more rooms
- [ ] Check hot room threshold: lower for faster promotion
- [ ] Enable release LTO: add `lto = true` to `[profile.release]` in `Cargo.toml`
- [ ] Check QUIC congestion control settings via `quinn` config
