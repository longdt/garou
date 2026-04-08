# Requirements Clarification Questions

I detected ambiguities in your responses that need clarification before I can proceed.

---

## Ambiguity 1: Persistence Storage (Q3 — "Other")

You want durability without full RDBMS overhead, and mentioned RocksDB, lightweight embedded DB, or append-only log storage. For a 100,000+ user chat server with horizontal scaling, the storage choice significantly impacts the architecture.

### Clarification Question 1
Which persistence approach fits best?

A) **RocksDB (embedded, per node)** — Each server node stores its own shard of messages locally in RocksDB. Fast writes, compaction handles old data. Requires consistent hashing to route users to the right node. Good fit for append-heavy chat workloads.

B) **NATS JetStream (distributed log)** — NATS server handles message persistence with a distributed log. Chat nodes are stateless and publish/subscribe via NATS. Excellent for horizontal scaling — add/remove nodes freely. Also solves Q5 cross-node messaging.

C) **Redis (AOF persistence)** — Use Redis as both the message store and pub/sub bus (solves Q3 + Q5 together). Redis AOF provides durability. Redis Cluster handles horizontal scale. Simpler ops than RocksDB.

D) **SQLite (per node, WAL mode)** — Lightweight embedded SQL. WAL mode gives good read/write concurrency. Simple, no external deps. Better for smaller nodes; less optimal at extreme scale.

E) Other (please describe after [Answer]: tag below)

[Answer]: E. Give me any advice based on your experience with similar systems. I'm open to other storage solutions that provide durability and are optimized for chat workloads, especially if they can simplify the architecture or improve performance at scale. Chat server need to deploy on K8s

---

## Ambiguity 2: Horizontal Scaling Approach (Q5 — "considering B or C")

You want horizontal scaling for 100,000+ users but weren't sure whether Redis pub/sub or full cluster support is needed. This choice drives the entire distributed architecture.

### Clarification Question 2
Which horizontal scaling model fits your operational constraints?

A) **Redis pub/sub for cross-node messaging** (simpler) — Chat nodes are mostly stateless; Redis acts as the message bus between nodes. Standard Redis Cluster for HA. Easier to operate than a custom cluster protocol. Suitable for most production deployments.

B) **Full peer-to-peer cluster** (complex) — Chat nodes discover each other, use consistent hashing to partition users/rooms, communicate directly. No single point of failure on the message bus. Higher operational complexity.

C) **NATS as the message bus** — NATS (or NATS JetStream) handles both pub/sub routing between nodes and optional persistence. Chat nodes are fully stateless. Easy horizontal scale, built-in load balancing.

D) Other (please describe after [Answer]: tag below)

[Answer]: D. Give me any advice based on your experience with similar systems. I'm open to other storage solutions that provide durability and are optimized for chat workloads, especially if they can simplify the architecture or improve performance at scale. Chat server need to deploy on K8s
