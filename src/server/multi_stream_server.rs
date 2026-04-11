//! Multi-stream QUIC chat server implementation
//!
//! This module provides the main server that accepts connections and manages
//! the chat system using the multi-stream QUIC architecture.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use quinn::Endpoint;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

use crate::auth::AuthValidator;
use crate::config::Config;
use crate::current_timestamp;
use crate::error::{ChatError, Result};
use crate::protocol::messages::*;
use crate::server::connection_handler::{ConnectionCommand, ConnectionHandler, ServerEvent};
use crate::server::room_manager::{RoomManager, RoomMember, RoomType};
use crate::storage::nats::{NatsClient, RoomState};
use crate::storage::redis::RedisClient;
use crate::transport::shards::{ShardConfig, ShardRouter};
use crate::transport::streams::StreamConfig;

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Address to bind to
    pub bind_addr: SocketAddr,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Stream configuration
    pub stream_config: StreamConfig,
    /// Shard configuration
    pub shard_config: ShardConfig,
    /// Connection idle timeout
    pub idle_timeout: Duration,
    /// Enable datagrams
    pub enable_datagrams: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:4433".parse().unwrap(),
            max_connections: 10000,
            stream_config: StreamConfig::default(),
            shard_config: ShardConfig::default(),
            idle_timeout: Duration::from_secs(300),
            enable_datagrams: true,
        }
    }
}

/// Active connection tracking
#[allow(dead_code)]
struct ActiveConnection {
    /// User ID (if authenticated)
    user_id: Option<UserId>,
    /// Username
    username: Option<String>,
    /// Command channel to send commands to this connection (bounded for backpressure)
    command_tx: mpsc::Sender<ConnectionCommand>,
    /// Remote address
    remote_addr: SocketAddr,
    /// Connection time
    connected_at: u64,
}

/// Multi-stream QUIC chat server
pub struct MultiStreamServer {
    /// Server configuration
    config: ServerConfig,
    /// QUIC endpoint
    endpoint: Option<Endpoint>,
    /// JWT authentication validator
    auth_validator: Arc<AuthValidator>,
    /// NATS JetStream client (optional — server works without it)
    nats_client: Option<Arc<NatsClient>>,
    /// Redis client for JWT caching, presence, and room rosters (optional)
    redis_client: Option<Arc<RedisClient>>,
    /// Room manager
    room_manager: Arc<RoomManager>,
    /// Shard router
    shard_router: Arc<ShardRouter>,
    /// Active connections by connection ID
    connections: Arc<RwLock<HashMap<String, ActiveConnection>>>,
    /// User ID to connection ID mapping
    user_connections: Arc<RwLock<HashMap<UserId, String>>>,
    /// Message ID counter (lock-free)
    next_message_id: Arc<AtomicU64>,
}

impl MultiStreamServer {
    /// Create a new multi-stream server with an explicit auth validator.
    pub fn new(config: ServerConfig, auth_validator: Arc<AuthValidator>) -> Self {
        let shard_router = Arc::new(ShardRouter::new(config.shard_config.clone()));
        let room_manager = Arc::new(RoomManager::new());

        Self {
            config,
            endpoint: None,
            auth_validator,
            nats_client: None,
            redis_client: None,
            room_manager,
            shard_router,
            connections: Arc::new(RwLock::new(HashMap::new())),
            user_connections: Arc::new(RwLock::new(HashMap::new())),
            next_message_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Create from application-level `Config`.
    ///
    /// Async because it optionally connects to NATS (failures are non-fatal).
    pub async fn from_config(cfg: &Config) -> Result<Self> {
        let bind_addr: SocketAddr = cfg
            .server
            .bind_addr
            .parse()
            .map_err(|e| ChatError::Config(format!("invalid server.bind_addr: {}", e)))?;

        let shard_config = ShardConfig {
            num_shards: cfg.shard.num_shards,
            hot_room_threshold: cfg.shard.hot_room_threshold,
            cool_down_threshold: cfg.shard.cool_down_threshold,
            cool_down_period_secs: cfg.shard.cool_down_period_secs,
            max_hot_rooms: cfg.shard.max_hot_rooms,
            rate_window_secs: cfg.shard.rate_window_secs,
        };

        let server_config = ServerConfig {
            bind_addr,
            max_connections: cfg.server.max_connections,
            stream_config: StreamConfig::default(),
            shard_config,
            idle_timeout: cfg.idle_timeout(),
            enable_datagrams: cfg.server.enable_datagrams,
        };

        // Attempt Redis connection; non-fatal if unavailable
        let redis_client: Option<Arc<RedisClient>> = if !cfg.redis.url.is_empty() {
            match RedisClient::connect(&cfg.redis.url, cfg.redis.jwt_cache_ttl_secs).await {
                Ok(client) => {
                    info!("Connected to Redis at {}", cfg.redis.url);
                    Some(Arc::new(client))
                }
                Err(e) => {
                    warn!(
                        "Redis unavailable ({}): running without JWT caching / presence",
                        e
                    );
                    None
                }
            }
        } else {
            None
        };

        // Build auth validator — attach Redis for claim caching if available
        let auth_validator = Arc::new({
            let v = AuthValidator::from_config(&cfg.auth)?;
            if let Some(ref r) = redis_client {
                v.with_redis_client(Arc::clone(r))
            } else {
                v
            }
        });

        let mut server = Self::new(server_config, auth_validator);
        server.redis_client = redis_client;

        // Attempt NATS connection; non-fatal if unavailable
        if !cfg.nats.url.is_empty() {
            match NatsClient::connect(&cfg.nats.url, &cfg.nats.stream_name).await {
                Ok(client) => {
                    info!("Connected to NATS at {}", cfg.nats.url);
                    server.nats_client = Some(Arc::new(client));
                }
                Err(e) => {
                    warn!(
                        "NATS unavailable ({}): running without message persistence",
                        e
                    );
                }
            }
        }

        Ok(server)
    }

    /// Create with default configuration and a dev-only auth validator.
    pub fn with_defaults() -> Self {
        let auth_validator = Arc::new(AuthValidator::new_hs256(
            b"garou-dev-only-insecure-secret-do-not-use-in-production",
        ));
        Self::new(ServerConfig::default(), auth_validator)
    }

    /// Get the room manager
    pub fn room_manager(&self) -> Arc<RoomManager> {
        Arc::clone(&self.room_manager)
    }

    /// Get the shard router
    pub fn shard_router(&self) -> Arc<ShardRouter> {
        Arc::clone(&self.shard_router)
    }

    /// Start the server
    pub async fn start(mut self) -> Result<()> {
        info!("Starting multi-stream server on {}", self.config.bind_addr);

        // Generate self-signed certificate for development
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])
            .map_err(|e| ChatError::config(format!("Failed to generate certificate: {}", e)))?;

        let cert_der = CertificateDer::from(cert.serialize_der().unwrap());
        let key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.serialize_private_key_der()));

        // Install the ring crypto provider for rustls
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok(); // Ignore error if already installed

        // Configure rustls
        let mut server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .map_err(|e| ChatError::config(format!("Failed to configure TLS: {}", e)))?;

        server_config.alpn_protocols = vec![b"chat".to_vec()];
        server_config.max_early_data_size = 0;

        // Configure QUIC
        let mut transport_config = quinn::TransportConfig::default();
        transport_config.max_concurrent_bidi_streams(100u32.into());
        transport_config.max_concurrent_uni_streams(100u32.into());
        transport_config.max_idle_timeout(Some(self.config.idle_timeout.try_into().unwrap()));

        if self.config.enable_datagrams {
            transport_config.datagram_receive_buffer_size(Some(65536));
        }

        let mut quic_server_config = quinn::ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(server_config)
                .map_err(|e| ChatError::config(format!("Failed to create QUIC config: {}", e)))?,
        ));
        quic_server_config.transport_config(Arc::new(transport_config));

        // Create endpoint
        let endpoint = Endpoint::server(quic_server_config, self.config.bind_addr)
            .map_err(|e| ChatError::network(format!("Failed to create endpoint: {}", e)))?;

        info!("Server listening on {}", endpoint.local_addr()?);

        self.endpoint = Some(endpoint.clone());

        // Create a default "general" room
        self.room_manager
            .create_room_with_id(1, "General".to_string(), RoomType::Channel)
            .await;

        // Wrap in Arc once — all spawned tasks clone this Arc (BUG-002 fix)
        let server = Arc::new(self);

        // Spawn NATS cross-node subscription task (if NATS is available)
        if let Some(nats) = server.nats_client.as_ref().map(Arc::clone) {
            let srv = Arc::clone(&server);
            tokio::spawn(async move {
                srv.run_nats_subscription(nats).await;
            });
        }

        // Accept connections
        server.accept_connections(endpoint).await
    }

    /// Background task: receive messages published by other server nodes via
    /// NATS Core and broadcast them to locally-connected clients.
    async fn run_nats_subscription(self: &Arc<Self>, nats: Arc<NatsClient>) {
        let mut subscription = match nats.subscribe_rooms().await {
            Ok(s) => s,
            Err(e) => {
                error!("NATS room subscription failed: {}", e);
                return;
            }
        };

        info!(
            "NATS cross-node subscription active (server_id={})",
            &nats.server_id()[..8]
        );

        loop {
            match subscription.next().await {
                Some((room_id, msg)) => {
                    let cmd = ConnectionCommand::SendRoomMessage(msg);
                    if let Err(e) = self.broadcast_to_room(room_id, cmd, None).await {
                        warn!("Cross-node broadcast to room {} failed: {}", room_id, e);
                    }
                }
                None => {
                    warn!("NATS room subscription closed");
                    break;
                }
            }
        }
    }

    /// Accept incoming connections
    async fn accept_connections(self: &Arc<Self>, endpoint: Endpoint) -> Result<()> {
        loop {
            match endpoint.accept().await {
                Some(incoming) => {
                    // Check connection limit
                    {
                        let conns = self.connections.read().await;
                        if conns.len() >= self.config.max_connections {
                            warn!("Connection limit reached, rejecting connection");
                            incoming.refuse();
                            continue;
                        }
                    }

                    // Clone Arc for spawned task — no re-wrapping
                    let server = Arc::clone(self);
                    tokio::spawn(async move {
                        if let Err(e) = server.handle_incoming(incoming).await {
                            error!("Connection handling failed: {}", e);
                        }
                    });
                }
                None => {
                    warn!("Endpoint stopped accepting connections");
                    break;
                }
            }
        }
        Ok(())
    }

    /// Handle an incoming connection
    async fn handle_incoming(self: &Arc<Self>, incoming: quinn::Incoming) -> Result<()> {
        let connection = incoming.await?;
        let remote_addr = connection.remote_address();
        let conn_id = uuid::Uuid::now_v7().to_string();

        debug!("New connection {} from {}", conn_id, remote_addr);

        // Create bounded channels for this connection (backpressure, BUG-004 fix)
        let (event_tx, event_rx) = mpsc::channel(1024);
        let (command_tx, command_rx) = mpsc::channel(1024);

        // Register connection (before auth, so we can track it)
        {
            let mut conns = self.connections.write().await;
            conns.insert(
                conn_id.clone(),
                ActiveConnection {
                    user_id: None,
                    username: None,
                    command_tx: command_tx.clone(),
                    remote_addr,
                    connected_at: current_timestamp(),
                },
            );
        }

        // Create connection handler
        let handler = Arc::new(ConnectionHandler::new(
            connection,
            self.config.stream_config.clone(),
            Arc::clone(&self.auth_validator),
            Arc::clone(&self.shard_router),
            event_tx,
            command_rx,
        ));

        // Spawn handler task
        let handler_clone = Arc::clone(&handler);
        let handler_task = tokio::spawn(async move { handler_clone.run().await });

        // Spawn event processor task
        let conn_id_clone = conn_id.clone();
        let server = Arc::clone(self);
        let event_task = tokio::spawn(async move {
            server.process_events(conn_id_clone, event_rx).await;
        });

        // Wait for either task to complete
        tokio::select! {
            result = handler_task => {
                if let Err(e) = result {
                    error!("Handler task error: {}", e);
                }
            }
            _ = event_task => {}
        }

        // Clean up connection
        self.cleanup_connection(&conn_id).await;

        Ok(())
    }

    /// Process events from a connection
    async fn process_events(&self, conn_id: String, mut event_rx: mpsc::Receiver<ServerEvent>) {
        while let Some(event) = event_rx.recv().await {
            if let Err(e) = self.handle_event(&conn_id, event).await {
                warn!("Event handling error for {}: {}", conn_id, e);
            }
        }
    }

    /// Handle a single event from a connection
    async fn handle_event(&self, conn_id: &str, event: ServerEvent) -> Result<()> {
        match event {
            ServerEvent::Authenticated { user_id, username } => {
                self.handle_authenticated(conn_id, user_id, username)
                    .await?;
            }

            ServerEvent::JoinRoom { user_id, room_id } => {
                self.handle_join_room(conn_id, user_id, room_id).await?;
            }

            ServerEvent::LeaveRoom { user_id, room_id } => {
                self.handle_leave_room(conn_id, user_id, room_id).await?;
            }

            ServerEvent::SendMessage {
                user_id,
                room_id,
                content,
                nonce,
                reply_to,
            } => {
                self.handle_send_message(conn_id, user_id, room_id, content, nonce, reply_to)
                    .await?;
            }

            ServerEvent::EditMessage {
                user_id,
                message_id,
                content,
            } => {
                self.handle_edit_message(conn_id, user_id, message_id, content)
                    .await?;
            }

            ServerEvent::DeleteMessage {
                user_id,
                message_id,
            } => {
                self.handle_delete_message(conn_id, user_id, message_id)
                    .await?;
            }

            ServerEvent::AddReaction {
                user_id,
                message_id,
                emoji,
            } => {
                self.handle_add_reaction(conn_id, user_id, message_id, emoji)
                    .await?;
            }

            ServerEvent::RemoveReaction {
                user_id,
                message_id,
                emoji,
            } => {
                self.handle_remove_reaction(conn_id, user_id, message_id, emoji)
                    .await?;
            }

            ServerEvent::CreateRoom {
                user_id,
                name,
                room_type,
                members,
            } => {
                self.handle_create_room(conn_id, user_id, name, room_type, members)
                    .await?;
            }

            ServerEvent::Typing { user_id, room_id } => {
                self.handle_typing(user_id, room_id).await?;
            }

            ServerEvent::StopTyping { user_id, room_id } => {
                self.handle_stop_typing(user_id, room_id).await?;
            }

            ServerEvent::PresenceUpdate { user_id, status } => {
                self.handle_presence_update(user_id, status).await?;
            }

            ServerEvent::Disconnected { user_id, reason } => {
                debug!(
                    "Connection {} disconnected: {} (user: {:?})",
                    conn_id, reason, user_id
                );
            }
        }

        Ok(())
    }

    /// Handle user authentication
    async fn handle_authenticated(
        &self,
        conn_id: &str,
        user_id: UserId,
        username: String,
    ) -> Result<()> {
        info!(
            "User {} ({}) authenticated on {}",
            username, user_id, conn_id
        );

        // Update connection with user info
        {
            let mut conns = self.connections.write().await;
            if let Some(conn) = conns.get_mut(conn_id) {
                conn.user_id = Some(user_id);
                conn.username = Some(username.clone());
            }
        }

        // Map user ID to connection
        {
            let mut user_conns = self.user_connections.write().await;
            user_conns.insert(user_id, conn_id.to_string());
        }

        // Update presence in Redis
        if let Some(ref redis) = self.redis_client {
            redis.set_presence(user_id, "online").await;
        }

        // Auto-join general room
        self.handle_join_room(conn_id, user_id, 1).await?;

        Ok(())
    }

    /// Handle user joining a room
    async fn handle_join_room(
        &self,
        conn_id: &str,
        user_id: UserId,
        room_id: RoomId,
    ) -> Result<()> {
        // Get username
        let username = {
            let conns = self.connections.read().await;
            conns
                .get(conn_id)
                .and_then(|c| c.username.clone())
                .unwrap_or_else(|| format!("user_{}", user_id))
        };

        // Get or create room
        let room = match self.room_manager.get_room(room_id).await {
            Some(r) => r,
            None => {
                // Room doesn't exist, create it
                let _member = RoomMember::new(user_id, username.clone());
                self.room_manager
                    .create_room_with_id(room_id, format!("Room {}", room_id), RoomType::Group)
                    .await;

                self.room_manager.get_room(room_id).await.unwrap()
            }
        };

        // Join room
        let member = RoomMember::new(user_id, username.clone());
        self.room_manager.join_room(room_id, member.clone()).await;

        // Register room with shard router
        self.shard_router.register_room(room_id).await;

        // Update Redis roster
        if let Some(ref redis) = self.redis_client {
            redis.add_to_roster(room_id, user_id).await;
        }

        // Send room init to the joining user
        let members = room.get_members().await;
        // Prefer NATS history (persisted across restarts); fall back to in-memory
        let recent_messages = if let Some(nats) = &self.nats_client {
            match nats.get_history(room_id, 50).await {
                Ok(msgs) if !msgs.is_empty() => msgs,
                Ok(_) => room.get_recent_messages(Some(50)).await,
                Err(e) => {
                    warn!("NATS history fetch failed for room {}: {}", room_id, e);
                    room.get_recent_messages(Some(50)).await
                }
            }
        } else {
            room.get_recent_messages(Some(50)).await
        };

        let room_init = RoomInit {
            room_id,
            name: room.name.clone(),
            room_type: match room.room_type {
                RoomType::Direct => "direct".to_string(),
                RoomType::Group => "group".to_string(),
                RoomType::Channel => "channel".to_string(),
            },
            members: members.iter().map(|m| m.to_user_info()).collect(),
            recent_messages,
        };

        self.send_to_connection(conn_id, ConnectionCommand::SendRoomInit(room_init))
            .await?;

        // Notify other room members
        let user_joined = RoomUserJoined {
            room_id,
            user: UserInfo {
                user_id,
                username: username.clone(),
                avatar_url: None,
            },
            joined_at: current_timestamp(),
        };

        self.broadcast_to_room(
            room_id,
            ConnectionCommand::SendUserJoined(user_joined),
            Some(user_id),
        )
        .await?;

        info!("User {} joined room {}", username, room_id);
        Ok(())
    }

    /// Handle user leaving a room
    async fn handle_leave_room(
        &self,
        conn_id: &str,
        user_id: UserId,
        room_id: RoomId,
    ) -> Result<()> {
        // Leave room
        self.room_manager.leave_room(room_id, user_id).await;

        // Update Redis roster
        if let Some(ref redis) = self.redis_client {
            redis.remove_from_roster(room_id, user_id).await;
        }

        // Notify room members
        let user_left = RoomUserLeft {
            room_id,
            user_id,
            left_at: current_timestamp(),
        };

        self.broadcast_to_room(
            room_id,
            ConnectionCommand::SendUserLeft(user_left),
            Some(user_id),
        )
        .await?;

        // Send room close to the leaving user
        let room_close = RoomClose {
            room_id,
            reason: "left".to_string(),
        };

        self.send_to_connection(conn_id, ConnectionCommand::SendRoomClose(room_close))
            .await?;

        Ok(())
    }

    /// Handle sending a message
    async fn handle_send_message(
        &self,
        conn_id: &str,
        user_id: UserId,
        room_id: RoomId,
        content: String,
        nonce: Option<String>,
        reply_to: Option<MessageId>,
    ) -> Result<()> {
        // Get username
        let username = {
            let conns = self.connections.read().await;
            conns
                .get(conn_id)
                .and_then(|c| c.username.clone())
                .unwrap_or_else(|| format!("user_{}", user_id))
        };

        // Generate message ID — lock-free fetch_add
        let message_id = self.next_message_id.fetch_add(1, Ordering::Relaxed);

        let timestamp = current_timestamp();

        // Create room message
        let room_message = RoomMessage {
            message_id,
            room_id,
            sender: UserInfo {
                user_id,
                username: username.clone(),
                avatar_url: None,
            },
            content: content.clone(),
            timestamp,
            reply_to,
            attachments: vec![],
            nonce: nonce.clone(),
        };

        // Publish to NATS JetStream first (if available).
        // Per acceptance criteria: NATS failure → return error, do NOT broadcast.
        if let Some(nats) = &self.nats_client {
            nats.publish_message(&room_message)
                .await
                .map_err(|e| ChatError::Internal(format!("NATS publish: {}", e)))?;

            // Store message→room mapping for BUG-001 (edit/delete/reaction lookup)
            if let Err(e) = nats.save_message_room(message_id, room_id).await {
                warn!("Failed to save message→room mapping in NATS: {}", e);
            }
        }

        // Add to in-memory room history
        if let Some(room) = self.room_manager.get_room(room_id).await {
            room.add_message(room_message.clone()).await;
            room.touch_member(user_id).await;
        }

        // Record message for shard routing
        self.shard_router.route_message(room_id).await;

        // Send ACK to sender
        let ack = MessageAck {
            nonce: nonce.unwrap_or_default(),
            message_id,
            timestamp,
        };
        self.send_to_connection(conn_id, ConnectionCommand::SendMessageAck(ack))
            .await?;

        // Broadcast to local clients (cross-node delivery handled by NATS subscription task)
        self.broadcast_to_room(
            room_id,
            ConnectionCommand::SendRoomMessage(room_message),
            None,
        )
        .await?;

        debug!(
            "Message {} from {} in room {}",
            message_id, username, room_id
        );
        Ok(())
    }

    /// Look up the room_id for a message from the NATS KV store (BUG-001 fix).
    async fn room_id_for_message(&self, message_id: MessageId) -> Option<RoomId> {
        let nats = self.nats_client.as_ref()?;
        match nats.get_room_id_for_message(message_id).await {
            Ok(Some(id)) => Some(id),
            Ok(None) => {
                warn!("room_id not found in NATS for message {}", message_id);
                None
            }
            Err(e) => {
                warn!(
                    "NATS room_id lookup failed for message {}: {}",
                    message_id, e
                );
                None
            }
        }
    }

    /// Handle editing a message (BUG-001 fixed: room_id from NATS KV).
    async fn handle_edit_message(
        &self,
        _conn_id: &str,
        user_id: UserId,
        message_id: MessageId,
        content: String,
    ) -> Result<()> {
        let room_id = match self.room_id_for_message(message_id).await {
            Some(id) => id,
            None => {
                debug!(
                    "Dropping EditMessage for {} — room_id unavailable (NATS required)",
                    message_id
                );
                return Ok(());
            }
        };

        let edited = RoomMessageEdited {
            message_id,
            room_id,
            editor_id: user_id,
            content,
            edited_at: current_timestamp(),
        };

        self.broadcast_to_room(room_id, ConnectionCommand::SendMessageEdited(edited), None)
            .await
    }

    /// Handle deleting a message (BUG-001 fixed: room_id from NATS KV).
    async fn handle_delete_message(
        &self,
        _conn_id: &str,
        user_id: UserId,
        message_id: MessageId,
    ) -> Result<()> {
        let room_id = match self.room_id_for_message(message_id).await {
            Some(id) => id,
            None => {
                debug!(
                    "Dropping DeleteMessage for {} — room_id unavailable",
                    message_id
                );
                return Ok(());
            }
        };

        let deleted = RoomMessageDeleted {
            message_id,
            room_id,
            deleted_by: user_id,
            deleted_at: current_timestamp(),
        };

        self.broadcast_to_room(
            room_id,
            ConnectionCommand::SendMessageDeleted(deleted),
            None,
        )
        .await
    }

    /// Handle adding a reaction (BUG-001 fixed: room_id from NATS KV).
    async fn handle_add_reaction(
        &self,
        _conn_id: &str,
        user_id: UserId,
        message_id: MessageId,
        emoji: String,
    ) -> Result<()> {
        let room_id = match self.room_id_for_message(message_id).await {
            Some(id) => id,
            None => {
                debug!(
                    "Dropping AddReaction for {} — room_id unavailable",
                    message_id
                );
                return Ok(());
            }
        };

        let reaction = RoomReactionAdded {
            message_id,
            room_id,
            user_id,
            emoji,
        };

        self.broadcast_to_room(
            room_id,
            ConnectionCommand::SendReactionAdded(reaction),
            None,
        )
        .await
    }

    /// Handle removing a reaction (BUG-001 fixed: room_id from NATS KV).
    async fn handle_remove_reaction(
        &self,
        _conn_id: &str,
        user_id: UserId,
        message_id: MessageId,
        emoji: String,
    ) -> Result<()> {
        let room_id = match self.room_id_for_message(message_id).await {
            Some(id) => id,
            None => {
                debug!(
                    "Dropping RemoveReaction for {} — room_id unavailable",
                    message_id
                );
                return Ok(());
            }
        };

        let reaction = RoomReactionRemoved {
            message_id,
            room_id,
            user_id,
            emoji,
        };

        self.broadcast_to_room(
            room_id,
            ConnectionCommand::SendReactionRemoved(reaction),
            None,
        )
        .await
    }

    /// Handle creating a room
    async fn handle_create_room(
        &self,
        conn_id: &str,
        user_id: UserId,
        name: String,
        room_type: String,
        members: Vec<UserId>,
    ) -> Result<()> {
        let username = {
            let conns = self.connections.read().await;
            conns
                .get(conn_id)
                .and_then(|c| c.username.clone())
                .unwrap_or_else(|| format!("user_{}", user_id))
        };

        let rt = match room_type.as_str() {
            "direct" => RoomType::Direct,
            "channel" => RoomType::Channel,
            _ => RoomType::Group,
        };

        let creator = RoomMember::new(user_id, username);
        let room = self
            .room_manager
            .create_room(name.clone(), rt, creator)
            .await;

        // Add other members
        for member_id in &members {
            if *member_id != user_id {
                let member = RoomMember::new(*member_id, format!("user_{}", member_id));
                self.room_manager.join_room(room.id, member).await;
            }
        }

        // Persist room state to NATS KV
        if let Some(nats) = &self.nats_client {
            let state = RoomState {
                room_id: room.id,
                name: name.clone(),
                room_type: room_type.clone(),
                created_at: current_timestamp(),
                member_count: members.len() + 1,
            };
            if let Err(e) = nats.save_room_state(&state).await {
                warn!("Failed to persist room {} state to NATS: {}", room.id, e);
            }
        }

        // Have the creator join
        self.handle_join_room(conn_id, user_id, room.id).await?;

        info!(
            "Created room {} ({}) by user {}",
            room.id, room.name, user_id
        );
        Ok(())
    }

    /// Handle typing indicator
    async fn handle_typing(&self, user_id: UserId, room_id: RoomId) -> Result<()> {
        let typing = Typing {
            room_id,
            user_id: Some(user_id),
        };

        self.broadcast_to_room(
            room_id,
            ConnectionCommand::SendTyping(typing),
            Some(user_id),
        )
        .await?;

        Ok(())
    }

    /// Handle stop typing indicator
    async fn handle_stop_typing(&self, user_id: UserId, room_id: RoomId) -> Result<()> {
        let stop = StopTyping {
            room_id,
            user_id: Some(user_id),
        };

        self.broadcast_to_room(
            room_id,
            ConnectionCommand::SendStopTyping(stop),
            Some(user_id),
        )
        .await?;

        Ok(())
    }

    /// Handle presence update
    async fn handle_presence_update(&self, user_id: UserId, status: String) -> Result<()> {
        let presence = PresenceOnline {
            user_id,
            status: Some(status),
        };

        // Broadcast to all rooms the user is in
        let room_ids = self.room_manager.get_user_room_ids(user_id).await;

        for room_id in room_ids {
            self.broadcast_to_room(
                room_id,
                ConnectionCommand::SendPresenceOnline(presence.clone()),
                Some(user_id),
            )
            .await?;
        }

        Ok(())
    }

    /// Send a command to a specific connection
    async fn send_to_connection(&self, conn_id: &str, cmd: ConnectionCommand) -> Result<()> {
        let conns = self.connections.read().await;
        if let Some(conn) = conns.get(conn_id) {
            // Bounded send: if channel is full, drop the message (backpressure — client is slow)
            let _ = conn.command_tx.try_send(cmd);
        }
        Ok(())
    }

    /// Send a command to a user by user ID
    #[allow(dead_code)]
    async fn send_to_user(&self, user_id: UserId, cmd: ConnectionCommand) -> Result<()> {
        let conn_id = {
            let user_conns = self.user_connections.read().await;
            user_conns.get(&user_id).cloned()
        };

        if let Some(conn_id) = conn_id {
            self.send_to_connection(&conn_id, cmd).await?;
        }

        Ok(())
    }

    /// Broadcast a command to all members of a room
    async fn broadcast_to_room(
        &self,
        room_id: RoomId,
        cmd: ConnectionCommand,
        exclude_user: Option<UserId>,
    ) -> Result<()> {
        // Get room members
        let room = match self.room_manager.get_room(room_id).await {
            Some(r) => r,
            None => return Ok(()),
        };

        let member_ids = room.get_member_ids().await;

        // Send to each member
        let user_conns = self.user_connections.read().await;
        let conns = self.connections.read().await;

        for member_id in member_ids {
            if Some(member_id) == exclude_user {
                continue;
            }

            if let Some(conn_id) = user_conns.get(&member_id) {
                if let Some(conn) = conns.get(conn_id) {
                    // Clone the command for each recipient; try_send drops if channel full
                    let cmd_clone = clone_command(&cmd);
                    let _ = conn.command_tx.try_send(cmd_clone);
                }
            }
        }

        Ok(())
    }

    /// Clean up a disconnected connection
    async fn cleanup_connection(&self, conn_id: &str) {
        // Get user ID before removing
        let user_id = {
            let conns = self.connections.read().await;
            conns.get(conn_id).and_then(|c| c.user_id)
        };

        // Remove from connections
        {
            let mut conns = self.connections.write().await;
            conns.remove(conn_id);
        }

        // Remove from user mappings
        if let Some(user_id) = user_id {
            {
                let mut user_conns = self.user_connections.write().await;
                user_conns.remove(&user_id);
            }

            // Remove user from all rooms
            let room_ids = self.room_manager.remove_user_from_all_rooms(user_id).await;

            // Clean up Redis presence + rosters
            if let Some(ref redis) = self.redis_client {
                redis.del_presence(user_id).await;
                redis.remove_from_all_rosters(user_id, &room_ids).await;
            }

            // Notify rooms of user leaving
            let user_left = RoomUserLeft {
                room_id: 0,
                user_id,
                left_at: current_timestamp(),
            };

            for room_id in room_ids {
                let mut left = user_left.clone();
                left.room_id = room_id;
                let _ = self
                    .broadcast_to_room(
                        room_id,
                        ConnectionCommand::SendUserLeft(left),
                        Some(user_id),
                    )
                    .await;
            }
        }

        debug!("Cleaned up connection {}", conn_id);
    }

    /// Get server statistics
    pub async fn get_stats(&self) -> ServerStats {
        let conns = self.connections.read().await;
        let authenticated_count = conns.values().filter(|c| c.user_id.is_some()).count();

        ServerStats {
            total_connections: conns.len(),
            authenticated_connections: authenticated_count,
            total_rooms: self.room_manager.room_count().await,
            total_users: self.room_manager.total_user_count().await,
            bind_address: self.config.bind_addr,
        }
    }

    /// Shutdown the server gracefully
    pub async fn shutdown(self: &Arc<Self>) -> Result<()> {
        if let Some(endpoint) = self.endpoint.clone() {
            // Notify all connections to close
            let conns = self.connections.read().await;
            for conn in conns.values() {
                let _ = conn
                    .command_tx
                    .send(ConnectionCommand::Close("Server shutdown".to_string()))
                    .await;
            }

            endpoint.close(0u32.into(), b"Server shutdown");
            info!("Server shutdown complete");
        }
        Ok(())
    }
}

/// Server statistics
#[derive(Debug, Clone)]
pub struct ServerStats {
    pub total_connections: usize,
    pub authenticated_connections: usize,
    pub total_rooms: usize,
    pub total_users: usize,
    pub bind_address: SocketAddr,
}

/// Clone a connection command (needed for broadcasting)
fn clone_command(cmd: &ConnectionCommand) -> ConnectionCommand {
    match cmd {
        ConnectionCommand::SendRoomMessage(m) => ConnectionCommand::SendRoomMessage(m.clone()),
        ConnectionCommand::SendRoomInit(m) => ConnectionCommand::SendRoomInit(m.clone()),
        ConnectionCommand::SendRoomClose(m) => ConnectionCommand::SendRoomClose(m.clone()),
        ConnectionCommand::SendUserJoined(m) => ConnectionCommand::SendUserJoined(m.clone()),
        ConnectionCommand::SendUserLeft(m) => ConnectionCommand::SendUserLeft(m.clone()),
        ConnectionCommand::SendMessageEdited(m) => ConnectionCommand::SendMessageEdited(m.clone()),
        ConnectionCommand::SendMessageDeleted(m) => {
            ConnectionCommand::SendMessageDeleted(m.clone())
        }
        ConnectionCommand::SendReactionAdded(m) => ConnectionCommand::SendReactionAdded(m.clone()),
        ConnectionCommand::SendReactionRemoved(m) => {
            ConnectionCommand::SendReactionRemoved(m.clone())
        }
        ConnectionCommand::SendMessageAck(m) => ConnectionCommand::SendMessageAck(m.clone()),
        ConnectionCommand::SendTyping(m) => ConnectionCommand::SendTyping(m.clone()),
        ConnectionCommand::SendStopTyping(m) => ConnectionCommand::SendStopTyping(m.clone()),
        ConnectionCommand::SendPresenceOnline(m) => {
            ConnectionCommand::SendPresenceOnline(m.clone())
        }
        ConnectionCommand::SendPresenceOffline(m) => {
            ConnectionCommand::SendPresenceOffline(m.clone())
        }
        ConnectionCommand::SendThrottle(m) => ConnectionCommand::SendThrottle(m.clone()),
        ConnectionCommand::PromoteRoom(id) => ConnectionCommand::PromoteRoom(*id),
        ConnectionCommand::DemoteRoom(id) => ConnectionCommand::DemoteRoom(*id),
        ConnectionCommand::Close(s) => ConnectionCommand::Close(s.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.bind_addr.port(), 4433);
        assert_eq!(config.max_connections, 10000);
    }

    #[tokio::test]
    async fn test_server_creation() {
        let server = MultiStreamServer::with_defaults();
        assert!(server.endpoint.is_none());
    }

    #[tokio::test]
    async fn test_server_stats() {
        let server = MultiStreamServer::with_defaults();
        let stats = server.get_stats().await;
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.total_rooms, 0);
    }
}
