# Unit of Work — Dependency Matrix

## Dependency Table

| Unit | Name | Depends On | Blocks |
|------|------|-----------|--------|
| 1 | Core Bug Fixes | (none) | 2 |
| 2 | Configuration Layer | 1 | 3, 4, 5, 6 |
| 3 | FlatBuffers Protocol | 2 | 4, 5 |
| 4 | JWT Authentication | 2, 3 | 6, 7 |
| 5 | NATS JetStream | 2, 3 | 7, 8 |
| 6 | Redis Cache | 2, 4 | 7 |
| 7 | Observability | 4, 5, 6 | 8 |
| 8 | Graceful Shutdown | 5, 6, 7 | 9 |
| 9 | K8s Deployment | 1-8 (all) | — |

## Dependency Graph

```
Unit 1: Core Fixes
    |
    v
Unit 2: Config
    |
    +----------+----------+
    |          |          |
    v          v          v
Unit 3:    Unit 4:    Unit 5:
FlatBuf    JWT Auth   NATS JS
    |          |          |
    +----+-----+    +-----+
         |          |
         v          v
      Unit 4:    Unit 5: (continued)
         |          |
         +----+-----+
              |
              v
           Unit 6:
          Redis Cache
              |
              v
           Unit 7:
        Observability
              |
              v
           Unit 8:
       Graceful Shutdown
              |
              v
           Unit 9:
          K8s Deploy
```

## Parallel Execution Opportunities

After Unit 2 (Config) completes, these units CAN be developed in parallel by different team members:
- **Track A**: Unit 3 (FlatBuffers) → Unit 4 (JWT, depends on 3)
- **Track B**: Unit 5 (NATS JetStream, depends on 3 for FlatBuffers encoding in NATS)

After Units 4, 5 complete:
- **Track C**: Unit 6 (Redis Cache) can proceed
- Unit 7 waits for 4 + 5 + 6

For a single developer, the strict sequential order is: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9

## Critical Path

The critical path (longest dependency chain):
```
1 → 2 → 3 → 4 → 6 → 7 → 8 → 9   (8 units deep)
```
OR
```
1 → 2 → 3 → 5 → 7 → 8 → 9        (7 units deep)
```

## Shared Resource Conflicts

| Resource | Units That Use It | Conflict Risk |
|----------|------------------|---------------|
| `src/server/multi_stream_server.rs` | 1, 2, 4, 5, 6, 7, 8 | High — central file, sequence carefully |
| `src/server/connection_handler.rs` | 1, 3, 4, 7 | Medium |
| `Cargo.toml` | 2, 3, 4, 5, 6, 7, 8 | Low — additive changes only |
| `src/main.rs` | 2, 7, 8 | Low — sequential additions |
| `src/lib.rs` | 4, 5, 6 | Low — additive re-exports |

## Integration Points

| Integration | Unit Pair | Description |
|-------------|-----------|-------------|
| FlatBuffers ↔ NATS | 3 + 5 | Messages serialized as FlatBuffers bytes stored in NATS JetStream |
| Auth ↔ Redis | 4 + 6 | JWT claims cached in Redis; auth degrades without Redis |
| NATS ↔ Health | 5 + 7 | Health check pings NATS stream for readiness |
| Redis ↔ Health | 6 + 7 | Health check pings Redis for readiness |
| Shutdown ↔ NATS | 8 + 5 | Graceful close flushes NATS publish queue |
| Shutdown ↔ Health | 8 + 7 | Readiness → 503 on shutdown signal |
