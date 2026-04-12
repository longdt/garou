# ── Stage 1: Builder ─────────────────────────────────────────────────────────
FROM rust:1.85-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev \
    pkg-config \
    protobuf-compiler \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependency compilation separately from source changes.
COPY Cargo.toml Cargo.lock build.rs ./
COPY fbs/ fbs/
# Stub src so `cargo build` can resolve deps without full source.
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release
RUN rm -f src/main.rs src/lib.rs

# Now compile the real source.
COPY src/ src/
RUN touch src/main.rs src/lib.rs && cargo build --release

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# Non-root user.
RUN useradd -m -u 1001 -s /bin/sh garou

COPY --from=builder /build/target/release/garou /usr/local/bin/garou

RUN mkdir -p /app /etc/garou && chown -R garou:garou /app /etc/garou

WORKDIR /app
USER garou

# QUIC (UDP) and health/metrics (TCP)
EXPOSE 4433/udp
EXPOSE 9090/tcp

ENTRYPOINT ["garou"]
CMD ["server", "--config", "/etc/garou/config.toml"]

HEALTHCHECK --interval=10s --timeout=3s --start-period=15s --retries=3 \
    CMD wget -qO- http://localhost:9090/health/live || exit 1
