# Unit of Work — Story Map

Note: User Stories phase was skipped (internal refactoring, no user persona work). This story map lists the technical requirements (FR-*/BUG-*) mapped to units of work.

## Requirement-to-Unit Mapping

| Requirement ID | Requirement Name | Unit(s) |
|---------------|-----------------|---------|
| BUG-001 | room_id=0 in edit/delete/reaction | 1 (placeholder), 5 (fix) |
| BUG-002 | clone_ref() Arc antipattern | 1 |
| BUG-003 | Vec::remove(0) O(n) message buffer | 1 |
| BUG-004 | Unbounded channels (no backpressure) | 1 |
| FR-002 | TOML configuration file + CLI overrides | 2 |
| FR-005 | FlatBuffers wire protocol | 3 |
| FR-001 | JWT authentication | 4 |
| FR-003 | Message persistence via NATS JetStream | 5 |
| FR-004 | Cross-node pub/sub via NATS | 5 |
| FR-011 | Redis caching layer | 6 |
| FR-006 | Prometheus metrics | 7 |
| FR-007 | Structured JSON logging | 7 |
| FR-008 | Health check endpoints | 7 |
| FR-010 | Graceful shutdown | 8 |
| NFR-005 | Kubernetes compatibility | 9 |

## Unit Summary

| Unit | Requirements Addressed | Deliverable |
|------|----------------------|-------------|
| Unit 1 | BUG-001 (partial), BUG-002, BUG-003, BUG-004 | Clean, correct single-node server |
| Unit 2 | FR-002 | Config-driven server with TOML file |
| Unit 3 | FR-005 | FlatBuffers wire protocol (all 40+ messages) |
| Unit 4 | FR-001 | JWT auth enforced on all connections |
| Unit 5 | FR-003, FR-004, BUG-001 (complete) | Persistent, horizontally scalable message delivery |
| Unit 6 | FR-011 | Redis JWT cache + presence + roster |
| Unit 7 | FR-006, FR-007, FR-008 | Full observability stack |
| Unit 8 | FR-010 | K8s-compatible graceful shutdown |
| Unit 9 | NFR-005 | Production container + K8s manifests |

## Technical Capability Progression

After each unit, the server gains a new production capability:

```
Unit 1: Bug-free in-memory chat server (correct, no data loss within session)
  +
Unit 2: Configurable (TOML config, no hardcoded values)
  +
Unit 3: Efficient binary protocol (FlatBuffers, zero-copy decode)
  +
Unit 4: Secure (JWT auth enforced, no anonymous access)
  +
Unit 5: Durable + Distributed (NATS: messages survive restart, multi-node)
  +
Unit 6: Fast (Redis cache reduces latency on auth + broadcast)
  +
Unit 7: Observable (Prometheus metrics, JSON logs, health probes)
  +
Unit 8: Operationally safe (graceful SIGTERM, no message loss on rolling update)
  +
Unit 9: K8s-deployable (Dockerfile, manifests, HPA, ServiceMonitor)
       = PRODUCTION READY
```

## NFR Coverage Map

| NFR | Requirement | Covered By |
|-----|------------|-----------|
| NFR-001 | Performance (100k+ users, p99 < 10ms) | Unit 1 (AtomicU64, VecDeque), Unit 3 (FlatBuffers), Unit 6 (Redis cache) |
| NFR-002 | Reliability (no message loss) | Unit 5 (NATS JetStream persist-before-ACK) |
| NFR-003 | Security (JWT, TLS) | Unit 4 (JWT), Unit 2 (config for TLS paths) |
| NFR-004 | Observability | Unit 7 (Prometheus + JSON logs) |
| NFR-005 | K8s compatibility | Unit 7 (health probes), Unit 8 (graceful shutdown), Unit 9 (manifests) |

## Security Baseline Coverage

| Security Rule | Unit | Implementation |
|--------------|------|----------------|
| Authentication required | 4 | JWT validated before any command |
| TLS on all connections | 2 | Config-driven cert loading |
| No secrets in code | 2 | All secrets in config / K8s Secrets |
| Input validation | 3 | FlatBuffers verifier rejects malformed messages |
| No unauthenticated data access | 4 | ConnectionHandler blocks commands pre-auth |

## Property-Based Testing Coverage

| PBT Target | Unit | Property |
|-----------|------|---------|
| FlatBuffers codec | 3 | encode(x) → decode → x (all message types) |
| Frame codec | 3 | any byte sequence → no panic in FrameCodec |
| JWT validation | 4 | wrong signature always fails |
| NATS publish | 5 | concurrent publishes → no duplicate sequence numbers |
| ShardRouter | 1 | room_shard(id) always in [0, NUM_SHARDS) |
| Bounded channels | 1 | channel never grows beyond capacity under load |
