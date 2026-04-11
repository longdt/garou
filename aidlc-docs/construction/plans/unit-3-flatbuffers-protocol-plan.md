# Unit 3 — FlatBuffers Protocol Layer: Code Generation Plan

## Status: COMPLETED

## Objective
Replace JSON wire encoding with FlatBuffers binary format for all 40+ message types. Provide a `debug-json` Cargo feature to fall back to JSON for debugging.

## Execution Checklist

- [x] Write FlatBuffers schema files (`fbs/`)
  - [x] `fbs/common.fbs` — `UserInfo` table
  - [x] `fbs/control.fbs` — Hello, HelloAck, Auth, AuthOk, AuthFailed, Ping, Pong, Throttle, Goodbye, ServerCommand
  - [x] `fbs/chat.fbs` — SendMessage, EditMessage, DeleteMessage, AddReaction, RemoveReaction, JoinRoom, LeaveRoom, CreateRoom
  - [x] `fbs/room.fbs` — RoomMessage, RoomMessageEdited, RoomMessageDeleted, RoomReactionAdded, RoomReactionRemoved, RoomUserJoined, RoomUserLeft, RoomInit, RoomClose
  - [x] `fbs/shard.fbs` — ShardStreamInfo, ShardAssignment, RoomPromoted, RoomDemoted
  - [x] `fbs/ack.fbs` — MessageDelivered, MessageRead, MessageAck
  - [x] `fbs/upload.fbs` — UploadStart, UploadChunk, UploadComplete, UploadCancel, UploadAck
  - [x] `fbs/presence.fbs` — Typing, StopTyping, PresenceOnline, PresenceOffline, PresenceAway
  - [x] `fbs/error.fbs` — Error

- [x] Add `flatbuffers = "24"` dependency and `debug-json = []` feature to `Cargo.toml`

- [x] Create `build.rs` to invoke `flatc --rust --gen-mutable --gen-name-strings`
  - [x] Download flatc 24.3.25 binary (sudo not available)
  - [x] Post-process generated files to fix `use crate::` → `use super::/super::super::` import paths

- [x] Generate Rust bindings and commit to `src/protocol/generated/`
  - [x] `mod.rs` with `#[allow]` attributes for generated code warnings
  - [x] All 9 generated modules

- [x] Rewrite `src/protocol/codec.rs` with dual backends
  - [x] FlatBuffers backend (default, `#[cfg(not(feature = "debug-json"))]`)
  - [x] JSON backend (`#[cfg(feature = "debug-json")]`)
  - [x] 43 `enc_xxx`/`dec_xxx` function pairs in `mod fb`
  - [x] `impl_fb_codec!` macro wiring
  - [x] 6 FlatBuffers round-trip tests

- [x] Fix `size`/`size_` naming issue (FlatBuffers renames `size` → `size_` to avoid trait collision)

- [x] Verify `cargo check` passes (0 errors)
- [x] Verify `cargo test` passes (49/49 tests)

## Key Decisions
- Optional scalar fields encoded as `has_xxx: bool` + `xxx: ulong` (compatible with flatc 24, avoids newer `= null` syntax)
- `serde_json::Value` in `ServerCommand.params` encoded as JSON string inside FlatBuffer
- flatc binary downloaded to `/tmp/flatc-bin/flatc` to avoid interactive sudo
