# Unit Test Execution

## Run All Unit Tests

```bash
cargo test
```

**Expected result:** `71 passed; 0 failed` (as of 2026-04-12)

## Run Tests for a Specific Module

```bash
cargo test config::          # config module tests
cargo test auth::            # JWT auth tests
cargo test storage::         # NATS + Redis client tests
cargo test transport::       # stream/shard/connection tests
cargo test server::          # room manager tests
cargo test shutdown::        # shutdown coordinator tests
```

## Run with Output Visible

```bash
cargo test -- --nocapture
```

## Test Coverage by Module

| Module | Tests | Coverage Areas |
|---|---|---|
| `config` | 5 | TOML parsing, validation, invalid URLs |
| `auth` | ~8 | JWT decode, HS256/RS256, expiry, caching |
| `protocol` | ~6 | FlatBuffer encode/decode round-trips |
| `transport::streams` | 5 | Stream state machine, stats |
| `transport::shards` | 5 | Routing, hot room promotion, shard stats |
| `transport::connection` | 1 | Connection builder |
| `storage::nats` | 1 | Unreachable server error path |
| `storage::redis` | 1 | Unreachable server error path |
| `server::room_manager` | ~10 | Room lifecycle, membership, roles |
| `shutdown` | 5 | Coordinator counting, drain, signal broadcast |
| **Total** | **71** | |

## Fix Failing Tests

1. Run `cargo test 2>&1 | grep FAILED` to isolate failures
2. Run the specific failing test with output: `cargo test <test_name> -- --nocapture`
3. Fix the code, rerun until all 71 pass
