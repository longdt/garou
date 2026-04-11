//! Application configuration
//!
//! Loads server configuration from a TOML file with optional CLI overrides.

use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

use crate::error::{ChatError, Result};

/// Top-level configuration loaded from `config.toml`
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerSettings,
    pub auth: AuthSettings,
    pub nats: NatsSettings,
    pub redis: RedisSettings,
    pub metrics: MetricsSettings,
    pub shard: ShardSettings,
    pub protocol: ProtocolSettings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerSettings::default(),
            auth: AuthSettings::default(),
            nats: NatsSettings::default(),
            redis: RedisSettings::default(),
            metrics: MetricsSettings::default(),
            shard: ShardSettings::default(),
            protocol: ProtocolSettings::default(),
        }
    }
}

impl Config {
    /// Load configuration from a TOML file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let contents = fs::read_to_string(path.as_ref()).map_err(|e| {
            ChatError::Config(format!(
                "failed to read config file '{}': {}",
                path.as_ref().display(),
                e
            ))
        })?;

        let config: Config = toml::from_str(&contents).map_err(|e| {
            ChatError::Config(format!("failed to parse config file: {}", e))
        })?;

        config.validate()?;
        Ok(config)
    }

    /// Validate configuration fields.
    pub fn validate(&self) -> Result<()> {
        // Validate bind address
        self.server
            .bind_addr
            .parse::<SocketAddr>()
            .map_err(|e| ChatError::Config(format!("invalid server.bind_addr: {}", e)))?;

        // Validate NATS URL (must start with nats:// or tls://)
        if !self.nats.url.is_empty() {
            let url = &self.nats.url;
            if !url.starts_with("nats://") && !url.starts_with("tls://") {
                return Err(ChatError::Config(format!(
                    "invalid nats.url '{}': must start with nats:// or tls://",
                    url
                )));
            }
        }

        // Validate Redis URL
        if !self.redis.url.is_empty() {
            let url = &self.redis.url;
            if !url.starts_with("redis://") && !url.starts_with("rediss://") {
                return Err(ChatError::Config(format!(
                    "invalid redis.url '{}': must start with redis:// or rediss://",
                    url
                )));
            }
        }

        if self.shard.num_shards == 0 {
            return Err(ChatError::Config(
                "shard.num_shards must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    /// Return idle timeout as a `Duration`.
    pub fn idle_timeout(&self) -> Duration {
        Duration::from_secs(self.server.idle_timeout_secs)
    }
}

// ---------------------------------------------------------------------------
// Sub-sections
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerSettings {
    /// Socket address to bind (e.g. "0.0.0.0:4433")
    pub bind_addr: String,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Connection idle timeout in seconds
    pub idle_timeout_secs: u64,
    /// Enable QUIC datagrams
    pub enable_datagrams: bool,
    /// Graceful shutdown drain window in seconds
    pub shutdown_timeout_secs: u64,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:4433".to_string(),
            max_connections: 10_000,
            idle_timeout_secs: 300,
            enable_datagrams: true,
            shutdown_timeout_secs: 30,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AuthSettings {
    /// JWT signing secret (HS256)
    pub jwt_secret: String,
    /// JWT public key path for RS256 (optional, overrides jwt_secret)
    pub jwt_public_key_path: String,
    /// JWT algorithm: "HS256" or "RS256"
    pub jwt_algorithm: String,
}

impl Default for AuthSettings {
    fn default() -> Self {
        Self {
            jwt_secret: String::new(),
            jwt_public_key_path: String::new(),
            jwt_algorithm: "HS256".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NatsSettings {
    /// NATS server URL (e.g. "nats://localhost:4222")
    pub url: String,
    /// JetStream stream name for chat messages
    pub stream_name: String,
    /// Maximum messages retained per stream
    pub max_messages: i64,
    /// Maximum age of messages (seconds, 0 = unlimited)
    pub max_age_secs: u64,
}

impl Default for NatsSettings {
    fn default() -> Self {
        Self {
            url: "nats://localhost:4222".to_string(),
            stream_name: "GAROU_CHAT".to_string(),
            max_messages: 1_000_000,
            max_age_secs: 0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RedisSettings {
    /// Redis URL (e.g. "redis://localhost:6379")
    pub url: String,
    /// Connection pool size
    pub pool_size: usize,
    /// JWT claim cache TTL in seconds (0 = match token expiry)
    pub jwt_cache_ttl_secs: u64,
}

impl Default for RedisSettings {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            pool_size: 16,
            jwt_cache_ttl_secs: 0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MetricsSettings {
    /// HTTP bind address for /metrics and /health endpoints
    pub bind_addr: String,
}

impl Default for MetricsSettings {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:9090".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ShardSettings {
    /// Number of shard streams per connection
    pub num_shards: u8,
    /// Messages/sec threshold to promote a room to hot status
    pub hot_room_threshold: u32,
    /// Messages/sec threshold to demote a hot room back to shard
    pub cool_down_threshold: u32,
    /// Seconds to wait before demoting a hot room
    pub cool_down_period_secs: u64,
    /// Maximum hot rooms per connection
    pub max_hot_rooms: usize,
    /// Rate calculation window in seconds
    pub rate_window_secs: u64,
}

impl Default for ShardSettings {
    fn default() -> Self {
        Self {
            num_shards: 8,
            hot_room_threshold: 100,
            cool_down_threshold: 20,
            cool_down_period_secs: 60,
            max_hot_rooms: 10,
            rate_window_secs: 10,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ProtocolSettings {
    /// Enable JSON debug mode (falls back to JSON encoding instead of FlatBuffers)
    pub debug_json: bool,
}

impl Default for ProtocolSettings {
    fn default() -> Self {
        Self { debug_json: false }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_config(content: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("garou_test_config_{}.toml", std::process::id()));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_default_config_is_valid() {
        let cfg = Config::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_load_valid_config() {
        let toml = r#"
[server]
bind_addr = "127.0.0.1:5000"
max_connections = 500

[nats]
url = "nats://nats:4222"

[redis]
url = "redis://redis:6379"
"#;
        let path = write_config(toml);
        let cfg = Config::load(&path).expect("should parse");
        let _ = std::fs::remove_file(&path);
        assert_eq!(cfg.server.bind_addr, "127.0.0.1:5000");
        assert_eq!(cfg.server.max_connections, 500);
    }

    #[test]
    fn test_invalid_bind_addr_returns_error() {
        let toml = r#"
[server]
bind_addr = "not_a_socket_addr"
"#;
        let path = write_config(toml);
        let err = Config::load(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(matches!(err, ChatError::Config(_)));
    }

    #[test]
    fn test_invalid_nats_url_returns_error() {
        let mut cfg = Config::default();
        cfg.nats.url = "http://localhost:4222".to_string();
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, ChatError::Config(_)));
    }

    #[test]
    fn test_invalid_redis_url_returns_error() {
        let mut cfg = Config::default();
        cfg.redis.url = "http://localhost:6379".to_string();
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, ChatError::Config(_)));
    }
}
