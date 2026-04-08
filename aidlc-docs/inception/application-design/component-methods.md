# Component Method Signatures

Note: Detailed business logic per method is defined in Functional Design (CONSTRUCTION phase, per-unit).

---

## Config Component (`src/config/mod.rs`)

```rust
/// Root configuration struct
pub struct Config {
    pub server: ServerConfig,
    pub tls: TlsConfig,
    pub auth: AuthConfig,
    pub nats: NatsConfig,
    pub redis: RedisConfig,
    pub metrics: MetricsConfig,
    pub shards: ShardConfigSettings,
    pub protocol: ProtocolConfig,
}

impl Config {
    /// Load config from TOML file, then apply CLI overrides
    pub fn load(config_path: &Path, cli_overrides: CliArgs) -> Result<Arc<Config>>;

    /// Load with default path ("config.toml") and no overrides
    pub fn load_default() -> Result<Arc<Config>>;

    /// Validate all config fields, return error on invalid settings
    pub fn validate(&self) -> Result<()>;
}

pub struct ServerConfig {
    pub bind_addr: SocketAddr,        // QUIC bind address
    pub max_connections: usize,
    pub idle_timeout_secs: u64,
    pub enable_datagrams: bool,
    pub shutdown_timeout_secs: u64,
    pub metrics_port: u16,            // HTTP port for /metrics and /health
}

pub struct AuthConfig {
    pub algorithm: String,            // "HS256" or "RS256"
    pub secret: Option<String>,       // HMAC secret (HS256)
    pub public_key_path: Option<PathBuf>, // RSA public key path (RS256)
}

pub struct NatsConfig {
    pub url: String,                  // e.g. "nats://nats:4222"
    pub pool_size: usize,
    pub stream_name: String,          // default "CHAT_MESSAGES"
    pub max_message_age_secs: u64,
    pub max_bytes: i64,               // stream storage limit
}

pub struct RedisConfig {
    pub url: String,                  // e.g. "redis://redis:6379"
    pub pool_size: u32,
    pub connection_timeout_ms: u64,
    pub presence_ttl_secs: u64,       // default 60
}

pub struct ProtocolConfig {
    pub debug_json: bool,             // fallback to JSON (dev only)
}
```

---

## Auth Component (`src/auth/mod.rs`)

```rust
pub struct AuthValidator {
    config: Arc<AuthConfig>,
    redis: Option<Arc<RedisClient>>,  // None if Redis not configured
}

/// Decoded JWT claims
pub struct AuthClaims {
    pub user_id: UserId,
    pub username: String,
    pub expires_at: u64,              // Unix timestamp
}

impl AuthValidator {
    /// Create a new validator from config and optional Redis cache
    pub fn new(config: Arc<AuthConfig>, redis: Option<Arc<RedisClient>>) -> Result<Self>;

    /// Validate a JWT token string, return claims or auth error
    /// Checks Redis cache first; validates inline on cache miss or Redis failure
    pub async fn validate(&self, token: &str) -> Result<AuthClaims>;

    /// Invalidate a cached token (e.g., on explicit logout)
    pub async fn invalidate(&self, token: &str) -> Result<()>;
}
```

---

## NATS Storage Component (`src/storage/nats.rs`)

```rust
pub struct NatsClient {
    pool: Vec<async_nats::Client>,    // connection pool
    jetstream: async_nats::jetstream::Context,
    config: Arc<NatsConfig>,
}

impl NatsClient {
    /// Connect to NATS with pool_size connections
    pub async fn connect(config: Arc<NatsConfig>) -> Result<Arc<Self>>;

    /// Ensure JetStream stream and KV buckets exist
    pub async fn init_infrastructure(&self) -> Result<()>;

    /// Persist a chat message to JetStream, returns sequence number
    pub async fn publish_message(&self, room_id: RoomId, msg: &RoomMessage) -> Result<u64>;

    /// Subscribe to a room subject for cross-node delivery
    /// Returns stream of incoming RoomMessage from other nodes
    pub async fn subscribe_room(&self, room_id: RoomId) 
        -> Result<impl Stream<Item = Result<RoomMessage>>>;

    /// Unsubscribe from a room subject
    pub async fn unsubscribe_room(&self, room_id: RoomId) -> Result<()>;

    /// Fetch last N messages for a room (for room join history replay)
    pub async fn get_room_history(&self, room_id: RoomId, limit: usize) 
        -> Result<Vec<RoomMessage>>;

    /// Store room state in NATS KV
    pub async fn put_room_state(&self, room_id: RoomId, state: &RoomState) -> Result<()>;

    /// Get room state from NATS KV (returns None if not found)
    pub async fn get_room_state(&self, room_id: RoomId) -> Result<Option<RoomState>>;

    /// Delete room state from NATS KV
    pub async fn delete_room_state(&self, room_id: RoomId) -> Result<()>;

    /// Check if NATS is healthy (for readiness probe)
    pub async fn health_check(&self) -> bool;

    /// Gracefully close all connections
    pub async fn close(&self) -> Result<()>;
}

/// Room state stored in NATS KV
pub struct RoomState {
    pub room_id: RoomId,
    pub name: String,
    pub room_type: String,
    pub created_at: u64,
    pub member_ids: Vec<UserId>,
}
```

---

## Redis Cache Component (`src/storage/redis.rs`)

```rust
pub struct RedisClient {
    pool: fred::pool::RedisPool,
    config: Arc<RedisConfig>,
}

impl RedisClient {
    /// Create connection pool to Redis
    pub async fn connect(config: Arc<RedisConfig>) -> Result<Arc<Self>>;

    /// Get cached JWT claims by token string (returns None on miss or error)
    pub async fn get_jwt_claims(&self, token: &str) -> Option<AuthClaims>;

    /// Cache JWT claims with TTL derived from token expiry
    pub async fn set_jwt_claims(&self, token: &str, claims: &AuthClaims) -> Result<()>;

    /// Invalidate cached JWT claims
    pub async fn del_jwt_claims(&self, token: &str) -> Result<()>;

    /// Set user presence (refresh TTL to presence_ttl_secs)
    pub async fn set_presence(&self, user_id: UserId, status: &str) -> Result<()>;

    /// Get user presence status (returns None if offline/expired)
    pub async fn get_presence(&self, user_id: UserId) -> Option<String>;

    /// Remove user presence key (on disconnect)
    pub async fn del_presence(&self, user_id: UserId) -> Result<()>;

    /// Add user to room roster set
    pub async fn add_to_roster(&self, room_id: RoomId, user_id: UserId) -> Result<()>;

    /// Remove user from room roster set
    pub async fn remove_from_roster(&self, room_id: RoomId, user_id: UserId) -> Result<()>;

    /// Get all online users in a room (from roster)
    pub async fn get_roster(&self, room_id: RoomId) -> Result<Vec<UserId>>;

    /// Check if Redis is healthy (PING command)
    pub async fn health_check(&self) -> bool;

    /// Close connection pool
    pub async fn close(&self) -> Result<()>;
}
```

---

## Metrics Component (`src/metrics/mod.rs`)

```rust
/// Initialize Prometheus metrics registry and install as global recorder
pub fn init_metrics() -> Result<PrometheusHandle>;

// Connection metrics
pub fn increment_connections_total();
pub fn set_connections_active(count: f64);
pub fn increment_authenticated_connections(delta: f64);

// Message metrics
pub fn increment_messages_total(room_id: RoomId);
pub fn record_message_latency_ms(latency_ms: f64);

// NATS metrics
pub fn increment_nats_publish_errors();
pub fn record_nats_publish_latency_ms(latency_ms: f64);

// Room metrics
pub fn set_rooms_active(count: f64);
pub fn set_hot_rooms_active(count: f64);
```

---

## Health Component (`src/health/mod.rs`)

```rust
pub struct HealthServer {
    nats: Arc<NatsClient>,
    redis: Arc<RedisClient>,
    metrics_handle: PrometheusHandle,
    port: u16,
}

impl HealthServer {
    pub fn new(
        nats: Arc<NatsClient>,
        redis: Arc<RedisClient>,
        metrics_handle: PrometheusHandle,
        port: u16,
    ) -> Self;

    /// Start the HTTP server (runs in its own Tokio task)
    /// Routes: GET /health/live, GET /health/ready, GET /metrics
    pub async fn start(self, shutdown: CancellationToken) -> Result<()>;

    /// Check liveness (always true if process is running)
    async fn liveness() -> impl IntoResponse;

    /// Check readiness (NATS + Redis must be healthy)
    async fn readiness(nats: Arc<NatsClient>, redis: Arc<RedisClient>) -> impl IntoResponse;

    /// Render Prometheus metrics
    async fn metrics(handle: PrometheusHandle) -> impl IntoResponse;
}
```

---

## Multi-Stream Server (`src/server/multi_stream_server.rs`)

```rust
pub struct MultiStreamServer {
    config: Arc<Config>,
    endpoint: Option<Endpoint>,
    auth: Arc<AuthValidator>,
    nats: Arc<NatsClient>,
    redis: Arc<RedisClient>,
    room_manager: Arc<RoomManager>,
    shard_router: Arc<ShardRouter>,
    connections: Arc<RwLock<HashMap<String, ActiveConnection>>>,
    user_connections: Arc<RwLock<HashMap<UserId, String>>>,
    next_message_id: Arc<AtomicU64>,     // was RwLock<u64> — fixed to AtomicU64
    shutdown_token: CancellationToken,
}

impl MultiStreamServer {
    /// Construct server from config and pre-initialized components
    pub fn new(
        config: Arc<Config>,
        auth: Arc<AuthValidator>,
        nats: Arc<NatsClient>,
        redis: Arc<RedisClient>,
    ) -> Arc<Self>;                      // returns Arc<Self> directly (BUG-002 fix)

    /// Start QUIC endpoint and accept connections
    pub async fn start(self: Arc<Self>) -> Result<()>;

    /// Initiate graceful shutdown (signal-driven)
    pub async fn shutdown(self: Arc<Self>) -> Result<()>;

    /// Get current server statistics
    pub async fn get_stats(&self) -> ServerStats;
}
```
