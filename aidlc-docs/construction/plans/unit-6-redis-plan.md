# Unit 6: Redis Cache Layer — Code Generation Plan

## Status: COMPLETED

## Steps

- [x] Add `fred = { version = "9", features = ["i-keys", "i-sets"] }` to `Cargo.toml`
- [x] Create `src/storage/redis.rs`:
  - [x] `RedisClient` struct wrapping `RedisPool` with manual `Debug` impl
  - [x] `RedisClient::connect(url, pool_size, jwt_cache_ttl_secs)` — pool init, non-fatal pattern
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

- Uses `fred = "10"` (latest); feature `tokio-runtime` does NOT exist — use `i-keys` + `i-sets`
- fred 10 renames: `RedisPool` → `Pool`, `RedisConfig` → `Config`, `RedisError` → `Error` (all in prelude)
- fred 10 key arguments must be `&str` (not `&String`); use `.as_str()` on owned String keys
- `Builder::from_config(config).build_pool(n)?` creates the pool (NOT `.set_pool_size(n).build_pool()`)
- `pool.init().await?` is async and returns `Result<(), Error>` for `Pool`
- `validate()` kept sync for tests; `validate_async()` adds the caching layer on top
- All Redis operations are non-fatal: failures logged at WARN, callers receive Ok
