# Build Instructions

## Prerequisites

| Requirement | Version | Notes |
|---|---|---|
| Rust toolchain | 1.85+ | `rustup update stable` |
| `flatc` compiler | any | Optional — pre-committed generated code used if absent |
| `protoc` | any | Required by `opentelemetry-otlp` (tonic/prost) |
| Docker | 24+ | For container image build only |

## Environment Variables

| Variable | Default | Purpose |
|---|---|---|
| `RUST_LOG` | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | Override OTLP endpoint without editing config |

## Build Steps

### 1. Install system dependencies (Debian/Ubuntu)

```bash
apt-get install -y libssl-dev pkg-config protobuf-compiler
```

### 2. Build debug (fast, for development)

```bash
cargo build
# Binary: target/debug/garou
```

### 3. Build release (optimised, for deployment)

```bash
cargo build --release
# Binary: target/release/garou
```

### 4. Build Docker image

```bash
docker build -t garou:latest .
```

## Verify Build Success

```
Finished `release` profile [optimized] target(s) in ~60s
```

**Build artifacts:**
- `target/release/garou` — server binary (~15 MB stripped)
- `Dockerfile` — multi-stage (rust:1.85-slim builder → debian:bookworm-slim runtime)

**Acceptable warnings:**
- Unused import in `src/storage/nats.rs` (pre-existing)
- Lifetime elision warnings in generated FlatBuffers code (generated, not hand-written)
- One unused import in `src/main.rs` (`warn`)

## Troubleshooting

### `protoc` not found
```bash
apt-get install protobuf-compiler   # Debian/Ubuntu
brew install protobuf               # macOS
```

### `flatc` not found
The build script falls back to pre-committed generated code automatically — this is expected in CI.

### Linker errors on macOS
```bash
brew install openssl
export OPENSSL_DIR=$(brew --prefix openssl)
```
