# Requirements Clarification Questions

Please answer each question by filling in the letter choice after the `[Answer]:` tag.
If none of the options match your needs, choose the last option (Other) and describe your preference.
Let me know when you're done.

---

## Question 1
What is the target scale for this production chat server (concurrent connections)?

A) Small scale: up to 1,000 concurrent users (single server, no clustering needed)
B) Medium scale: up to 10,000 concurrent users (single powerful server)
C) Large scale: up to 100,000+ concurrent users (requires clustering/sharding)
D) Other (please describe after [Answer]: tag below)

[Answer]: C

---

## Question 2
What authentication mechanism should be used?

A) Token-based (JWT) — clients present a JWT token from an external auth service
B) Username/password — server validates credentials against a user store
C) API key — simple pre-shared key per client
D) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Question 3
What persistence/storage backend should be used for messages and room state?

A) In-memory only (fast, no persistence — acceptable for PoC or ephemeral chat)
B) Redis (fast cache-first with optional persistence via RDB/AOF)
C) PostgreSQL (relational, full durability, ACID transactions)
D) Other (please describe after [Answer]: tag below)

[Answer]: D. I'm not sure if we need a full relational database like PostgreSQL, but we do want durability and persistence. Maybe something like RockDB or a lightweight embedded database could work? Or perhaps a log-based storage system optimized for append-only workloads? We should evaluate options that provide durability without the overhead of a full RDBMS if our use case is primarily chat messages and room state.

---

## Question 4
What serialization format should be used for the wire protocol?

A) Keep JSON (current) — human-readable, easier debugging
B) MessagePack — binary, fast, ~2-3x more compact than JSON
C) FlatBuffers — zero-copy, ultra-low-latency for hot paths
D) Other (please describe after [Answer]: tag below)

[Answer]: C

---

## Question 5
Should the server support horizontal scaling (multiple server instances)?

A) No — single server instance is fine for our scale
B) Yes — need distributed state (Redis pub/sub or similar for cross-node messaging)
C) Yes — need full cluster support with consistent hashing and node discovery
D) Other (please describe after [Answer]: tag below)

[Answer]: D. We want to support horizontal scaling, but I'm considering B or C options.

---

## Question 6
What should be done about TLS certificates in production?

A) Keep self-signed (acceptable for internal/dev use)
B) Load from file (PEM cert + key files specified via config)
C) Let's Encrypt / ACME auto-provisioning
D) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Question 7
Should observability/metrics be added?

A) Yes — Prometheus metrics endpoint + structured logging
B) Yes — structured logging (JSON) only, no metrics endpoint
C) No — basic logging is sufficient
D) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Question 8
Should rate limiting be implemented to protect against abuse?

A) Yes — per-connection rate limiting with configurable limits
B) Yes — per-user rate limiting (requires auth)
C) No — not needed for our use case
D) Other (please describe after [Answer]: tag below)

[Answer]: C

---

## Question 9
How should the server configuration be managed?

A) TOML configuration file + CLI overrides
B) Environment variables only
C) Keep existing CLI args only
D) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Question: Security Extensions
Should security extension rules be enforced for this project?

A) Yes — enforce all SECURITY rules as blocking constraints (recommended for production-grade applications)
B) No — skip all SECURITY rules (suitable for PoCs, prototypes, and experimental projects)
X) Other (please describe after [Answer]: tag below)

[Answer]: A

---

## Question: Property-Based Testing Extension
Should property-based testing (PBT) rules be enforced for this project?

A) Yes — enforce all PBT rules as blocking constraints (recommended for projects with business logic, data transformations, serialization, or stateful components)
B) Partial — enforce PBT rules only for pure functions and serialization round-trips
C) No — skip all PBT rules
X) Other (please describe after [Answer]: tag below)

[Answer]: A
