//! Connection handler for server-side multi-stream connection management
//!
//! This module handles individual client connections, managing the protocol
//! handshake, stream setup, and message routing.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use quinn::{Connection, RecvStream, SendStream};
use tokio::sync::{RwLock, mpsc};
use tokio::time::Instant;
use tracing::{debug, error, info, warn};

use crate::current_timestamp;
use crate::error::{ChatError, Result};
use crate::protocol::codec::{Decodable, Encodable};
use crate::protocol::frame::{Frame, FrameCodec, FrameType};
use crate::protocol::messages::*;
use crate::transport::shards::ShardRouter;
use crate::transport::streams::{StreamConfig, StreamHandle, StreamSet, StreamState, StreamType};

/// Events emitted by the connection handler to the server
#[derive(Debug)]
pub enum ServerEvent {
    /// Client successfully authenticated
    Authenticated { user_id: UserId, username: String },

    /// Client wants to join a room
    JoinRoom { user_id: UserId, room_id: RoomId },

    /// Client wants to leave a room
    LeaveRoom { user_id: UserId, room_id: RoomId },

    /// Client sent a message
    SendMessage {
        user_id: UserId,
        room_id: RoomId,
        content: String,
        nonce: Option<String>,
        reply_to: Option<MessageId>,
    },

    /// Client edited a message
    EditMessage {
        user_id: UserId,
        message_id: MessageId,
        content: String,
    },

    /// Client deleted a message
    DeleteMessage {
        user_id: UserId,
        message_id: MessageId,
    },

    /// Client added a reaction
    AddReaction {
        user_id: UserId,
        message_id: MessageId,
        emoji: String,
    },

    /// Client removed a reaction
    RemoveReaction {
        user_id: UserId,
        message_id: MessageId,
        emoji: String,
    },

    /// Client wants to create a room
    CreateRoom {
        user_id: UserId,
        name: String,
        room_type: String,
        members: Vec<UserId>,
    },

    /// Client sent typing indicator
    Typing { user_id: UserId, room_id: RoomId },

    /// Client stopped typing
    StopTyping { user_id: UserId, room_id: RoomId },

    /// Client sent presence update
    PresenceUpdate { user_id: UserId, status: String },

    /// Client disconnected
    Disconnected {
        user_id: Option<UserId>,
        reason: String,
    },
}

/// Commands that can be sent to the connection handler
#[derive(Debug)]
pub enum ConnectionCommand {
    /// Send a room message to this client
    SendRoomMessage(RoomMessage),

    /// Send room init info
    SendRoomInit(RoomInit),

    /// Send room close notification
    SendRoomClose(RoomClose),

    /// Send user joined notification
    SendUserJoined(RoomUserJoined),

    /// Send user left notification
    SendUserLeft(RoomUserLeft),

    /// Send message edited notification
    SendMessageEdited(RoomMessageEdited),

    /// Send message deleted notification
    SendMessageDeleted(RoomMessageDeleted),

    /// Send reaction added notification
    SendReactionAdded(RoomReactionAdded),

    /// Send reaction removed notification
    SendReactionRemoved(RoomReactionRemoved),

    /// Send message acknowledgment
    SendMessageAck(MessageAck),

    /// Send typing indicator
    SendTyping(Typing),

    /// Send stop typing indicator
    SendStopTyping(StopTyping),

    /// Send presence online
    SendPresenceOnline(PresenceOnline),

    /// Send presence offline
    SendPresenceOffline(PresenceOffline),

    /// Send throttle warning
    SendThrottle(Throttle),

    /// Promote a room to hot status
    PromoteRoom(RoomId),

    /// Demote a room from hot status
    DemoteRoom(RoomId),

    /// Close the connection
    Close(String),
}

/// State of the connection handshake
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandshakeState {
    /// Waiting for Hello from client
    AwaitingHello,
    /// Hello received, sent HelloAck, waiting for Auth
    AwaitingAuth,
    /// Fully authenticated
    Authenticated,
}

/// Per-connection handler that manages streams and protocol
pub struct ConnectionHandler {
    /// Underlying QUIC connection
    connection: Connection,

    /// User ID (set after authentication)
    user_id: RwLock<Option<UserId>>,

    /// Username
    username: RwLock<Option<String>>,

    /// Handshake state
    handshake_state: RwLock<HandshakeState>,

    /// Session ID
    session_id: String,

    /// Stream set tracking
    streams: RwLock<StreamSet>,

    /// Stream configuration
    config: StreamConfig,

    /// Shard router reference
    shard_router: Arc<ShardRouter>,

    /// Rooms this connection is subscribed to
    subscribed_rooms: RwLock<HashSet<RoomId>>,

    /// Channel for sending events to the server
    event_tx: mpsc::UnboundedSender<ServerEvent>,

    /// Channel for receiving commands from the server
    command_rx: RwLock<Option<mpsc::UnboundedReceiver<ConnectionCommand>>>,

    /// Control stream sender
    control_send: RwLock<Option<SendStream>>,

    /// Shard stream senders (indexed by shard ID)
    shard_sends: RwLock<Vec<Option<SendStream>>>,

    /// Hot room stream senders
    hot_room_sends: RwLock<std::collections::HashMap<RoomId, SendStream>>,

    /// Connection creation time
    created_at: Instant,

    /// Last activity timestamp
    last_activity: RwLock<Instant>,

    /// Ping timestamp for RTT calculation
    last_ping_time: RwLock<Option<Instant>>,
}

impl ConnectionHandler {
    /// Create a new connection handler
    pub fn new(
        connection: Connection,
        config: StreamConfig,
        shard_router: Arc<ShardRouter>,
        event_tx: mpsc::UnboundedSender<ServerEvent>,
        command_rx: mpsc::UnboundedReceiver<ConnectionCommand>,
    ) -> Self {
        let num_shards = config.num_shards as usize;
        let session_id = uuid::Uuid::new_v4().to_string();

        let shard_sends_vec: Vec<Option<SendStream>> = (0..num_shards).map(|_| None).collect();

        Self {
            connection,
            user_id: RwLock::new(None),
            username: RwLock::new(None),
            handshake_state: RwLock::new(HandshakeState::AwaitingHello),
            session_id,
            streams: RwLock::new(StreamSet::new(config.clone())),
            config,
            shard_router,
            subscribed_rooms: RwLock::new(HashSet::new()),
            event_tx,
            command_rx: RwLock::new(Some(command_rx)),
            control_send: RwLock::new(None),
            shard_sends: RwLock::new(shard_sends_vec),
            hot_room_sends: RwLock::new(std::collections::HashMap::new()),
            created_at: Instant::now(),
            last_activity: RwLock::new(Instant::now()),
            last_ping_time: RwLock::new(None),
        }
    }

    /// Get the remote address
    pub fn remote_address(&self) -> std::net::SocketAddr {
        self.connection.remote_address()
    }

    /// Get user ID if authenticated
    pub async fn user_id(&self) -> Option<UserId> {
        *self.user_id.read().await
    }

    /// Get username if authenticated
    pub async fn username(&self) -> Option<String> {
        self.username.read().await.clone()
    }

    /// Check if authenticated
    pub async fn is_authenticated(&self) -> bool {
        *self.handshake_state.read().await == HandshakeState::Authenticated
    }

    /// Get connection uptime
    pub fn uptime(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Update last activity
    async fn touch(&self) {
        *self.last_activity.write().await = Instant::now();
    }

    /// Run the connection handler
    /// This is the main entry point that should be spawned as a task
    pub async fn run(self: Arc<Self>) -> Result<()> {
        let addr = self.remote_address();
        info!("New connection from {}", addr);

        // Accept the control stream (client opens first)
        let result = self.accept_and_run_arc(Arc::clone(&self)).await;

        // Clean up on disconnect
        let user_id = self.user_id().await;
        let reason = match &result {
            Ok(()) => "normal".to_string(),
            Err(e) => e.to_string(),
        };

        let _ = self
            .event_tx
            .send(ServerEvent::Disconnected { user_id, reason });

        info!("Connection from {} closed", addr);
        result
    }

    /// Accept streams and run the main loop (takes Arc for spawning tasks)
    async fn accept_and_run_arc(self: &Arc<Self>, handler: Arc<Self>) -> Result<()> {
        // Accept the control bidirectional stream from the client
        let (send, recv) = self.connection.accept_bi().await.map_err(|e| {
            ChatError::connection(format!("Failed to accept control stream: {}", e))
        })?;

        // Store the control send stream
        {
            let mut control = self.control_send.write().await;
            *control = Some(send);
        }

        // Update stream set
        {
            let mut streams = self.streams.write().await;
            let mut handle = StreamHandle::new(StreamType::Control, current_timestamp());
            handle.set_state(StreamState::Open);
            streams.control = Some(handle);
        }

        debug!("Control stream accepted from {}", self.remote_address());

        // Spawn control stream receiver
        let recv_handle = {
            let h = Arc::clone(&handler);
            tokio::spawn(async move {
                if let Err(e) = h.handle_control_stream_arc(recv).await {
                    error!("Control stream error: {}", e);
                }
            })
        };

        // Spawn command handler
        let cmd_handle = {
            let h = Arc::clone(&handler);
            tokio::spawn(async move {
                h.handle_commands_arc().await;
            })
        };

        // Spawn client uni stream acceptor (for ChatCommands, ACKs, etc.)
        let uni_handle = {
            let h = Arc::clone(&handler);
            tokio::spawn(async move {
                h.accept_client_uni_streams_arc().await;
            })
        };

        // Spawn datagram receiver
        let dgram_handle = {
            let h = Arc::clone(&handler);
            tokio::spawn(async move {
                h.handle_datagrams_arc().await;
            })
        };

        // Spawn ping task
        let ping_handle = {
            let h = Arc::clone(&handler);
            tokio::spawn(async move {
                h.ping_loop_arc().await;
            })
        };

        // Wait for any task to complete (usually means disconnect)
        tokio::select! {
            _ = recv_handle => {},
            _ = cmd_handle => {},
            _ = uni_handle => {},
            _ = dgram_handle => {},
            _ = ping_handle => {},
        }

        Ok(())
    }

    /// Handle incoming frames on the control stream (Arc version for spawned tasks)
    async fn handle_control_stream_arc(self: &Arc<Self>, mut recv: RecvStream) -> Result<()> {
        let mut codec = FrameCodec::new();
        let mut buf = vec![0u8; 4096];

        loop {
            match recv.read(&mut buf).await {
                Ok(Some(n)) => {
                    self.touch().await;
                    codec.feed(&buf[..n]);

                    // Process all available frames
                    loop {
                        match codec.decode_next() {
                            Ok(Some(frame)) => {
                                if let Err(e) = self.handle_control_frame(frame).await {
                                    warn!("Error handling control frame: {}", e);
                                    self.send_error(e).await?;
                                }
                            }
                            Ok(None) => break,
                            Err(e) => {
                                return Err(ChatError::protocol(format!(
                                    "Frame decode error: {}",
                                    e
                                )));
                            }
                        }
                    }
                }
                Ok(None) => {
                    debug!("Control stream finished");
                    break;
                }
                Err(e) => {
                    return Err(ChatError::network(format!(
                        "Control stream read error: {}",
                        e
                    )));
                }
            }
        }

        Ok(())
    }

    /// Handle a single control frame
    async fn handle_control_frame(&self, frame: Frame) -> Result<()> {
        let state = *self.handshake_state.read().await;

        match (state, frame.frame_type) {
            // Handshake: Hello
            (HandshakeState::AwaitingHello, FrameType::Hello) => {
                let hello = Hello::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid Hello: {}", e)))?;

                debug!(
                    "Received Hello v{} with caps: {:?}",
                    hello.version, hello.capabilities
                );

                // Send HelloAck
                let hello_ack = HelloAck {
                    version: 1,
                    session_id: self.session_id.clone(),
                    num_shards: self.config.num_shards,
                };

                self.send_control_frame(&hello_ack).await?;

                // Update state
                *self.handshake_state.write().await = HandshakeState::AwaitingAuth;
                debug!("Sent HelloAck, awaiting Auth");
            }

            // Handshake: Auth
            (HandshakeState::AwaitingAuth, FrameType::Auth) => {
                let auth = Auth::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid Auth: {}", e)))?;

                debug!("Received Auth method={}", auth.method);

                // Simple auth for now - accept any username
                // In production, validate token/credentials here
                let (user_id, username) = self.authenticate(&auth).await?;

                // Store user info
                *self.user_id.write().await = Some(user_id);
                *self.username.write().await = Some(username.clone());

                // Get user's rooms (would come from database in production)
                let rooms = vec![]; // Empty for now

                // Send AuthOk
                let auth_ok = AuthOk {
                    user_id,
                    username: username.clone(),
                    rooms,
                };
                self.send_control_frame(&auth_ok).await?;

                // Update state
                *self.handshake_state.write().await = HandshakeState::Authenticated;

                // Open shard streams for the client
                self.open_shard_streams().await?;

                // Notify server
                let _ = self
                    .event_tx
                    .send(ServerEvent::Authenticated { user_id, username });

                info!(
                    "User {} authenticated from {}",
                    user_id,
                    self.remote_address()
                );
            }

            // Ping/Pong
            (HandshakeState::Authenticated, FrameType::Ping) => {
                let ping = Ping::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid Ping: {}", e)))?;

                let pong = Pong {
                    timestamp: ping.timestamp,
                };
                self.send_control_frame(&pong).await?;
            }

            (HandshakeState::Authenticated, FrameType::Pong) => {
                let _pong = Pong::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid Pong: {}", e)))?;

                // Calculate RTT
                if let Some(ping_time) = *self.last_ping_time.read().await {
                    let rtt = ping_time.elapsed();
                    debug!("RTT: {:?}", rtt);
                }
            }

            // Goodbye
            (_, FrameType::Goodbye) => {
                let goodbye = Goodbye::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid Goodbye: {}", e)))?;

                info!("Client sent Goodbye: {}", goodbye.reason);
                self.connection
                    .close(0u32.into(), goodbye.reason.as_bytes());
            }

            // Invalid state/frame combination
            (state, frame_type) => {
                warn!("Unexpected frame {:?} in state {:?}", frame_type, state);
                return Err(ChatError::protocol(format!(
                    "Unexpected frame {:?} in state {:?}",
                    frame_type, state
                )));
            }
        }

        Ok(())
    }

    /// Authenticate user (simplified - in production, validate tokens)
    async fn authenticate(&self, auth: &Auth) -> Result<(UserId, String)> {
        match auth.method.as_str() {
            "username" => {
                // Simple username auth - generate user ID
                let username = auth.credentials.trim().to_string();
                if username.is_empty() || username.len() > 50 {
                    return Err(ChatError::auth("Invalid username"));
                }

                // Generate a user ID (in production, look up from database)
                let user_id = {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut hasher = DefaultHasher::new();
                    username.hash(&mut hasher);
                    hasher.finish()
                };

                Ok((user_id, username))
            }
            "token" => {
                // Token auth - would validate JWT or similar
                // For now, just reject
                Err(ChatError::auth("Token auth not implemented"))
            }
            _ => Err(ChatError::auth(format!(
                "Unknown auth method: {}",
                auth.method
            ))),
        }
    }

    /// Open shard streams to the client
    async fn open_shard_streams(&self) -> Result<()> {
        let num_shards = self.config.num_shards;
        let mut shard_infos = Vec::with_capacity(num_shards as usize);

        for shard_id in 0..num_shards {
            // Open uni stream for this shard
            let send = self.connection.open_uni().await.map_err(|e| {
                ChatError::connection(format!("Failed to open shard stream {}: {}", shard_id, e))
            })?;

            // Store the stream
            {
                let mut shard_sends = self.shard_sends.write().await;
                shard_sends[shard_id as usize] = Some(send);
            }

            // Update stream set
            {
                let mut streams = self.streams.write().await;
                let mut handle =
                    StreamHandle::new(StreamType::Shard(shard_id), current_timestamp());
                handle.set_state(StreamState::Open);
                streams.shards[shard_id as usize] = Some(handle);
            }

            shard_infos.push(ShardStreamInfo {
                shard_id,
                room_ids: vec![], // Will be populated as client joins rooms
            });
        }

        // Send shard assignment to client
        let assignment = ShardAssignment {
            shard_streams: shard_infos,
        };
        self.send_control_frame(&assignment).await?;

        debug!("Opened {} shard streams", num_shards);
        Ok(())
    }

    /// Accept client-initiated unidirectional streams (Arc version)
    async fn accept_client_uni_streams_arc(self: &Arc<Self>) {
        loop {
            match self.connection.accept_uni().await {
                Ok(recv) => {
                    let handler = Arc::clone(self);
                    tokio::spawn(async move {
                        if let Err(e) = handler.handle_client_uni_stream_arc(recv).await {
                            warn!("Client uni stream error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    debug!("Stopped accepting uni streams: {}", e);
                    break;
                }
            }
        }
    }

    /// Handle a client-initiated unidirectional stream (Arc version)
    async fn handle_client_uni_stream_arc(self: &Arc<Self>, mut recv: RecvStream) -> Result<()> {
        // Read the first frame to determine stream type
        let mut codec = FrameCodec::new();
        let mut buf = vec![0u8; 4096];

        // Determine stream type from first frame
        let first_frame = loop {
            match recv.read(&mut buf).await {
                Ok(Some(n)) => {
                    codec.feed(&buf[..n]);
                    if let Some(frame) = codec.decode_next()? {
                        break frame;
                    }
                }
                Ok(None) => return Ok(()), // Stream closed without data
                Err(e) => {
                    return Err(ChatError::network(format!("Stream read error: {}", e)));
                }
            }
        };

        // Determine stream type and handle accordingly
        let stream_type = if first_frame.frame_type.is_chat_command() {
            StreamType::ChatCommands
        } else if first_frame.frame_type.is_ack() {
            StreamType::Acks
        } else if first_frame.frame_type.is_upload() {
            StreamType::BulkUpload
        } else {
            return Err(ChatError::protocol(format!(
                "Unexpected frame type on uni stream: {:?}",
                first_frame.frame_type
            )));
        };

        debug!("Accepted {:?} stream", stream_type);

        // Process the first frame
        self.handle_client_frame(first_frame).await?;

        // Continue reading frames
        loop {
            match recv.read(&mut buf).await {
                Ok(Some(n)) => {
                    self.touch().await;
                    codec.feed(&buf[..n]);

                    loop {
                        match codec.decode_next() {
                            Ok(Some(frame)) => {
                                if let Err(e) = self.handle_client_frame(frame).await {
                                    warn!("Error handling frame: {}", e);
                                }
                            }
                            Ok(None) => break,
                            Err(e) => {
                                return Err(ChatError::protocol(format!(
                                    "Frame decode error: {}",
                                    e
                                )));
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    return Err(ChatError::network(format!("Stream read error: {}", e)));
                }
            }
        }

        Ok(())
    }

    /// Handle a frame from a client uni stream (chat commands, ACKs, uploads)
    async fn handle_client_frame(&self, frame: Frame) -> Result<()> {
        if !self.is_authenticated().await {
            return Err(ChatError::auth("Not authenticated"));
        }

        let user_id = self.user_id().await.unwrap();

        match frame.frame_type {
            // Chat commands
            FrameType::SendMessage => {
                let msg = SendMessage::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid SendMessage: {}", e)))?;

                let _ = self.event_tx.send(ServerEvent::SendMessage {
                    user_id,
                    room_id: msg.room_id,
                    content: msg.content,
                    nonce: Some(msg.nonce),
                    reply_to: msg.reply_to,
                });
            }

            FrameType::EditMessage => {
                let msg = EditMessage::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid EditMessage: {}", e)))?;

                let _ = self.event_tx.send(ServerEvent::EditMessage {
                    user_id,
                    message_id: msg.message_id,
                    content: msg.content,
                });
            }

            FrameType::DeleteMessage => {
                let msg = DeleteMessage::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid DeleteMessage: {}", e)))?;

                let _ = self.event_tx.send(ServerEvent::DeleteMessage {
                    user_id,
                    message_id: msg.message_id,
                });
            }

            FrameType::AddReaction => {
                let msg = AddReaction::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid AddReaction: {}", e)))?;

                let _ = self.event_tx.send(ServerEvent::AddReaction {
                    user_id,
                    message_id: msg.message_id,
                    emoji: msg.emoji,
                });
            }

            FrameType::RemoveReaction => {
                let msg = RemoveReaction::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid RemoveReaction: {}", e)))?;

                let _ = self.event_tx.send(ServerEvent::RemoveReaction {
                    user_id,
                    message_id: msg.message_id,
                    emoji: msg.emoji,
                });
            }

            FrameType::JoinRoom => {
                let msg = JoinRoom::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid JoinRoom: {}", e)))?;

                // Track subscription
                self.subscribed_rooms.write().await.insert(msg.room_id);

                // Register with shard router
                self.shard_router.register_room(msg.room_id).await;

                let _ = self.event_tx.send(ServerEvent::JoinRoom {
                    user_id,
                    room_id: msg.room_id,
                });
            }

            FrameType::LeaveRoom => {
                let msg = LeaveRoom::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid LeaveRoom: {}", e)))?;

                // Remove subscription
                self.subscribed_rooms.write().await.remove(&msg.room_id);

                let _ = self.event_tx.send(ServerEvent::LeaveRoom {
                    user_id,
                    room_id: msg.room_id,
                });
            }

            FrameType::CreateRoom => {
                let msg = CreateRoom::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid CreateRoom: {}", e)))?;

                let _ = self.event_tx.send(ServerEvent::CreateRoom {
                    user_id,
                    name: msg.name,
                    room_type: msg.room_type,
                    members: msg.members,
                });
            }

            // ACK messages
            FrameType::MessageDelivered => {
                let _msg = MessageDelivered::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid MessageDelivered: {}", e)))?;
                // Handle delivery confirmation
            }

            FrameType::MessageRead => {
                let _msg = MessageRead::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid MessageRead: {}", e)))?;
                // Handle read receipt
            }

            // Upload messages
            FrameType::UploadStart => {
                let _msg = UploadStart::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid UploadStart: {}", e)))?;
                // Handle upload start
                warn!("Uploads not yet implemented");
            }

            _ => {
                warn!("Unexpected frame type: {:?}", frame.frame_type);
            }
        }

        Ok(())
    }

    /// Handle incoming datagrams (typing, presence) - Arc version
    async fn handle_datagrams_arc(self: &Arc<Self>) {
        loop {
            match self.connection.read_datagram().await {
                Ok(data) => {
                    self.touch().await;

                    if let Err(e) = self.handle_datagram(data).await {
                        warn!("Datagram handling error: {}", e);
                    }
                }
                Err(e) => {
                    debug!("Datagram receive ended: {}", e);
                    break;
                }
            }
        }
    }

    /// Handle a single datagram
    async fn handle_datagram(&self, data: Bytes) -> Result<()> {
        if !self.is_authenticated().await {
            return Ok(()); // Silently ignore datagrams before auth
        }

        let user_id = self.user_id().await.unwrap();
        let frame = Frame::decode_complete(&data)
            .map_err(|e| ChatError::protocol(format!("Invalid datagram frame: {}", e)))?;

        match frame.frame_type {
            FrameType::Typing => {
                let msg = Typing::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid Typing: {}", e)))?;

                let _ = self.event_tx.send(ServerEvent::Typing {
                    user_id,
                    room_id: msg.room_id,
                });
            }

            FrameType::StopTyping => {
                let msg = StopTyping::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid StopTyping: {}", e)))?;

                let _ = self.event_tx.send(ServerEvent::StopTyping {
                    user_id,
                    room_id: msg.room_id,
                });
            }

            FrameType::PresenceOnline => {
                let msg = PresenceOnline::decode_frame(&frame)
                    .map_err(|e| ChatError::protocol(format!("Invalid PresenceOnline: {}", e)))?;

                let _ = self.event_tx.send(ServerEvent::PresenceUpdate {
                    user_id,
                    status: msg.status.unwrap_or_else(|| "online".to_string()),
                });
            }

            _ => {
                warn!("Unexpected datagram frame type: {:?}", frame.frame_type);
            }
        }

        Ok(())
    }

    /// Handle commands from the server (Arc version)
    async fn handle_commands_arc(self: &Arc<Self>) {
        let rx = self.command_rx.write().await.take();
        if rx.is_none() {
            return;
        }
        let mut rx = rx.unwrap();

        while let Some(cmd) = rx.recv().await {
            if let Err(e) = self.handle_command(cmd).await {
                warn!("Command handling error: {}", e);
            }
        }
    }

    /// Handle a single command
    async fn handle_command(&self, cmd: ConnectionCommand) -> Result<()> {
        match cmd {
            ConnectionCommand::SendRoomMessage(msg) => {
                self.send_room_message(msg).await?;
            }
            ConnectionCommand::SendRoomInit(msg) => {
                self.send_room_frame(&msg, msg.room_id).await?;
            }
            ConnectionCommand::SendRoomClose(msg) => {
                self.send_room_frame(&msg, msg.room_id).await?;
            }
            ConnectionCommand::SendUserJoined(msg) => {
                self.send_room_frame(&msg, msg.room_id).await?;
            }
            ConnectionCommand::SendUserLeft(msg) => {
                self.send_room_frame(&msg, msg.room_id).await?;
            }
            ConnectionCommand::SendMessageEdited(msg) => {
                self.send_room_frame(&msg, msg.room_id).await?;
            }
            ConnectionCommand::SendMessageDeleted(msg) => {
                self.send_room_frame(&msg, msg.room_id).await?;
            }
            ConnectionCommand::SendReactionAdded(msg) => {
                self.send_room_frame(&msg, msg.room_id).await?;
            }
            ConnectionCommand::SendReactionRemoved(msg) => {
                self.send_room_frame(&msg, msg.room_id).await?;
            }
            ConnectionCommand::SendMessageAck(msg) => {
                self.send_control_frame(&msg).await?;
            }
            ConnectionCommand::SendTyping(msg) => {
                self.send_datagram(&msg).await?;
            }
            ConnectionCommand::SendStopTyping(msg) => {
                self.send_datagram(&msg).await?;
            }
            ConnectionCommand::SendPresenceOnline(msg) => {
                self.send_datagram(&msg).await?;
            }
            ConnectionCommand::SendPresenceOffline(msg) => {
                self.send_datagram(&msg).await?;
            }
            ConnectionCommand::SendThrottle(msg) => {
                self.send_control_frame(&msg).await?;
            }
            ConnectionCommand::PromoteRoom(room_id) => {
                self.promote_room(room_id).await?;
            }
            ConnectionCommand::DemoteRoom(room_id) => {
                self.demote_room(room_id).await?;
            }
            ConnectionCommand::Close(reason) => {
                self.connection.close(0u32.into(), reason.as_bytes());
            }
        }

        Ok(())
    }

    /// Send a room message (routed through shard or hot room stream)
    async fn send_room_message(&self, msg: RoomMessage) -> Result<()> {
        let room_id = msg.room_id;

        // Check if room is hot (has dedicated stream)
        let hot_rooms = self.hot_room_sends.read().await;
        if hot_rooms.contains_key(&room_id) {
            drop(hot_rooms);
            return self.send_hot_room_frame(&msg, room_id).await;
        }
        drop(hot_rooms);

        // Otherwise, route through shard
        self.send_room_frame(&msg, room_id).await
    }

    /// Send a frame to a room via its shard
    async fn send_room_frame<T: Encodable>(&self, msg: &T, room_id: RoomId) -> Result<()> {
        let shard_id = room_shard(room_id);
        let frame = msg
            .encode_frame()
            .map_err(|e| ChatError::serialization(format!("Failed to encode frame: {}", e)))?;

        let mut shard_sends = self.shard_sends.write().await;
        if let Some(Some(send)) = shard_sends.get_mut(shard_id as usize) {
            let data = frame.encode_to_bytes();
            send.write_all(&data).await.map_err(|e| {
                ChatError::network(format!("Failed to write to shard stream: {}", e))
            })?;
        } else {
            return Err(ChatError::connection(format!(
                "Shard stream {} not open",
                shard_id
            )));
        }

        Ok(())
    }

    /// Send a frame to a hot room stream
    async fn send_hot_room_frame<T: Encodable>(&self, msg: &T, room_id: RoomId) -> Result<()> {
        let frame = msg
            .encode_frame()
            .map_err(|e| ChatError::serialization(format!("Failed to encode frame: {}", e)))?;

        let mut hot_room_sends = self.hot_room_sends.write().await;
        if let Some(send) = hot_room_sends.get_mut(&room_id) {
            let data = frame.encode_to_bytes();
            send.write_all(&data).await.map_err(|e| {
                ChatError::network(format!("Failed to write to hot room stream: {}", e))
            })?;
        } else {
            return Err(ChatError::connection(format!(
                "Hot room stream {} not open",
                room_id
            )));
        }

        Ok(())
    }

    /// Send a frame on the control stream
    async fn send_control_frame<T: Encodable>(&self, msg: &T) -> Result<()> {
        let frame = msg
            .encode_frame()
            .map_err(|e| ChatError::serialization(format!("Failed to encode frame: {}", e)))?;

        let mut control = self.control_send.write().await;
        if let Some(send) = control.as_mut() {
            let data = frame.encode_to_bytes();
            send.write_all(&data).await.map_err(|e| {
                ChatError::network(format!("Failed to write to control stream: {}", e))
            })?;
        } else {
            return Err(ChatError::connection("Control stream not open"));
        }

        Ok(())
    }

    /// Send a datagram
    async fn send_datagram<T: Encodable>(&self, msg: &T) -> Result<()> {
        let frame = msg
            .encode_frame()
            .map_err(|e| ChatError::serialization(format!("Failed to encode frame: {}", e)))?;

        let data = frame.encode_to_bytes();
        self.connection
            .send_datagram(data)
            .map_err(|e| ChatError::network(format!("Failed to send datagram: {}", e)))?;

        Ok(())
    }

    /// Send an error frame
    async fn send_error(&self, error: ChatError) -> Result<()> {
        let err = Error::new(error.code(), error.message().to_string());
        self.send_control_frame(&err).await
    }

    /// Promote a room to hot status (dedicated stream)
    async fn promote_room(&self, room_id: RoomId) -> Result<()> {
        // Check if already hot
        {
            let hot_rooms = self.hot_room_sends.read().await;
            if hot_rooms.contains_key(&room_id) {
                return Ok(());
            }
        }

        // Open dedicated stream
        let send =
            self.connection.open_uni().await.map_err(|e| {
                ChatError::connection(format!("Failed to open hot room stream: {}", e))
            })?;

        let from_shard = room_shard(room_id);

        // Store the stream
        {
            let mut hot_rooms = self.hot_room_sends.write().await;
            hot_rooms.insert(room_id, send);
        }

        // Update stream set
        {
            let mut streams = self.streams.write().await;
            let mut handle = StreamHandle::new(StreamType::HotRoom(room_id), current_timestamp());
            handle.set_state(StreamState::Open);
            streams.hot_rooms.insert(room_id, handle);
        }

        // Notify client
        let promoted = RoomPromoted {
            room_id,
            from_shard,
        };
        self.send_control_frame(&promoted).await?;

        info!("Promoted room {} to hot status", room_id);
        Ok(())
    }

    /// Demote a room from hot status
    async fn demote_room(&self, room_id: RoomId) -> Result<()> {
        let to_shard = room_shard(room_id);

        // Remove and close the stream
        {
            let mut hot_rooms = self.hot_room_sends.write().await;
            if let Some(mut send) = hot_rooms.remove(&room_id) {
                let _ = send.finish();
            }
        }

        // Update stream set
        {
            let mut streams = self.streams.write().await;
            streams.hot_rooms.remove(&room_id);
        }

        // Notify client
        let demoted = RoomDemoted { room_id, to_shard };
        self.send_control_frame(&demoted).await?;

        info!("Demoted room {} back to shard {}", room_id, to_shard);
        Ok(())
    }

    /// Ping loop for keepalive (Arc version)
    async fn ping_loop_arc(self: &Arc<Self>) {
        let mut interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            interval.tick().await;

            if !self.is_authenticated().await {
                continue;
            }

            let ping = Ping {
                timestamp: current_timestamp(),
            };

            *self.last_ping_time.write().await = Some(Instant::now());

            if let Err(e) = self.send_control_frame(&ping).await {
                warn!("Failed to send ping: {}", e);
                break;
            }
        }
    }

    /// Get subscribed room IDs
    pub async fn subscribed_rooms(&self) -> Vec<RoomId> {
        self.subscribed_rooms.read().await.iter().copied().collect()
    }

    /// Check if subscribed to a room
    pub async fn is_subscribed(&self, room_id: RoomId) -> bool {
        self.subscribed_rooms.read().await.contains(&room_id)
    }
}

/// Builder for ConnectionHandler
pub struct ConnectionHandlerBuilder {
    config: StreamConfig,
    shard_router: Option<Arc<ShardRouter>>,
}

impl ConnectionHandlerBuilder {
    pub fn new() -> Self {
        Self {
            config: StreamConfig::default(),
            shard_router: None,
        }
    }

    pub fn with_config(mut self, config: StreamConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_shard_router(mut self, router: Arc<ShardRouter>) -> Self {
        self.shard_router = Some(router);
        self
    }

    pub fn build(
        self,
        connection: Connection,
        event_tx: mpsc::UnboundedSender<ServerEvent>,
        command_rx: mpsc::UnboundedReceiver<ConnectionCommand>,
    ) -> ConnectionHandler {
        let shard_router = self
            .shard_router
            .unwrap_or_else(|| Arc::new(ShardRouter::with_defaults()));

        ConnectionHandler::new(connection, self.config, shard_router, event_tx, command_rx)
    }
}

impl Default for ConnectionHandlerBuilder {
    fn default() -> Self {
        Self::new()
    }
}
