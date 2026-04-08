# Unit 1: Core Bug Fixes — Code Generation Plan

## Unit Context
- **Requirements**: BUG-001 (partial), BUG-002, BUG-003, BUG-004
- **Dependencies**: None
- **Risk**: Low — mechanical fixes, no new logic

## Steps

### Step 1: Fix BUG-003 — VecDeque in room_manager.rs
- [x] Replace `Vec<RoomMessage>` with `VecDeque<RoomMessage>` in `Room` struct
- [x] Update `add_message()`: use `push_back()` + `pop_front()`
- [x] Update import to include `VecDeque`
- [x] Verify `get_recent_messages()` still works (iterates `.iter().rev()`)
- [x] Update tests that construct `RoomMessage` directly

### Step 2: Fix BUG-002 + AtomicU64 — multi_stream_server.rs
- [x] Change `next_message_id: Arc<RwLock<MessageId>>` to `Arc<AtomicU64>`
- [x] Update `handle_send_message()` to use `fetch_add(1, Ordering::Relaxed)`
- [x] Change `start(mut self)` to create `Arc<Self>` after endpoint setup, remove `clone_ref()`
- [x] Update `accept_connections` to `self: &Arc<Self>` pattern
- [x] Update `handle_incoming` to `self: &Arc<Self>` pattern
- [x] Remove `clone_ref()` method entirely
- [x] Add `use std::sync::atomic::{AtomicU64, Ordering};`

### Step 3: Fix BUG-004 — bounded channels in multi_stream_server.rs + connection_handler.rs
- [x] Change `mpsc::unbounded_channel()` to `mpsc::channel(1024)` for event + command channels
- [x] Update `ActiveConnection.command_tx` type to `mpsc::Sender<ConnectionCommand>`
- [x] Update `ConnectionHandler` field `event_tx` to `mpsc::Sender<ServerEvent>`
- [x] Update `ConnectionHandler` field `command_rx` to `mpsc::Receiver<ConnectionCommand>`
- [x] Update `ConnectionHandler::new()` + builder signature to accept bounded types
- [x] Replace `.send()` calls with `.try_send()` for fire-and-forget sends

### Step 4: Fix BUG-001 partial — remove room_id: 0 hardcodes
- [x] `handle_edit_message`: replaced with TODO(Unit 5) placeholder returning `Ok(())`
- [x] `handle_delete_message`: same
- [x] `handle_add_reaction`: same
- [x] `handle_remove_reaction`: same

### Step 5: Verify compilation + run tests
- [x] `cargo check` passes with no errors
- [x] `cargo test` — all 40 tests pass
