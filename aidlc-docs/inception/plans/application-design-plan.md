# Application Design Plan

## Design Checklist
- [x] Answer design questions below
- [x] Generate components.md
- [x] Generate component-methods.md
- [x] Generate services.md
- [x] Generate component-dependency.md
- [x] Generate application-design.md (consolidated)

---

## Design Questions

Please answer each question by filling in the letter after `[Answer]:`.
Let me know when done.

---

## Question 1
How should the new modules (config, auth, storage/nats, storage/redis, metrics, health) be organized?

A) Single crate with new submodules (`src/config/`, `src/auth/`, `src/storage/`, `src/metrics/`, `src/health/`)
B) Cargo workspace — split into separate crates (e.g., `garou-core`, `garou-auth`, `garou-storage`)
C) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Question 2
If NATS JetStream is temporarily unavailable when a client sends a message, what should happen?

A) Fail fast — return error frame to client immediately (simplest, client can retry)
B) Buffer locally — queue message in-memory with bounded queue, retry with backoff (risk of data loss on pod restart)
C) Reject connection — if NATS goes down, close all connections and let K8s restart the pod
D) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Question 3
How should FlatBuffers Rust code be managed?

A) Build-time generation — `build.rs` calls `flatc` compiler during `cargo build` (requires `flatc` in PATH/Docker image)
B) Pre-generated — run `flatc` manually, commit generated code to repo (simpler CI, no build-time dep)
C) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Question 4
How should authentication failures be handled when Redis cache is unavailable?

A) Degrade gracefully — validate JWT inline (no cache), allow auth to succeed (slightly slower but resilient)
B) Fail closed — reject auth if Redis is unreachable (strict security: no cache = no auth)
C) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Question 5
For the NATS client, which connection model should be used?

A) Single shared connection per pod (NATS multiplexes internally — recommended by NATS docs)
B) Connection pool with configurable size
C) Other (please describe after [Answer]: tag below)

[Answer]: B
