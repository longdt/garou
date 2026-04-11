# Unit 4 — JWT Authentication: Code Generation Plan

## Status: COMPLETED

## Objective
Replace the auth stub in `ConnectionHandler` with real JWT validation (HS256 and RS256). No unauthenticated commands accepted after this unit.

## Execution Checklist

- [x] Add `jsonwebtoken = "9.3"` to `Cargo.toml`

- [x] Create `src/auth/mod.rs`
  - [x] `AuthClaims` struct: `sub`, `username`, `exp`, `iat`; `user_id()` helper
  - [x] `AuthValidator` struct with `Debug` impl (manual — `DecodingKey` doesn't derive `Debug`)
  - [x] `AuthValidator::new_hs256(secret)` — creates HS256 validator
  - [x] `AuthValidator::new_rs256(pem)` — creates RS256 validator from PEM public key
  - [x] `AuthValidator::from_config(&AuthSettings)` — dev fallback when secret is empty
  - [x] `AuthValidator::validate(token)` — dispatches by expiry/signature error kind

- [x] Declare `pub mod auth;` in `src/lib.rs`; re-export `AuthValidator`, `AuthClaims`

- [x] Update `ConnectionHandler`
  - [x] Add `auth_validator: Arc<AuthValidator>` field
  - [x] Update `new()` to accept `auth_validator` parameter
  - [x] Replace `authenticate()` stub — `"token"` method delegates to `auth_validator.validate()`; `"username"` method kept for dev
  - [x] Auth failure path: send `AuthFailed` frame, close connection, return error
  - [x] Update `ConnectionHandlerBuilder` to hold `auth_validator` with dev fallback

- [x] Update `MultiStreamServer`
  - [x] Add `auth_validator: Arc<AuthValidator>` field
  - [x] Update `new()` to take `Arc<AuthValidator>`
  - [x] `with_defaults()` creates dev-only validator
  - [x] `from_config()` builds validator via `AuthValidator::from_config(&cfg.auth)?`
  - [x] Pass `Arc::clone(&self.auth_validator)` to `ConnectionHandler::new()` in `handle_incoming()`

- [x] Fix `examples/basic_usage.rs` to use `MultiStreamServer::with_defaults()`

- [x] Fix `test_hs256_expired_token` to use `-120s` offset (past the 60s jsonwebtoken leeway)

- [x] Verify `cargo check` passes (0 errors)
- [x] Verify `cargo test` passes (60/60 tests)

## Key Decisions
- `authenticate()` changed from `async fn` to `fn` — JWT validation is pure computation; becomes async in Unit 6 when Redis cache is added
- `jsonwebtoken` default 60s leeway retained for production clock-skew tolerance; tests use `-120s`
- `"username"` dev method retained so existing integration paths keep working without a real JWT
- `ConnectionHandlerBuilder` updated with `with_auth_validator()` for programmatic construction
