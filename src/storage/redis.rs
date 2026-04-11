//! Redis cache client for JWT caching, user presence, and room rosters.
//!
//! Uses the `redis` crate with `ConnectionManager` (auto-reconnecting multiplexed
//! connection). All operations are non-fatal: failures are logged at WARN level
//! so the server degrades gracefully when Redis is unavailable.

use std::time::Duration;

use redis::AsyncCommands;
use redis::aio::ConnectionManager;
use tracing::{debug, info, warn};

use crate::auth::AuthClaims;
use crate::error::{ChatError, Result};
use crate::protocol::messages::{RoomId, UserId};

/// Presence TTL: 5 minutes idle before key expires.
const PRESENCE_TTL_SECS: u64 = 300;

// ---------------------------------------------------------------------------
// RedisClient
// ---------------------------------------------------------------------------

/// Auto-reconnecting Redis client for JWT caching, presence, and room rosters.
///
/// Built on `redis::aio::ConnectionManager` which multiplexes commands over a
/// single connection and reconnects transparently on failure.
pub struct RedisClient {
    conn: ConnectionManager,
    jwt_cache_ttl_secs: u64,
}

impl std::fmt::Debug for RedisClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisClient")
            .field("jwt_cache_ttl_secs", &self.jwt_cache_ttl_secs)
            .finish_non_exhaustive()
    }
}

impl RedisClient {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Connect to Redis and return an auto-reconnecting client.
    ///
    /// Returns `Err(ChatError::Network)` if the URL is invalid or the server
    /// is unreachable on the initial connection attempt.
    pub async fn connect(url: &str, jwt_cache_ttl_secs: u64) -> Result<Self> {
        let client = redis::Client::open(url)
            .map_err(|e| ChatError::Network(format!("invalid Redis URL '{}': {}", url, e)))?;

        let conn = tokio::time::timeout(Duration::from_secs(5), ConnectionManager::new(client))
            .await
            .map_err(|_| ChatError::Network(format!("Redis connect timed out for '{}'", url)))?
            .map_err(|e| ChatError::Network(format!("Redis connect failed: {}", e)))?;

        info!("Redis connected (url={})", url);
        Ok(Self { conn, jwt_cache_ttl_secs })
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Derive a stable cache key from a JWT string.
    ///
    /// Uses the signature segment (after the last `.`) which is globally unique
    /// per token — no extra hash dependency required.
    fn jwt_key(token: &str) -> String {
        let sig = token.rfind('.').map(|i| &token[i + 1..]).unwrap_or(token);
        format!("jwt:{}", sig)
    }

    // -----------------------------------------------------------------------
    // JWT claim cache
    // -----------------------------------------------------------------------

    /// Return cached `AuthClaims` for a JWT, or `None` on a cache miss or error.
    pub async fn get_cached_claims(&self, token: &str) -> Option<AuthClaims> {
        let key = Self::jwt_key(token);
        let mut conn = self.conn.clone();
        let result: redis::RedisResult<Option<String>> = conn.get(&key).await;
        match result {
            Ok(Some(json)) => match serde_json::from_str::<AuthClaims>(&json) {
                Ok(claims) => {
                    debug!("JWT cache hit");
                    Some(claims)
                }
                Err(e) => {
                    warn!("JWT cache deserialize failed: {}", e);
                    None
                }
            },
            Ok(None) => None,
            Err(e) => {
                warn!("Redis JWT cache get failed: {}", e);
                None
            }
        }
    }

    /// Store `AuthClaims` in Redis for a JWT.
    ///
    /// TTL is either the configured `jwt_cache_ttl_secs` or derived from the
    /// token's `exp` claim (when `jwt_cache_ttl_secs == 0`).
    /// Failures are silently logged.
    pub async fn cache_claims(&self, token: &str, claims: &AuthClaims) {
        let ttl: u64 = if self.jwt_cache_ttl_secs > 0 {
            self.jwt_cache_ttl_secs
        } else {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if claims.exp > now { claims.exp - now } else { return }
        };

        let json = match serde_json::to_string(claims) {
            Ok(j) => j,
            Err(e) => { warn!("JWT cache serialize failed: {}", e); return; }
        };

        let key = Self::jwt_key(token);
        let mut conn = self.conn.clone();
        if let Err(e) = conn.set_ex::<_, _, ()>(&key, &json, ttl).await {
            warn!("Redis JWT cache set failed: {}", e);
        }
    }

    // -----------------------------------------------------------------------
    // Presence
    // -----------------------------------------------------------------------

    /// Set a user's presence status. TTL resets on every update (sliding window).
    pub async fn set_presence(&self, user_id: UserId, status: &str) {
        let key = format!("presence:{}", user_id);
        let mut conn = self.conn.clone();
        if let Err(e) = conn.set_ex::<_, _, ()>(&key, status, PRESENCE_TTL_SECS).await {
            warn!("Redis presence set failed for user {}: {}", user_id, e);
        }
    }

    /// Delete a user's presence key (called on disconnect).
    pub async fn del_presence(&self, user_id: UserId) {
        let key = format!("presence:{}", user_id);
        let mut conn = self.conn.clone();
        if let Err(e) = conn.del::<_, ()>(&key).await {
            warn!("Redis presence del failed for user {}: {}", user_id, e);
        }
    }

    // -----------------------------------------------------------------------
    // Room rosters
    // -----------------------------------------------------------------------

    /// Add a user to a room's roster set.
    pub async fn add_to_roster(&self, room_id: RoomId, user_id: UserId) {
        let key = format!("roster:{}", room_id);
        let mut conn = self.conn.clone();
        if let Err(e) = conn.sadd::<_, _, ()>(&key, user_id.to_string()).await {
            warn!("Redis roster add failed room={} user={}: {}", room_id, user_id, e);
        }
    }

    /// Remove a user from a room's roster set.
    pub async fn remove_from_roster(&self, room_id: RoomId, user_id: UserId) {
        let key = format!("roster:{}", room_id);
        let mut conn = self.conn.clone();
        if let Err(e) = conn.srem::<_, _, ()>(&key, user_id.to_string()).await {
            warn!("Redis roster remove failed room={} user={}: {}", room_id, user_id, e);
        }
    }

    /// Remove a user from every room in `room_ids` (called on disconnect).
    pub async fn remove_from_all_rosters(&self, user_id: UserId, room_ids: &[RoomId]) {
        for &room_id in room_ids {
            self.remove_from_roster(room_id, user_id).await;
        }
    }

    // -----------------------------------------------------------------------
    // Health
    // -----------------------------------------------------------------------

    /// Returns `true` if the Redis connection is alive.
    pub async fn health_check(&self) -> bool {
        let mut conn = self.conn.clone();
        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .is_ok()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_key_uses_signature_segment() {
        let token = "header.payload.signature";
        assert_eq!(RedisClient::jwt_key(token), "jwt:signature");
    }

    #[test]
    fn test_jwt_key_no_dots() {
        let token = "nodots";
        assert_eq!(RedisClient::jwt_key(token), "jwt:nodots");
    }

    #[test]
    fn test_jwt_key_real_token_shape() {
        let token = "eyJhbGc.eyJzdWIiOiI0MiJ9.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let key = RedisClient::jwt_key(token);
        assert!(key.starts_with("jwt:"));
        assert!(key.contains("SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"));
    }

    /// Connecting to an unreachable Redis server must fail with `ChatError::Network`.
    #[tokio::test]
    async fn test_connect_unreachable_fails() {
        let result = RedisClient::connect("redis://127.0.0.1:16379", 300).await;
        assert!(result.is_err(), "should fail when Redis unreachable");
        let err = result.unwrap_err();
        assert!(
            matches!(err, ChatError::Network(_)),
            "expected ChatError::Network, got: {:?}",
            err
        );
    }
}
