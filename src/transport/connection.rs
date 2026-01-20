//! Connection handler with multi-stream management
//!
//! This module provides the core connection handling logic that manages
//! multiple QUIC streams according to our protocol design.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use quinn::{Connection, RecvStream, SendStream};
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio::time::Instant;

use crate::error::{ChatError, Result};
use crate::protocol::frame::{Frame, FrameCodec};
use crate::protocol::messages::{RoomId, ShardId, UserId};
use crate::transport::shards::{RoutingAction, ShardRouter};
use crate::transport::streams::{StreamConfig, StreamHandle, StreamSet, StreamState, StreamType};

/// Events that can be received from a connection
#[derive(Debug)]
pub enum ConnectionEvent {
    /// A frame was received on a stream
    FrameReceived {
        stream_type: StreamType,
        frame: Frame,
    },

    /// A datagram was received
    DatagramReceived(Bytes),

    /// A stream was opened by the peer
    StreamOpened(StreamType),

    /// A stream was closed
    StreamClosed(StreamType),

    /// Connection error
    Error(ChatError),

    /// Connection closed
    Closed(String),
}

/// Commands that can be sent to a connection
#[derive(Debug)]
pub enum ConnectionCommand {
    /// Send a frame on a specific stream
    SendFrame {
        stream_type: StreamType,
        frame: Frame,
        response: Option<oneshot::Sender<Result<()>>>,
    },

    /// Send a datagram
    SendDatagram {
        data: Bytes,
        response: Option<oneshot::Sender<Result<()>>>,
    },

    /// Open a new stream
    OpenStream {
        stream_type: StreamType,
        response: oneshot::Sender<Result<()>>,
    },

    /// Close a stream
    CloseStream {
        stream_type: StreamType,
        response: Option<oneshot::Sender<Result<()>>>,
    },

    /// Close the connection
    Close {
        reason: String,
        response: Option<oneshot::Sender<Result<()>>>,
    },
}

/// Managed QUIC connection with multi-stream support
#[allow(dead_code)]
pub struct ManagedConnection {
    /// Underlying QUIC connection
    connection: Connection,

    /// User ID (set after authentication)
    user_id: RwLock<Option<UserId>>,

    /// Stream management
    streams: RwLock<StreamSet>,

    /// Stream configuration
    config: StreamConfig,

    /// Shard router for room routing
    shard_router: Arc<ShardRouter>,

    /// Active send streams by type
    send_streams: RwLock<HashMap<StreamType, SendStreamHandle>>,

    /// Channel for incoming events
    event_tx: mpsc::UnboundedSender<ConnectionEvent>,

    /// Connection creation time
    created_at: Instant,

    /// Last activity timestamp
    last_activity: RwLock<Instant>,
}

/// Handle for a send stream with buffering
#[allow(dead_code)]
struct SendStreamHandle {
    stream: SendStream,
    buffer: BytesMut,
    frames_sent: u64,
    bytes_sent: u64,
}

impl SendStreamHandle {
    fn new(stream: SendStream) -> Self {
        Self {
            stream,
            buffer: BytesMut::with_capacity(4096),
            frames_sent: 0,
            bytes_sent: 0,
        }
    }

    async fn send_frame(&mut self, frame: &Frame) -> Result<()> {
        let data = frame.encode_to_bytes();
        let len = data.len();

        self.stream
            .write_all(&data)
            .await
            .map_err(|e| ChatError::network(format!("Failed to write frame: {}", e)))?;

        self.frames_sent += 1;
        self.bytes_sent += len as u64;

        Ok(())
    }

    #[allow(dead_code)]
    async fn flush(&mut self) -> Result<()> {
        // Quinn streams are automatically flushed, but we could add
        // explicit flushing logic here if needed
        Ok(())
    }
}

impl ManagedConnection {
    /// Create a new managed connection
    pub fn new(
        connection: Connection,
        config: StreamConfig,
        shard_router: Arc<ShardRouter>,
        event_tx: mpsc::UnboundedSender<ConnectionEvent>,
    ) -> Self {
        let stream_set = StreamSet::new(config.clone());

        Self {
            connection,
            user_id: RwLock::new(None),
            streams: RwLock::new(stream_set),
            config,
            shard_router,
            send_streams: RwLock::new(HashMap::new()),
            event_tx,
            created_at: Instant::now(),
            last_activity: RwLock::new(Instant::now()),
        }
    }

    /// Get the remote address
    pub fn remote_address(&self) -> std::net::SocketAddr {
        self.connection.remote_address()
    }

    /// Set the authenticated user ID
    pub async fn set_user_id(&self, user_id: UserId) {
        let mut uid = self.user_id.write().await;
        *uid = Some(user_id);
    }

    /// Get the authenticated user ID
    pub async fn user_id(&self) -> Option<UserId> {
        let uid = self.user_id.read().await;
        *uid
    }

    /// Check if the connection is authenticated
    pub async fn is_authenticated(&self) -> bool {
        self.user_id().await.is_some()
    }

    /// Update last activity timestamp
    pub async fn touch(&self) {
        let mut last = self.last_activity.write().await;
        *last = Instant::now();
    }

    /// Get time since last activity
    pub async fn idle_duration(&self) -> Duration {
        let last = self.last_activity.read().await;
        last.elapsed()
    }

    /// Get connection uptime
    pub fn uptime(&self) -> Duration {
        self.created_at.elapsed()
    }

    // =========================================================================
    // Stream Management
    // =========================================================================

    /// Open the control stream (bidirectional)
    pub async fn open_control_stream(&self) -> Result<()> {
        let (send, recv) =
            self.connection.open_bi().await.map_err(|e| {
                ChatError::connection(format!("Failed to open control stream: {}", e))
            })?;

        // Store send stream
        {
            let mut send_streams = self.send_streams.write().await;
            send_streams.insert(StreamType::Control, SendStreamHandle::new(send));
        }

        // Update stream set
        {
            let mut streams = self.streams.write().await;
            let mut handle = StreamHandle::new(StreamType::Control, crate::current_timestamp());
            handle.set_state(StreamState::Open);
            streams.control = Some(handle);
        }

        // Spawn receiver task
        self.spawn_stream_receiver(StreamType::Control, recv);

        // Emit event
        let _ = self
            .event_tx
            .send(ConnectionEvent::StreamOpened(StreamType::Control));

        Ok(())
    }

    /// Open a unidirectional client->server stream
    pub async fn open_uni_stream(&self, stream_type: StreamType) -> Result<()> {
        if !stream_type.is_client_initiated() || stream_type.is_bidirectional() {
            return Err(ChatError::protocol(format!(
                "Invalid stream type for uni stream: {:?}",
                stream_type
            )));
        }

        let send = self
            .connection
            .open_uni()
            .await
            .map_err(|e| ChatError::connection(format!("Failed to open uni stream: {}", e)))?;

        // Store send stream
        {
            let mut send_streams = self.send_streams.write().await;
            send_streams.insert(stream_type, SendStreamHandle::new(send));
        }

        // Update stream set
        {
            let mut streams = self.streams.write().await;
            let mut handle = StreamHandle::new(stream_type, crate::current_timestamp());
            handle.set_state(StreamState::Open);

            match stream_type {
                StreamType::ChatCommands => streams.chat_commands = Some(handle),
                StreamType::BulkUpload => streams.bulk_upload = Some(handle),
                StreamType::Acks => streams.acks = Some(handle),
                _ => {}
            }
        }

        // Emit event
        let _ = self
            .event_tx
            .send(ConnectionEvent::StreamOpened(stream_type));

        Ok(())
    }

    /// Accept an incoming unidirectional stream (server->client)
    pub async fn accept_uni_stream(&self) -> Result<(StreamType, RecvStream)> {
        let recv =
            self.connection.accept_uni().await.map_err(|e| {
                ChatError::connection(format!("Failed to accept uni stream: {}", e))
            })?;

        // For now, we determine stream type from the first frame
        // In production, you might want a stream header
        Ok((StreamType::Shard(0), recv)) // Placeholder - actual type determined later
    }

    /// Accept an incoming bidirectional stream
    pub async fn accept_bi_stream(&self) -> Result<(SendStream, RecvStream)> {
        let (send, recv) = self
            .connection
            .accept_bi()
            .await
            .map_err(|e| ChatError::connection(format!("Failed to accept bi stream: {}", e)))?;

        Ok((send, recv))
    }

    /// Open a shard stream (server-side, for sending room messages)
    pub async fn open_shard_stream(&self, shard_id: ShardId) -> Result<()> {
        let send =
            self.connection.open_uni().await.map_err(|e| {
                ChatError::connection(format!("Failed to open shard stream: {}", e))
            })?;

        let stream_type = StreamType::Shard(shard_id);

        // Store send stream
        {
            let mut send_streams = self.send_streams.write().await;
            send_streams.insert(stream_type, SendStreamHandle::new(send));
        }

        // Update stream set
        {
            let mut streams = self.streams.write().await;
            let mut handle = StreamHandle::new(stream_type, crate::current_timestamp());
            handle.set_state(StreamState::Open);
            streams.shards[shard_id as usize] = Some(handle);
        }

        // Emit event
        let _ = self
            .event_tx
            .send(ConnectionEvent::StreamOpened(stream_type));

        Ok(())
    }

    /// Open a hot room stream (server-side)
    pub async fn open_hot_room_stream(&self, room_id: RoomId) -> Result<()> {
        let send =
            self.connection.open_uni().await.map_err(|e| {
                ChatError::connection(format!("Failed to open hot room stream: {}", e))
            })?;

        let stream_type = StreamType::HotRoom(room_id);

        // Store send stream
        {
            let mut send_streams = self.send_streams.write().await;
            send_streams.insert(stream_type, SendStreamHandle::new(send));
        }

        // Update stream set
        {
            let mut streams = self.streams.write().await;
            let mut handle = StreamHandle::new(stream_type, crate::current_timestamp());
            handle.set_state(StreamState::Open);
            streams.hot_rooms.insert(room_id, handle);
        }

        // Emit event
        let _ = self
            .event_tx
            .send(ConnectionEvent::StreamOpened(stream_type));

        Ok(())
    }

    /// Close a hot room stream (when demoting)
    pub async fn close_hot_room_stream(&self, room_id: RoomId) -> Result<()> {
        let stream_type = StreamType::HotRoom(room_id);

        // Remove and close send stream
        {
            let mut send_streams = self.send_streams.write().await;
            if let Some(mut handle) = send_streams.remove(&stream_type) {
                let _ = handle.stream.finish();
            }
        }

        // Update stream set
        {
            let mut streams = self.streams.write().await;
            streams.hot_rooms.remove(&room_id);
        }

        // Emit event
        let _ = self
            .event_tx
            .send(ConnectionEvent::StreamClosed(stream_type));

        Ok(())
    }

    // =========================================================================
    // Frame Sending
    // =========================================================================

    /// Send a frame on a specific stream
    pub async fn send_frame(&self, stream_type: StreamType, frame: Frame) -> Result<()> {
        self.touch().await;

        let mut send_streams = self.send_streams.write().await;
        let handle = send_streams
            .get_mut(&stream_type)
            .ok_or_else(|| ChatError::connection(format!("Stream not open: {:?}", stream_type)))?;

        handle.send_frame(&frame).await?;

        // Update stats in stream set
        let streams = self.streams.read().await;
        if let Some(stream_handle) = match stream_type {
            StreamType::Control => streams.control.as_ref(),
            StreamType::ChatCommands => streams.chat_commands.as_ref(),
            StreamType::BulkUpload => streams.bulk_upload.as_ref(),
            StreamType::Acks => streams.acks.as_ref(),
            StreamType::Shard(id) => streams.shards.get(id as usize).and_then(|s| s.as_ref()),
            StreamType::HotRoom(id) => streams.hot_rooms.get(&id),
        } {
            stream_handle.stats.record_send(frame.encoded_size() as u64);
        }

        Ok(())
    }

    /// Send a frame to a room (routes through shard or hot room)
    pub async fn send_room_frame(&self, room_id: RoomId, frame: Frame) -> Result<()> {
        let action = self.shard_router.route_message(room_id).await;

        let stream_type = match action {
            RoutingAction::RouteToShard(shard_id) => StreamType::Shard(shard_id),
            RoutingAction::RouteToHotRoom(room_id) => StreamType::HotRoom(room_id),
            _ => {
                // Promotion/demotion actions should be handled separately
                let shard_id = self.shard_router.shard_for_room(room_id);
                StreamType::Shard(shard_id)
            }
        };

        self.send_frame(stream_type, frame).await
    }

    /// Send a control frame
    pub async fn send_control_frame(&self, frame: Frame) -> Result<()> {
        if !frame.frame_type.is_control() {
            return Err(ChatError::protocol(format!(
                "Invalid frame type for control stream: {:?}",
                frame.frame_type
            )));
        }
        self.send_frame(StreamType::Control, frame).await
    }

    // =========================================================================
    // Datagram Support
    // =========================================================================

    /// Send a datagram (unreliable)
    pub async fn send_datagram(&self, data: Bytes) -> Result<()> {
        self.touch().await;

        self.connection
            .send_datagram(data)
            .map_err(|e| ChatError::network(format!("Failed to send datagram: {}", e)))?;

        Ok(())
    }

    /// Send a frame as a datagram
    pub async fn send_datagram_frame(&self, frame: Frame) -> Result<()> {
        if !frame.frame_type.is_datagram() {
            return Err(ChatError::protocol(format!(
                "Invalid frame type for datagram: {:?}",
                frame.frame_type
            )));
        }

        let data = frame.encode_to_bytes();
        self.send_datagram(data).await
    }

    // =========================================================================
    // Connection Lifecycle
    // =========================================================================

    /// Close the connection gracefully
    pub fn close(&self, reason: &str) {
        self.connection.close(0u32.into(), reason.as_bytes());
        let _ = self
            .event_tx
            .send(ConnectionEvent::Closed(reason.to_string()));
    }

    /// Check if the connection is still alive
    pub fn is_alive(&self) -> bool {
        self.connection.close_reason().is_none()
    }

    /// Get connection statistics
    pub fn stats(&self) -> quinn::ConnectionStats {
        self.connection.stats()
    }

    /// Get the number of active streams
    pub async fn active_stream_count(&self) -> usize {
        let streams = self.streams.read().await;
        streams.active_count()
    }

    // =========================================================================
    // Internal Helpers
    // =========================================================================

    /// Spawn a task to receive frames from a stream
    fn spawn_stream_receiver(&self, stream_type: StreamType, mut recv: RecvStream) {
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let mut codec = FrameCodec::new();
            let mut buf = vec![0u8; 4096];

            loop {
                match recv.read(&mut buf).await {
                    Ok(Some(n)) => {
                        codec.feed(&buf[..n]);

                        // Decode all available frames
                        loop {
                            match codec.decode_next() {
                                Ok(Some(frame)) => {
                                    let _ = event_tx.send(ConnectionEvent::FrameReceived {
                                        stream_type,
                                        frame,
                                    });
                                }
                                Ok(None) => break, // Need more data
                                Err(e) => {
                                    let _ = event_tx.send(ConnectionEvent::Error(
                                        ChatError::protocol(format!("Frame decode error: {}", e)),
                                    ));
                                    break;
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        // Stream finished
                        let _ = event_tx.send(ConnectionEvent::StreamClosed(stream_type));
                        break;
                    }
                    Err(e) => {
                        let _ = event_tx.send(ConnectionEvent::Error(ChatError::network(format!(
                            "Stream read error: {}",
                            e
                        ))));
                        break;
                    }
                }
            }
        });
    }
}

/// Builder for creating managed connections
pub struct ConnectionBuilder {
    config: StreamConfig,
    shard_router: Option<Arc<ShardRouter>>,
}

impl ConnectionBuilder {
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
        event_tx: mpsc::UnboundedSender<ConnectionEvent>,
    ) -> ManagedConnection {
        let shard_router = self
            .shard_router
            .unwrap_or_else(|| Arc::new(ShardRouter::with_defaults()));

        ManagedConnection::new(connection, self.config, shard_router, event_tx)
    }
}

impl Default for ConnectionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_builder() {
        let builder = ConnectionBuilder::new().with_config(StreamConfig {
            num_shards: 16,
            ..Default::default()
        });

        assert_eq!(builder.config.num_shards, 16);
    }
}
