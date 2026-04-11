# Unit 2: Configuration Layer — Code Generation Plan

## Unit Context
- **Requirements**: FR-008 (config loading), CLI overrides
- **Dependencies**: Unit 1 (completed)
- **Risk**: Low

## Steps

### Step 1: Add `toml` dependency to Cargo.toml
- [x] Add `toml = "0.8"` to `[dependencies]`

### Step 2: Create `src/config/mod.rs`
- [x] Define `Config`, `ServerSettings`, `AuthSettings`, `NatsSettings`, `RedisSettings`, `MetricsSettings`, `ShardSettings`, `ProtocolSettings` structs (all with serde Deserialize + Default)
- [x] Implement `Config::load(path)` — reads file, deserializes TOML, calls validate
- [x] Implement `Config::validate()` — checks required fields, returns `ChatError::Config` on invalid
- [x] Unit tests: valid config parses; invalid URL returns error

### Step 3: Update `src/server/multi_stream_server.rs`
- [x] Keep existing `ServerConfig` struct in place (used internally); add `MultiStreamServer::from_config(cfg)` constructor that maps `Config` → `ServerConfig`
- [x] Pass bind_addr, max_connections, idle_timeout, shard settings, datagram flag from config

### Step 4: Update `src/transport/shards.rs`
- [x] No changes needed — `ShardConfig` already has sensible defaults; mapping is done in Step 3

### Step 5: Update `src/main.rs`
- [x] Add `--config <path>` CLI parsing
- [x] On `server` command: if `--config` provided, load via `Config::load()`; else build default `Config`
- [x] Pass port/max-conn CLI overrides into config before use
- [x] Update `run_server` to accept `Config` and use `MultiStreamServer::from_config()`
- [x] Use config for info logging

### Step 6: Create `config.toml.example`
- [x] Document every field with type, description, and default value

### Step 7: Verify compilation + run tests
- [x] `cargo check` passes
- [x] `cargo test` — all 45 tests pass (5 new config tests added)
