# Unit 5: NATS JetStream Storage — Code Generation Plan

## Status: COMPLETED

## Steps

- [x] Add `async-nats = "0.35"` and `futures-core = "0.3"` to `Cargo.toml`
- [x] Create `src/storage/mod.rs` (module declarations and re-exports)
- [x] Create `src/storage/nats.rs`:
  - [x] `RoomState` struct (Serialize/Deserialize)
  - [x] `RoomSubscription` struct + `next()` async (cross-node, filtered by `Server-Id`)
  - [x] `NatsClient` struct with manual `Debug` impl
  - [x] `NatsClient::connect()` — creates stream + KV bucket
  - [x] `NatsClient::publish_message()` — JetStream publish with headers + ACK wait
  - [x] `NatsClient::get_history()` — ephemeral pull consumer, poll_fn stream drain
  - [x] `NatsClient::save_message_room()` / `get_room_id_for_message()` — BUG-001 fix
  - [x] `NatsClient::save_room_state()` / `get_room_state()` — KV persistence
  - [x] `NatsClient::subscribe_rooms()` — NATS Core wildcard subscription
  - [x] `NatsClient::health_check()` — flush-based liveness probe
  - [x] Unit tests: `test_connect_unreachable_fails`, `test_room_state_serialise_roundtrip`
- [x] Update `src/lib.rs` — add `pub mod storage`, re-export `NatsClient`, `RoomState`
- [x] Update `src/server/multi_stream_server.rs`:
  - [x] Add `nats_client: Option<Arc<NatsClient>>` field
  - [x] Make `from_config()` async — non-fatal NATS connection attempt
  - [x] `handle_send_message()` — publish to NATS before ACK
  - [x] `handle_join_room()` — prefer NATS history over in-memory
  - [x] `handle_create_room()` — save `RoomState` to KV
  - [x] `room_id_for_message()` helper — KV lookup (BUG-001)
  - [x] `handle_edit_message`, `handle_delete_message`, reactions — use BUG-001 fix
  - [x] `run_nats_subscription()` background task for cross-node delivery
- [x] Update `src/main.rs` — `from_config().await?`

## Notes

- Used `futures-core` + `std::future::poll_fn` instead of `futures-util` to avoid
  transitive `futures-macro 0.3.32` conflict with local crate index (max 0.3.31)
- `max_messages_per_subject` (not `max_msgs_per_subject`) is the correct stream config field
- Config test `test_load_valid_config` is a pre-existing flaky test (shared temp file path
  across parallel test processes); isolated run passes cleanly
