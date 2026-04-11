# Unit 6: Redis Cache Layer — Code Generation Plan

## Status: COMPLETED

## Steps

- [x] Add `redis = { version = "0.27", features = ["tokio-comp", "connection-manager"] }` to `Cargo.toml`
- [x] Add `v7` feature to `uuid` dependency in `Cargo.toml`
- [x] Create `src/storage/redis.rs`:
  - [x] `RedisClient` struct wrapping `ConnectionManager` with manual `Debug` impl
  - [x] `RedisClient::connect(url, jwt_cache_ttl_secs)` — auto-reconnecting connection, timeout-wrapped
  - [x] `jwt_key(token)` helper — uses JWT signature segment as cache key (no extra dep)
  - [x] `get_cached_claims(token)` — GET + deserialize AuthClaims JSON
  - [x] `cache_claims(token, claims)` — SET EX with configurable or exp-derived TTL
  - [x] `set_presence(user_id, status)` — SET EX(300) sliding window
  - [x] `del_presence(user_id)` — DEL on disconnect
  - [x] `add_to_roster(room_id, user_id)` — SADD
  - [x] `remove_from_roster(room_id, user_id)` — SREM
  - [x] `remove_from_all_rosters(user_id, room_ids)` — iterate SREM
  - [x] `health_check()` — GET nil check
  - [x] Unit tests: jwt_key variants, `test_connect_unreachable_fails`
- [x] Update `src/storage/mod.rs` — add `pub mod redis;`, re-export `RedisClient`
- [x] Update `src/auth/mod.rs`:
  - [x] Add `redis_client: Option<Arc<RedisClient>>` field
  - [x] Update `new_hs256()`, `new_rs256()` constructors with `redis_client: None`
  - [x] Add `with_redis_client(Arc<RedisClient>) -> Self` builder method
  - [x] Add `validate_async(token) -> Result<AuthClaims>` (cache-aware, calls `validate()`)
- [x] Update `src/server/connection_handler.rs`:
  - [x] Change `fn authenticate()` → `async fn authenticate()`
  - [x] Change `self.auth_validator.validate()` → `self.auth_validator.validate_async().await`
  - [x] Update call site: `self.authenticate(&auth)` → `self.authenticate(&auth).await`
- [x] Update `src/server/multi_stream_server.rs`:
  - [x] Add `redis_client: Option<Arc<RedisClient>>` field
  - [x] Initialize to `None` in `new()`
  - [x] `from_config()`: attempt Redis connection before NATS; wire into auth validator
  - [x] `handle_authenticated()`: `redis.set_presence(user_id, "online")`
  - [x] `handle_join_room()`: `redis.add_to_roster(room_id, user_id)`
  - [x] `handle_leave_room()`: `redis.remove_from_roster(room_id, user_id)`
  - [x] `cleanup_connection()`: `redis.del_presence(user_id)` + `redis.remove_from_all_rosters()`
- [x] Update `src/lib.rs` — re-export `RedisClient`
- [x] Verify `cargo test` passes (66 tests, 0 failures)

## Notes

- Uses `redis = "0.27"` (redis-rs, actively maintained); replaces abandoned `fred` crate
- `ConnectionManager` auto-reconnects and multiplexes over a single connection; `Clone` is cheap (Arc-backed)
- Commands require `&mut self` on the connection — callers clone: `let mut conn = self.conn.clone()`
- `ConnectionManager::new()` retries on failure rather than returning error; wrap with `tokio::time::timeout`
- UUID v7 (time-ordered) available via `uuid = { features = ["v7"] }` — used for unique temp file names in tests
- `validate()` kept sync for tests; `validate_async()` adds the caching layer on top
- All Redis operations are non-fatal: failures logged at WARN, callers receive Ok
